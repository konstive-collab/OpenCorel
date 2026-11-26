//! Distance-based extraction and resampling of native vector paths.
//! Output is explicitly an approximation made of editable straight segments.
//! Use the existing cubic split tool when exact Bézier preservation is needed.
use crate::{Document,Point,Shape};
use crate::path_measure::{measure_shape,MeasuredContour};

const MAX_OUTPUT_VERTICES:usize=60_000;

#[derive(Clone,Copy,Debug,PartialEq)]
pub enum PathRange{
    /// Extract an interval, or wrap across the seam of a closed contour.
    Interval{start_mm:f64,end_mm:f64},
    /// Sample the complete contour at a uniformly distributed spacing.
    Uniform{maximum_step_mm:f64},
}

fn append(points:&mut Vec<Point>,p:Point)->Result<(),String>{
    if points.last()==Some(&p){return Ok(());}
    if points.len()>=MAX_OUTPUT_VERTICES{return Err("Output vertex limit exceeded".into());}
    points.push(p);Ok(())
}
fn interval(c:&MeasuredContour,start:f64,end:f64)->Result<Vec<Point>,String>{
    let length=c.length_mm();
    if !start.is_finite()||!end.is_finite()||start<0.||start>=length
        ||end<=start||end-start>length+1e-9||(!c.closed&&end>length){
        return Err("Invalid extraction interval".into());
    }
    let mut points=Vec::new();
    append(&mut points,c.sample(start)?.point)?;
    // Traverse the same flattened contour, including all corners in the
    // extracted interval. The second cycle allows a closed seam crossing.
    let cycles=if c.closed&&end>length{2}else{1};
    for cycle in 0..cycles{
        let shift=cycle as f64*length;
        let from=c.distances.partition_point(|d|*d+shift<=start);
        let to=c.distances.partition_point(|d|*d+shift<end);
        for p in &c.vertices[from.min(to)..to]{append(&mut points,*p)?;}
    }
    append(&mut points,c.sample(end)?.point)?;
    if points.len()<2{return Err("Extracted path is degenerate".into());}
    Ok(points)
}
fn uniform(c:&MeasuredContour,maximum_step_mm:f64)->Result<Vec<Point>,String>{
    if !maximum_step_mm.is_finite()||!(0.001..=100_000.).contains(&maximum_step_mm){
        return Err("Invalid resampling step".into());
    }
    let length=c.length_mm();
    let segments=(length/maximum_step_mm).ceil().max(if c.closed{3.}else{1.});
    if segments>=MAX_OUTPUT_VERTICES as f64{
        return Err("Resampling would exceed the vertex limit".into());
    }
    let n=segments as usize;
    let count=if c.closed{n}else{n+1};
    let mut points=Vec::with_capacity(count);
    for i in 0..count{
        append(&mut points,c.sample(length*i as f64/n as f64)?.point)?;
    }
    if points.len()<if c.closed{3}else{2}{return Err("Resampled path is degenerate".into());}
    Ok(points)
}
pub fn extract_path(shape:&Shape,range:PathRange,tolerance_mm:f64)->Result<Shape,String>{
    let contours=measure_shape(shape,tolerance_mm)?;
    if contours.len()!=1{return Err("Select a single contour, not a compound path".into());}
    let c=&contours[0];
    match range{
        PathRange::Interval{start_mm,end_mm}=>Ok(Shape::Polyline{
            points:interval(c,start_mm,end_mm)?,closed:false}),
        PathRange::Uniform{maximum_step_mm}=>Ok(Shape::Polyline{
            points:uniform(c,maximum_step_mm)?,closed:c.closed}),
    }
}
impl Document{
    /// Add an approximation immediately above its source. In particular,
    /// extraction never changes the source's closure, gradient or clipping.
    pub fn extract_path_object(&mut self,id:u64,range:PathRange,tolerance_mm:f64)
        ->Result<u64,String>{
        let source=self.get(id).ok_or("Source object not found")?.clone();
        if source.group_id.is_some()||!self.layer(source.layer_id).is_some_and(|l|l.visible&&!l.locked)
            ||self.clip_links.iter().any(|l|l.source_id==id||l.target_id==id){
            return Err("Use an ungrouped, unclipped vector on an editable layer".into());
        }
        if self.all_object_count()>=100_000||self.next_id>=u64::MAX-1{
            return Err("Document object limit reached".into());
        }
        let shape=extract_path(&source.shape,range,tolerance_mm)?;
        let mut draft=self.clone();
        let position=draft.objects.iter().position(|o|o.id==id).ok_or("Source not found")?;
        let copy=draft.copy_object(&source,source.layer_id,0.,0.);
        let o=draft.get_mut(copy).ok_or("Cannot allocate extracted path")?;
        o.shape=shape;
        if matches!(range,PathRange::Interval{..}){
            o.style.fill="none".into();
            // A trimmed open curve is a stroke, not an implicitly closed
            // region. Drop the source fill effects and ensure it is visible.
            if o.style.stroke=="none"||o.style.stroke_width_mm<=0.{
                o.style.stroke="#18283C".into();o.style.stroke_width_mm=0.25;
            }
            draft.appearances.remove(&copy);
        }
        let item=draft.objects.pop().ok_or("Cannot allocate extracted path")?;
        draft.objects.insert(position+1,item);
        draft.validate()?;
        *self=draft;
        Ok(copy)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::Style;
    fn p(x:f32,y:f32)->Point{Point{x,y}}
    fn line()->Shape{Shape::Polyline{points:vec![p(0.,0.),p(10.,0.),p(10.,10.)],closed:false}}
    #[test] fn interval_retains_intermediate_corners(){
        let shape=extract_path(&line(),PathRange::Interval{start_mm:5.,end_mm:15.},0.01).unwrap();
        assert_eq!(shape,Shape::Polyline{points:vec![p(5.,0.),p(10.,0.),p(10.,5.)],closed:false});
    }
    #[test] fn closed_interval_wraps_without_duplicate_seam(){
        let r=Shape::Rectangle{x:0.,y:0.,w:10.,h:10.};
        let shape=extract_path(&r,PathRange::Interval{start_mm:35.,end_mm:45.},0.01).unwrap();
        assert_eq!(shape,Shape::Polyline{points:vec![p(0.,5.),p(0.,0.),p(5.,0.)],closed:false});
    }
    #[test] fn uniform_samples_include_endpoints_and_avoid_closed_duplicate(){
        let shape=extract_path(&line(),PathRange::Uniform{maximum_step_mm:5.},0.01).unwrap();
        assert_eq!(shape,Shape::Polyline{points:vec![p(0.,0.),p(5.,0.),p(10.,0.),p(10.,5.),p(10.,10.)],closed:false});
        let r=Shape::Rectangle{x:0.,y:0.,w:10.,h:10.};
        let shape=extract_path(&r,PathRange::Uniform{maximum_step_mm:10.},0.01).unwrap();
        assert_eq!(shape,Shape::Polyline{points:vec![p(0.,0.),p(10.,0.),p(10.,10.),p(0.,10.)],closed:true});
    }
    #[test] fn invalid_ranges_and_excessive_sampling_fail(){
        for range in [PathRange::Interval{start_mm:15.,end_mm:25.},
            PathRange::Interval{start_mm:5.,end_mm:5.},
            PathRange::Interval{start_mm:f64::NAN,end_mm:5.},
            PathRange::Uniform{maximum_step_mm:0.}]{
            assert!(extract_path(&line(),range,0.01).is_err());
        }
        let long=Shape::Polyline{points:vec![p(0.,0.),p(100_000.,0.)],closed:false};
        assert!(extract_path(&long,PathRange::Uniform{maximum_step_mm:0.001},0.01).is_err());
    }
    #[test] fn document_extraction_retains_original_and_is_transactional(){
        let mut d=Document::default();
        let id=d.add(line(),Style::default());
        let old=d.clone();
        let new=d.extract_path_object(id,PathRange::Interval{start_mm:1.,end_mm:19.},0.01).unwrap();
        assert_eq!(d.get(id),old.get(id));
        assert_eq!(d.get(new).unwrap().style.fill,"none");
        assert_eq!(d.objects[1].id,new);
        d.validate().unwrap();
        d.layers[0].locked=true;
        let old=d.clone();
        assert!(d.extract_path_object(id,PathRange::Uniform{maximum_step_mm:1.},0.01).is_err());
        assert_eq!(d,old);
    }
}
