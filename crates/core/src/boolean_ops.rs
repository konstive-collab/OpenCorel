//! Editable closed-path boolean operations for print-oriented vector art.
 //! Geometry is in millimetres, flattened to <=0.1mm chord deviation.
 //! Inputs are never mutated. Only well-defined, nondegenerate filled paths
 //! can participate. The resulting geometry is a KDR-editable CompoundPath.
use crate::{Contour,CubicSegment,Point,Shape};
use geo::{Area,BooleanOps,Coord,LineString,MultiPolygon,Polygon};
use std::f64::consts::{PI,TAU};

const FLATTEN_TOL_MM:f64=0.1;
const MAX_INPUT_POINTS:usize=20_000;
const MAX_OUTPUT_POINTS:usize=25_000;
const MAX_CONTOURS:usize=128;
const MAX_DEPTH:u32=12;
const MAX_ABS_MM:f64=200_000.0;
type XY=(f64,f64);

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum BooleanOperation{
    Union,Subtract,Intersect,Xor
}
impl BooleanOperation{
    pub fn label(self)->&'static str{
        match self{
            Self::Union=>"Объединение",
            Self::Subtract=>"Вычитание",
            Self::Intersect=>"Пересечение",
            Self::Xor=>"Исключение",
        }
    }
}
fn valid_xy(p:XY)->bool{
    p.0.is_finite()&&p.1.is_finite()
        &&p.0.abs()<=MAX_ABS_MM&&p.1.abs()<=MAX_ABS_MM
}
fn xy(p:Point)->Result<XY,String>{
    let v=(f64::from(p.x),f64::from(p.y));
    if !valid_xy(v){return Err("Vector coordinates are out of supported bounds".into());}
    Ok(v)
}
fn append(out:&mut Vec<XY>,point:XY)->Result<(),String>{
    if !valid_xy(point){return Err("Invalid nonfinite/oversized coordinates".into());}
    if out.last().map(|p|(p.0-point.0).abs()<1e-10 &&
        (p.1-point.1).abs()<1e-10).unwrap_or(false){return Ok(());}
    if out.len()>=MAX_INPUT_POINTS{return Err("Path exceeds boolean complexity budget".into());}
    out.push(point);Ok(())
}
fn mid(a:XY,b:XY)->XY{((a.0+b.0)*0.5,(a.1+b.1)*0.5)}
fn distance_point_to_chord(p:XY,a:XY,b:XY)->f64{
    let dx=b.0-a.0;let dy=b.1-a.1;
    let den=(dx*dx+dy*dy).sqrt();
    if den<1e-12{return ((p.0-a.0).powi(2)+(p.1-a.1).powi(2)).sqrt();}
    ((p.0-a.0)*dy-(p.1-a.1)*dx).abs()/den
}
fn flatten_cubic(a:XY,b:XY,c:XY,d:XY,depth:u32,
    out:&mut Vec<XY>)->Result<(),String>{
    let error=distance_point_to_chord(b,a,d)
        .max(distance_point_to_chord(c,a,d));
    if error<=FLATTEN_TOL_MM{
        return append(out,d);
    }
    if depth>=MAX_DEPTH{
        return Err("Curve exceeds boolean precision/complexity budget".into());
    }
    let ab=mid(a,b);let bc=mid(b,c);let cd=mid(c,d);
    let abc=mid(ab,bc);let bcd=mid(bc,cd);let abcd=mid(abc,bcd);
    flatten_cubic(a,ab,abc,abcd,depth+1,out)?;
    flatten_cubic(abcd,bcd,cd,d,depth+1,out)
}
fn close_loop(mut points:Vec<XY>)->Result<LineString<f64>,String>{
    if points.len()>=2 && points.first()==points.last(){points.pop();}
    if points.len()<3{return Err("Boolean input has fewer than three vertices".into());}
    // A ring with zero area or non-finite coordinates is never passed to
    // the topology engine, which expects valid polygons.
    let mut signed=0.0;
    for i in 0..points.len(){
        let p=points[i];let q=points[(i+1)%points.len()];
        signed+=p.0*q.1-q.0*p.1;
    }
    if !signed.is_finite()||signed.abs()<1e-8{
        return Err("Boolean input ring is degenerate or self-canceling".into());
    }
    points.push(points[0]);
    Ok(LineString(points.into_iter().map(|(x,y)|Coord{x,y}).collect()))
}
fn flatten_bezier(start:Point,segments:&[CubicSegment])->Result<LineString<f64>,String>{
    if segments.is_empty(){return Err("Boolean path has no segments".into());}
    let mut points=Vec::new();
    let mut prev=xy(start)?;
    append(&mut points,prev)?;
    for seg in segments{
        let b=xy(seg.control1)?;
        let c=xy(seg.control2)?;
        let d=xy(seg.end)?;
        flatten_cubic(prev,b,c,d,0,&mut points)?;
        prev=d;
    }
    close_loop(points)
}
fn poly(points:&[Point],closed:bool)->Result<Polygon<f64>,String>{
    if !closed{return Err("Open paths cannot participate in filled boolean operations".into());}
    if points.len()<3||points.len()>MAX_INPUT_POINTS{
        return Err("Path node count is unsupported".into());
    }
    let coords=points.iter().map(|p|xy(*p)).collect::<Result<Vec<_>,_>>()?;
    Ok(Polygon::new(close_loop(coords)?,vec![]))
}
fn rectangle(x:f32,y:f32,w:f32,h:f32)->Result<Polygon<f64>,String>{
    if !w.is_finite()||!h.is_finite()||w<=0.0||h<=0.0{
        return Err("Rectangle must have positive dimensions".into());
    }
    let p=[(f64::from(x),f64::from(y)),
        (f64::from(x)+f64::from(w),f64::from(y)),
        (f64::from(x)+f64::from(w),f64::from(y)+f64::from(h)),
        (f64::from(x),f64::from(y)+f64::from(h))];
    if p.iter().any(|pt|!valid_xy(*pt)){
        return Err("Rectangle lies outside boolean coordinate bounds".into());
    }
    Ok(Polygon::new(close_loop(p.to_vec())?,vec![]))
}
fn ellipse(x:f32,y:f32,w:f32,h:f32)->Result<Polygon<f64>,String>{
    if !w.is_finite()||!h.is_finite()||w<=0.0||h<=0.0{
        return Err("Ellipse must have positive dimensions".into());
    }
    let (x,y,w,h)=(f64::from(x),f64::from(y),f64::from(w),f64::from(h));
    let radius=w.max(h)*0.5;
    let steps=(radius*PI*PI/(2.0*FLATTEN_TOL_MM)).sqrt()
        .ceil().clamp(64.0,4096.0) as usize;
    let cx=x+w*0.5;let cy=y+h*0.5;
    let mut out=Vec::with_capacity(steps);
    for i in 0..steps{
        let a=i as f64*TAU/steps as f64;
        append(&mut out,(cx+a.cos()*w*0.5,cy+a.sin()*h*0.5))?;
    }
    Ok(Polygon::new(close_loop(out)?,vec![]))
}
fn shape_to_polygon(shape:&Shape)->Result<MultiPolygon<f64>,String>{
    let result=match shape{
        Shape::Rectangle{x,y,w,h}=>MultiPolygon(vec![rectangle(*x,*y,*w,*h)?]),
        Shape::Ellipse{x,y,w,h}=>MultiPolygon(vec![ellipse(*x,*y,*w,*h)?]),
        Shape::Polyline{points,closed}=>MultiPolygon(vec![poly(points,*closed)?]),
        Shape::Bezier{start,segments,closed}=>{
            if !closed{return Err("Open Bézier curves must be closed before Boolean operations".into());}
            MultiPolygon(vec![Polygon::new(flatten_bezier(*start,segments)?,vec![])])
        }
        Shape::CompoundPath{contours}=>{
            if contours.is_empty()||contours.len()>MAX_CONTOURS{
                return Err("Compound path has too many or no contours".into());
            }
            // Compound vector paths use even-odd filling. XORing each ring
            // reconstructs this topology, including counters inside letters.
            let mut region=MultiPolygon(vec![]);
            let mut total=0usize;
            for contour in contours{
                if !contour.closed{return Err("Compound contour must be closed".into());}
                total=total.saturating_add(contour.segments.len());
                if total>MAX_INPUT_POINTS{return Err("Compound path exceeds complexity budget".into());}
                let one=MultiPolygon(vec![Polygon::new(flatten_bezier(contour.start,
                    &contour.segments)?,vec![])]);
                region=if region.0.is_empty(){one}else{region.xor(&one)};
            }
            region
        }
        Shape::Text{..}=>{
            return Err("Convert text to curves before Boolean operations".into());
        }
        Shape::EmbeddedSvg{..}|Shape::Raster{..}=>{
            return Err("Embedded images are not editable Boolean vector shapes".into());
        }
    };
    let area=result.unsigned_area();
    if !area.is_finite()||area<1e-8{
        return Err("Vector input is empty or has zero filled area".into());
    }
    Ok(result)
}
fn ring_to_contour(ring:&LineString<f64>)->Result<Contour,String>{
    let mut coords:Vec<XY>=ring.0.iter().map(|c|(c.x,c.y)).collect();
    if coords.len()>=2&&coords.first()==coords.last(){coords.pop();}
    if coords.len()<3{return Err("Boolean result contains a degenerate contour".into());}
    if coords.iter().any(|p|!valid_xy(*p)){
        return Err("Boolean result lies outside allowed coordinate bounds".into());
    }
    let start=Point{x:coords[0].0 as f32,y:coords[0].1 as f32};
    let mut segments=Vec::with_capacity(coords.len());
    for i in 0..coords.len(){
        let a=coords[i];let b=coords[(i+1)%coords.len()];
        segments.push(CubicSegment{
            control1:Point{x:(a.0+(b.0-a.0)/3.0) as f32,
                y:(a.1+(b.1-a.1)/3.0) as f32},
            control2:Point{x:(a.0+2.0*(b.0-a.0)/3.0) as f32,
                y:(a.1+2.0*(b.1-a.1)/3.0) as f32},
            end:Point{x:b.0 as f32,y:b.1 as f32},
        });
    }
    Ok(Contour{start,segments,closed:true})
}
/// Perform an editable set operation on two closed, filled shapes.
///
/// The order matters for Subtract: A - B. Empty results are returned as None
/// so the editor can warn without destroying source objects. Curves become
/// bounded-resolution straight-line cubic contours (approximation <=0.1mm).
/// The original Shape inputs are never modified, including on error.
pub fn combine(a:&Shape,b:&Shape,operation:BooleanOperation)
    ->Result<Option<Shape>,String>{
    let ga=shape_to_polygon(a)?;
    let gb=shape_to_polygon(b)?;
    let result=std::panic::catch_unwind(||match operation{
        BooleanOperation::Union=>ga.union(&gb),
        BooleanOperation::Subtract=>ga.difference(&gb),
        BooleanOperation::Intersect=>ga.intersection(&gb),
        BooleanOperation::Xor=>ga.xor(&gb),
    }).map_err(|_|"Geometry topology operation failed (invalid boundaries)")?;
    if result.0.is_empty()||result.unsigned_area()<1e-8{return Ok(None);}
    let count:usize=result.0.iter().map(|p|1+p.interiors().len()).sum();
    if count>MAX_CONTOURS{
        return Err("Boolean output contains too many contours".into());
    }
    let nodes:usize=result.0.iter().map(|p|
        p.exterior().0.len()+p.interiors().iter().map(|r|r.0.len()).sum::<usize>()
    ).sum();
    if nodes>MAX_OUTPUT_POINTS{
        return Err("Boolean result exceeds output vertex budget".into());
    }
    let mut contours=Vec::with_capacity(count);
    for polygon in &result.0{
        contours.push(ring_to_contour(polygon.exterior())?);
        for hole in polygon.interiors(){
            contours.push(ring_to_contour(hole)?);
        }
    }
    if contours.is_empty(){return Ok(None);}
    Ok(Some(Shape::CompoundPath{contours}))
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Document,Style,kdr};
    fn rect(x:f32,y:f32,w:f32,h:f32)->Shape{
        Shape::Rectangle{x,y,w,h}
    }
    fn result_area(shape:&Shape)->f64{
        shape_to_polygon(shape).unwrap().unsigned_area()
    }
    #[test] fn overlap_area_math_all_four_operations(){
        let a=rect(0.,0.,100.,100.);
        let b=rect(50.,0.,100.,100.);
        for (op,expected) in [
            (BooleanOperation::Union,15_000.),
            (BooleanOperation::Subtract,5_000.),
            (BooleanOperation::Intersect,5_000.),
            (BooleanOperation::Xor,10_000.),
        ]{
            let result=combine(&a,&b,op).unwrap().unwrap();
            assert!((result_area(&result)-expected).abs()<0.01,
                "failed operation {op:?}");
            let mut doc=Document::default();doc.add(result,Style::default());
            doc.validate().unwrap();
        }
    }
    #[test] fn subtract_contained_shape_creates_a_hole(){
        let outside=rect(0.,0.,100.,100.);
        let inner=rect(30.,30.,40.,40.);
        let result=combine(&outside,&inner,BooleanOperation::Subtract)
            .unwrap().unwrap();
        let Shape::CompoundPath{contours}=&result else{panic!("must be compound")};
        assert_eq!(contours.len(),2);
        assert!((result_area(&result)-8_400.).abs()<0.01);
        let mut doc=Document::default();doc.add(result,Style::default());
        doc.validate().unwrap();
        assert!(doc.to_svg().contains("fill-rule=\"evenodd\""));
        let dir=tempfile::tempdir().unwrap();
        let file=dir.path().join("hole.kdr");
        kdr::save_kdr(&doc,&file).unwrap();
        assert_eq!(kdr::read_kdr(&file).unwrap(),doc);
    }
    #[test] fn disjoint_union_preserves_multiple_regions(){
        let a=rect(0.,0.,20.,20.);
        let b=rect(100.,0.,20.,20.);
        let result=combine(&a,&b,BooleanOperation::Union).unwrap().unwrap();
        let Shape::CompoundPath{contours}=&result else{panic!()};
        assert_eq!(contours.len(),2);
        assert!((result_area(&result)-800.).abs()<0.01);
        assert!(combine(&a,&b,BooleanOperation::Intersect).unwrap().is_none());
    }
    #[test] fn subtract_self_is_empty_and_inputs_unchanged(){
        let a=rect(1.,3.,9.,9.);let copy=a.clone();
        assert!(combine(&a,&a,BooleanOperation::Subtract).unwrap().is_none());
        assert_eq!(a,copy);
    }
    #[test] fn ellipse_boolean_area_stays_within_one_percent(){
        let circle=Shape::Ellipse{x:0.,y:0.,w:100.,h:100.};
        let cover=rect(-10.,-10.,200.,200.);
        let result=combine(&circle,&cover,BooleanOperation::Intersect)
            .unwrap().unwrap();
        let ideal=std::f64::consts::PI*50.0*50.0;
        assert!(((result_area(&result)-ideal)/ideal).abs()<0.01);
    }
    #[test] fn reject_open_paths_and_unsupported_shapes_without_mutation(){
        let line=Shape::Polyline{points:vec![
            Point{x:0.,y:0.},Point{x:10.,y:10.},Point{x:20.,y:0.}],closed:false};
        let before=line.clone();
        assert!(combine(&line,&rect(0.,0.,10.,10.),BooleanOperation::Union).is_err());
        assert_eq!(line,before);
        let text=Shape::Text{x:0.,y:0.,value:"ABC".into(),size_mm:10.};
        assert!(combine(&text,&rect(0.,0.,10.,10.),BooleanOperation::Union).is_err());
    }
    #[test] fn repeatable_randomized_rectangle_area_invariants(){
        for i in 1..=128{
            let a=rect(0.,0.,100.,100.);
            let b=rect((i%87) as f32,(i%39) as f32,70.,60.);
            let u=combine(&a,&b,BooleanOperation::Union).unwrap().unwrap();
            let inter=combine(&a,&b,BooleanOperation::Intersect).unwrap().unwrap();
            let expected=result_area(&a)+result_area(&b);
            assert!((result_area(&u)+result_area(&inter)-expected).abs()<0.01);
            let x=combine(&a,&b,BooleanOperation::Xor).unwrap().unwrap();
            assert!((result_area(&x)+2.0*result_area(&inter)-expected).abs()<0.01);
        }
    }
}
