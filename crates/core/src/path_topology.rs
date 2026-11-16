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
        JoinMode::Snap{..}=>false,
    };
    if matches!(mode,JoinMode::Snap{..}) && end!=b_start{
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
    let joined=Shape::Bezier{start:a_start,segments:a_segments,closed:false};
    node_edit::validate_modes(&joined,&modes_a)?;
    Ok((joined,modes_a,JoinReport{
        reversed_first:reverse_a,reversed_second:reverse_b,
        gap_mm:gap,added_connector:bridged,
    }))
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
    if i+1>=segments.len(){
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
    }
}
