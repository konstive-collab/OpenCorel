//! Structured, deterministic editing of polyline, cubic Bézier and compound path nodes.
//! Coordinates in millimetres. All operations reject invalid indices and nonfinite coordinates.
use crate::{CubicSegment,Point,Shape};

#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
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
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum TangentOperation{
    /// Set both handle points to their anchor (visible corner / cusp).
    Corner,
    /// Align handle directions while retaining each existing length.
    Smooth,
    /// Align directions and equalize the two handle lengths.
    Symmetric,
}

/// Only interior anchors have an incoming and an outgoing cubic control.
pub fn has_tangent_pair(shape:&Shape,address:NodeAddress)->bool{
    match (shape,address){
        (Shape::Bezier{segments,..},NodeAddress::BezierEnd(i))=>i+1<segments.len(),
        (Shape::CompoundPath{contours},NodeAddress::CompoundEnd(j,i))=>
            contours.get(j).is_some_and(|c|i+1<c.segments.len()),
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
    let (anchor,incoming,outgoing)=match (&*shape,address){
        (Shape::Bezier{segments,..},NodeAddress::BezierEnd(i))=>
            (segments[i].end,segments[i].control2,segments[i+1].control1),
        (Shape::CompoundPath{contours},NodeAddress::CompoundEnd(j,i))=>{
            let segments=&contours[j].segments;
            (segments[i].end,segments[i].control2,segments[i+1].control1)
        },
        _=>unreachable!(),
    };
    if !valid(anchor)||!valid(incoming)||!valid(outgoing){
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
        let length=direction.0.hypot(direction.1);
        if length<1e-9{
            return Err("Both tangent handles have zero length".into());
        }
        let unit=(direction.0/length,direction.1/length);
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
    if before==incoming && after==outgoing{return Ok(false);}
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
            NodeAddress::BezierEnd(100),NodeAddress::BezierControl1(1)] {
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
    fn zero_length_tangents_are_rejected_for_smooth_and_symmetric(){
        let mut shape=two_cubics();
        set_tangent(&mut shape,NodeAddress::BezierEnd(0),
            TangentOperation::Corner).unwrap();
        let old=shape.clone();
        assert!(set_tangent(&mut shape,NodeAddress::BezierEnd(0),
            TangentOperation::Smooth).is_err());
        assert!(set_tangent(&mut shape,NodeAddress::BezierEnd(0),
            TangentOperation::Symmetric).is_err());
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