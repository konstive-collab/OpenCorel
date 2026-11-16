//! Geometry rules for Corel-style drag-to-shape cubic Bézier handles.
//! A drag at an anchor produces a symmetric tangent: the incoming handle
//! points against the drag, and the outgoing handle follows the drag.
use opencorel_core::Point;

const MAX_COORD_MM:f32=100_000.0;

fn valid(p:Point)->bool{
    p.x.is_finite()&&p.y.is_finite()
        &&p.x.abs()<=MAX_COORD_MM&&p.y.abs()<=MAX_COORD_MM
}

/// Returns (incoming, outgoing) for a smooth anchor. This function is pure and
/// rejects overflow rather than writing invalid geometry into the document.
pub(super) fn symmetric_tangent(anchor:Point,cursor:Point)->Option<(Point,Point)>{
    if !valid(anchor)||!valid(cursor){return None;}
    let incoming=Point{
        x:2.0*anchor.x-cursor.x,
        y:2.0*anchor.y-cursor.y,
    };
    valid(incoming).then_some((incoming,cursor))
}

/// Lightweight temporary stroke displayed between the last anchor and pointer.
/// Uses the same incoming straight-line fallback as a future curve_click.
pub(super) fn preview_segment(start:Point,outgoing:Option<Point>,end:Point)->Vec<Point>{
    let a=outgoing.unwrap_or(Point{
        x:start.x+(end.x-start.x)/3.0,
        y:start.y+(end.y-start.y)/3.0,
    });
    let b=Point{
        x:start.x+2.0*(end.x-start.x)/3.0,
        y:start.y+2.0*(end.y-start.y)/3.0,
    };
    let mut points=Vec::with_capacity(17);
    for i in 0..=16{
        let t=i as f32/16.0;
        let u=1.0-t;
        points.push(Point{
            x:u*u*u*start.x+3.0*u*u*t*a.x+3.0*u*t*t*b.x+t*t*t*end.x,
            y:u*u*u*start.y+3.0*u*u*t*a.y+3.0*u*t*t*b.y+t*t*t*end.y,
        });
    }
    points
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn preview_segment_preserves_endpoints(){
        let start=Point{x:1.0,y:2.0};
        let end=Point{x:20.0,y:30.0};
        let line=preview_segment(start,Some(Point{x:10.0,y:0.0}),end);
        assert_eq!(line.len(),17);
        assert_eq!(line[0],start);
        assert_eq!(line[16],end);
    }
    #[test]
    fn horizontal_drag_creates_opposed_handles(){
        let (before,after)=symmetric_tangent(
            Point{x:20.0,y:30.0},Point{x:35.0,y:30.0}).unwrap();
        assert_eq!(before,Point{x:5.0,y:30.0});
        assert_eq!(after,Point{x:35.0,y:30.0});
    }
    #[test]
    fn diagonal_drag_preserves_collinearity_and_equal_lengths(){
        let a=Point{x:10.0,y:20.0};
        let (before,after)=symmetric_tangent(a,Point{x:14.0,y:27.0}).unwrap();
        assert_eq!(before,Point{x:6.0,y:13.0});
        assert_eq!(after,Point{x:14.0,y:27.0});
    }
    #[test]
    fn rejects_nonfinite_and_out_of_range_tangents(){
        let a=Point{x:10.0,y:10.0};
        assert!(symmetric_tangent(a,Point{x:f32::NAN,y:20.0}).is_none());
        assert!(symmetric_tangent(a,Point{x:f32::INFINITY,y:0.0}).is_none());
        assert!(symmetric_tangent(Point{x:90_000.0,y:0.0},
            Point{x:-90_000.0,y:0.0}).is_none());
    }
    #[test]
    fn zero_drag_is_a_cusp_equivalent(){
        let a=Point{x:1.0,y:2.0};
        assert_eq!(symmetric_tangent(a,a),Some((a,a)));
    }
}
