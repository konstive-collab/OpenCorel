//! Editable freehand stroke fitting for the native cubic Bézier model.
//! All input is bounded and validated before returning any geometry.
//! The Ramer-Douglas-Peucker pass is iterative to avoid deep recursion
//! on long hand-drawn contours and uses f64 for distance calculations.
use crate::{CubicSegment,Point,Shape};

pub const MAX_SAMPLES:usize=4096;
pub const MAX_FITTED_SEGMENTS:usize=2048;
const MAX_COORD_MM:f32=100_000.0;

fn valid(p:Point)->bool{
    p.x.is_finite() && p.y.is_finite() &&
    p.x.abs()<=MAX_COORD_MM && p.y.abs()<=MAX_COORD_MM
}
fn length(a:Point,b:Point)->f64{
    ((a.x as f64-b.x as f64).powi(2)+
     (a.y as f64-b.y as f64).powi(2)).sqrt()
}
fn distance_segment(p:Point,a:Point,b:Point)->f64{
    let dx=b.x as f64-a.x as f64;
    let dy=b.y as f64-a.y as f64;
    let den=dx*dx+dy*dy;
    if den<=1e-18{return length(p,a);}
    let t=(((p.x as f64-a.x as f64)*dx+
        (p.y as f64-a.y as f64)*dy)/den).clamp(0.0,1.0);
    let projected=Point{
        x:(a.x as f64+t*dx) as f32,
        y:(a.y as f64+t*dy) as f32,
    };
    length(p,projected)
}
/// Removes redundant collinear vertices within a millimetre tolerance,
/// preserving endpoints exactly. No callback or document mutation occurs.
pub fn simplify_points(raw:&[Point],tolerance_mm:f32)->Result<Vec<Point>,String>{
    if !(0.0..=100.0).contains(&tolerance_mm) || !tolerance_mm.is_finite(){
        return Err("Simplification tolerance must be between 0 and 100 mm".into());
    }
    if raw.len()>MAX_SAMPLES{return Err("Stroke exceeds 4096 sampled points".into());}
    if raw.len()<2{return Err("At least two points are required".into());}
    if raw.iter().any(|p|!valid(*p)){return Err("Invalid stroke coordinates".into());}
    let mut points:Vec<Point>=Vec::with_capacity(raw.len());
    for &p in raw{
        if points.last().copied()!=Some(p){points.push(p);}
    }
    if points.len()<2{return Err("Stroke must contain two distinct points".into());}
    let n=points.len();
    let mut keep=vec![false;n];
    keep[0]=true;keep[n-1]=true;
    let mut stack=vec![(0usize,n-1)];
    while let Some((begin,end))=stack.pop(){
        if end<=begin+1{continue;}
        let mut far=0.0f64;
        let mut argmax=None;
        for i in (begin+1)..end{
            let d=distance_segment(points[i],points[begin],points[end]);
            if d>far{far=d;argmax=Some(i);}
        }
        if far>tolerance_mm as f64{
            if let Some(i)=argmax{
                keep[i]=true;
                stack.push((begin,i));
                stack.push((i,end));
            }
        }
    }
    Ok(points.into_iter().enumerate().filter_map(|(i,p)|keep[i].then_some(p)).collect())
}
/// Catmull-Rom-inspired cubic fitting of a simplified open stroke. The
/// smoothness is 0..1: 0 gives exact straight cubic segments; 1 provides
/// a smoothly interpolated tangent. The exact endpoints are retained.
pub fn smooth_open_points(points:&[Point],smoothness:f32)->Result<Shape,String>{
    if !smoothness.is_finite()||!(0.0..=1.0).contains(&smoothness){
        return Err("Smoothness must be between 0 and 1".into());
    }
    if points.len()<2||points.len()>MAX_FITTED_SEGMENTS+1{
        return Err("Result must have 2..2049 path anchors".into());
    }
    if points.iter().any(|p|!valid(*p)){return Err("Invalid anchor coordinates".into());}
    let mut segments=Vec::with_capacity(points.len()-1);
    for i in 0..points.len()-1{
        let a=points[i];
        let b=points[i+1];
        let (mut c1,mut c2)=(
            Point{x:a.x+(b.x-a.x)/3.0,y:a.y+(b.y-a.y)/3.0},
            Point{x:a.x+2.0*(b.x-a.x)/3.0,y:a.y+2.0*(b.y-a.y)/3.0},
        );
        if smoothness>0.0{
            let prev=if i==0{a}else{points[i-1]};
            let after=if i+2>=points.len(){b}else{points[i+2]};
            let s=smoothness as f64/6.0;
            let p1=Point{x:(a.x as f64+(b.x as f64-prev.x as f64)*s) as f32,
                y:(a.y as f64+(b.y as f64-prev.y as f64)*s) as f32};
            let p2=Point{x:(b.x as f64-(after.x as f64-a.x as f64)*s) as f32,
                y:(b.y as f64-(after.y as f64-a.y as f64)*s) as f32};
            c1=Point{x:c1.x+(p1.x-c1.x)*smoothness,
                y:c1.y+(p1.y-c1.y)*smoothness};
            c2=Point{x:c2.x+(p2.x-c2.x)*smoothness,
                y:c2.y+(p2.y-c2.y)*smoothness};
        }
        if !valid(c1)||!valid(c2){
            return Err("Smooth curve would leave safe page coordinates".into());
        }
        segments.push(CubicSegment{control1:c1,control2:c2,end:b});
    }
    Ok(Shape::Bezier{start:points[0],segments,closed:false})
}
/// Fit a closed freehand loop. Each cubic has cyclic Catmull-Rom
/// tangents, including the closing segment, so there is no sharp seam.
/// The final endpoint equals the first anchor exactly.
pub fn fit_closed_stroke(raw:&[Point],tolerance_mm:f32,smoothness:f32)
    ->Result<Shape,String>{
    if raw.len()<3{return Err("Closed loop requires at least three samples".into());}
    if raw.len()>MAX_SAMPLES{return Err("Stroke exceeds 4096 sampled points".into());}
    if raw.iter().any(|p|!valid(*p)){return Err("Invalid stroke coordinates".into());}
    if !smoothness.is_finite()||!(0.0..=1.0).contains(&smoothness){
        return Err("Smoothness must be between 0 and 1".into());
    }
    if !tolerance_mm.is_finite()||!(0.0..=100.0).contains(&tolerance_mm){
        return Err("Simplification tolerance must be between 0 and 100 mm".into());
    }
    let mut samples=raw.to_vec();
    let initial=samples[0];
    // A near-duplicate final click should not add a tiny closing segment.
    while samples.len()>3 &&
        length(initial,*samples.last().unwrap())<=tolerance_mm.max(0.0001) as f64{
        samples.pop();
    }
    let anchors=simplify_points(&samples,tolerance_mm)?;
    if anchors.len()<3{
        return Err("Draw a loop containing at least three distinct corners".into());
    }
    if anchors.len()>MAX_FITTED_SEGMENTS{
        return Err("Closed result would exceed 2048 cubic segments".into());
    }
    let n=anchors.len();
    let mut segments=Vec::with_capacity(n);
    for i in 0..n{
        let a=anchors[i];
        let b=anchors[(i+1)%n];
        let prev=anchors[(i+n-1)%n];
        let after=anchors[(i+2)%n];
        let t=smoothness as f64;
        let straight1=Point{x:a.x+(b.x-a.x)/3.0,y:a.y+(b.y-a.y)/3.0};
        let straight2=Point{x:a.x+2.0*(b.x-a.x)/3.0,y:a.y+2.0*(b.y-a.y)/3.0};
        let curved1=Point{
            x:(a.x as f64+(b.x as f64-prev.x as f64)*t/6.0) as f32,
            y:(a.y as f64+(b.y as f64-prev.y as f64)*t/6.0) as f32,
        };
        let curved2=Point{
            x:(b.x as f64-(after.x as f64-a.x as f64)*t/6.0) as f32,
            y:(b.y as f64-(after.y as f64-a.y as f64)*t/6.0) as f32,
        };
        let control1=Point{
            x:straight1.x+(curved1.x-straight1.x)*smoothness,
            y:straight1.y+(curved1.y-straight1.y)*smoothness,
        };
        let control2=Point{
            x:straight2.x+(curved2.x-straight2.x)*smoothness,
            y:straight2.y+(curved2.y-straight2.y)*smoothness,
        };
        if !valid(control1)||!valid(control2){
            return Err("Closed curve would exceed safe page coordinates".into());
        }
        segments.push(CubicSegment{control1,control2,end:b});
    }
    Ok(Shape::Bezier{start:anchors[0],segments,closed:true})
}
/// Simplify raw mouse/tablet samples and produce editable cubic segments.
pub fn fit_stroke(raw:&[Point],tolerance_mm:f32,smoothness:f32)
    ->Result<Shape,String>{
    let simplified=simplify_points(raw,tolerance_mm)?;
    smooth_open_points(&simplified,smoothness)
}
/// Convert an existing open polyline into a simplified editable Bézier,
/// or simplify an existing open cubic by sampling its geometry. This
/// operation deliberately re-fits the curve; caller must checkpoint Undo.
pub fn refit_path(shape:&Shape,tolerance_mm:f32,smoothness:f32)
    ->Result<Shape,String>{
    match shape{
        Shape::Polyline{points,closed:false}=>fit_stroke(points,tolerance_mm,smoothness),
        Shape::Polyline{points,closed:true}=>fit_closed_stroke(points,tolerance_mm,smoothness),
        Shape::Bezier{start,segments,closed} if !segments.is_empty()=>{
            if segments.len()>500{
                return Err("Curves with more than 500 segments cannot be refitted".into());
            }
            let mut raw=Vec::with_capacity(segments.len()*8+1);
            raw.push(*start);
            let mut a=*start;
            for seg in segments{
                for j in 1..=8{
                    let t=j as f64/8.0;
                    let u=1.0-t;
                    let sample=Point{
                        x:(u*u*u*a.x as f64+3.0*u*u*t*seg.control1.x as f64+
                            3.0*u*t*t*seg.control2.x as f64+t*t*t*seg.end.x as f64) as f32,
                        y:(u*u*u*a.y as f64+3.0*u*u*t*seg.control1.y as f64+
                            3.0*u*t*t*seg.control2.y as f64+t*t*t*seg.end.y as f64) as f32,
                    };
                    raw.push(sample);
                }
                a=seg.end;
            }
            if *closed{
                fit_closed_stroke(&raw,tolerance_mm,smoothness)
            }else{
                fit_stroke(&raw,tolerance_mm,smoothness)
            }
        },
        _=>Err("Select one editable native Bézier or polyline path".into()),
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    fn p(x:f32,y:f32)->Point{Point{x,y}}
    fn end_points(shape:&Shape)->(Point,Point){
        match shape{
            Shape::Bezier{start,segments,..}=>(*start,segments.last().unwrap().end),
            _=>panic!("Expected cubic"),
        }
    }
    #[test]
    fn closed_stroke_has_smooth_seam_and_no_duplicate_anchor(){
        let raw=[p(0.,0.),p(10.,0.),p(10.,10.),p(0.,10.),p(0.1,0.1)];
        let fitted=fit_closed_stroke(&raw,0.25,1.0).unwrap();
        let Shape::Bezier{start,segments,closed}=&fitted else{panic!()};
        assert!(*closed);
        assert_eq!(*start,segments.last().unwrap().end);
        assert_eq!(segments.len(),4);
        let n=segments.len();
        let incoming=p(start.x-segments[n-1].control2.x,
            start.y-segments[n-1].control2.y);
        let outgoing=p(segments[0].control1.x-start.x,
            segments[0].control1.y-start.y);
        assert!((incoming.x*outgoing.y-incoming.y*outgoing.x).abs()<0.001);
    }
    #[test]
    fn closed_polyline_refitting_retains_editable_closed_path(){
        let poly=Shape::Polyline{points:vec![
            p(0.,0.),p(10.,0.),p(10.,10.),p(0.,10.),
        ],closed:true};
        let fitted=refit_path(&poly,0.1,0.85).unwrap();
        let Shape::Bezier{start,segments,closed}=fitted else{panic!()};
        assert!(closed);
        assert_eq!(segments.last().unwrap().end,start);
    }
    #[test]
    fn degenerate_closed_loop_is_rejected_without_geometry(){
        let raw=[p(0.,0.),p(0.,0.),p(0.,0.),p(0.,0.)];
        assert!(fit_closed_stroke(&raw,0.2,0.8).is_err());
        assert!(fit_closed_stroke(&[p(0.,0.),p(1.,0.)],0.2,0.8).is_err());
    }
    #[test]
    fn straight_samples_reduce_to_two_exact_endpoints(){
        let raw:Vec<_>=(0..100).map(|i|p(i as f32,2.*i as f32)).collect();
        let out=simplify_points(&raw,0.2).unwrap();
        assert_eq!(out,vec![raw[0],raw[99]]);
        let shape=fit_stroke(&raw,0.2,0.75).unwrap();
        assert_eq!(end_points(&shape),(raw[0],raw[99]));
        let Shape::Bezier{segments,..}=shape else{panic!()};
        assert_eq!(segments.len(),1);
    }
    #[test]
    fn sharp_corner_is_retained_with_small_tolerance(){
        let raw=[p(0.,0.),p(10.,0.),p(10.,10.)];
        assert_eq!(simplify_points(&raw,0.1).unwrap(),raw);
        assert_eq!(simplify_points(&raw,20.).unwrap(),[raw[0],raw[2]]);
    }
    #[test]
    fn smoothness_zero_is_straight_cubic_and_one_has_tangency(){
        let pts=[p(0.,0.),p(10.,0.),p(10.,10.),p(20.,20.)];
        let Shape::Bezier{segments:zero,..}=smooth_open_points(&pts,0.).unwrap()
            else{panic!()};
        assert_eq!(zero[0].control1,p(10./3.,0.));
        let Shape::Bezier{segments:smooth,..}=smooth_open_points(&pts,1.).unwrap()
            else{panic!()};
        let pivot=pts[1];
        let left=p(pivot.x-smooth[0].control2.x,pivot.y-smooth[0].control2.y);
        let right=p(smooth[1].control1.x-pivot.x,smooth[1].control1.y-pivot.y);
        assert!((left.x*right.y-left.y*right.x).abs()<0.0005);
    }
    #[test]
    fn duplicates_and_invalid_points_are_rejected_safely(){
        assert!(fit_stroke(&[p(1.,1.)],0.2,0.8).is_err());
        assert!(fit_stroke(&[p(1.,1.),p(1.,1.)],0.2,0.8).is_err());
        assert!(fit_stroke(&[p(f32::NAN,1.),p(1.,1.)],0.2,0.8).is_err());
        assert!(fit_stroke(&[p(0.,0.),p(1.,0.)],f32::INFINITY,0.8).is_err());
        assert!(fit_stroke(&[p(0.,0.),p(1.,0.)],0.2,-0.2).is_err());
        assert!(fit_stroke(&[p(0.,0.),p(1.,0.)],0.2,1.2).is_err());
        assert!(fit_stroke(&[p(0.,0.),p(100_001.,0.)],0.2,0.8).is_err());
    }
    #[test]
    fn large_4096_point_stroke_is_bounded_and_nonrecursive(){
        let raw:Vec<_>=(0..4096).map(|i|{
            let x=i as f32*0.1;
            p(x,(x*0.05).sin()*20.)
        }).collect();
        let shape=fit_stroke(&raw,0.5,0.6).unwrap();
        let Shape::Bezier{segments,..}=&shape else{panic!()};
        assert!(segments.len()<=2048);
        assert!(segments.len()<raw.len()-1);
        assert!(fit_stroke(&[raw,vec![p(5.,5.)]].concat(),0.5,0.6).is_err());
    }
    #[test]
    fn strict_tolerance_rejects_more_than_2048_output_segments(){
        let raw:Vec<_>=(0..4096).map(|i|p(i as f32*0.1,
            if i%2==0{0.0}else{2.0})).collect();
        let simplified=simplify_points(&raw,0.0).unwrap();
        assert_eq!(simplified.len(),4096);
        assert!(smooth_open_points(&simplified,0.6).is_err());
    }
    #[test]
    fn refit_bezier_and_polyline_preserve_endpoints(){
        let poly=Shape::Polyline{points:vec![
            p(0.,0.),p(2.,1.),p(4.,2.),p(10.,5.),
        ],closed:false};
        assert_eq!(end_points(&refit_path(&poly,0.5,0.8).unwrap()),
            (p(0.,0.),p(10.,5.)));
        let bez=refit_path(&poly,0.5,0.8).unwrap();
        let bez2=refit_path(&bez,0.5,0.5).unwrap();
        assert_eq!(end_points(&bez2),(p(0.,0.),p(10.,5.)));
    }
    #[test]
    fn unsupported_shapes_do_not_refit(){
        let closed=Shape::Rectangle{x:0.,y:0.,w:10.,h:10.};
        assert!(refit_path(&closed,0.5,0.5).is_err());
    }
}
