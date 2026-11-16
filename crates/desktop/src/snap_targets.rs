//! Screen-tolerance-aware geometric snapping to visible native anchors.
//!
//! Deliberately excludes bitmap pixels, SVG fragments and cubic control
//! handles. Those are not reliable anchors and would make drawing sticky.
//! The scan is bounded to avoid an unbounded cost on imported outlines.
use opencorel_core::{Document,Point,Shape};

const MAX_VISITED_POINTS:usize=30_000;
const MAX_OBJECTS:usize=8_000;

#[derive(Clone,Copy,Debug,PartialEq)]
pub(super) struct SnapHit{
    pub point:Point,
    pub object_id:u64,
    pub distance_mm:f32,
}
fn valid(p:Point)->bool{
    p.x.is_finite()&&p.y.is_finite()
        && p.x.abs()<=100_000.0&&p.y.abs()<=100_000.0
}
pub(super) fn nearest_anchor(
    doc:&Document,p:Point,tolerance_mm:f32,excluded_id:Option<u64>
)->Option<SnapHit>{
    if !valid(p)||!tolerance_mm.is_finite()||tolerance_mm<=0.0{
        return None;
    }
    let tol=tolerance_mm.min(500.0);
    let mut best_dist=tol as f64*tol as f64;
    let mut best=None;
    let mut visited=0usize;
    for obj in doc.objects.iter().take(MAX_OBJECTS){
        if visited>=MAX_VISITED_POINTS{break;}
        if Some(obj.id)==excluded_id{continue;}
        if !doc.layer(obj.layer_id).is_some_and(|layer|layer.visible){continue;}
        let mut check=|candidate:Point|{
            if visited>=MAX_VISITED_POINTS{return;}
            visited+=1;
            if !valid(candidate){return;}
            let dx=candidate.x as f64-p.x as f64;
            let dy=candidate.y as f64-p.y as f64;
            let d=dx*dx+dy*dy;
            if d<=best_dist && (d<best_dist || best.is_none()){
                best_dist=d;
                best=Some(SnapHit{
                    point:candidate,object_id:obj.id,
                    distance_mm:(d.sqrt()) as f32,
                });
            }
        };
        match &obj.shape{
            Shape::Bezier{start,segments,..}=>{
                check(*start);
                for seg in segments{
                    check(seg.end);
                    if visited>=MAX_VISITED_POINTS{break;}
                }
            }
            Shape::CompoundPath{contours}=>{
                for contour in contours{
                    check(contour.start);
                    for seg in &contour.segments{
                        check(seg.end);
                        if visited>=MAX_VISITED_POINTS{break;}
                    }
                    if visited>=MAX_VISITED_POINTS{break;}
                }
            }
            Shape::Polyline{points,..}=>{
                for point in points{
                    check(*point);
                    if visited>=MAX_VISITED_POINTS{break;}
                }
            }
            Shape::Rectangle{x,y,w,h}=>{
                for corner in [
                    Point{x:*x,y:*y},Point{x:*x+*w,y:*y},
                    Point{x:*x+*w,y:*y+*h},Point{x:*x,y:*y+*h},
                ]{check(corner);}
            }
            _=>{}
        }
    }
    best
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{CubicSegment,Style};
    fn pt(x:f32,y:f32)->Point{Point{x,y}}
    #[test]
    fn nearest_visible_anchor_wins_including_cubic_end(){
        let mut d=Document::default();
        let far=d.add(Shape::Polyline{points:vec![pt(20.,20.),pt(15.,15.)],
            closed:false},Style::default());
        let close=d.add(Shape::Bezier{start:pt(10.,10.),
            segments:vec![CubicSegment{control1:pt(11.,11.),
                control2:pt(12.,12.),end:pt(13.,13.)}],closed:false},
            Style::default());
        let h=nearest_anchor(&d,pt(13.25,13.1),3.,None).unwrap();
        assert_eq!(h.point,pt(13.,13.));
        assert_eq!(h.object_id,close);
        assert_eq!(nearest_anchor(&d,pt(13.,13.),1.,Some(close)),
            None);
        assert_ne!(far,close);
    }
    #[test]
    fn respects_visibility_and_locked_snap_targets(){
        let mut d=Document::default();
        let id=d.add(Shape::Polyline{points:vec![pt(5.,5.),pt(10.,10.)],
            closed:false},Style::default());
        d.layers[0].locked=true;
        assert_eq!(nearest_anchor(&d,pt(5.1,5.),1.,None).unwrap().object_id,id,
            "locked objects are valid reference targets");
        d.layers[0].visible=false;
        assert!(nearest_anchor(&d,pt(5.,5.),1.,None).is_none());
    }
    #[test]
    fn rectangle_corners_not_raster_pixels(){
        let mut d=Document::default();
        d.add(Shape::Rectangle{x:5.,y:5.,w:10.,h:15.},Style::default());
        assert_eq!(nearest_anchor(&d,pt(14.8,20.1),1.,None).unwrap().point,
            pt(15.,20.));
        assert!(nearest_anchor(&d,pt(9.,12.),1.,None).is_none());
    }
    #[test]
    fn rejecting_invalid_input_and_malformed_geometry(){
        let mut d=Document::default();
        d.add(Shape::Polyline{points:vec![pt(f32::NAN,1.),pt(4.,4.)],
            closed:false},Style::default());
        assert!(nearest_anchor(&d,pt(f32::NAN,0.),5.,None).is_none());
        for tol in [f32::NAN,f32::INFINITY,0.,-1.]{
            assert!(nearest_anchor(&d,pt(4.,4.),tol,None).is_none());
        }
        assert_eq!(nearest_anchor(&d,pt(4.,4.),0.01,None).unwrap().point,
            pt(4.,4.));
    }
    #[test]
    fn dense_geometry_is_bounded_and_deterministic(){
        let mut d=Document::default();
        let mut points=vec![pt(200.,200.);50_000];
        points[0]=pt(4.,4.);
        d.add(Shape::Polyline{points,closed:false},Style::default());
        assert_eq!(nearest_anchor(&d,pt(4.2,4.),1.,None).unwrap().point,
            pt(4.,4.));
        assert!(nearest_anchor(&d,pt(9.,9.),1.,None).is_none());
    }
}
