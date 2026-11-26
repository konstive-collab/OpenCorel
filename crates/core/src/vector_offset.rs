//! Filled vector offsets and conversion of solid centerline strokes into
//! editable filled contours. Uses the same topology backend as Boolean ops.
use crate::{Document,Shape,Style};
use crate::boolean_ops::{shape_to_polygon,polygon_to_shape};
use crate::path_measure::measure_shape;
use geo::{Area,BooleanOps,Buffer,Coord,LineString,MultiLineString,MultiPolygon};
use geo::algorithm::buffer::{BufferStyle,LineJoin,LineCap};

#[derive(Clone,Copy,Debug,PartialEq,Eq,serde::Serialize,serde::Deserialize)]
#[serde(rename_all="snake_case")]
pub enum OffsetMode{Region,Border,Stroke}
#[derive(Clone,Copy,Debug,PartialEq,Eq,serde::Serialize,serde::Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Corner{Round,Bevel,Miter}
#[derive(Clone,Copy,Debug,PartialEq,serde::Serialize,serde::Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct OffsetSettings{
    pub mode:OffsetMode,
    /// Signed region or border distance, positive stroke width.
    pub distance_mm:f64,
    pub corner:Corner,
    pub square_cap:bool,
    pub tolerance_mm:f64,
}
impl Default for OffsetSettings{
    fn default()->Self{Self{mode:OffsetMode::Region,distance_mm:2.,corner:Corner::Round,
        square_cap:false,tolerance_mm:0.05}}
}
impl OffsetSettings{
    pub fn validate(self)->Result<(),String>{
        if !self.distance_mm.is_finite()||self.distance_mm.abs()<0.001
            ||self.distance_mm.abs()>5000.||!self.tolerance_mm.is_finite()
            ||!(0.001..=10.).contains(&self.tolerance_mm)
            ||(self.mode==OffsetMode::Stroke&&self.distance_mm<=0.){
            return Err("Invalid offset distance, stroke width or tolerance".into());
        }
        Ok(())
    }
}
fn style(distance:f64,s:OffsetSettings)->BufferStyle<f64>{
    let mut style=BufferStyle::new(distance);
    style=match s.corner{
        Corner::Round=>style,
        Corner::Bevel=>style.line_join(LineJoin::Bevel),
        // geo's miter setting is the minimum included angle in radians,
        // not the SVG miter-length ratio. About 28.6 degrees avoids spikes.
        Corner::Miter=>style.line_join(LineJoin::Miter(0.5)),
    };
    if s.square_cap{style=style.line_cap(LineCap::Square);}
    style
}
fn buffer(region:&MultiPolygon<f64>,distance:f64,s:OffsetSettings)->MultiPolygon<f64>{
    region.buffer_with_style(style(distance,s))
}
fn calculate(shape:&Shape,s:OffsetSettings)->Result<Option<Shape>,String>{
    s.validate()?;
    let result=match s.mode{
        OffsetMode::Region|OffsetMode::Border=>{
            let region=shape_to_polygon(shape)?;
            let shifted=buffer(&region,s.distance_mm,s);
            if s.mode==OffsetMode::Region{shifted}
            else if s.distance_mm>0.{shifted.difference(&region)}
            else{region.difference(&shifted)}
        }
        OffsetMode::Stroke=>{
            // For a filled closed contour, outer-minus-inner also handles
            // holes and the seam without applying open-line caps there.
            let contours=measure_shape(shape,s.tolerance_mm)?;
            if contours.iter().map(|c|c.vertices.len()).sum::<usize>()>20_000{
                return Err("Stroke conversion exceeds input complexity budget".into());
            }
            let half=s.distance_mm*0.5;
            if contours.iter().all(|c|c.closed){
                let region=shape_to_polygon(shape)?;
                buffer(&region,half,s).difference(&buffer(&region,-half,s))
            }else{
                if contours.iter().any(|c|c.closed){
                    return Err("Mixed open and closed contours are not supported for stroke conversion".into());
                }
                let lines=MultiLineString(contours.iter().map(|c|LineString(
                    c.vertices.iter().map(|p|Coord{x:f64::from(p.x),y:f64::from(p.y)}).collect()
                )).collect());
                lines.buffer_with_style(style(half,s))
            }
        }
    };
    if !result.unsigned_area().is_finite(){return Err("Nonfinite offset geometry".into());}
    polygon_to_shape(&result)
}
/// Offset output has bounded polygon complexity and consists of straight
/// cubic segments. An empty inward region is not an error until insertion.
pub fn offset_shape(shape:&Shape,s:OffsetSettings)->Result<Option<Shape>,String>{
    std::panic::catch_unwind(||calculate(shape,s))
        .unwrap_or_else(|_|Err("Offset failed on malformed contours".into()))
}
impl Document{
    /// Retain the original and insert a new editable filled outline above it.
    /// This converts a solid geometric stroke, not gradients, dash patterns
    /// or other appearance effects.
    pub fn create_offset_object(&mut self,id:u64,s:OffsetSettings,use_source_stroke:bool)
        ->Result<u64,String>{
        let source=self.get(id).ok_or("Source not found")?.clone();
        if source.group_id.is_some()||!self.layer(source.layer_id).is_some_and(|l|l.visible&&!l.locked)
            ||self.clip_links.iter().any(|l|l.source_id==id||l.target_id==id){
            return Err("Use an ungrouped, unclipped vector on an editable layer".into());
        }
        if self.all_object_count()>=100_000||self.next_id>=u64::MAX-1{
            return Err("Object capacity exceeded".into());
        }
        let mut settings=s;
        if use_source_stroke{
            if s.mode!=OffsetMode::Stroke||source.style.stroke=="none"
                ||source.style.stroke_width_mm<=0.{
                return Err("Source does not have a solid positive-width stroke".into());
            }
            settings.distance_mm=f64::from(source.style.stroke_width_mm);
        }
        let shape=offset_shape(&source.shape,settings)?.ok_or("Offset region is empty")?;
        let preferred=if s.mode==OffsetMode::Stroke{&source.style.stroke}else{&source.style.fill};
        let fill=if preferred=="none"{"#18283C".into()}else{preferred.clone()};
        let mut draft=self.clone();
        let position=draft.objects.iter().position(|o|o.id==id).ok_or("Source not found")?;
        let copy=draft.add_to_layer(shape,Style{fill,stroke:"none".into(),stroke_width_mm:0.},source.layer_id);
        draft.get_mut(copy).ok_or("Allocation failed")?.name=match s.mode{
            OffsetMode::Region=>"Offset region",OffsetMode::Border=>"Offset border",
            OffsetMode::Stroke=>"Expanded stroke"}.into();
        let object=draft.objects.pop().ok_or("Allocation failed")?;
        draft.objects.insert(position+1,object);
        draft.validate()?;*self=draft;Ok(copy)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Point,Bounds};
    fn rect()->Shape{Shape::Rectangle{x:10.,y:10.,w:20.,h:20.}}
    fn area(shape:&Shape)->f64{shape_to_polygon(shape).unwrap().unsigned_area()}
    #[test] fn outward_and_inward_offsets(){
        let a=offset_shape(&rect(),OffsetSettings{distance_mm:2.,corner:Corner::Bevel,..Default::default()}).unwrap().unwrap();
        let b=offset_shape(&rect(),OffsetSettings{distance_mm:-2.,..Default::default()}).unwrap().unwrap();
        assert!((area(&a)-568.).abs()<0.1);
        assert!((area(&b)-256.).abs()<0.1);
        assert!(offset_shape(&rect(),OffsetSettings{distance_mm:-11.,..Default::default()}).unwrap().is_none());
    }
    #[test] fn border_preserves_a_hole(){
        let a=offset_shape(&rect(),OffsetSettings{mode:OffsetMode::Border,distance_mm:2.,
            corner:Corner::Bevel,..Default::default()}).unwrap().unwrap();
        assert!((area(&a)-168.).abs()<0.1);
        assert!(matches!(a,Shape::CompoundPath{contours} if contours.len()==2));
    }
    #[test] fn closed_stroke_is_filled_ring(){
        let a=offset_shape(&rect(),OffsetSettings{mode:OffsetMode::Stroke,distance_mm:4.,
            corner:Corner::Bevel,..Default::default()}).unwrap().unwrap();
        assert!((area(&a)-312.).abs()<0.1);
        assert!(matches!(a,Shape::CompoundPath{contours} if contours.len()==2));
    }
    #[test] fn open_line_square_caps_extend_half_width(){
        let line=Shape::Polyline{points:vec![Point{x:10.,y:10.},Point{x:30.,y:10.}],closed:false};
        let a=offset_shape(&line,OffsetSettings{mode:OffsetMode::Stroke,distance_mm:4.,
            square_cap:true,..Default::default()}).unwrap().unwrap();
        let b=a.bounds();
        let expected=Bounds{x:8.,y:8.,w:24.,h:4.};
        assert!((b.x-expected.x).abs()<0.01&&(b.y-expected.y).abs()<0.01);
        assert!((b.w-expected.w).abs()<0.01&&(b.h-expected.h).abs()<0.01);
        assert!((area(&a)-96.).abs()<0.1);
    }
    #[test] fn document_expansion_preserves_original_and_style(){
        let mut d=Document::default();
        let id=d.add(rect(),Style{fill:"#FFFFFF".into(),stroke:"#123456".into(),stroke_width_mm:2.});
        let before=d.clone();
        let out=d.create_offset_object(id,OffsetSettings{mode:OffsetMode::Stroke,..Default::default()},true).unwrap();
        assert_eq!(d.get(id),before.get(id));
        assert_eq!(d.get(out).unwrap().style.fill,"#123456");
        assert_eq!(d.get(out).unwrap().style.stroke,"none");
        d.validate().unwrap();
        let before=d.clone();
        assert!(d.create_offset_object(id,OffsetSettings{distance_mm:-50.,..Default::default()},false).is_err());
        assert_eq!(d,before);
    }
}
