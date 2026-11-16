//! Approximate editable offset paths from native vector objects.
//! Signed distance is measured in millimetres: positive is to the left
//! of the path direction. Closed CCW shapes therefore offset inward.
//! Result is an editable polyline (not a stroke-expansion or exact cubic offset).
use crate::{Point,Shape};
const MAX_POINTS:usize=4096;
const MAX_MM:f64=100_000.0;
const EPS:f64=1e-9;

fn valid(p:Point)->bool{
    p.x.is_finite()&&p.y.is_finite()&&
    (p.x as f64).abs()<=MAX_MM&&(p.y as f64).abs()<=MAX_MM
}
fn cross(a:(f64,f64),b:(f64,f64))->f64{a.0*b.1-a.1*b.0}
fn normal(a:Point,b:Point)->Result<(f64,f64),String>{
    let dx=b.x as f64-a.x as f64;
    let dy=b.y as f64-a.y as f64;
    let length=dx.hypot(dy);
    if length<EPS{return Err("Cannot offset duplicate consecutive points".into());}
    Ok((-dy/length,dx/length))
}
fn tangent(a:Point,b:Point)->(f64,f64){
    (b.x as f64-a.x as f64,b.y as f64-a.y as f64)
}
fn combine(p:Point,n:(f64,f64),distance:f64)->Result<Point,String>{
    let x=p.x as f64+n.0*distance;
    let y=p.y as f64+n.1*distance;
    if !x.is_finite()||!y.is_finite()||x.abs()>MAX_MM||y.abs()>MAX_MM{
        return Err("Offset goes outside the safe coordinate range".into());
    }
    Ok(Point{x:x as f32,y:y as f32})
}
/// Prevent a large inward offset from turning tiny shapes inside out.
/// Per-edge direction tests catch the common "10 mm square + 6 mm inward"
/// inversion that winding/area comparisons alone do NOT detect.
fn validate_offset_topology(original:&[Point],offset:&[Point],closed:bool)
    ->Result<(),String>{
    let n=original.len();
    if n!=offset.len(){return Err("Offset topology count mismatch".into());}
    let edges=if closed{n}else{n-1};
    for i in 0..edges{
        let j=(i+1)%n;
        let a=(original[j].x as f64-original[i].x as f64,
               original[j].y as f64-original[i].y as f64);
        let b=(offset[j].x as f64-offset[i].x as f64,
               offset[j].y as f64-offset[i].y as f64);
        let original_len=a.0.hypot(a.1);
        let projected=a.0*b.0+a.1*b.1;
        if projected<=original_len*1e-6{
            return Err("Offset collapses or reverses an edge; reduce distance".into());
        }
    }
    if closed{
        let area=|points:&[Point]|{
            let mut sum=0.0f64;
            for i in 0..points.len(){
                let a=points[i];let b=points[(i+1)%points.len()];
                sum+=(a.x as f64*b.y as f64-b.x as f64*a.y as f64);
            }
            sum/2.0
        };
        let original_area=area(original);
        let output_area=area(offset);
        if original_area.abs()<1e-7||
            output_area.abs()<1e-7||
            original_area.signum()!=output_area.signum(){
            return Err("Offset collapses or inverts a closed polygon".into());
        }
    }
    // Reject self-intersections instead of silently returning a malformed
    // fill polygon. Sweep by x-bounding boxes to avoid O(n²) on smooth rings.
    #[derive(Clone,Copy)]
    struct Edge{idx:usize,a:Point,b:Point,x_min:f64,x_max:f64,
        y_min:f64,y_max:f64}
    let mut lines=Vec::with_capacity(edges);
    for i in 0..edges{
        let a=offset[i];let b=offset[(i+1)%n];
        lines.push(Edge{idx:i,a,b,x_min:(a.x.min(b.x)) as f64,
            x_max:(a.x.max(b.x)) as f64,
            y_min:(a.y.min(b.y)) as f64,
            y_max:(a.y.max(b.y)) as f64});
    }
    lines.sort_by(|a,b|a.x_min.total_cmp(&b.x_min));
    let mut checks=0usize;
    for (i,a) in lines.iter().enumerate(){
        for b in &lines[(i+1)..]{
            if b.x_min>a.x_max+1e-7{break;}
            if a.idx.abs_diff(b.idx)<=1||
                (closed && a.idx.abs_diff(b.idx)==edges-1){
                continue; // adjacent segments have a shared endpoint
            }
            if b.y_min>a.y_max+1e-7||a.y_min>b.y_max+1e-7{continue;}
            checks+=1;
            if checks>2_000_000{
                return Err("Offset too complex for safe topology validation".into());
            }
            let orientation=|p:Point,q:Point,r:Point|{
                (q.x as f64-p.x as f64)*(r.y as f64-p.y as f64)-
                (q.y as f64-p.y as f64)*(r.x as f64-p.x as f64)
            };
            let o1=orientation(a.a,a.b,b.a);
            let o2=orientation(a.a,a.b,b.b);
            let o3=orientation(b.a,b.b,a.a);
            let o4=orientation(b.a,b.b,a.b);
            // Bounds already overlap. Strictly crossing segments OR a
            // collinear overlap/touching point can corrupt a filled contour.
            let crosses=(o1>1e-8&&o2< -1e-8 || o1< -1e-8&&o2>1e-8)
                && (o3>1e-8&&o4< -1e-8 || o3< -1e-8&&o4>1e-8);
            let near_zero=o1.abs()<=1e-8||o2.abs()<=1e-8||
                o3.abs()<=1e-8||o4.abs()<=1e-8;
            if crosses || near_zero{
                return Err("Offset contains overlapping or self-intersecting edges".into());
            }
        }
    }
    Ok(())
}
/// Offset a polyline with a limited miter at each vertex. No source point
/// is mutated. For closed paths, the sign follows the input winding.
pub fn offset_points(input:&[Point],closed:bool,distance_mm:f32)
    ->Result<Vec<Point>,String>{
    if !distance_mm.is_finite()||distance_mm.abs()<0.0001||
        distance_mm.abs()>1000.0{
        return Err("Offset must be signed 0.0001–1000 mm".into());
    }
    if input.len()>MAX_POINTS{return Err("Offset exceeds 4096 points".into());}
    let mut points=input.to_vec();
    if closed && points.len()>3 && points.first()==points.last(){
        points.pop();
    }
    let n=points.len();
    if n<if closed{3}else{2}{
        return Err("Insufficient unique nodes for offset".into());
    }
    if points.iter().any(|p|!valid(*p)){
        return Err("Invalid source coordinates".into());
    }
    let count=if closed{n}else{n-1};
    let mut normals=Vec::with_capacity(count);
    for i in 0..count{
        normals.push(normal(points[i],points[(i+1)%n])?);
    }
    let d=distance_mm as f64;
    let mut output=Vec::with_capacity(n);
    for i in 0..n{
        if !closed && i==0{
            output.push(combine(points[i],normals[0],d)?);
            continue;
        }
        if !closed && i==n-1{
            output.push(combine(points[i],normals[n-2],d)?);
            continue;
        }
        let previous=if i==0{n-1}else{i-1};
        let next=i;
        let n1=normals[previous];
        let n2=normals[next];
        let t1=tangent(points[previous],points[i]);
        let t2=tangent(points[i],points[(i+1)%n]);
        let den=cross(t1,t2);
        let direction=if den.abs()<=1e-12{
            // Parallel and same direction: use one normal. A U-turn would
            // be intrinsically ambiguous; reject rather than self-intersect.
            if t1.0*t2.0+t1.1*t2.1<=0.0{
                return Err("Cannot offset a 180-degree reversal".into());
            }
            n1
        }else{
            // Intersect two offset infinite segment lines, both anchored at p.
            let delta=((n2.0-n1.0)*d,(n2.1-n1.1)*d);
            let t=cross(delta,t2)/den;
            let raw=(n1.0*d+t1.0*t,n1.1*d+t1.1*t);
            let radius=raw.0.hypot(raw.1);
            let max_miter=d.abs()*8.0;
            if radius>max_miter{
                // Clamp long acute miters; may require later shape cleanup.
                (raw.0/radius*max_miter/d,raw.1/radius*max_miter/d)
            }else{
                (raw.0/d,raw.1/d)
            }
        };
        output.push(combine(points[i],direction,d)?);
    }
    validate_offset_topology(&points,&output,closed)?;
    Ok(output)
}
/// Converts supported native geometry to a sampled editable polyline offset.
/// Cubic/elliptical offsets approximate rather than preserve exact curvature.
pub fn offset_shape(shape:&Shape,distance_mm:f32)->Result<Shape,String>{
    let (points,closed)=match shape{
        Shape::Polyline{points,closed}=>(points.clone(),*closed),
        Shape::Rectangle{x,y,w,h} if *w>0.0&&*h>0.0=>(
            vec![
                Point{x:*x,y:*y},Point{x:*x+*w,y:*y},
                Point{x:*x+*w,y:*y+*h},Point{x:*x,y:*y+*h},
            ],true),
        Shape::Ellipse{x,y,w,h} if *w>0.0&&*h>0.0=>{
            let mut points=Vec::with_capacity(96);
            for i in 0..96{
                let t=std::f64::consts::TAU*i as f64/96.0;
                points.push(Point{
                    x:(*x as f64+*w as f64/2.0+
                        (*w as f64/2.0)*t.cos()) as f32,
                    y:(*y as f64+*h as f64/2.0+
                        (*h as f64/2.0)*t.sin()) as f32,
                });
            }
            (points,true)
        },
        Shape::Bezier{start,segments,closed} if !segments.is_empty()=>{
            if segments.len()>300{
                return Err("Cubic offset limited to 300 segments".into());
            }
            let mut pts=Vec::with_capacity(segments.len()*12+1);
            pts.push(*start);
            let mut a=*start;
            for s in segments{
                for j in 1..=12{
                    let t=j as f64/12.0;
                    let u=1.0-t;
                    pts.push(Point{
                        x:(u*u*u*a.x as f64+3.0*u*u*t*s.control1.x as f64+
                            3.0*u*t*t*s.control2.x as f64+t*t*t*s.end.x as f64) as f32,
                        y:(u*u*u*a.y as f64+3.0*u*u*t*s.control1.y as f64+
                            3.0*u*t*t*s.control2.y as f64+t*t*t*s.end.y as f64) as f32,
                    });
                }
                a=s.end;
            }
            (pts,*closed)
        },
        _=>return Err("Select a rectangle, ellipse, polyline or Bézier path".into()),
    };
    let points=offset_points(&points,closed,distance_mm)?;
    Ok(Shape::Polyline{points,closed})
}

#[cfg(test)]
mod tests{
    use super::*;
    fn p(x:f32,y:f32)->Point{Point{x,y}}
    #[test]
    fn horizontal_left_offset_is_exact(){
        assert_eq!(offset_points(&[p(0.,0.),p(10.,0.)],false,2.).unwrap(),
            vec![p(0.,2.),p(10.,2.)]);
        assert_eq!(offset_points(&[p(0.,0.),p(10.,0.)],false,-2.).unwrap(),
            vec![p(0.,-2.),p(10.,-2.)]);
    }
    #[test]
    fn closed_square_offset_respects_orientation(){
        let square=[p(0.,0.),p(10.,0.),p(10.,10.),p(0.,10.)];
        let inside=offset_points(&square,true,1.).unwrap();
        assert_eq!(inside,[p(1.,1.),p(9.,1.),p(9.,9.),p(1.,9.)]);
        let outside=offset_points(&square,true,-1.).unwrap();
        assert_eq!(outside,[p(-1.,-1.),p(11.,-1.),p(11.,11.),p(-1.,11.)]);
    }
    #[test]
    fn open_l_shaped_offset_intersects_correctly(){
        let q=offset_points(&[p(0.,0.),p(10.,0.),p(10.,10.)],
            false,2.).unwrap();
        assert_eq!(q,vec![p(0.,2.),p(8.,2.),p(8.,10.)]);
    }
    #[test]
    fn inward_offset_must_not_collapse_or_invert_square(){
        let square=[p(0.,0.),p(10.,0.),p(10.,10.),p(0.,10.)];
        // An old regression silently produced a tiny inside-out 2mm square.
        assert!(offset_points(&square,true,6.).is_err());
        assert!(offset_points(&square,true,5.).is_err());
        assert_eq!(offset_points(&square,true,4.).unwrap(),
            [p(4.,4.),p(6.,4.),p(6.,6.),p(4.,6.)]);
        assert!(offset_points(&square,true,-4.).is_ok());
    }
    #[test]
    fn reject_offset_self_intersections_instead_of_corrupt_fills(){
        let bow=[p(0.,0.),p(12.,12.),p(0.,12.),p(12.,0.)];
        assert!(offset_points(&bow,true,1.).is_err());
    }
    #[test]
    fn near_zero_open_segments_cannot_be_reversed(){
        let line=[p(0.,0.),p(1.,0.),p(1.,1.)];
        assert!(offset_points(&line,false,5.).is_err());
        assert!(offset_points(&line,false,-1.).is_ok());
    }
    #[test]
    fn rejects_nonfinite_duplicate_overlong_and_uturn(){
        assert!(offset_points(&[p(0.,0.),p(1.,1.)],false,f32::NAN).is_err());
        assert!(offset_points(&[p(0.,0.),p(1.,1.)],false,0.).is_err());
        assert!(offset_points(&[p(0.,0.),p(1.,1.)],false,1001.).is_err());
        assert!(offset_points(&[p(0.,0.),p(0.,0.)],false,1.).is_err());
        assert!(offset_points(&[p(0.,0.),p(10.,0.),p(0.,0.)],false,1.).is_err());
        assert!(offset_points(&vec![p(0.,0.);MAX_POINTS+1],false,1.).is_err());
    }
    #[test]
    fn native_cubic_offset_is_editable_approximation(){
        let shape=Shape::Bezier{start:p(0.,0.),
            segments:vec![crate::CubicSegment{
                control1:p(4.,5.),control2:p(7.,5.),end:p(10.,0.),
            }],closed:false};
        let offset=offset_shape(&shape,1.).unwrap();
        let Shape::Polyline{points,closed}=offset else{panic!()};
        assert!(!closed);assert_eq!(points.len(),13);
        assert!(points.iter().all(|p|valid(*p)));
    }
    #[test]
    fn rectangle_offset_is_a_closed_native_path(){
        let shape=Shape::Rectangle{x:0.,y:0.,w:20.,h:10.};
        let Shape::Polyline{points,closed}=offset_shape(&shape,-2.).unwrap()
            else{panic!()};
        assert!(closed);
        assert_eq!(points,[p(-2.,-2.),p(22.,-2.),p(22.,12.),p(-2.,12.)]);
    }
}
