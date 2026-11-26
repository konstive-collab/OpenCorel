//! Adaptive cubic subdivision, arc-length measurement and distance-based
//! path sampling for native document geometry. All distances are millimetres.
use crate::{Contour,CubicSegment,Point,Shape};

const MAX_SAMPLES:usize=250_000;
const MAX_DEPTH:u8=24;

#[derive(Clone,Copy,Debug,PartialEq)]
pub struct PathSample{
    pub point:Point,
    pub tangent:Point,
    pub distance_mm:f64,
}
#[derive(Clone,Debug,PartialEq)]
pub struct MeasuredContour{
    pub vertices:Vec<Point>,
    pub distances:Vec<f64>,
    pub closed:bool,
}
fn distance(a:Point,b:Point)->f64{
    (f64::from(a.x)-f64::from(b.x)).hypot(f64::from(a.y)-f64::from(b.y))
}
fn finite(p:Point)->bool{p.x.is_finite()&&p.y.is_finite()&&p.x.abs()<=100_000.&&p.y.abs()<=100_000.}
fn lerp(a:Point,b:Point,t:f32)->Point{
    Point{x:a.x+(b.x-a.x)*t,y:a.y+(b.y-a.y)*t}
}
fn midpoint(a:Point,b:Point)->Point{lerp(a,b,0.5)}
fn line_distance(p:Point,a:Point,b:Point)->f64{
    let dx=f64::from(b.x)-f64::from(a.x);
    let dy=f64::from(b.y)-f64::from(a.y);
    let px=f64::from(p.x)-f64::from(a.x);
    let py=f64::from(p.y)-f64::from(a.y);
    let den=dx*dx+dy*dy;
    if den<=1e-24{return distance(p,a);}
    let t=((px*dx+py*dy)/den).clamp(0.,1.);
    (px-dx*t).hypot(py-dy*t)
}
fn push(points:&mut Vec<Point>,p:Point,budget:&mut usize)->Result<(),String>{
    if !finite(p){return Err("Invalid path coordinate".into());}
    if points.last()==Some(&p){return Ok(());}
    if *budget>=MAX_SAMPLES{return Err("Adaptive path sample budget exceeded".into());}
    points.push(p);*budget+=1;Ok(())
}
fn flatten_cubic(points:&mut Vec<Point>,control:[Point;4],tolerance:f64,
    depth:u8,budget:&mut usize)->Result<(),String>{
    let [a,b,c,d]=control;
    let chord=distance(a,d);
    let polygon=distance(a,b)+distance(b,c)+distance(c,d);
    // The length-excess condition also catches collinear backtracking,
    // which a perpendicular-flatness-only test would silently erase.
    if line_distance(b,a,d).max(line_distance(c,a,d))<=tolerance
        &&polygon-chord<=tolerance{
        return push(points,d,budget);
    }
    if depth>=MAX_DEPTH{return Err("Path tolerance cannot be achieved safely".into());}
    let ab=midpoint(a,b);let bc=midpoint(b,c);let cd=midpoint(c,d);
    let abc=midpoint(ab,bc);let bcd=midpoint(bc,cd);let m=midpoint(abc,bcd);
    flatten_cubic(points,[a,ab,abc,m],tolerance,depth+1,budget)?;
    flatten_cubic(points,[m,bcd,cd,d],tolerance,depth+1,budget)
}
impl MeasuredContour{
    fn from_points(mut points:Vec<Point>,closed:bool,budget:&mut usize)->Result<Self,String>{
        if closed&&points.len()>1{
            let start=points[0];push(&mut points,start,budget)?;
        }
        if points.len()<2{return Err("Contour has no measurable length".into());}
        let mut distances=Vec::with_capacity(points.len());distances.push(0.);
        for pair in points.windows(2){
            distances.push(distances.last().copied().unwrap_or(0.)+distance(pair[0],pair[1]));
        }
        if distances.last().copied().unwrap_or(0.)<=1e-9{
            return Err("Contour has no measurable length".into());
        }
        Ok(Self{vertices:points,distances,closed})
    }
    pub fn length_mm(&self)->f64{self.distances.last().copied().unwrap_or(0.)}
    /// Endpoint clamping for open contours, wrapping for closed contours.
    /// The tangent follows the local sampled segment in traversal order.
    pub fn sample(&self,offset_mm:f64)->Result<PathSample,String>{
        let length=self.length_mm();
        if !offset_mm.is_finite()||length<=0.||self.vertices.len()!=self.distances.len()
            ||self.vertices.len()<2{return Err("Invalid path sample distance or contour".into());}
        let offset=if self.closed{offset_mm.rem_euclid(length)}else{offset_mm.clamp(0.,length)};
        let i=self.distances.partition_point(|d|*d<=offset).saturating_sub(1)
            .min(self.vertices.len()-2);
        let a=self.vertices[i];let b=self.vertices[i+1];
        let segment=self.distances[i+1]-self.distances[i];
        if segment<=0.{return Err("Degenerate path segment".into());}
        let t=((offset-self.distances[i])/segment).clamp(0.,1.);
        let point=Point{x:(f64::from(a.x)+(f64::from(b.x)-f64::from(a.x))*t) as f32,
            y:(f64::from(a.y)+(f64::from(b.y)-f64::from(a.y))*t) as f32};
        Ok(PathSample{point,tangent:Point{
            x:((f64::from(b.x)-f64::from(a.x))/segment) as f32,
            y:((f64::from(b.y)-f64::from(a.y))/segment) as f32},distance_mm:offset})
    }
    pub fn signed_area_mm2(&self)->Option<f64>{
        if !self.closed{return None;}
        Some(self.vertices.windows(2).map(|p|
            f64::from(p[0].x)*f64::from(p[1].y)-f64::from(p[1].x)*f64::from(p[0].y))
            .sum::<f64>()*0.5)
    }
}
fn measure_cubic(start:Point,segments:&[CubicSegment],closed:bool,tolerance:f64,
    budget:&mut usize)->Result<MeasuredContour,String>{
    let mut points=Vec::new();push(&mut points,start,budget)?;
    let mut from=start;
    for s in segments{
        if ![s.control1,s.control2,s.end].into_iter().all(finite){
            return Err("Invalid cubic coordinate".into());
        }
        flatten_cubic(&mut points,[from,s.control1,s.control2,s.end],tolerance,0,budget)?;
        from=s.end;
    }
    MeasuredContour::from_points(points,closed,budget)
}
pub fn measure_shape(shape:&Shape,tolerance_mm:f64)->Result<Vec<MeasuredContour>,String>{
    if !tolerance_mm.is_finite()||!(0.001..=10.).contains(&tolerance_mm){
        return Err("Path tolerance must be 0.001–10 mm".into());
    }
    let mut budget=0;
    match shape{
        Shape::Polyline{points,closed}=>{
            let mut vertices=Vec::new();
            for p in points{push(&mut vertices,*p,&mut budget)?;}
            Ok(vec![MeasuredContour::from_points(vertices,*closed,&mut budget)?])
        }
        Shape::Bezier{start,segments,closed}=>Ok(vec![
            measure_cubic(*start,segments,*closed,tolerance_mm,&mut budget)?]),
        Shape::CompoundPath{contours}=>{
            if contours.is_empty(){return Err("Empty compound path".into());}
            contours.iter().map(|c|
                measure_cubic(c.start,&c.segments,c.closed,tolerance_mm,&mut budget)).collect()
        }
        Shape::Rectangle{..}|Shape::Ellipse{..}=>{
            let curve=crate::path_workflow::to_editable_bezier(shape)?;
            measure_shape(&curve,tolerance_mm)
        }
        _=>Err("Measure native vector geometry; convert embedded artwork first".into()),
    }
}
pub fn contour_from_vertices(points:&[Point],closed:bool)->Result<Contour,String>{
    if points.len()<2||points.len()>MAX_SAMPLES||!points.iter().copied().all(finite){
        return Err("Invalid contour vertices".into());
    }
    let line=|a,b|CubicSegment{control1:lerp(a,b,1./3.),control2:lerp(a,b,2./3.),end:b};
    let mut segments:Vec<_>=points.windows(2).map(|p|line(p[0],p[1])).collect();
    if closed&&points.first()!=points.last(){segments.push(line(*points.last().unwrap(),points[0]));}
    Ok(Contour{start:points[0],segments,closed})
}
#[cfg(test)]
mod tests{
    use super::*;
    fn p(x:f32,y:f32)->Point{Point{x,y}}
    #[test] fn rectangular_perimeter_and_area(){
        let c=measure_shape(&Shape::Rectangle{x:0.,y:0.,w:30.,h:40.},0.01).unwrap();
        assert!((c[0].length_mm()-140.).abs()<1e-6);
        assert_eq!(c[0].signed_area_mm2(),Some(1200.));
        let s=c[0].sample(35.).unwrap();
        assert_eq!(s.point,p(30.,5.));assert_eq!(s.tangent,p(0.,1.));
        assert_eq!(c[0].sample(145.).unwrap().point,p(5.,0.));
    }
    #[test] fn circle_perimeter_converges(){
        let shape=Shape::Ellipse{x:0.,y:0.,w:40.,h:40.};
        let fine=measure_shape(&shape,0.001).unwrap()[0].length_mm();
        let coarse=measure_shape(&shape,1.).unwrap()[0].length_mm();
        assert!(fine>=coarse);
        assert!((fine-40.*std::f64::consts::PI).abs()<0.05);
    }
    #[test] fn open_samples_clamp_and_ignore_duplicate_vertices(){
        let c=measure_shape(&Shape::Polyline{points:vec![p(0.,0.),p(0.,0.),p(3.,4.)],closed:false},0.1).unwrap();
        assert_eq!(c[0].length_mm(),5.);
        assert_eq!(c[0].sample(-1.).unwrap().point,p(0.,0.));
        assert_eq!(c[0].sample(9.).unwrap().point,p(3.,4.));
        assert_eq!(c[0].signed_area_mm2(),None);
    }
    #[test] fn collinear_cubic_backtracking_is_not_erased(){
        let c=measure_shape(&Shape::Bezier{start:p(0.,0.),segments:vec![
            CubicSegment{control1:p(20.,0.),control2:p(-20.,0.),end:p(1.,0.)}],closed:false},0.001).unwrap();
        assert!(c[0].length_mm()>10.);
    }
    #[test] fn rejects_invalid_geometry_and_parameters(){
        assert!(measure_shape(&Shape::Polyline{points:vec![p(0.,0.),p(0.,0.)],closed:false},0.1).is_err());
        assert!(measure_shape(&Shape::Rectangle{x:0.,y:0.,w:1.,h:1.},f64::NAN).is_err());
        assert!(measure_shape(&Shape::Bezier{start:p(0.,0.),segments:vec![
            CubicSegment{control1:p(f32::NAN,0.),control2:p(1.,0.),end:p(2.,0.)}],closed:false},0.1).is_err());
    }
}
