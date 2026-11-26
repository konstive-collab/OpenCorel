//! Curve-preserving interval extraction using bounded adaptive arc length
//! integration and De Casteljau subdivision. The parameter lookup is numerical;
//! the returned segments are actual subcurves, not a flattened approximation.
use crate::{CubicSegment,Document,Point,Shape};

const MAX_LEAVES:usize=250_000;
const MAX_SEGMENTS:usize=10_000;
const MAX_DEPTH:u8=24;
type XY=[f64;2];

fn xy(p:Point)->XY{[f64::from(p.x),f64::from(p.y)]}
fn point(p:XY)->Point{Point{x:p[0] as f32,y:p[1] as f32}}
fn valid(p:XY)->bool{p.iter().all(|v|v.is_finite()&&v.abs()<=100_000.)}
fn lerp(a:XY,b:XY,t:f64)->XY{[a[0]+(b[0]-a[0])*t,a[1]+(b[1]-a[1])*t]}
fn dist(a:XY,b:XY)->f64{(a[0]-b[0]).hypot(a[1]-b[1])}

#[derive(Clone,Copy,Debug)]
struct Cubic([XY;4]);
impl Cubic{
    fn line(a:XY,b:XY)->Self{Self([a,lerp(a,b,1./3.),lerp(a,b,2./3.),b])}
    fn split(self,t:f64)->(Self,Self){
        let [a,b,c,d]=self.0;
        let ab=lerp(a,b,t);let bc=lerp(b,c,t);let cd=lerp(c,d,t);
        let abc=lerp(ab,bc,t);let bcd=lerp(bc,cd,t);
        let m=lerp(abc,bcd,t);
        (Self([a,ab,abc,m]),Self([m,bcd,cd,d]))
    }
    fn subcurve(self,a:f64,b:f64)->Self{
        if a<=0.&&b>=1.{return self;}
        let left=self.split(b).0;
        if a<=0.{left}else{left.split(a/b).1}
    }
    fn chord(self)->f64{dist(self.0[0],self.0[3])}
    fn polygon(self)->f64{self.0.windows(2).map(|p|dist(p[0],p[1])).sum()}
    fn segment(self)->CubicSegment{
        CubicSegment{control1:point(self.0[1]),control2:point(self.0[2]),end:point(self.0[3])}
    }
}
#[derive(Clone,Copy,Debug)]
struct Leaf{segment:usize,t0:f64,t1:f64,start:f64,end:f64}
#[derive(Clone,Debug)]
pub struct CubicPath{
    curves:Vec<Cubic>,
    leaves:Vec<Leaf>,
    length:f64,
    closed:bool,
}
fn collect_leaves(curve:Cubic,index:usize,range:(f64,f64),epsilon:f64,
    depth:u8,leaves:&mut Vec<Leaf>,length:&mut f64)->Result<(),String>{
    let chord=curve.chord();let polygon=curve.polygon();
    // Unlike a chord-flatness test, this also bounds collinear backtracking.
    if polygon-chord<=epsilon{
        let estimate=(chord+polygon)*0.5;
        if estimate<=1e-12{return Ok(());}
        if leaves.len()>=MAX_LEAVES{return Err("Arc length integration budget exceeded".into());}
        let start=*length;*length+=estimate;
        leaves.push(Leaf{segment:index,t0:range.0,t1:range.1,start,end:*length});
        return Ok(());
    }
    if depth>=MAX_DEPTH{return Err("Arc length integration precision unavailable".into());}
    let (left,right)=curve.split(0.5);
    let mid=(range.0+range.1)*0.5;
    // Each descendant receives half the parent's error budget, bounding the
    // sum of the leaf errors over the full segment.
    collect_leaves(left,index,(range.0,mid),epsilon*0.5,depth+1,leaves,length)?;
    collect_leaves(right,index,(mid,range.1),epsilon*0.5,depth+1,leaves,length)
}
impl CubicPath{
    pub fn from_shape(shape:&Shape,tolerance_mm:f64)->Result<Self,String>{
        if !tolerance_mm.is_finite()||!(0.001..=10.).contains(&tolerance_mm){
            return Err("Tolerance must be 0.001–10 mm".into());
        }
        let converted=crate::path_workflow::to_editable_bezier(shape)?;
        let Shape::Bezier{start,segments,closed}=converted else{return Err("Expected one contour".into());};
        if segments.is_empty()||segments.len()>MAX_SEGMENTS{return Err("Invalid segment count".into());}
        let mut from=xy(start);
        let mut curves=Vec::with_capacity(segments.len()+1);
        for segment in segments{
            let c=Cubic([from,xy(segment.control1),xy(segment.control2),xy(segment.end)]);
            if !c.0.into_iter().all(valid){return Err("Invalid cubic coordinate".into());}
            from=c.0[3];curves.push(c);
        }
        if closed&&from!=xy(start){curves.push(Cubic::line(from,xy(start)));}
        let epsilon=tolerance_mm/curves.len() as f64;
        let mut leaves=Vec::new();let mut length=0.;
        for (i,c) in curves.iter().enumerate(){
            collect_leaves(*c,i,(0.,1.),epsilon,0,&mut leaves,&mut length)?;
        }
        if length<=1e-9||leaves.is_empty(){return Err("Path has no measurable length".into());}
        Ok(Self{curves,leaves,length,closed})
    }
    pub fn length_mm(&self)->f64{self.length}
    pub fn is_closed(&self)->bool{self.closed}
    fn locate(&self,distance:f64)->(usize,f64){
        if distance<=0.{return (self.leaves[0].segment,self.leaves[0].t0);}
        if distance>=self.length{
            let last=self.leaves.last().unwrap();
            return (last.segment,last.t1);
        }
        let i=self.leaves.partition_point(|l|l.end<=distance).min(self.leaves.len()-1);
        let leaf=self.leaves[i];
        // A leaf may be perfectly straight yet have highly non-uniform speed.
        // Invert distance by subdividing its actual cubic, not by linear t.
        let c=self.curves[leaf.segment].subcurve(leaf.t0,leaf.t1);
        let target=(distance-leaf.start)/(leaf.end-leaf.start);
        let mut lo=0.;let mut hi=1.;
        for _ in 0..40{
            let mid=(lo+hi)*0.5;
            let (left,right)=c.split(mid);
            let a=(left.chord()+left.polygon())*0.5;
            let b=(right.chord()+right.polygon())*0.5;
            if a/(a+b).max(1e-30)<target{lo=mid;}else{hi=mid;}
        }
        (leaf.segment,leaf.t0+(leaf.t1-leaf.t0)*(lo+hi)*0.5)
    }
    fn append_range(&self,start:f64,end:f64,out:&mut Vec<Cubic>)->Result<(),String>{
        let (first,a)=self.locate(start);
        let (last,b)=self.locate(end);
        for i in first..=last{
            let lo=if i==first{a}else{0.};
            let hi=if i==last{b}else{1.};
            if hi-lo<=1e-12{continue;}
            let curve=self.curves[i].subcurve(lo,hi);
            if curve.polygon()<=1e-12{continue;}
            if out.len()>=MAX_SEGMENTS{return Err("Extracted segment count exceeds limit".into());}
            out.push(curve);
        }
        Ok(())
    }
    /// Closed contours may wrap once across their original seam.
    pub fn extract(&self,start:f64,end:f64)->Result<Shape,String>{
        if !start.is_finite()||!end.is_finite()||start<0.||start>=self.length
            ||end<=start||end-start>self.length+1e-9||(!self.closed&&end>self.length){
            return Err("Invalid curve extraction interval".into());
        }
        let mut pieces=Vec::new();
        self.append_range(start,end.min(self.length),&mut pieces)?;
        if end>self.length{self.append_range(0.,end-self.length,&mut pieces)?;}
        let first=pieces.first().ok_or("Extracted interval is empty")?;
        Ok(Shape::Bezier{start:point(first.0[0]),
            segments:pieces.iter().map(|c|c.segment()).collect(),closed:false})
    }
}
impl Document{
    pub fn extract_cubic_interval(&mut self,id:u64,start:f64,end:f64,tolerance:f64)
        ->Result<u64,String>{
        let source=self.get(id).ok_or("Source not found")?.clone();
        if source.group_id.is_some()||!self.layer(source.layer_id).is_some_and(|l|l.visible&&!l.locked)
            ||self.clip_links.iter().any(|l|l.source_id==id||l.target_id==id){
            return Err("Use an ungrouped, unclipped vector on an editable layer".into());
        }
        if self.all_object_count()>=100_000||self.next_id>=u64::MAX-1{
            return Err("Object capacity exceeded".into());
        }
        let path=CubicPath::from_shape(&source.shape,tolerance)?;
        let shape=path.extract(start,end)?;
        let mut draft=self.clone();
        let index=draft.objects.iter().position(|o|o.id==id).ok_or("Source not found")?;
        let copy=draft.copy_object(&source,source.layer_id,0.,0.);
        let obj=draft.get_mut(copy).ok_or("Copy failed")?;
        obj.shape=shape;obj.style.fill="none".into();
        if obj.style.stroke=="none"||obj.style.stroke_width_mm<=0.{
            obj.style.stroke="#18283C".into();obj.style.stroke_width_mm=0.25;
        }
        draft.appearances.remove(&copy);
        let object=draft.objects.pop().ok_or("Copy failed")?;
        draft.objects.insert(index+1,object);
        draft.validate()?;*self=draft;Ok(copy)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::Style;
    fn p(x:f32,y:f32)->Point{Point{x,y}}
    fn curved()->Shape{Shape::Bezier{start:p(0.,0.),segments:vec![
        CubicSegment{control1:p(0.,100.),control2:p(100.,100.),end:p(100.,0.)}],closed:false}}
    #[test] fn exact_subcurve_matches_de_casteljau(){
        let path=CubicPath::from_shape(&curved(),0.001).unwrap();
        assert!((path.length_mm()-200.).abs()<0.001);
        let Shape::Bezier{start,segments,closed}=path.extract(0.,path.length_mm()*0.5).unwrap() else{panic!()};
        assert!(!closed);assert_eq!(start,p(0.,0.));assert_eq!(segments.len(),1);
        let s=&segments[0];
        assert!((s.end.x-50.).abs()<0.001&&(s.end.y-75.).abs()<0.001);
        assert!((s.control1.y-50.).abs()<0.001);
        assert!((s.control2.x-25.).abs()<0.001);
    }
    #[test] fn straight_nonuniform_speed_is_inverted(){
        let shape=Shape::Bezier{start:p(0.,0.),segments:vec![
            CubicSegment{control1:p(0.,0.),control2:p(0.,0.),end:p(100.,0.)}],closed:false};
        let c=CubicPath::from_shape(&shape,0.001).unwrap();
        let Shape::Bezier{start,segments,..}=c.extract(20.,60.).unwrap() else{panic!()};
        assert!((start.x-20.).abs()<0.001);
        assert!((segments[0].end.x-60.).abs()<0.001);
    }
    #[test] fn closed_seam_and_implicit_closure(){
        let shape=Shape::Bezier{start:p(0.,0.),segments:vec![
            Cubic::line([0.,0.],[10.,0.]).segment(),
            Cubic::line([10.,0.],[10.,10.]).segment(),
            Cubic::line([10.,10.],[0.,10.]).segment()],closed:true};
        let c=CubicPath::from_shape(&shape,0.001).unwrap();
        assert!((c.length_mm()-40.).abs()<0.001);
        let Shape::Bezier{start,segments,..}=c.extract(35.,45.).unwrap() else{panic!()};
        assert_eq!(start,p(0.,5.));assert_eq!(segments.len(),2);
        assert_eq!(segments[0].end,p(0.,0.));assert_eq!(segments[1].end,p(5.,0.));
    }
    #[test] fn interval_crosses_multiple_curves_and_skips_zero_segments(){
        let shape=Shape::Polyline{points:vec![p(0.,0.),p(10.,0.),p(10.,0.),p(10.,10.)],closed:false};
        let c=CubicPath::from_shape(&shape,0.001).unwrap();
        let Shape::Bezier{start,segments,..}=c.extract(5.,15.).unwrap() else{panic!()};
        assert_eq!(start,p(5.,0.));assert_eq!(segments.len(),2);assert_eq!(segments[1].end,p(10.,5.));
    }
    #[test] fn document_copy_is_atomic_and_native(){
        let mut d=Document::default();let id=d.add(curved(),Style::default());
        let before=d.clone();
        assert!(d.extract_cubic_interval(id,30.,999.,0.001).is_err());
        assert_eq!(d,before);
        let new=d.extract_cubic_interval(id,30.,70.,0.001).unwrap();
        assert_eq!(d.get(id),before.get(id));
        assert!(matches!(d.get(new).unwrap().shape,Shape::Bezier{..}));
        assert_eq!(d.get(new).unwrap().style.fill,"none");
        d.validate().unwrap();
    }
}
