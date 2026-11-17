//! Precise object picking for the desktop canvas. Broad-phase bounds are
//! handled by Document; here we test painted shape instead of selecting empty
//! space inside a large rectangular cubic bounding box.
use crate::{Object,Point,Shape,CubicSegment,Contour};

fn dist2(a:Point,b:Point)->f64{
    let dx=a.x as f64-b.x as f64;
    let dy=a.y as f64-b.y as f64;
    dx*dx+dy*dy
}
fn segment_dist2(p:Point,a:Point,b:Point)->f64{
    let dx=b.x as f64-a.x as f64;
    let dy=b.y as f64-a.y as f64;
    let len2=dx*dx+dy*dy;
    if len2<=1e-12{return dist2(p,a);}
    let t=(((p.x as f64-a.x as f64)*dx+
        (p.y as f64-a.y as f64)*dy)/len2).clamp(0.0,1.0);
    let px=a.x as f64+t*dx;
    let py=a.y as f64+t*dy;
    (p.x as f64-px).powi(2)+(p.y as f64-py).powi(2)
}
fn ray_crosses(p:Point,a:Point,b:Point)->bool{
    let y=p.y as f64;
    let ay=a.y as f64;
    let by=b.y as f64;
    if (ay>y)==(by>y){return false;}
    let x=a.x as f64+(y-ay)*(b.x as f64-a.x as f64)/(by-ay);
    x>p.x as f64
}
fn trace(points:&[Point],closed:bool,p:Point,tol2:f64)->(bool,bool){
    if points.is_empty(){return (false,false);}
    if points.len()==1{return (dist2(p,points[0])<=tol2,false);}
    let mut hit=false;
    let mut inside=false;
    for i in 1..points.len(){
        let a=points[i-1];let b=points[i];
        if segment_dist2(p,a,b)<=tol2{hit=true;}
        if closed && ray_crosses(p,a,b){inside=!inside;}
    }
    if closed{
        let a=*points.last().unwrap();let b=points[0];
        if segment_dist2(p,a,b)<=tol2{hit=true;}
        if ray_crosses(p,a,b){inside=!inside;}
    }
    (hit,inside)
}
fn cubic(a:Point,s:&CubicSegment,t:f64)->Point{
    let u=1.0-t;
    Point{
        x:(u*u*u*a.x as f64+3.0*u*u*t*s.control1.x as f64+
            3.0*u*t*t*s.control2.x as f64+t*t*t*s.end.x as f64) as f32,
        y:(u*u*u*a.y as f64+3.0*u*u*t*s.control1.y as f64+
            3.0*u*t*t*s.control2.y as f64+t*t*t*s.end.y as f64) as f32,
    }
}
/// Scan a cubic using 16 line spans per segment. A fixed bounded sampling
/// count is a screen-space approximation; picking precision is increased at
/// high zoom by the caller's smaller mm tolerance. No allocations per point.
fn trace_cubic(start:Point,segments:&[CubicSegment],closed:bool,p:Point,
    tol2:f64)->(bool,bool){
    if segments.is_empty(){return (dist2(p,start)<=tol2,false);}
    const STEPS:usize=16;
    let mut current=start;
    let mut hit=false;
    let mut inside=false;
    for s in segments.iter().take(10_000){
        let a=current;
        for step in 1..=STEPS{
            let b=cubic(a,s,step as f64/STEPS as f64);
            if segment_dist2(p,current,b)<=tol2{hit=true;}
            if closed && ray_crosses(p,current,b){inside=!inside;}
            current=b;
        }
    }
    if closed{
        if segment_dist2(p,current,start)<=tol2{hit=true;}
        if ray_crosses(p,current,start){inside=!inside;}
    }
    (hit,inside)
}
/// Returns true when the pointer is on a stroke (including a small pixel
/// selection tolerance) or inside an explicitly filled closed shape.
/// Raw images/text/SVGs use their layout bounds in the caller.
pub fn hit_object(object:&Object,p:Point,tolerance_mm:f32)->bool{
    if !p.x.is_finite()||!p.y.is_finite()||
        !tolerance_mm.is_finite()||tolerance_mm<0.0{
        return false;
    }
    let fill=object.style.fill!="none";
    let tolerance=tolerance_mm as f64+
        (object.style.stroke_width_mm.max(0.0)/2.0) as f64;
    let tol2=tolerance*tolerance;
    match &object.shape{
        Shape::Rectangle{x,y,w,h}=>{
            let left=*x as f64;let top=*y as f64;
            let right=left+*w as f64;let bottom=top+*h as f64;
            let px=p.x as f64;let py=p.y as f64;
            let inside=px>=left&&px<=right&&py>=top&&py<=bottom;
            if fill && inside{return true;}
            if !inside && (px<left-tolerance||px>right+tolerance||
                py<top-tolerance||py>bottom+tolerance){return false;}
            let edge=(px-left).abs().min((px-right).abs())
                .min((py-top).abs()).min((py-bottom).abs());
            inside&&edge<=tolerance ||
            (px>=left-tolerance&&px<=right+tolerance&&
             (py-top).abs()<=tolerance || (py-bottom).abs()<=tolerance) ||
            (py>=top-tolerance&&py<=bottom+tolerance&&
             (px-left).abs()<=tolerance || (px-right).abs()<=tolerance)
        },
        Shape::Ellipse{x,y,w,h}=>{
            if *w<=0.0||*h<=0.0{return false;}
            let rx=*w as f64/2.0;let ry=*h as f64/2.0;
            let dx=p.x as f64-(*x as f64+rx);
            let dy=p.y as f64-(*y as f64+ry);
            let r=(dx*dx/(rx*rx)+dy*dy/(ry*ry)).sqrt();
            (fill && r<=1.0) ||
            (r-1.0).abs()*rx.min(ry)<=tolerance
        },
        Shape::Polyline{points,closed}=>{
            let (on_stroke,inside)=trace(points,*closed,p,tol2);
            on_stroke || (fill && *closed && inside)
        },
        Shape::Bezier{start,segments,closed}=>{
            let (on_stroke,inside)=trace_cubic(*start,segments,*closed,p,tol2);
            on_stroke || (fill && *closed && inside)
        },
        Shape::CompoundPath{contours}=>{
            let mut stroke=false;
            let mut fill_inside=false;
            for Contour{start,segments,closed} in contours.iter().take(1024){
                let (on,inside)=trace_cubic(*start,segments,*closed,p,tol2);
                stroke|=on;
                if *closed{fill_inside^=inside;}
            }
            stroke || (fill && fill_inside)
        },
        Shape::Text{..}|Shape::EmbeddedSvg{..}|Shape::Raster{..}=>true,
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Document,Style};
    fn p(x:f32,y:f32)->Point{Point{x,y}}
    fn style(fill:&str)->Style{
        Style{fill:fill.into(),stroke:"#111111".into(),stroke_width_mm:0.2}
    }
    #[test]
    fn unfilled_rectangle_ignores_empty_middle_but_picks_border(){
        let mut d=Document::default();
        let id=d.add(Shape::Rectangle{x:0.,y:0.,w:100.,h:100.},style("none"));
        assert!(!hit_object(d.get_mut(id).unwrap(),p(50.,50.),1.));
        assert!(hit_object(d.get_mut(id).unwrap(),p(0.3,50.),1.));
    }
    #[test]
    fn filled_rectangle_and_ellipse_accept_interior_only(){
        let mut d=Document::default();
        let r=d.add(Shape::Rectangle{x:0.,y:0.,w:100.,h:50.},style("#FFFFFF"));
        let e=d.add(Shape::Ellipse{x:0.,y:0.,w:100.,h:100.},style("#FFFFFF"));
        assert!(hit_object(d.get_mut(r).unwrap(),p(30.,30.),1.));
        assert!(hit_object(d.get_mut(e).unwrap(),p(50.,50.),1.));
        assert!(!hit_object(d.get_mut(e).unwrap(),p(95.,95.),1.));
    }
    #[test]
    fn open_cubic_only_picks_near_its_visible_trace(){
        let mut d=Document::default();
        let id=d.add(Shape::Bezier{start:p(0.,0.),closed:false,
            segments:vec![CubicSegment{
                control1:p(0.,100.),control2:p(100.,100.),end:p(100.,0.),
            }]},style("#EEEEEE"));
        let o=d.get_mut(id).unwrap();
        assert!(hit_object(o,p(50.,75.),1.));
        assert!(!hit_object(o,p(50.,20.),1.));
    }
    #[test]
    fn closed_polyline_fill_uses_polygon_not_bounding_box(){
        let mut d=Document::default();
        let id=d.add(Shape::Polyline{points:vec![
            p(0.,0.),p(100.,0.),p(0.,100.),
        ],closed:true},style("#FFFFFF"));
        let o=d.get_mut(id).unwrap();
        assert!(hit_object(o,p(10.,10.),1.));
        assert!(!hit_object(o,p(85.,85.),1.));
    }
    #[test]
    fn intersecting_open_lines_cannot_select_empty_gap(){
        let mut d=Document::default();
        let id=d.add(Shape::Polyline{points:vec![p(0.,0.),p(100.,100.)],
            closed:false},style("none"));
        assert!(!hit_object(d.get_mut(id).unwrap(),p(50.,30.),2.));
        assert!(hit_object(d.get_mut(id).unwrap(),p(50.,50.),2.));
    }
}
