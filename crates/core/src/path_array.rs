//! Distribution of editable vector motifs along an arbitrary measurable path.
use crate::{Document,Point,Shape};
use crate::path_measure::{measure_shape,MeasuredContour};

#[derive(Clone,Copy,Debug,PartialEq,serde::Serialize,serde::Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Spacing{Count(u16),StepMm(f32)}
#[derive(Clone,Copy,Debug,PartialEq,serde::Serialize,serde::Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct PathArraySettings{
    pub spacing:Spacing,
    pub start_offset_mm:f32,
    pub end_offset_mm:f32,
    pub normal_offset_mm:f32,
    pub rotate_to_tangent:bool,
    pub angle_offset_deg:f32,
    pub start_scale:f32,
    pub end_scale:f32,
    pub tolerance_mm:f32,
}
impl Default for PathArraySettings{
    fn default()->Self{Self{spacing:Spacing::Count(10),start_offset_mm:0.,
        end_offset_mm:0.,normal_offset_mm:0.,rotate_to_tangent:true,
        angle_offset_deg:0.,start_scale:1.,end_scale:1.,tolerance_mm:0.05}}
}
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Placement{pub center:Point,pub angle_deg:f32,pub scale:f32}
impl PathArraySettings{
    pub fn validate(self)->Result<(),String>{
        if ![self.start_offset_mm,self.end_offset_mm,self.normal_offset_mm,self.angle_offset_deg,
            self.start_scale,self.end_scale,self.tolerance_mm].iter().all(|v|v.is_finite())
            ||self.start_offset_mm<0.||self.end_offset_mm<0.||self.normal_offset_mm.abs()>100_000.
            ||self.angle_offset_deg.abs()>360.
            ||!(0.01..=100.).contains(&self.start_scale)||!(0.01..=100.).contains(&self.end_scale)
            ||!(0.001..=10.).contains(&self.tolerance_mm){
            return Err("Invalid path array parameters".into());
        }
        match self.spacing{
            Spacing::Count(n) if (1..=1000).contains(&n)=>Ok(()),
            Spacing::StepMm(v) if v.is_finite()&&(0.01..=100_000.).contains(&v)=>Ok(()),
            _=>Err("Path array requires 1–1000 placements or positive spacing".into()),
        }
    }
}
pub fn placements(contour:&MeasuredContour,s:PathArraySettings)->Result<Vec<Placement>,String>{
    s.validate()?;
    let length=contour.length_mm();
    let start=f64::from(s.start_offset_mm);
    let end=length-f64::from(s.end_offset_mm);
    let span=end-start;
    if !span.is_finite()||span<=0.{return Err("Path offsets consume the whole contour".into());}
    let full_closed=contour.closed&&s.start_offset_mm==0.&&s.end_offset_mm==0.;
    let (count,step)=match s.spacing{
        Spacing::Count(n)=>{
            let n=usize::from(n);
            let div=if full_closed{n}else{n.saturating_sub(1)};
            (n,if div==0{0.}else{span/div as f64})
        }
        Spacing::StepMm(step)=>{
            let step=f64::from(step);
            let n=if full_closed{(span/step).ceil()}else{(span/step).floor()+1.};
            if n>1000.{return Err("Path array exceeds 1000 placements".into());}
            (n.max(1.) as usize,step)
        }
    };
    let mut result=Vec::with_capacity(count);
    for i in 0..count{
        let at=start+(i as f64*step).min(span);
        let sample=contour.sample(at)?;
        let t=((at-start)/span) as f32;
        let center=Point{x:sample.point.x-sample.tangent.y*s.normal_offset_mm,
            y:sample.point.y+sample.tangent.x*s.normal_offset_mm};
        let angle=if s.rotate_to_tangent{
            sample.tangent.y.atan2(sample.tangent.x).to_degrees()
        }else{0.};
        result.push(Placement{center,angle_deg:angle+s.angle_offset_deg,
            scale:s.start_scale+(s.end_scale-s.start_scale)*t});
    }
    Ok(result)
}
fn is_vector(shape:&Shape)->bool{
    matches!(shape,Shape::Rectangle{..}|Shape::Ellipse{..}|Shape::Polyline{..}|
        Shape::Bezier{..}|Shape::CompoundPath{..})
}
impl Document{
    /// Keep the guide and original motif untouched. Generated objects are
    /// ordinary native vectors, not an opaque procedural or external resource.
    pub fn repeat_along_path(&mut self,guide:u64,motif:u64,s:PathArraySettings)->Result<Vec<u64>,String>{
        if guide==motif{return Err("Choose distinct guide and motif objects".into());}
        let path=self.get(guide).ok_or("Guide object not found")?;
        let source=self.get(motif).ok_or("Motif object not found")?.clone();
        for o in [path,&source]{
            if !is_vector(&o.shape)||o.group_id.is_some()
                ||!self.layer(o.layer_id).is_some_and(|l|l.visible&&!l.locked)
                ||self.clip_links.iter().any(|link|link.source_id==o.id||link.target_id==o.id){
                return Err("Path array needs ungrouped, unclipped native vector objects on editable layers".into());
            }
        }
        s.validate()?;
        let contours=measure_shape(&path.shape,f64::from(s.tolerance_mm))?;
        if contours.len()!=1{return Err("Guide must contain exactly one contour".into());}
        let positions=placements(&contours[0],s)?;
        if self.all_object_count().saturating_add(positions.len())>100_000
            ||self.next_id.checked_add(positions.len() as u64).is_none_or(|n|n==u64::MAX){
            return Err("Document object capacity exceeded".into());
        }
        let bounds=source.bounds();
        let pivot=Point{x:bounds.x+bounds.w*0.5,y:bounds.y+bounds.h*0.5};
        // Bound the cloned geometry before doing expensive transformations.
        let nodes=match &source.shape{
            Shape::Polyline{points,..}=>points.len(),
            Shape::Bezier{segments,..}=>segments.len()*3+1,
            Shape::CompoundPath{contours}=>contours.iter().map(|c|c.segments.len()*3+1).sum(),
            _=>16,
        };
        if nodes.saturating_mul(positions.len())>1_000_000{
            return Err("Path array exceeds one million geometry points".into());
        }
        let mut shapes=Vec::with_capacity(positions.len());
        for position in positions{
            let mut shape=source.shape.clone();
            crate::transform::scale_shape(&mut shape,pivot,position.scale,position.scale)?;
            crate::transform::rotate_shape(&mut shape,pivot,position.angle_deg)?;
            shape.translate(position.center.x-pivot.x,position.center.y-pivot.y);
            shapes.push(shape);
        }
        let mut draft=self.clone();
        let mut ids=Vec::with_capacity(shapes.len());
        for shape in shapes{
            let id=draft.copy_object(&source,source.layer_id,0.,0.);
            let o=draft.get_mut(id).ok_or("Copy failed")?;
            o.shape=shape;
            o.name=if source.name.is_empty(){"Path array".into()}else{source.name.clone()};
            ids.push(id);
        }
        draft.validate()?;
        *self=draft;
        Ok(ids)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Style,Bounds};
    fn p(x:f32,y:f32)->Point{Point{x,y}}
    #[test] fn open_count_and_scale(){
        let shape=Shape::Polyline{points:vec![p(0.,0.),p(100.,0.)],closed:false};
        let c=measure_shape(&shape,0.01).unwrap();
        let a=placements(&c[0],PathArraySettings{spacing:Spacing::Count(3),normal_offset_mm:5.,
            start_scale:1.,end_scale:2.,..Default::default()}).unwrap();
        assert_eq!(a.iter().map(|v|v.center).collect::<Vec<_>>(),vec![p(0.,5.),p(50.,5.),p(100.,5.)]);
        assert_eq!(a[1].scale,1.5);
    }
    #[test] fn closed_count_never_repeats_seam(){
        let c=measure_shape(&Shape::Rectangle{x:0.,y:0.,w:10.,h:10.},0.01).unwrap();
        let a=placements(&c[0],PathArraySettings{spacing:Spacing::Count(4),..Default::default()}).unwrap();
        assert_eq!(a.iter().map(|v|v.center).collect::<Vec<_>>(),vec![p(0.,0.),p(10.,0.),p(10.,10.),p(0.,10.)]);
        assert_eq!(a[1].angle_deg,90.);
    }
    #[test] fn step_with_offsets_and_limits(){
        let c=measure_shape(&Shape::Polyline{points:vec![p(0.,0.),p(100.,0.)],closed:false},0.01).unwrap();
        let a=placements(&c[0],PathArraySettings{spacing:Spacing::StepMm(25.),
            start_offset_mm:10.,end_offset_mm:10.,..Default::default()}).unwrap();
        assert_eq!(a.iter().map(|v|v.center.x).collect::<Vec<_>>(),vec![10.,35.,60.,85.]);
        assert!(placements(&c[0],PathArraySettings{spacing:Spacing::StepMm(0.01),..Default::default()}).is_err());
    }
    #[test] fn native_array_preserves_source_and_atomic_failure(){
        let mut d=Document::default();
        let guide=d.add(Shape::Polyline{points:vec![p(10.,10.),p(110.,10.)],closed:false},Style::default());
        let motif=d.add(Shape::Rectangle{x:20.,y:30.,w:4.,h:6.},Style::default());
        let before=d.clone();
        let ids=d.repeat_along_path(guide,motif,PathArraySettings{spacing:Spacing::Count(3),..Default::default()}).unwrap();
        assert_eq!(ids.len(),3);
        assert_eq!(d.get(ids[1]).unwrap().bounds(),Bounds{x:58.,y:7.,w:4.,h:6.});
        assert_eq!(d.get(guide),before.get(guide));
        assert_eq!(d.get(motif),before.get(motif));
        d.validate().unwrap();
        let before=d.clone();
        assert!(d.repeat_along_path(guide,motif,PathArraySettings{start_offset_mm:150.,..Default::default()}).is_err());
        assert_eq!(d,before);
    }
}
