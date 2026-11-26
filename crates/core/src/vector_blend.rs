//! Editable vector blends. Samples equal arc-length positions, aligns the
//! contour seams and interpolates geometry and supported solid-color styles.
//! Original objects are retained; intermediate shapes are native polylines.
use crate::{Document,Point,Shape,Style};
use crate::path_measure::{measure_shape,MeasuredContour};

const MAX_BLEND_POINTS:usize=250_000;
#[derive(Clone,Copy,Debug,PartialEq,serde::Serialize,serde::Deserialize)]
#[serde(rename_all="snake_case")]
pub enum BlendEasing{Linear,EaseIn,EaseOut,SmoothStep}
impl BlendEasing{
    pub fn map(self,t:f64)->f64{
        match self{Self::Linear=>t,Self::EaseIn=>t*t,
            Self::EaseOut=>1.-(1.-t)*(1.-t),Self::SmoothStep=>t*t*(3.-2.*t)}
    }
}
#[derive(Clone,Copy,Debug,PartialEq,serde::Serialize,serde::Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct BlendSettings{
    pub steps:u16,
    pub samples:u16,
    pub tolerance_mm:f64,
    pub align_seam:bool,
    pub allow_reverse:bool,
    pub easing:BlendEasing,
    pub interpolate_style:bool,
}
impl Default for BlendSettings{
    fn default()->Self{
        Self{steps:8,samples:96,tolerance_mm:0.05,align_seam:true,
            allow_reverse:true,easing:BlendEasing::Linear,interpolate_style:true}
    }
}
impl BlendSettings{
    pub fn validate(self)->Result<(),String>{
        if !(1..=256).contains(&self.steps)||!(3..=1024).contains(&self.samples)
            ||!self.tolerance_mm.is_finite()||!(0.001..=10.).contains(&self.tolerance_mm)
            ||usize::from(self.steps)*usize::from(self.samples)>MAX_BLEND_POINTS{
            return Err("Blend settings exceed supported bounds or point budget".into());
        }
        Ok(())
    }
}
#[derive(Clone,Debug,PartialEq)]
pub struct BlendPart{pub shape:Shape,pub style:Style}
fn lerp(a:f64,b:f64,t:f64)->f64{a+(b-a)*t}
fn mix(a:Point,b:Point,t:f64)->Point{
    Point{x:lerp(f64::from(a.x),f64::from(b.x),t) as f32,
        y:lerp(f64::from(a.y),f64::from(b.y),t) as f32}
}
fn sample(c:&MeasuredContour,n:usize)->Result<Vec<Point>,String>{
    let div=if c.closed{n}else{n-1};
    (0..n).map(|i|c.sample(c.length_mm()*i as f64/div as f64).map(|p|p.point)).collect()
}
fn centered(points:&[Point])->Vec<[f64;2]>{
    let n=points.len() as f64;
    let cx=points.iter().map(|p|f64::from(p.x)).sum::<f64>()/n;
    let cy=points.iter().map(|p|f64::from(p.y)).sum::<f64>()/n;
    let scale=(points.iter().map(|p|(f64::from(p.x)-cx).powi(2)
        +(f64::from(p.y)-cy).powi(2)).sum::<f64>()/n).sqrt().max(1e-12);
    points.iter().map(|p|[(f64::from(p.x)-cx)/scale,(f64::from(p.y)-cy)/scale]).collect()
}
fn index(i:usize,shift:usize,n:usize,reversed:bool,closed:bool)->usize{
    if closed{
        if reversed{(shift+n-i)%n}else{(i+shift)%n}
    }else if reversed{n-1-i}else{i}
}
fn align(a:&[Point],b:&mut Vec<Point>,closed:bool,s:BlendSettings){
    let n=a.len();let aa=centered(a);let bb=centered(b);
    let mut best=(f64::INFINITY,0,false);
    // Closed contours are cyclic; open contours keep their two endpoints.
    // The normalized score ignores translation and uniform scale.
    let shifts=if closed&&s.align_seam{n}else{1};
    for reversed in [false,true]{
        if reversed&&!s.allow_reverse{continue;}
        for shift in 0..shifts{
            let mut score=0.;
            for (i,p) in aa.iter().enumerate(){
                let q=bb[index(i,shift,n,reversed,closed)];
                score+=(p[0]-q[0]).powi(2)+(p[1]-q[1]).powi(2);
                if score>best.0{break;}
            }
            if score<best.0{best=(score,shift,reversed);}
        }
    }
    *b=(0..n).map(|i|b[index(i,best.1,n,best.2,closed)]).collect();
}
fn rgb(text:&str)->Option<[u8;3]>{
    let hex=text.strip_prefix('#')?;
    if !hex.is_ascii(){return None;}
    if hex.len()==6{
        Some([u8::from_str_radix(&hex[0..2],16).ok()?,
            u8::from_str_radix(&hex[2..4],16).ok()?,
            u8::from_str_radix(&hex[4..6],16).ok()?])
    }else if hex.len()==3{
        let mut out=[0;3];
        for (i,b) in hex.bytes().enumerate(){
            out[i]=(b as char).to_digit(16)? as u8*17;
        }
        Some(out)
    }else{None}
}
fn color(a:&str,b:&str,t:f64)->String{
    if let (Some(a),Some(b))=(rgb(a),rgb(b)){
        let c:[u8;3]=std::array::from_fn(|i|
            lerp(f64::from(a[i]),f64::from(b[i]),t).round().clamp(0.,255.) as u8);
        format!("#{:02X}{:02X}{:02X}",c[0],c[1],c[2])
    }else if t<0.5{a.to_owned()}else{b.to_owned()}
}
fn blend_style(a:&Style,b:&Style,t:f64,closed:bool,interpolate:bool)->Style{
    let mut style=if interpolate{
        Style{fill:color(&a.fill,&b.fill,t),stroke:color(&a.stroke,&b.stroke,t),
            stroke_width_mm:lerp(f64::from(a.stroke_width_mm),f64::from(b.stroke_width_mm),t) as f32}
    }else{a.clone()};
    if !closed{style.fill="none".into();}
    style
}
pub fn blend_shapes(a:&Shape,sa:&Style,b:&Shape,sb:&Style,s:BlendSettings)
    ->Result<Vec<BlendPart>,String>{
    s.validate()?;
    let aa=measure_shape(a,s.tolerance_mm)?;
    let bb=measure_shape(b,s.tolerance_mm)?;
    if aa.len()!=1||bb.len()!=1{
        return Err("Blend requires two single-contour vectors".into());
    }
    let (a,b)=(&aa[0],&bb[0]);
    if a.closed!=b.closed{return Err("Both blend contours must have the same closure".into());}
    let n=usize::from(s.samples);
    let pa=sample(a,n)?;
    let mut pb=sample(b,n)?;
    align(&pa,&mut pb,a.closed,s);
    let mut output=Vec::with_capacity(usize::from(s.steps));
    for step in 1..=s.steps{
        let t=s.easing.map(f64::from(step)/(f64::from(s.steps)+1.));
        let mut points=Vec::with_capacity(n);
        for (a,b) in pa.iter().zip(&pb){
            let p=mix(*a,*b,t);
            if points.last()!=Some(&p){points.push(p);}
        }
        if a.closed&&points.first()==points.last(){points.pop();}
        if points.len()<if a.closed{3}else{2}{
            return Err("An intermediate blend contour collapsed; adjust alignment".into());
        }
        output.push(BlendPart{shape:Shape::Polyline{points,closed:a.closed},
            style:blend_style(sa,sb,t,a.closed,s.interpolate_style)});
    }
    Ok(output)
}
impl Document{
    /// Intermediate objects are inserted in the source layer above both
    /// originals in drawing order. Effects on endpoints remain untouched;
    /// intermediates intentionally interpolate only their base styles.
    pub fn blend_objects(&mut self,first:u64,second:u64,s:BlendSettings)->Result<Vec<u64>,String>{
        if first==second{return Err("Choose two distinct vectors".into());}
        let a=self.get(first).ok_or("First object not found")?;
        let b=self.get(second).ok_or("Second object not found")?;
        for o in [a,b]{
            if o.group_id.is_some()||!self.layer(o.layer_id).is_some_and(|l|l.visible&&!l.locked)
                ||self.clip_links.iter().any(|l|l.source_id==o.id||l.target_id==o.id){
                return Err("Blend requires ungrouped, unclipped vectors on editable layers".into());
            }
        }
        if a.layer_id!=b.layer_id{return Err("Blend endpoints must share one layer".into());}
        s.validate()?;
        let count=usize::from(s.steps);
        if self.all_object_count().saturating_add(count)>100_000
            ||self.next_id.checked_add(count as u64).is_none_or(|v|v==u64::MAX){
            return Err("Document object limit exceeded".into());
        }
        let parts=blend_shapes(&a.shape,&a.style,&b.shape,&b.style,s)?;
        let layer=a.layer_id;
        let index=self.objects.iter().rposition(|o|o.id==first||o.id==second)
            .ok_or("Blend source not found")?+1;
        let mut draft=self.clone();
        let mut ids=Vec::with_capacity(count);
        for (i,part) in parts.into_iter().enumerate(){
            let id=draft.add_to_layer(part.shape,part.style,layer);
            draft.get_mut(id).ok_or("Allocation failed")?.name=format!("Blend {}",i+1);
            ids.push(id);
        }
        let tail=draft.objects.split_off(draft.objects.len()-ids.len());
        draft.objects.splice(index..index,tail);
        draft.validate()?;*self=draft;
        Ok(ids)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::Bounds;
    fn p(x:f32,y:f32)->Point{Point{x,y}}
    fn line(a:Point,b:Point)->Shape{Shape::Polyline{points:vec![a,b],closed:false}}
    #[test] fn open_reversal_and_color_interpolation(){
        let a=line(p(0.,0.),p(10.,0.));
        let b=line(p(10.,10.),p(0.,10.));
        let sa=Style{fill:"none".into(),stroke:"#FF0000".into(),stroke_width_mm:1.};
        let sb=Style{fill:"none".into(),stroke:"#0000FF".into(),stroke_width_mm:3.};
        let r=blend_shapes(&a,&sa,&b,&sb,BlendSettings{steps:1,samples:3,..Default::default()}).unwrap();
        assert_eq!(r[0].shape,Shape::Polyline{points:vec![p(0.,5.),p(5.,5.),p(10.,5.)],closed:false});
        assert_eq!(r[0].style.stroke,"#800080");
        assert_eq!(r[0].style.stroke_width_mm,2.);
    }
    #[test] fn closed_alignment_handles_shift_and_reverse(){
        let a=Shape::Rectangle{x:0.,y:0.,w:10.,h:10.};
        let b=Shape::Polyline{points:vec![p(30.,10.),p(30.,0.),p(20.,0.),p(20.,10.)],closed:true};
        let style=Style::default();
        let r=blend_shapes(&a,&style,&b,&style,BlendSettings{steps:1,samples:4,..Default::default()}).unwrap();
        assert_eq!(r[0].shape.bounds(),Bounds{x:10.,y:0.,w:10.,h:10.});
        assert!(matches!(&r[0].shape,Shape::Polyline{closed:true,points} if points.len()==4));
    }
    #[test] fn easing_is_monotone_and_bounded(){
        for e in [BlendEasing::Linear,BlendEasing::EaseIn,BlendEasing::EaseOut,BlendEasing::SmoothStep]{
            assert_eq!(e.map(0.),0.);assert_eq!(e.map(1.),1.);
            let mut previous=0.;
            for i in 1..=100{let v=e.map(f64::from(i)/100.);assert!(v>=previous&&v<=1.);previous=v;}
        }
    }
    #[test] fn invalid_settings_and_mixed_closure_are_rejected(){
        let a=line(p(0.,0.),p(10.,0.));
        let b=Shape::Rectangle{x:0.,y:0.,w:10.,h:10.};
        let s=Style::default();
        assert!(blend_shapes(&a,&s,&b,&s,BlendSettings::default()).is_err());
        assert!(BlendSettings{steps:256,samples:1024,..Default::default()}.validate().is_err());
        assert!(BlendSettings{tolerance_mm:f64::NAN,..Default::default()}.validate().is_err());
    }
    #[test] fn document_blend_keeps_sources_and_z_order(){
        let mut d=Document::default();
        let a=d.add(line(p(0.,0.),p(10.,0.)),Style::default());
        let b=d.add(line(p(0.,10.),p(10.,10.)),Style::default());
        let c=d.add(line(p(0.,20.),p(10.,20.)),Style::default());
        let before=d.clone();
        let ids=d.blend_objects(a,b,BlendSettings{steps:3,..Default::default()}).unwrap();
        assert_eq!(ids.len(),3);
        assert_eq!(d.objects.iter().map(|o|o.id).collect::<Vec<_>>(),vec![a,b,ids[0],ids[1],ids[2],c]);
        assert_eq!(d.get(a),before.get(a));assert_eq!(d.get(b),before.get(b));
        d.validate().unwrap();
        d.layers[0].locked=true;let before=d.clone();
        assert!(d.blend_objects(a,b,BlendSettings::default()).is_err());
        assert_eq!(d,before);
    }
}
