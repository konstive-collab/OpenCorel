//! Structured, deterministic editing of polyline, cubic Bézier and compound path nodes.
//! Coordinates in millimetres. All operations reject invalid indices and nonfinite coordinates.
use crate::{CubicSegment,Point,Shape};
use serde::{Deserialize,Serialize};
use std::collections::HashSet;

#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash,Serialize,Deserialize)]
pub enum NodeAddress{
    Polyline(usize),
    BezierStart,
    BezierEnd(usize),
    BezierControl1(usize),
    BezierControl2(usize),
    CompoundStart(usize),
    CompoundEnd(usize,usize),
    CompoundControl1(usize,usize),
    CompoundControl2(usize,usize),
}
impl NodeAddress{
    pub fn is_handle(self)->bool{
        matches!(self,Self::BezierControl1(_) | Self::BezierControl2(_)
            | Self::CompoundControl1(_,_) | Self::CompoundControl2(_,_))
    }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub enum SegmentAddress{
    Polyline(usize),
    Bezier(usize),
    Compound(usize,usize),
}
pub fn editable(shape:&Shape)->bool{
    matches!(shape,Shape::Polyline{..}|Shape::Bezier{..}|Shape::CompoundPath{..})
}
/// Limit visible handles for huge outlined text paths.
fn push_handle(nodes:&mut Vec<(NodeAddress,Point)>,limit:usize,
    address:NodeAddress,point:Point){
    if nodes.len()<limit{nodes.push((address,point));}
}
pub fn handles(shape:&Shape,limit:usize)->Vec<(NodeAddress,Point)>{
    let mut nodes=Vec::new();
    if limit==0{return nodes;}

    match shape{
        Shape::Polyline{points,..}=>{
            for (i,p) in points.iter().enumerate().take(limit){push_handle(&mut nodes,limit,NodeAddress::Polyline(i),*p);}
        }
        Shape::Bezier{start,segments,..}=>{
            push_handle(&mut nodes,limit,NodeAddress::BezierStart,*start);
            for (i,s) in segments.iter().enumerate().take(limit){
                push_handle(&mut nodes,limit,NodeAddress::BezierEnd(i),s.end);
                push_handle(&mut nodes,limit,NodeAddress::BezierControl1(i),s.control1);
                push_handle(&mut nodes,limit,NodeAddress::BezierControl2(i),s.control2);
            }
        }
        Shape::CompoundPath{contours}=>{
            for (j,c) in contours.iter().enumerate(){
                push_handle(&mut nodes,limit,NodeAddress::CompoundStart(j),c.start);
                for (i,s) in c.segments.iter().enumerate(){
                    push_handle(&mut nodes,limit,NodeAddress::CompoundEnd(j,i),s.end);
                    push_handle(&mut nodes,limit,NodeAddress::CompoundControl1(j,i),s.control1);
                    push_handle(&mut nodes,limit,NodeAddress::CompoundControl2(j,i),s.control2);
                    if nodes.len()>=limit{break;}
                }
                if nodes.len()>=limit{break;}
            }
        }
        _=>{}
    }
    nodes
}
pub fn point_at(shape:&Shape,address:NodeAddress)->Option<Point>{
    match (shape,address){
        (Shape::Polyline{points,..},NodeAddress::Polyline(i))=>points.get(i).copied(),
        (Shape::Bezier{start,..},NodeAddress::BezierStart)=>Some(*start),
        (Shape::Bezier{segments,..},NodeAddress::BezierEnd(i))=>segments.get(i).map(|s|s.end),
        (Shape::Bezier{segments,..},NodeAddress::BezierControl1(i))=>segments.get(i).map(|s|s.control1),
        (Shape::Bezier{segments,..},NodeAddress::BezierControl2(i))=>segments.get(i).map(|s|s.control2),
        (Shape::CompoundPath{contours},NodeAddress::CompoundStart(j))=>contours.get(j).map(|c|c.start),
        (Shape::CompoundPath{contours},NodeAddress::CompoundEnd(j,i))=>
            contours.get(j)?.segments.get(i).map(|s|s.end),
        (Shape::CompoundPath{contours},NodeAddress::CompoundControl1(j,i))=>
            contours.get(j)?.segments.get(i).map(|s|s.control1),
        (Shape::CompoundPath{contours},NodeAddress::CompoundControl2(j,i))=>
            contours.get(j)?.segments.get(i).map(|s|s.control2),
        _=>None
    }
}
pub fn move_node(shape:&mut Shape,address:NodeAddress,to:Point)->Result<(),String>{
    if !valid(to){return Err("Node coordinates must be finite and within ±100000 mm".into());}
    let Some(old)=point_at(shape,address) else{return Err("No such editable node".into());};
    let delta=Point{x:to.x-old.x,y:to.y-old.y};
    // Moving an anchor also moves attached Bézier handles. Check their
    // destination BEFORE touching geometry: otherwise a legal anchor target
    // could overflow an existing handle and make the project unsaveable.
    let attached:Vec<Point>=match (&*shape,address){
        (Shape::Bezier{segments,..},NodeAddress::BezierStart)=>
            segments.first().map(|s|vec![s.control1]).unwrap_or_default(),
        (Shape::Bezier{segments,..},NodeAddress::BezierEnd(i))=>{
            let mut v=Vec::with_capacity(2);
            if let Some(s)=segments.get(i){v.push(s.control2);}
            if let Some(s)=segments.get(i+1){v.push(s.control1);}
            v
        }
        (Shape::CompoundPath{contours},NodeAddress::CompoundStart(j))=>
            contours.get(j).and_then(|c|c.segments.first())
                .map(|s|vec![s.control1]).unwrap_or_default(),
        (Shape::CompoundPath{contours},NodeAddress::CompoundEnd(j,i))=>{
            let mut v=Vec::with_capacity(2);
            if let Some(c)=contours.get(j){
                if let Some(s)=c.segments.get(i){v.push(s.control2);}
                if let Some(s)=c.segments.get(i+1){v.push(s.control1);}
            }
            v
        }
        _=>Vec::new(),
    };
    if attached.iter().any(|p|!valid(Point{x:p.x+delta.x,y:p.y+delta.y})){
        return Err("Moving this anchor would place a control handle outside safe bounds".into());
    }
    match (shape,address){
        (Shape::Polyline{points,..},NodeAddress::Polyline(i))=>{points[i]=to;}
        (Shape::Bezier{start,segments,..},NodeAddress::BezierStart)=>{
            *start=to;
            if let Some(first)=segments.first_mut(){shift(&mut first.control1,delta);}
        }
        (Shape::Bezier{segments,..},NodeAddress::BezierEnd(i))=>{
            segments[i].end=to;
            shift(&mut segments[i].control2,delta);
            if i+1<segments.len(){shift(&mut segments[i+1].control1,delta);}
        }
        (Shape::Bezier{segments,..},NodeAddress::BezierControl1(i))=>segments[i].control1=to,
        (Shape::Bezier{segments,..},NodeAddress::BezierControl2(i))=>segments[i].control2=to,
        (Shape::CompoundPath{contours},NodeAddress::CompoundStart(j))=>{
            let c=&mut contours[j]; c.start=to;
            if let Some(first)=c.segments.first_mut(){shift(&mut first.control1,delta);}
        }
        (Shape::CompoundPath{contours},NodeAddress::CompoundEnd(j,i))=>{
            let segments=&mut contours[j].segments;
            segments[i].end=to;
            shift(&mut segments[i].control2,delta);
            if i+1<segments.len(){shift(&mut segments[i+1].control1,delta);}
        }
        (Shape::CompoundPath{contours},NodeAddress::CompoundControl1(j,i))=>
            contours[j].segments[i].control1=to,
        (Shape::CompoundPath{contours},NodeAddress::CompoundControl2(j,i))=>
            contours[j].segments[i].control2=to,
        _=>return Err("Node does not belong to this shape".into()),
    }
    Ok(())
}
/// One-shot operations on the two handles adjacent to an *interior* cubic
/// anchor. No persistent coupling is implied: individual handles remain freely
/// editable afterward. These commands preserve the native document schema.
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum TangentOperation{
    /// Set both handle points to their anchor (visible corner / cusp).
    Corner,
    /// Align handle directions while retaining each existing length.
    Smooth,
    /// Align directions and equalize the two handle lengths.
    Symmetric,
}

/// Persistent per-anchor mode. Missing metadata in older KDR means independent
/// control handles, preserving legacy document behavior.
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct NodeModeBinding{
    pub address:NodeAddress,
    pub mode:TangentOperation,
}
pub fn mode_at(modes:&[NodeModeBinding],address:NodeAddress)->Option<TangentOperation>{
    modes.iter().find(|entry|entry.address==address).map(|entry|entry.mode)
}
/// Changes or adds a mode without editing any geometry. Caller must first
/// validate that address has a tangent pair.
pub fn set_mode(modes:&mut Vec<NodeModeBinding>,address:NodeAddress,
    mode:TangentOperation)->bool{
    if let Some(existing)=modes.iter_mut().find(|entry|entry.address==address){
        if existing.mode==mode{return false;}
        existing.mode=mode;return true;
    }
    modes.push(NodeModeBinding{address,mode});
    true
}
pub fn validate_modes(shape:&Shape,modes:&[NodeModeBinding])->Result<(),String>{
    if modes.len()>60_000{return Err("Too many node tangent modes".into());}
    let mut seen=HashSet::new();
    for entry in modes{
        if !has_tangent_pair(shape,entry.address)||!seen.insert(entry.address){
            return Err("Invalid or duplicate node tangent mode".into());
        }
    }
    Ok(())
}
/// After de Casteljau insertion all old end anchors from this segment
/// forward move by one slot. The newly inserted anchor is independent.
pub fn modes_after_insert(modes:&mut [NodeModeBinding],segment:SegmentAddress){
    for entry in modes{
        match (segment,&mut entry.address){
            (SegmentAddress::Bezier(index),NodeAddress::BezierEnd(old))
                if *old>=index=>*old+=1,
            (SegmentAddress::Compound(contour,index),
                NodeAddress::CompoundEnd(j,old))
                if *j==contour && *old>=index=>*old+=1,
            _=>{},
        }
    }
}
/// When deleting an end anchor, its node mode is removed; every later
/// end anchor shifts one slot toward the beginning of the path.
pub fn modes_after_remove(modes:&mut Vec<NodeModeBinding>,address:NodeAddress){
    modes.retain(|entry|match (address,entry.address){
        (NodeAddress::BezierStart,NodeAddress::BezierEnd(0))=>false,
        (NodeAddress::CompoundStart(j),NodeAddress::CompoundEnd(k,0)) if j==k=>false,
        _=>entry.address!=address,
    });
    for entry in modes{
        match (address,&mut entry.address){
            (NodeAddress::BezierStart,NodeAddress::BezierEnd(old))=>{
                if *old>0{*old-=1;}
            },
            (NodeAddress::BezierEnd(index),NodeAddress::BezierEnd(old))
                if *old>index=>*old-=1,
            (NodeAddress::CompoundStart(contour),NodeAddress::CompoundEnd(j,old))
                if *j==contour && *old>0=>*old-=1,
            (NodeAddress::CompoundEnd(contour,index),NodeAddress::CompoundEnd(j,old))
                if *j==contour && *old>index=>*old-=1,
            _=>{},
        }
    }
}
/// Move a handle with persistent tangent coupling (if selected anchor mode
/// requests it). No whole-shape clone is needed on each drag frame.
pub fn move_node_with_modes(shape:&mut Shape,address:NodeAddress,
    to:Point,modes:&[NodeModeBinding])->Result<(),String>{
    if !valid(to){return Err("Handle coordinates outside valid bounds".into());}
    let paired=match address{
        NodeAddress::BezierControl1(i) if i>0=>
            Some((NodeAddress::BezierEnd(i-1),NodeAddress::BezierControl2(i-1))),
        NodeAddress::BezierControl2(i)=>
            i.checked_add(1).map(|next|
                (NodeAddress::BezierEnd(i),NodeAddress::BezierControl1(next))),
        NodeAddress::CompoundControl1(j,i) if i>0=>
            Some((NodeAddress::CompoundEnd(j,i-1),
                NodeAddress::CompoundControl2(j,i-1))),
        NodeAddress::CompoundControl2(j,i)=>
            i.checked_add(1).map(|next|
                (NodeAddress::CompoundEnd(j,i),
                    NodeAddress::CompoundControl1(j,next))),
        _=>None,
    };
    let other=paired.and_then(|(anchor,opposite)|{
        let mode=mode_at(modes,anchor)?;
        if mode==TangentOperation::Corner{return None;}
        let centre=point_at(shape,anchor)?;
        let old_opposite=point_at(shape,opposite)?;
        let dx=to.x as f64-centre.x as f64;
        let dy=to.y as f64-centre.y as f64;
        let length=dx.hypot(dy);
        let old_length=(centre.x as f64-old_opposite.x as f64)
            .hypot(centre.y as f64-old_opposite.y as f64);
        let other_length=if mode==TangentOperation::Symmetric{length}else{old_length};
        let next=if length<1e-9{
            centre
        }else{
            Point{
                x:(centre.x as f64-dx/length*other_length) as f32,
                y:(centre.y as f64-dy/length*other_length) as f32,
            }
        };
        Some((opposite,next))
    });
    if let Some((_,other_target))=other{
        if !valid(other_target){
            return Err("Coupled tangent would exceed document bounds".into());
        }
    }
    move_node(shape,address,to)?;
    if let Some((opposite,target))=other{
        // The opposite address was read and validated before the first edit.
        move_node(shape,opposite,target)?;
    }
    Ok(())
}
/// Convert a native polyline to exactly the same geometric trace in cubic form.
/// Each line is represented by cubic controls at one and two thirds.
/// Closed paths retain their implicit closing edge; no smoothing is applied.
/// The operation is pure, allowing callers to checkpoint only on success.
pub fn polyline_to_bezier_exact(shape:&Shape)->Result<Shape,String>{
    let Shape::Polyline{points,closed}=shape else{
        return Err("Select an editable polyline".into());
    };
    if points.len()<2 || (*closed && points.len()<3){
        return Err("Polyline does not contain enough vertices".into());
    }
    if points.len()>10_001{
        return Err("Exact conversion is limited to 10,000 cubic segments".into());
    }
    if points.iter().any(|p|!valid(*p)){
        return Err("Polyline contains invalid coordinates".into());
    }
    let mut segments=Vec::with_capacity(points.len()-1);
    for edge in points.windows(2){
        let a=edge[0];
        let b=edge[1];
        let control1=Point{
            x:(a.x as f64+(b.x as f64-a.x as f64)/3.0) as f32,
            y:(a.y as f64+(b.y as f64-a.y as f64)/3.0) as f32,
        };
        let control2=Point{
            x:(a.x as f64+2.0*(b.x as f64-a.x as f64)/3.0) as f32,
            y:(a.y as f64+2.0*(b.y as f64-a.y as f64)/3.0) as f32,
        };
        if !valid(control1)||!valid(control2){
            return Err("Converted controls exceed document bounds".into());
        }
        segments.push(CubicSegment{control1,control2,end:b});
    }
    Ok(Shape::Bezier{start:points[0],segments,closed:*closed})
}

#[cfg(test)]
mod exact_conversion_tests {
    use super::*;
    fn pt(x:f32,y:f32)->Point{Point{x,y}}
    fn approx(a:f32,b:f32)->bool{(a-b).abs()<0.002}
    #[test]
    fn exact_polyline_conversion_keeps_line_segments_exact(){
        let input=Shape::Polyline{
            points:vec![pt(0.,0.),pt(12.,6.),pt(16.,-4.)],closed:false,
        };
        let shape=polyline_to_bezier_exact(&input).unwrap();
        let Shape::Bezier{start,segments,closed:false}=shape else{panic!("cubic")};
        assert_eq!(start,pt(0.,0.));
        assert_eq!(segments.len(),2);
        assert_eq!(segments[0].end,pt(12.,6.));
        assert_eq!(segments[1].end,pt(16.,-4.));
        for (a,s) in [(start,&segments[0]),(segments[0].end,&segments[1])]{
            for (t,p) in [(1./3.,s.control1),(2./3.,s.control2)]{
                assert!(approx(p.x,a.x+(s.end.x-a.x)*t));
                assert!(approx(p.y,a.y+(s.end.y-a.y)*t));
            }
        }
        assert_eq!(input,Shape::Polyline{
            points:vec![pt(0.,0.),pt(12.,6.),pt(16.,-4.)],closed:false,
        });
    }
    #[test]
    fn exact_polyline_conversion_preserves_closed_flag(){
        let shape=polyline_to_bezier_exact(&Shape::Polyline{
            points:vec![pt(0.,0.),pt(10.,0.),pt(10.,10.)],closed:true,
        }).unwrap();
        let Shape::Bezier{segments,closed,..}=shape else{panic!()};
        assert!(closed);
        assert_eq!(segments.len(),2);
    }
    #[test]
    fn exact_polyline_conversion_rejects_invalid_inputs(){
        for input in [
            Shape::Polyline{points:vec![pt(1.,1.)],closed:false},
            Shape::Polyline{points:vec![pt(1.,1.),pt(2.,2.)],closed:true},
            Shape::Polyline{points:vec![pt(1.,1.),pt(f32::NAN,2.)],closed:false},
            Shape::Polyline{points:vec![pt(1.,1.),pt(100_001.,2.)],closed:false},
            Shape::Rectangle{x:0.,y:0.,w:2.,h:2.},
        ]{
            assert!(polyline_to_bezier_exact(&input).is_err());
        }
    }
}

/// Reverse a path in-place while preserving every cubic's geometric trace.
/// Cubic controls swap order; for compound shapes every contour is reversed.
pub fn reverse_path(shape:&mut Shape)->Result<(),String>{
    fn reverse_cubic(start:&mut Point,segments:&mut Vec<CubicSegment>)->Result<(),String>{
        if segments.is_empty(){return Err("Curve has no segments".into());}
        let old_start=*start;
        let old=std::mem::take(segments);
        *start=old.last().expect("not empty").end;
        let mut reversed=Vec::with_capacity(old.len());
        for i in (0..old.len()).rev(){
            let s=&old[i];
            reversed.push(CubicSegment{
                control1:s.control2,
                control2:s.control1,
                end:if i==0{old_start}else{old[i-1].end},
            });
        }
        *segments=reversed;
        Ok(())
    }
    match shape{
        Shape::Bezier{start,segments,..}=>reverse_cubic(start,segments),
        Shape::CompoundPath{contours}=>{
            if contours.is_empty()||contours.iter().any(|c|c.segments.is_empty()){
                return Err("Compound path has no segments".into());
            }
            for contour in contours{
                reverse_cubic(&mut contour.start,&mut contour.segments)?;
            }
            Ok(())
        },
        Shape::Polyline{points,..} if points.len()>=2=>{
            points.reverse();
            Ok(())
        },
        _=>Err("Select a line, Bézier or compound vector path".into()),
    }
}
/// Preserve persistent mode bindings when reversing the path direction.
pub fn modes_after_reverse(shape:&Shape,modes:&mut [NodeModeBinding]){
    for entry in modes{
        match (shape,&mut entry.address){
            (Shape::Bezier{segments,..},NodeAddress::BezierEnd(i))=>
                *i=segments.len().saturating_sub(2).saturating_sub(*i),
            (Shape::CompoundPath{contours},NodeAddress::CompoundEnd(j,i))=>{
                if let Some(contour)=contours.get(*j){
                    *i=contour.segments.len().saturating_sub(2).saturating_sub(*i);
                }
            },
            _=>{},
        }
    }
}
/// Change explicit closure without adding/deleting any anchor or handle.
pub fn set_closed(shape:&mut Shape,closed:bool)->Result<bool,String>{
    match shape{
        Shape::Bezier{segments,closed:flag,..} if !segments.is_empty()=>{
            if *flag==closed{return Ok(false);}
            *flag=closed;Ok(true)
        },
        Shape::Polyline{points,closed:flag} if points.len()>=2=>{
            if *flag==closed{return Ok(false);}
            *flag=closed;Ok(true)
        },
        Shape::CompoundPath{contours} if !contours.is_empty()=>{
            if contours.iter().all(|c|c.closed==closed){return Ok(false);}
            for c in contours{c.closed=closed;}
            Ok(true)
        },
        _=>Err("This object cannot be opened or closed".into()),
    }
}
/// Only interior anchors have an incoming and an outgoing cubic control.
pub fn has_tangent_pair(shape:&Shape,address:NodeAddress)->bool{
    match (shape,address){
        (Shape::Bezier{segments,..},NodeAddress::BezierEnd(i))=>
            i.checked_add(1).is_some_and(|next|next<segments.len()),
        (Shape::CompoundPath{contours},NodeAddress::CompoundEnd(j,i))=>
            contours.get(j).is_some_and(|c|
                i.checked_add(1).is_some_and(|next|next<c.segments.len())),
        _=>false,
    }
}
/// Change adjoining cubic handles atomically. Invalid geometry and terminal
/// anchors are rejected without changing any part of the input shape.
pub fn set_tangent(shape:&mut Shape,address:NodeAddress,operation:TangentOperation)
    ->Result<bool,String>{
    if !has_tangent_pair(shape,address){
        return Err("Select an interior Bézier anchor with two adjacent handles".into());
    }
    let (anchor,incoming,outgoing,previous,next)=match (&*shape,address){
        (Shape::Bezier{start,segments,..},NodeAddress::BezierEnd(i))=>
            (segments[i].end,segments[i].control2,segments[i+1].control1,
             if i==0{*start}else{segments[i-1].end},segments[i+1].end),
        (Shape::CompoundPath{contours},NodeAddress::CompoundEnd(j,i))=>{
            let contour=&contours[j];
            let segments=&contour.segments;
            (segments[i].end,segments[i].control2,segments[i+1].control1,
             if i==0{contour.start}else{segments[i-1].end},segments[i+1].end)
        },
        _=>unreachable!(),
    };
    if ![anchor,incoming,outgoing,previous,next].into_iter().all(valid){
        return Err("Cannot adjust handles with invalid coordinates".into());
    }
    let (before,after)=if operation==TangentOperation::Corner{
        (anchor,anchor)
    }else{
        // Relative vectors in f64 avoid overflow when endpoints are at
        // opposite ends of the permitted coordinate range.
        let vin=(anchor.x as f64-incoming.x as f64,
                  anchor.y as f64-incoming.y as f64);
        let vout=(outgoing.x as f64-anchor.x as f64,
                   outgoing.y as f64-anchor.y as f64);
        let li=vin.0.hypot(vin.1);
        let lo=vout.0.hypot(vout.1);
        let mut direction=(vin.0+vout.0,vin.1+vout.1);
        if direction.0.hypot(direction.1)<1e-9{
            direction=if lo>=li{vout}else{vin};
        }
        if direction.0.hypot(direction.1)<1e-9{
            // Both handles were collapsed by Corner. Reconstruct a tangent
            // along the neighboring anchors instead of trapping this node
            // in an irreversible sharp-corner state.
            direction=(next.x as f64-previous.x as f64,
                       next.y as f64-previous.y as f64);
        }
        let length=direction.0.hypot(direction.1);
        if length<1e-9{
            return Err("Neighboring anchors cannot define a tangent".into());
        }
        let unit=(direction.0/length,direction.1/length);
        let incoming_span=(anchor.x as f64-previous.x as f64)
            .hypot(anchor.y as f64-previous.y as f64)/3.0;
        let outgoing_span=(next.x as f64-anchor.x as f64)
            .hypot(next.y as f64-anchor.y as f64)/3.0;
        let li=if li<1e-9{incoming_span}else{li};
        let lo=if lo<1e-9{outgoing_span}else{lo};
        if li<1e-9||lo<1e-9{
            return Err("Adjacent anchors do not provide usable handles".into());
        }
        let (len_in,len_out)=match operation{
            TangentOperation::Smooth=>(li,lo),
            TangentOperation::Symmetric=>((li+lo)*0.5,(li+lo)*0.5),
            TangentOperation::Corner=>unreachable!(),
        };
        let new_before=Point{
            x:(anchor.x as f64-unit.0*len_in) as f32,
            y:(anchor.y as f64-unit.1*len_in) as f32,
        };
        let new_after=Point{
            x:(anchor.x as f64+unit.0*len_out) as f32,
            y:(anchor.y as f64+unit.1*len_out) as f32,
        };
        (new_before,new_after)
    };
    if !valid(before)||!valid(after){
        return Err("Adjusted handles would exceed the document bounds".into());
    }
    let unchanged=[(before.x,incoming.x),(before.y,incoming.y),
        (after.x,outgoing.x),(after.y,outgoing.y)]
        .into_iter().all(|(new,old)|(new-old).abs()<=1e-5);
    if unchanged{return Ok(false);}
    match (shape,address){
        (Shape::Bezier{segments,..},NodeAddress::BezierEnd(i))=>{
            segments[i].control2=before;
            segments[i+1].control1=after;
        },
        (Shape::CompoundPath{contours},NodeAddress::CompoundEnd(j,i))=>{
            contours[j].segments[i].control2=before;
            contours[j].segments[i+1].control1=after;
        },
        _=>unreachable!(),
    }
    Ok(true)
}
fn shift(p:&mut Point,d:Point){p.x+=d.x;p.y+=d.y;}
fn valid(p:Point)->bool{
    p.x.is_finite()&&p.y.is_finite()&&p.x.abs()<=100_000.&&p.y.abs()<=100_000.
}
fn lerp(a:Point,b:Point,t:f32)->Point{
    Point{x:a.x+(b.x-a.x)*t,y:a.y+(b.y-a.y)*t}
}
fn cubic(a:Point,s:&CubicSegment,t:f32)->Point{
    let ab=lerp(a,s.control1,t);
    let bc=lerp(s.control1,s.control2,t);
    let cd=lerp(s.control2,s.end,t);
    lerp(lerp(ab,bc,t),lerp(bc,cd,t),t)
}
fn split_cubic(start:Point,seg:&mut CubicSegment,t:f32)->CubicSegment{
    let a=lerp(start,seg.control1,t);
    let b=lerp(seg.control1,seg.control2,t);
    let c=lerp(seg.control2,seg.end,t);
    let d=lerp(a,b,t);let e=lerp(b,c,t);let midpoint=lerp(d,e,t);
    let latter=CubicSegment{control1:e,control2:c,end:seg.end};
    *seg=CubicSegment{control1:a,control2:d,end:midpoint};
    latter
}
/// Split a segment using de Casteljau (exact curve preservation for cubics).
pub fn insert_node(shape:&mut Shape,address:SegmentAddress,t:f32)->Result<NodeAddress,String>{
    if !t.is_finite() || !(0.02..=0.98).contains(&t){
        return Err("Insert parameter must be inside the segment".into());
    }
    match (shape,address){
        (Shape::Polyline{points,closed},SegmentAddress::Polyline(i))=>{
            if i>=points.len() || (!*closed && i+1>=points.len()){
                return Err("Invalid polyline segment index".into());
            }
            if points.len()>=60_000{return Err("Too many polyline nodes".into());}
            let next=(i+1)%points.len();
            let mid=lerp(points[i],points[next],t);
            points.insert(i+1,mid);
            Ok(NodeAddress::Polyline(i+1))
        }
        (Shape::Bezier{start,segments,..},SegmentAddress::Bezier(i))=>{
            if i>=segments.len(){return Err("Invalid Bézier segment".into());}
            if segments.len()>=60_000{return Err("Too many Bézier segments".into());}
            let p=if i==0{*start}else{segments[i-1].end};
            let second=split_cubic(p,&mut segments[i],t);
            segments.insert(i+1,second);
            Ok(NodeAddress::BezierEnd(i))
        }
        (Shape::CompoundPath{contours},SegmentAddress::Compound(j,i))=>{
            let Some(c)=contours.get_mut(j) else{return Err("Invalid contour".into());};
            if i>=c.segments.len(){return Err("Invalid contour segment".into());}
            if c.segments.len()>=60_000{return Err("Contour is too complex".into());}
            let p=if i==0{c.start}else{c.segments[i-1].end};
            let second=split_cubic(p,&mut c.segments[i],t);
            c.segments.insert(i+1,second);
            Ok(NodeAddress::CompoundEnd(j,i))
        }
        _=>Err("Shape has no such editable segment".into()),
    }
}
pub fn remove_node(shape:&mut Shape,address:NodeAddress)->Result<(),String>{
    match (shape,address){
        (Shape::Polyline{points,closed},NodeAddress::Polyline(i))=>{
            let min=if *closed{3}else{2};
            if i>=points.len()||points.len()<=min{
                return Err("Cannot remove node: path would become invalid".into());
            }
            points.remove(i);Ok(())
        }
        (Shape::Bezier{start,segments,..},NodeAddress::BezierStart)=>{
            if segments.len()<2{return Err("Bézier requires at least one segment".into());}
            *start=segments.remove(0).end;
            Ok(())
        }
        (Shape::Bezier{segments,..},NodeAddress::BezierEnd(i))=>
            remove_cubic_anchor(segments,i),
        (Shape::CompoundPath{contours},NodeAddress::CompoundStart(j))=>{
            let Some(c)=contours.get_mut(j) else{return Err("No such contour".into());};
            if c.segments.len()<2{return Err("Contour requires at least one segment".into());}
            c.start=c.segments.remove(0).end;
            Ok(())
        }
        (Shape::CompoundPath{contours},NodeAddress::CompoundEnd(j,i))=>{
            let Some(c)=contours.get_mut(j) else{return Err("No such contour".into());};
            remove_cubic_anchor(&mut c.segments,i)
        }
        _=>Err("Only anchor nodes can be deleted; control handles cannot".into()),
    }
}
fn remove_cubic_anchor(segments:&mut Vec<CubicSegment>,i:usize)->Result<(),String>{
    if i>=segments.len() || segments.len()<2{
        return Err("Cannot delete the last segment".into());
    }
    if i+1==segments.len(){
        segments.remove(i);
    }else{
        let next=segments.remove(i+1);
        segments[i].end=next.end;
        segments[i].control2=next.control2;
    }
    Ok(())
}
fn examine_cubic(address:SegmentAddress,start:Point,segment:&CubicSegment,
    target:Point,best:&mut f32,hit:&mut Option<(SegmentAddress,f32)>){
    for sample in 1..=24{
        let t=sample as f32/25.0;
        let q=cubic(start,segment,t);
        let d=(target.x-q.x).powi(2)+(target.y-q.y).powi(2);
        if d<*best{*best=d;*hit=Some((address,t));}
    }
}
/// Return nearest path segment and parameter for double-click insertion.
/// Segment search is deliberately capped for heavy compound glyph outlines.
pub fn nearest_segment(shape:&Shape,p:Point,tolerance:f32)
    ->Option<(SegmentAddress,f32)>{
    let mut result=None;
    let mut best=tolerance*tolerance;
    let mut tested=0usize;
    match shape{
        Shape::Polyline{points,closed}=>{
            if points.len()<2{return None;}
            let edges=if *closed{points.len()}else{points.len()-1};
            for i in 0..edges.min(4000){
                let a=points[i];let b=points[(i+1)%points.len()];
                let d=Point{x:b.x-a.x,y:b.y-a.y};
                let len=d.x*d.x+d.y*d.y;
                if len<=1e-12{continue;}
                let t=((p.x-a.x)*d.x+(p.y-a.y)*d.y)/len;
                let t=t.clamp(0.02,0.98);
                let q=lerp(a,b,t);
                let sq=(p.x-q.x).powi(2)+(p.y-q.y).powi(2);
                if sq<best{best=sq;result=Some((SegmentAddress::Polyline(i),t));}
            }
        }
        Shape::Bezier{start,segments,..}=>{
            let mut prev=*start;
            for (i,s) in segments.iter().enumerate(){
                examine_cubic(SegmentAddress::Bezier(i),prev,s,p,&mut best,&mut result);
                tested+=1;
                prev=s.end;
                if tested>=4000{break;}
            }
        }
        Shape::CompoundPath{contours}=>{
            'outer:for (j,c) in contours.iter().enumerate(){
                let mut prev=c.start;
                for (i,s) in c.segments.iter().enumerate(){
                    examine_cubic(SegmentAddress::Compound(j,i),prev,s,p,&mut best,&mut result);
                    tested+=1;
                    prev=s.end;
                    if tested>=4000{break 'outer;}
                }
            }
        }
        _=>{}
    }
    result
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Contour,Document,Style};
    fn path()->Shape{
        Shape::Bezier{start:Point{x:0.,y:0.},segments:vec![
            CubicSegment{control1:Point{x:0.,y:10.},control2:Point{x:10.,y:10.},
                end:Point{x:10.,y:0.}}
        ],closed:false}
    }
    fn two_cubics()->Shape{
        Shape::Bezier{start:Point{x:0.0,y:0.0},segments:vec![
            CubicSegment{
                control1:Point{x:1.0,y:1.0},
                control2:Point{x:7.0,y:5.0},
                end:Point{x:10.0,y:10.0},
            },
            CubicSegment{
                control1:Point{x:16.0,y:11.0},
                control2:Point{x:19.0,y:19.0},
                end:Point{x:20.0,y:20.0},
            },
        ],closed:false}
    }
    fn tangent_vectors(shape:&Shape)->((f64,f64),(f64,f64)){
        let Shape::Bezier{segments,..}=shape else{panic!("not Bezier")};
        let a=segments[0].end;
        let prev=segments[0].control2;
        let next=segments[1].control1;
        ((a.x as f64-prev.x as f64,a.y as f64-prev.y as f64),
         (next.x as f64-a.x as f64,next.y as f64-a.y as f64))
    }
    #[test]
    fn smooth_handles_are_collinear_and_keep_individual_lengths(){
        let mut shape=two_cubics();
        let (before,after)=tangent_vectors(&shape);
        let first=before.0.hypot(before.1);
        let second=after.0.hypot(after.1);
        assert!(set_tangent(&mut shape,NodeAddress::BezierEnd(0),
            TangentOperation::Smooth).unwrap());
        let (before,after)=tangent_vectors(&shape);
        assert!((before.0*after.1-before.1*after.0).abs()<0.0001);
        assert!((before.0.hypot(before.1)-first).abs()<0.0001);
        assert!((after.0.hypot(after.1)-second).abs()<0.0001);
        assert!(!set_tangent(&mut shape,NodeAddress::BezierEnd(0),
            TangentOperation::Smooth).unwrap());
    }
    #[test]
    fn symmetric_handles_have_equal_lengths_and_collinear_tangents(){
        let mut shape=two_cubics();
        assert!(set_tangent(&mut shape,NodeAddress::BezierEnd(0),
            TangentOperation::Symmetric).unwrap());
        let (before,after)=tangent_vectors(&shape);
        assert!((before.0*after.1-before.1*after.0).abs()<0.0001);
        assert!((before.0.hypot(before.1)-after.0.hypot(after.1)).abs()<0.0001);
        let mut doc=Document::default();
        doc.add(shape,Style::default());
        doc.validate().unwrap();
        let restored:Document=serde_json::from_str(
            &serde_json::to_string(&doc).unwrap()).unwrap();
        assert_eq!(doc,restored);
    }
    #[test]
    fn corner_collapses_both_handles_and_allows_independent_editing(){
        let mut shape=two_cubics();
        assert!(set_tangent(&mut shape,NodeAddress::BezierEnd(0),
            TangentOperation::Corner).unwrap());
        let Shape::Bezier{segments,..}=&shape else{panic!()};
        assert_eq!(segments[0].control2,segments[0].end);
        assert_eq!(segments[1].control1,segments[0].end);
        assert!(!set_tangent(&mut shape,NodeAddress::BezierEnd(0),
            TangentOperation::Corner).unwrap());
        move_node(&mut shape,NodeAddress::BezierControl1(1),
            Point{x:15.0,y:5.0}).unwrap();
        let Shape::Bezier{segments,..}=&shape else{panic!()};
        assert_eq!(segments[0].control2,segments[0].end);
        assert_eq!(segments[1].control1,Point{x:15.0,y:5.0});
    }
    #[test]
    fn rejects_terminal_nodes_and_invalid_inputs_without_mutation(){
        let mut shape=two_cubics();
        for node in [NodeAddress::BezierStart,NodeAddress::BezierEnd(1),
            NodeAddress::BezierEnd(100),NodeAddress::BezierEnd(usize::MAX),
            NodeAddress::BezierControl1(1)] {
            let old=shape.clone();
            assert!(set_tangent(&mut shape,node,TangentOperation::Smooth).is_err());
            assert_eq!(shape,old);
        }
        let Shape::Bezier{segments,..}=&mut shape else{panic!()};
        segments[0].control2.x=f32::NAN;
        let previous=shape.clone();
        assert!(set_tangent(&mut shape,NodeAddress::BezierEnd(0),
            TangentOperation::Symmetric).is_err());
        assert_eq!(format!("{:?}",shape),format!("{:?}",previous));
    }
    #[test]
    fn collapsed_corner_can_become_smooth_and_symmetric_again(){
        let mut shape=two_cubics();
        // Use unequal adjacent spans: the Smooth command must regenerate
        // nonzero handles, and Symmetric must then equalize their lengths.
        if let Shape::Bezier{segments,..}=&mut shape{
            segments[1].end=Point{x:35.0,y:20.0};
        }
        set_tangent(&mut shape,NodeAddress::BezierEnd(0),
            TangentOperation::Corner).unwrap();
        assert!(set_tangent(&mut shape,NodeAddress::BezierEnd(0),
            TangentOperation::Smooth).unwrap());
        let (before,after)=tangent_vectors(&shape);
        assert!(before.0.hypot(before.1)>0.001);
        assert!(after.0.hypot(after.1)>0.001);
        assert!((before.0*after.1-before.1*after.0).abs()<0.001);
        assert!(set_tangent(&mut shape,NodeAddress::BezierEnd(0),
            TangentOperation::Symmetric).unwrap());
        let (before,after)=tangent_vectors(&shape);
        assert!((before.0.hypot(before.1)-after.0.hypot(after.1)).abs()<1e-4);
    }
    #[test]
    fn fully_degenerate_neighboring_anchors_remain_atomic(){
        let origin=Point{x:10.0,y:10.0};
        let mut shape=Shape::Bezier{start:origin,segments:vec![
            CubicSegment{control1:origin,control2:origin,end:origin},
            CubicSegment{control1:origin,control2:origin,end:origin},
        ],closed:false};
        let old=shape.clone();
        assert!(set_tangent(&mut shape,NodeAddress::BezierEnd(0),
            TangentOperation::Smooth).is_err());
        assert_eq!(shape,old);
    }
    #[test]
    fn compound_path_supports_interior_tangent_commands(){
        let Shape::Bezier{start,segments,..}=two_cubics() else{panic!()};
        let mut shape=Shape::CompoundPath{contours:vec![Contour{
            start,segments,closed:false,
        }]};
        assert!(has_tangent_pair(&shape,NodeAddress::CompoundEnd(0,0)));
        assert!(!has_tangent_pair(&shape,NodeAddress::CompoundEnd(0,1)));
        assert!(set_tangent(&mut shape,NodeAddress::CompoundEnd(0,0),
            TangentOperation::Symmetric).unwrap());
        let Shape::CompoundPath{contours}=&shape else{panic!()};
        let s=&contours[0].segments;
        let anchor=s[0].end;
        let in_len=(anchor.x-s[0].control2.x).hypot(anchor.y-s[0].control2.y);
        let out_len=(anchor.x-s[1].control1.x).hypot(anchor.y-s[1].control1.y);
        assert!((in_len-out_len).abs()<1e-4);
    }
    #[test] fn insert_bezier_preserves_entire_curve(){
        let mut shape=path();
        let before=shape.clone();
        let selected=insert_node(&mut shape,SegmentAddress::Bezier(0),0.4).unwrap();
        assert_eq!(selected,NodeAddress::BezierEnd(0));
        let Shape::Bezier{start,segments,..}=&shape else{panic!("not Bezier");};
        assert_eq!(segments.len(),2);
        let Shape::Bezier{segments:old,..}=before else{panic!("not Bezier");};
        for n in 0..100{
            let t=n as f32/99.0;
            let expected=cubic(*start,&old[0],t);
            let observed=if t<=0.4{cubic(*start,&segments[0],t/0.4)}
                else{cubic(segments[0].end,&segments[1],(t-0.4)/0.6)};
            assert!((expected.x-observed.x).abs()<1e-3);
            assert!((expected.y-observed.y).abs()<1e-3);
        }
    }
    #[test] fn moving_anchor_translates_its_adjacent_handle(){
        let mut shape=path();
        move_node(&mut shape,NodeAddress::BezierStart,Point{x:5.,y:6.}).unwrap();
        let Shape::Bezier{start,segments,..}=shape else{panic!("not Bezier");};
        assert_eq!(start,Point{x:5.,y:6.});
        assert_eq!(segments[0].control1,Point{x:5.,y:16.});
    }
    #[test] fn polyline_insertion_and_minimum_size_guard(){
        let mut poly=Shape::Polyline{points:vec![
            Point{x:0.,y:0.},Point{x:10.,y:0.},Point{x:10.,y:10.}],closed:true};
        let node=insert_node(&mut poly,SegmentAddress::Polyline(2),0.5).unwrap();
        assert_eq!(node,NodeAddress::Polyline(3));
        assert!(remove_node(&mut poly,node).is_ok());
        assert!(remove_node(&mut poly,NodeAddress::Polyline(0)).is_err());
    }
    #[test] fn editing_compound_keeps_holes_and_roundtrips(){
        let original=path();
        let Shape::Bezier{start,segments,..}=original else{panic!();};
        let mut shape=Shape::CompoundPath{contours:vec![Contour{
            start,segments,closed:true
        }]};
        insert_node(&mut shape,SegmentAddress::Compound(0,0),0.5).unwrap();
        move_node(&mut shape,NodeAddress::CompoundEnd(0,0),
            Point{x:5.,y:7.}).unwrap();
        let mut doc=Document::default();
        doc.add(shape,Style::default());
        doc.validate().unwrap();
        let copied:Document=serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
        assert_eq!(doc,copied);
    }
    #[test] fn anchor_move_is_atomic_if_attached_handle_overflows(){
        let mut shape=Shape::Bezier{start:Point{x:0.,y:0.},
            segments:vec![CubicSegment{
                control1:Point{x:99_999.,y:0.},
                control2:Point{x:10.,y:0.},
                end:Point{x:20.,y:0.}}],closed:false};
        let before=shape.clone();
        assert!(move_node(&mut shape,NodeAddress::BezierStart,
            Point{x:100_000.,y:0.}).is_err());
        assert_eq!(shape,before);
    }
    #[test] fn reject_bad_addresses_and_nonfinite_coordinates(){
        let mut shape=path();
        let previous=shape.clone();
        assert!(move_node(&mut shape,NodeAddress::BezierEnd(10),Point{x:2.,y:2.}).is_err());
        assert!(move_node(&mut shape,NodeAddress::BezierStart,
            Point{x:f32::NAN,y:0.}).is_err());
        assert_eq!(shape,previous);
        assert!(insert_node(&mut shape,SegmentAddress::Bezier(100),0.5).is_err());
    }
    #[test] fn nearest_polyline_segment_respects_tolerance(){
        let shape=Shape::Polyline{points:vec![
            Point{x:0.,y:0.},Point{x:20.,y:0.}],closed:false};
        assert!(matches!(nearest_segment(&shape,Point{x:10.,y:0.2},1.),
            Some((SegmentAddress::Polyline(0),_))));
        assert!(nearest_segment(&shape,Point{x:10.,y:10.},1.).is_none());
    }
}