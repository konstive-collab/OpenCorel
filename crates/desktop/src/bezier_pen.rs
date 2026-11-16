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

#[cfg(test)]
mod tests{
    use super::*;
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
