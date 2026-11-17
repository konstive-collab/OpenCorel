//! Non-destructive topology operations on native editable cubic paths.
//!
//! These functions operate on copies, refuse unsupported closed paths, and
//! return remapped persistent tangent metadata. Callers decide when to commit
//! one document undo snapshot and how to handle object-level references.
use crate::{CubicSegment,Point,Shape};
use crate::node_edit::{self,NodeAddress,NodeModeBinding};

const MAX_SEGMENTS:usize=10_000;
const EPSILON_MM:f32=0.000_1;

#[derive(Clone,Copy,Debug,PartialEq)]
pub enum JoinMode{
    /// Automatically match the nearest two endpoints, merging only if they
    /// are within the specified distance. The second start point and its
    /// incoming handle are moved to the first endpoint, without adding a span.
    Snap{tolerance_mm:f32},
    /// Snap endpoints then align tangent handles on the joining anchor.
    /// Retains the lengths of both handles and stores a persistent Smooth
    /// node mode, so subsequent mouse edits remain tangent-continuous.
    SmoothSnap{tolerance_mm:f32},
    /// Connect the nearest endpoints with a new straight cubic segment,
    /// without moving any existing endpoint or control handle.
    Bridge,
}
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct JoinReport{
    pub reversed_first:bool,
    pub reversed_second:bool,
    pub gap_mm:f32,
    pub added_connector:bool,
}
fn valid(p:Point)->bool{
    p.x.is_finite() && p.y.is_finite() &&
    p.x.abs()<=100_000.0 && p.y.abs()<=100_000.0
}
fn distance(a:Point,b:Point)->f32{(a.x-b.x).hypot(a.y-b.y)}
fn endpoints(shape:&Shape)->Result<(Point,Point),String>{
    match shape{
        Shape::Bezier{start,segments,closed:false} if !segments.is_empty()=>{
            if !valid(*start) || segments.iter().any(|s|
                !valid(s.control1)||!valid(s.control2)||!valid(s.end)){
                return Err("Curve contains invalid or out-of-range coordinates".into());
            }
            Ok((*start,segments.last().expect("nonempty").end))
        },
        _=>Err("Select two open editable Bézier curves".into()),
    }
}
/// Find the shortest possible connection between the first and second open
/// curves, reversing their directions where necessary. The first object keeps
/// its identity and all existing cubic segments. Both paths must be on the
/// same editable layer and share appearance; that is validated by the caller.
pub fn join_beziers(
    first:&Shape,first_modes:&[NodeModeBinding],
    second:&Shape,second_modes:&[NodeModeBinding],mode:JoinMode
)->Result<(Shape,Vec<NodeModeBinding>,JoinReport),String>{
    let (a_start,a_end)=endpoints(first)?;
    let (b_start,b_end)=endpoints(second)?;
    node_edit::validate_modes(first,first_modes)?;
    node_edit::validate_modes(second,second_modes)?;
    let tolerance=match mode{
        JoinMode::Snap{tolerance_mm}|JoinMode::SmoothSnap{tolerance_mm}=>{
            if !tolerance_mm.is_finite() || !(0.0..=1000.0).contains(&tolerance_mm){
                return Err("Snap tolerance must be within 0–1000 mm".into());
            }
            Some(tolerance_mm)
        },
        JoinMode::Bridge=>None,
    };
    let mut best=(f32::INFINITY,false,false);
    for rev_a in [false,true]{
        for rev_b in [false,true]{
            let a=if rev_a{a_start}else{a_end};
            let b=if rev_b{b_end}else{b_start};
            let gap=distance(a,b);
            if gap<best.0{best=(gap,rev_a,rev_b);}
        }
    }
    let (gap,reverse_a,reverse_b)=best;
    if !gap.is_finite(){return Err("Cannot compare non-finite endpoints".into());}
    if tolerance.is_some_and(|max|gap>max){
        return Err(format!(
            "Endpoints are {:.2} mm apart; adjust nodes or use Bridge",gap));
    }
    let mut a=first.clone();
    let mut b=second.clone();
    let mut modes_a=first_modes.to_vec();
    let mut modes_b=second_modes.to_vec();
    if reverse_a{
        node_edit::reverse_path(&mut a)?;
        node_edit::modes_after_reverse(&a,&mut modes_a);
    }
    if reverse_b{
        node_edit::reverse_path(&mut b)?;
        node_edit::modes_after_reverse(&b,&mut modes_b);
    }
    let Shape::Bezier{start:a_start,segments:mut a_segments,..}=a else{unreachable!()};
    let Shape::Bezier{start:b_start,segments:mut b_segments,..}=b else{unreachable!()};
    let end=a_segments.last().expect("nonempty").end;
    let bridged=match mode{
        JoinMode::Bridge=>gap>EPSILON_MM,
        JoinMode::Snap{..}|JoinMode::SmoothSnap{..}=>false,
    };
    if matches!(mode,JoinMode::Snap{..}|JoinMode::SmoothSnap{..}) && end!=b_start{
        let delta=Point{x:end.x-b_start.x,y:end.y-b_start.y};
        let first_b=&mut b_segments[0];
        first_b.control1=Point{
            x:first_b.control1.x+delta.x,
            y:first_b.control1.y+delta.y,
        };
        if !valid(first_b.control1){
            return Err("Snapping would move a control handle outside safe bounds".into());
        }
    }
    let total=a_segments.len()
        .checked_add(b_segments.len())
        .and_then(|count|count.checked_add(usize::from(bridged)))
        .ok_or("Path segment count overflow")?;
    if total>MAX_SEGMENTS{
        return Err("Joined path would exceed 10,000 cubic segments".into());
    }
    if bridged{
        let delta=Point{x:b_start.x-end.x,y:b_start.y-end.y};
        a_segments.push(CubicSegment{
            control1:Point{x:end.x+delta.x/3.0,y:end.y+delta.y/3.0},
            control2:Point{x:end.x+2.0*delta.x/3.0,y:end.y+2.0*delta.y/3.0},
            end:b_start,
        });
    }
    let offset=a_segments.len();
    a_segments.extend(b_segments);
    for entry in &mut modes_b{
        let NodeAddress::BezierEnd(i)=entry.address else{
            return Err("Invalid mode address in second path".into());
        };
        entry.address=NodeAddress::BezierEnd(offset+i);
    }
    modes_a.extend(modes_b);
    let mut joined=Shape::Bezier{start:a_start,segments:a_segments,closed:false};
    if matches!(mode,JoinMode::SmoothSnap{..}){
        let seam=node_edit::NodeAddress::BezierEnd(offset-1);
        // set_tangent validates geometry before changing either control
        // handle, so malformed joins return an error without side effects.
        node_edit::set_tangent(&mut joined,seam,
            node_edit::TangentOperation::Smooth)?;
        node_edit::set_mode(&mut modes_a,seam,
            node_edit::TangentOperation::Smooth);
    }
    node_edit::validate_modes(&joined,&modes_a)?;
    Ok((joined,modes_a,JoinReport{
        reversed_first:reverse_a,reversed_second:reverse_b,
        gap_mm:gap,added_connector:bridged,
    }))
}
/// Join two open polylines without flattening or converting their geometry.
/// The first object retains its start/end identity and its existing vertices.
/// SmoothSnap is deliberately unsupported: polylines have no cubic handles.
pub fn join_polylines(
    first:&Shape,second:&Shape,mode:JoinMode
)->Result<(Shape,JoinReport),String>{
    fn vertices(shape:&Shape)->Result<&[Point],String>{
        match shape{
            Shape::Polyline{points,closed:false} if points.len()>=2=>{
                if points.iter().any(|p|!valid(*p)){
                    return Err("Polyline contains invalid coordinates".into());
                }
                Ok(points)
            },
            _=>Err("Select two open polylines with at least two vertices each".into())
        }
    }
    let a=vertices(first)?;
    let b=vertices(second)?;
    let tolerance=match mode{
        JoinMode::SmoothSnap{..}=>return Err("Smooth joining requires Bézier handles".into()),
        JoinMode::Snap{tolerance_mm}=>{
            if !tolerance_mm.is_finite() || !(0.0..=1000.0).contains(&tolerance_mm){
                return Err("Snap tolerance must be within 0–1000 mm".into());
            }
            Some(tolerance_mm)
        },
        JoinMode::Bridge=>None,
    };
    let mut best=(f32::INFINITY,false,false);
    for rev_a in [false,true]{
        for rev_b in [false,true]{
            let end=if rev_a{a[0]}else{a[a.len()-1]};
            let start=if rev_b{b[b.len()-1]}else{b[0]};
            let gap=distance(end,start);
            if gap<best.0{best=(gap,rev_a,rev_b);}
        }
    }
    let (gap,reverse_a,reverse_b)=best;
    if !gap.is_finite(){return Err("Polyline endpoint distance is invalid".into());}
    if tolerance.is_some_and(|t|gap>t){
        return Err(format!("Endpoints are {:.2} mm apart; adjust nodes or use Bridge",gap));
    }
    let needed=a.len().checked_add(b.len())
        .ok_or("Polyline vertex count overflow")?;
    if needed>60_000{return Err("Joined polyline exceeds 60,000 vertices".into());}
    let mut points=a.to_vec();
    let mut tail=b.to_vec();
    if reverse_a{points.reverse();}
    if reverse_b{tail.reverse();}
    let end=*points.last().expect("validated length");
    let bridged=matches!(mode,JoinMode::Bridge) && gap>EPSILON_MM;
    if !bridged{
        // A snap is a single shared vertex, not two almost-coincident nodes.
        tail.remove(0);
    }
    points.extend(tail);
    Ok((Shape::Polyline{points,closed:false},JoinReport{
        reversed_first:reverse_a,reversed_second:reverse_b,
        gap_mm:gap,added_connector:bridged,
    }))
}

/// Split an open polyline at an existing interior vertex. The vertex is
/// shared by both resulting objects, so the original visual trace is exact.
pub fn split_polyline(shape:&Shape,cut:NodeAddress)->Result<(Shape,Shape),String>{
    let Shape::Polyline{points,closed:false}=shape else{
        return Err("Only an open polyline can be split".into());
    };
    let NodeAddress::Polyline(i)=cut else{
        return Err("Select an interior polyline vertex".into());
    };
    if points.len()<3 || i==0 || i>=points.len()-1{
        return Err("Cannot split at a terminal polyline vertex".into());
    }
    if points.len()>60_000 || points.iter().any(|p|!valid(*p)){
        return Err("Polyline has too many or invalid vertices".into());
    }
    Ok((
        Shape::Polyline{points:points[..=i].to_vec(),closed:false},
        Shape::Polyline{points:points[i..].to_vec(),closed:false},
    ))
}

/// Cut an open Bézier at an existing interior anchor. The cut anchor becomes
/// both the left end and the right start, so neither resulting path has a
/// missing segment. Its persistent tangent mode is removed at the seam.
pub fn split_bezier(
    shape:&Shape,modes:&[NodeModeBinding],cut:NodeAddress
)->Result<((Shape,Vec<NodeModeBinding>),(Shape,Vec<NodeModeBinding>)),String>{
    let Shape::Bezier{start,segments,closed:false}=shape else{
        return Err("Only open Bézier paths can be split at an anchor".into());
    };
    node_edit::validate_modes(shape,modes)?;
    let NodeAddress::BezierEnd(i)=cut else{
        return Err("Select an interior Bézier anchor to split the path".into());
    };
    if segments.len()<2 || i>=segments.len()-1{
        return Err("Cannot split a path at its terminal node".into());
    }
    if segments.iter().any(|s|!valid(s.control1)||!valid(s.control2)||!valid(s.end))
        || !valid(*start){
        return Err("Cannot split invalid path geometry".into());
    }
    let left=Shape::Bezier{
        start:*start,segments:segments[..=i].to_vec(),closed:false};
    let right=Shape::Bezier{
        start:segments[i].end,segments:segments[i+1..].to_vec(),closed:false};
    let mut left_modes=Vec::new();
    let mut right_modes=Vec::new();
    for entry in modes{
        if let NodeAddress::BezierEnd(index)=entry.address{
            if index<i{left_modes.push(*entry);}
            if index>i {
                right_modes.push(NodeModeBinding{
                    address:NodeAddress::BezierEnd(index-i-1),
                    mode:entry.mode,
                });
            }
        }
    }
    node_edit::validate_modes(&left,&left_modes)?;
    node_edit::validate_modes(&right,&right_modes)?;
    Ok(((left,left_modes),(right,right_modes)))
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::node_edit::TangentOperation as M;
    fn p(x:f32,y:f32)->Point{Point{x,y}}
    fn path(start:Point,mid:Point,end:Point)->Shape{
        let segment=|a:Point,b:Point|CubicSegment{
            control1:p(a.x+(b.x-a.x)/3.,a.y+(b.y-a.y)/3.),
            control2:p(a.x+2.*(b.x-a.x)/3.,a.y+2.*(b.y-a.y)/3.),end:b,
        };
        Shape::Bezier{start,segments:vec![segment(start,mid),segment(mid,end)],closed:false}
    }
    fn two()->(Shape,Shape){
        (path(p(0.,0.),p(10.,0.),p(20.,0.)),
         path(p(20.,0.),p(30.,0.),p(40.,0.)))
    }
    #[test]
    fn exact_endpoint_join_does_not_add_bridge(){
        let (a,b)=two();
        let (joined,modes,report)=join_beziers(&a,&[],&b,&[],
            JoinMode::Snap{tolerance_mm:2.}).unwrap();
        assert_eq!(report.gap_mm,0.);
        assert!(!report.added_connector);
        assert!(!report.reversed_first);
        assert!(!report.reversed_second);
        assert!(modes.is_empty());
        let Shape::Bezier{segments,..}=joined else{panic!()};
        assert_eq!(segments.len(),4);
        assert_eq!(segments.last().unwrap().end,p(40.,0.));
    }
    #[test]
    fn reversed_second_path_keeps_shape_and_node_modes(){
        let (a,mut b)=two();
        node_edit::reverse_path(&mut b).unwrap();
        let mode=[NodeModeBinding{address:NodeAddress::BezierEnd(0),mode:M::Smooth}];
        let (joined,modes,report)=join_beziers(&a,&[],&b,&mode,
            JoinMode::Snap{tolerance_mm:0.}).unwrap();
        assert!(report.reversed_second);
        assert_eq!(modes,[NodeModeBinding{address:NodeAddress::BezierEnd(2),mode:M::Smooth}]);
        node_edit::validate_modes(&joined,&modes).unwrap();
    }
    #[test]
    fn closest_endpoint_orientation_selected_even_for_first_path(){
        let (a,b)=two();
        let (joined,_,report)=join_beziers(&b,&[],&a,&[],
            JoinMode::Snap{tolerance_mm:0.}).unwrap();
        assert!(report.reversed_first);
        assert!(report.reversed_second);
        let Shape::Bezier{start,segments,..}=joined else{panic!()};
        assert_eq!(start,p(40.,0.));
        assert_eq!(segments.last().unwrap().end,p(0.,0.));
    }
    #[test]
    fn bridge_preserves_original_start_and_end_geometry(){
        let (a,_)=two();
        let b=path(p(100.,20.),p(120.,20.),p(140.,20.));
        let (joined,_,report)=join_beziers(&a,&[],&b,&[],JoinMode::Bridge).unwrap();
        assert!(report.added_connector);
        assert_eq!(report.gap_mm,(p(100.,20.).x-20.).hypot(20.));
        let Shape::Bezier{segments,..}=joined else{panic!()};
        assert_eq!(segments.len(),5);
        assert_eq!(segments[2].end,p(100.,20.));
        assert_eq!(segments[4].end,p(140.,20.));
    }
    #[test]
    fn snapping_endpoint_shifts_first_incoming_handle_only(){
        let (a,_)=two();
        let b=path(p(20.3,0.4),p(30.,0.),p(40.,0.));
        let (joined,_,report)=join_beziers(&a,&[],&b,&[],
            JoinMode::Snap{tolerance_mm:1.}).unwrap();
        assert!(report.gap_mm>0.);
        let Shape::Bezier{segments,..}=joined else{panic!()};
        // Segment 2 belongs to the second original curve. Its first control
        // handle is shifted by the endpoint delta, while its end is unchanged.
        assert_eq!(segments[1].end,p(20.,0.));
        assert_eq!(segments[3].end,p(40.,0.));
        let original_control=p(20.3+(30.-20.3)/3.,0.4+(0.-0.4)/3.);
        assert!((segments[2].control1.x-(original_control.x-0.3)).abs()<0.0001);
        assert!((segments[2].control1.y-(original_control.y-0.4)).abs()<0.0001);
    }
    #[test]
    fn smooth_join_creates_persistent_continuous_seam(){
        let (mut a,mut b)=two();
        if let Shape::Bezier{segments,..}=&mut a{
            segments[1].control2=p(13.,8.);
        }
        if let Shape::Bezier{segments,..}=&mut b{
            segments[0].control1=p(23.,-3.);
        }
        let (joined,modes,_)=join_beziers(
            &a,&[],&b,&[],JoinMode::SmoothSnap{tolerance_mm:0.}).unwrap();
        let seam=NodeAddress::BezierEnd(1);
        assert_eq!(node_edit::mode_at(&modes,seam),
            Some(node_edit::TangentOperation::Smooth));
        let Shape::Bezier{segments,..}=&joined else{panic!()};
        let anchor=segments[1].end;
        let incoming=p(anchor.x-segments[1].control2.x,
                       anchor.y-segments[1].control2.y);
        let outgoing=p(segments[2].control1.x-anchor.x,
                       segments[2].control1.y-anchor.y);
        assert!((incoming.x*outgoing.y-incoming.y*outgoing.x).abs()<0.0001);
        assert!(incoming.x*outgoing.x+incoming.y*outgoing.y>0.);
        node_edit::validate_modes(&joined,&modes).unwrap();
    }
    #[test]
    fn smooth_join_rejects_invalid_tolerance_and_distance(){
        let (a,b)=two();
        for t in [f32::NAN,f32::INFINITY,-1.,1000.01]{
            assert!(join_beziers(&a,&[],&b,&[],
                JoinMode::SmoothSnap{tolerance_mm:t}).is_err());
        }
        let b=path(p(100.,30.),p(130.,30.),p(140.,30.));
        assert!(join_beziers(&a,&[],&b,&[],
            JoinMode::SmoothSnap{tolerance_mm:1.}).is_err());
    }
    #[test]
    fn snap_rejects_far_endpoints_without_side_effects(){
        let (a,_)=two();
        let b=path(p(100.,100.),p(120.,100.),p(140.,100.));
        let original=a.clone();
        assert!(join_beziers(&a,&[],&b,&[],
            JoinMode::Snap{tolerance_mm:1.}).is_err());
        assert_eq!(a,original);
    }
    #[test]
    fn split_retains_segments_and_per_path_modes(){
        let original=Shape::Bezier{start:p(0.,0.),segments:vec![
            CubicSegment{control1:p(2.,0.),control2:p(6.,0.),end:p(10.,0.)},
            CubicSegment{control1:p(12.,0.),control2:p(16.,0.),end:p(20.,0.)},
            CubicSegment{control1:p(22.,0.),control2:p(26.,0.),end:p(30.,0.)},
            CubicSegment{control1:p(32.,0.),control2:p(36.,0.),end:p(40.,0.)},
        ],closed:false};
        let modes=[
            NodeModeBinding{address:NodeAddress::BezierEnd(0),mode:M::Smooth},
            NodeModeBinding{address:NodeAddress::BezierEnd(1),mode:M::Symmetric},
            NodeModeBinding{address:NodeAddress::BezierEnd(2),mode:M::Smooth},
        ];
        let ((left,l_modes),(right,r_modes))=split_bezier(
            &original,&modes,NodeAddress::BezierEnd(1)).unwrap();
        let Shape::Bezier{segments:l,..}=left else{panic!()};
        let Shape::Bezier{start:r_start,segments:r,..}=right else{panic!()};
        assert_eq!(l.len(),2);assert_eq!(r.len(),2);
        assert_eq!(r_start,p(20.,0.));
        assert_eq!(l_modes.len(),1);
        assert_eq!(l_modes[0].address,NodeAddress::BezierEnd(0));
        assert_eq!(r_modes.len(),1);
        assert_eq!(r_modes[0].address,NodeAddress::BezierEnd(0));
    }
    #[test]
    fn split_rejects_closed_paths_and_terminal_anchor(){
        let (mut a,_)=two();
        if let Shape::Bezier{closed,..}=&mut a{*closed=true;}
        assert!(split_bezier(&a,&[],NodeAddress::BezierEnd(0)).is_err());
        if let Shape::Bezier{closed,..}=&mut a{*closed=false;}
        assert!(split_bezier(&a,&[],NodeAddress::BezierEnd(1)).is_err());
        assert!(split_bezier(&a,&[],NodeAddress::BezierEnd(usize::MAX)).is_err());
        let empty=Shape::Bezier{start:p(0.,0.),segments:vec![],closed:false};
        assert!(split_bezier(&empty,&[],NodeAddress::BezierEnd(0)).is_err());
    }
    fn line(xs:&[(f32,f32)])->Shape{
        Shape::Polyline{
            points:xs.iter().map(|(x,y)|p(*x,*y)).collect(),
            closed:false,
        }
    }
    #[test]
    fn polyline_snap_preserves_vertex_type_and_removes_duplicate_joint(){
        let a=line(&[(0.,0.),(10.,0.)]);
        let b=line(&[(10.,0.),(20.,0.)]);
        let (joined,report)=join_polylines(&a,&b,JoinMode::Snap{tolerance_mm:0.}).unwrap();
        assert!(!report.added_connector);
        assert_eq!(joined,line(&[(0.,0.),(10.,0.),(20.,0.)]));
    }
    #[test]
    fn polyline_snap_reorients_nearest_ends(){
        let a=line(&[(0.,0.),(10.,0.)]);
        let b=line(&[(20.,0.),(10.,0.)]);
        let (joined,report)=join_polylines(&a,&b,JoinMode::Snap{tolerance_mm:0.}).unwrap();
        assert!(report.reversed_second);
        assert_eq!(joined,line(&[(0.,0.),(10.,0.),(20.,0.)]));
    }
    #[test]
    fn polyline_bridge_retains_both_endpoints(){
        let a=line(&[(0.,0.),(10.,0.)]);
        let b=line(&[(20.,0.),(30.,0.)]);
        let (joined,report)=join_polylines(&a,&b,JoinMode::Bridge).unwrap();
        assert!(report.added_connector);
        assert_eq!(report.gap_mm,10.);
        assert_eq!(joined,line(&[(0.,0.),(10.,0.),(20.,0.),(30.,0.)]));
    }
    #[test]
    fn polyline_snap_rejects_invalid_input_without_mutating_sources(){
        let a=line(&[(0.,0.),(10.,0.)]);
        let b=line(&[(20.,0.),(30.,0.)]);
        let original=a.clone();
        for tolerance in [-1.,f32::NAN,f32::INFINITY,1.]{
            assert!(join_polylines(&a,&b,JoinMode::Snap{
                tolerance_mm:tolerance
            }).is_err());
        }
        assert!(join_polylines(&a,&b,JoinMode::SmoothSnap{
            tolerance_mm:100.
        }).is_err());
        assert_eq!(a,original);
        let malformed=line(&[(0.,0.),(f32::NAN,1.)]);
        assert!(join_polylines(&malformed,&b,JoinMode::Bridge).is_err());
    }

    #[test]
    fn polyline_split_preserves_original_visual_trace(){
        let original=line(&[(0.,0.),(10.,5.),(20.,10.),(30.,20.)]);
        let (left,right)=split_polyline(&original,NodeAddress::Polyline(2)).unwrap();
        assert_eq!(left,line(&[(0.,0.),(10.,5.),(20.,10.)]));
        assert_eq!(right,line(&[(20.,10.),(30.,20.)]));
        assert_eq!(original,line(&[(0.,0.),(10.,5.),(20.,10.),(30.,20.)]));
    }
    #[test]
    fn polyline_split_rejects_terminal_closed_and_invalid_vertices(){
        let a=line(&[(0.,0.),(10.,0.),(20.,0.)]);
        for i in [0,2,3,usize::MAX]{
            assert!(split_polyline(&a,NodeAddress::Polyline(i)).is_err());
        }
        assert!(split_polyline(&a,NodeAddress::BezierStart).is_err());
        let mut closed=a.clone();
        if let Shape::Polyline{closed,..}=&mut closed{*closed=true;}
        assert!(split_polyline(&closed,NodeAddress::Polyline(1)).is_err());
        let broken=line(&[(0.,0.),(f32::NAN,0.),(20.,0.)]);
        assert!(split_polyline(&broken,NodeAddress::Polyline(1)).is_err());
    }

}
