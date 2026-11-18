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
fn cubic_distance_sq(start:Point,segment:&CubicSegment,target:Point,t:f32)->f32{
    let q=cubic(start,segment,t);
    (target.x-q.x).powi(2)+(target.y-q.y).powi(2)
}
/// Find the nearest position on a cubic, not just the nearest sampled vertex.
/// Bracket *each* sampled local minimum: a cubic can loop and have several
/// locally nearest points. Golden-section refinement is bounded and stable.
fn examine_cubic(address:SegmentAddress,start:Point,segment:&CubicSegment,
    target:Point,best:&mut f32,hit:&mut Option<(SegmentAddress,f32)>){
    const SAMPLES:usize=32;
    const REFINE:usize=18;
    let mut distances=[0.0f32;SAMPLES+1];
    for (i,d) in distances.iter_mut().enumerate(){
        *d=cubic_distance_sq(start,segment,target,i as f32/SAMPLES as f32);
    }
    for i in 1..SAMPLES{
        if distances[i]>distances[i-1] || distances[i]>distances[i+1]{
            continue;
        }
        let mut a=(i-1) as f32/SAMPLES as f32;
        let mut b=(i+1) as f32/SAMPLES as f32;
        let ratio=0.618_034_f32;
        let mut left=b-(b-a)*ratio;
        let mut right=a+(b-a)*ratio;
        let mut dl=cubic_distance_sq(start,segment,target,left);
        let mut dr=cubic_distance_sq(start,segment,target,right);
        for _ in 0..REFINE{
            if dl<=dr{
                b=right;
                right=left;dr=dl;
                left=b-(b-a)*ratio;
                dl=cubic_distance_sq(start,segment,target,left);
            }else{
                a=left;
                left=right;dl=dr;
                right=a+(b-a)*ratio;
                dr=cubic_distance_sq(start,segment,target,right);
            }
        }
        let t=((a+b)*0.5).clamp(0.02,0.98);
        let distance=cubic_distance_sq(start,segment,target,t);
        if distance.is_finite() && distance<*best{
            *best=distance;
            *hit=Some((address,t));
        }
    }
    // The closest point can be near an endpoint with no interior sampled
    // minimum. Insert at the closest legal parameter in that case.
    for t in [0.02_f32,0.98_f32]{
        let d=cubic_distance_sq(start,segment,target,t);
        if d.is_finite() && d<*best{*best=d;*hit=Some((address,t));}
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

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum AnchorMode{Cusp,Smooth,Symmetric}

/// Set tangent continuity at an interior anchor. The incoming control is
/// retained as the direction reference when possible; endpoint anchors are
/// not eligible because they have only one adjacent segment.
pub fn set_anchor_mode(shape:&mut Shape,address:NodeAddress,mode:AnchorMode)
    ->Result<(),String>{
    let (segments,index)=match (shape,address){
        (Shape::Bezier{segments,..},NodeAddress::BezierEnd(i))=> (segments.as_mut_slice(),i),
        (Shape::CompoundPath{contours},NodeAddress::CompoundEnd(j,i))=>
            (contours.get_mut(j).ok_or("Invalid contour")?.segments.as_mut(),i),
        _=>return Err("Choose an interior Bézier anchor".into()),
    };
    if index+1>=segments.len(){return Err("Anchor has no following segment".into());}
    let anchor=segments[index].end;
    let incoming=segments[index].control2;
    let outgoing=segments[index+1].control1;
    if mode==AnchorMode::Cusp{
        segments[index].control2=anchor;
        segments[index+1].control1=anchor;
        return Ok(());
    }
    let in_dx=anchor.x-incoming.x;
    let in_dy=anchor.y-incoming.y;
    let out_dx=outgoing.x-anchor.x;
    let out_dy=outgoing.y-anchor.y;
    let in_len=in_dx.hypot(in_dy);
    let out_len=out_dx.hypot(out_dy);
    let (dx,dy)=if in_len>1e-6{(in_dx/in_len,in_dy/in_len)}
        else if out_len>1e-6{(out_dx/out_len,out_dy/out_len)}
        else{return Err("Cannot infer tangent from two collapsed handles".into());};
    let length=if mode==AnchorMode::Symmetric{(in_len+out_len)*0.5}else{out_len};
    let in_length=if mode==AnchorMode::Symmetric{length}else{in_len};
    let a=Point{x:anchor.x-dx*in_length,y:anchor.y-dy*in_length};
    let b=Point{x:anchor.x+dx*length,y:anchor.y+dy*length};
    if !valid(a)||!valid(b){return Err("Resulting control handle exceeds safe bounds".into());}
    segments[index].control2=a;
    segments[index+1].control1=b;
    Ok(())
}

/// Replace one cubic with a mathematically straight cubic, preserving its
/// endpoints and every other segment in the path.
pub fn straighten_segment(shape:&mut Shape,address:SegmentAddress)->Result<(),String>{
    let (start,segment)=match (shape,address){
        (Shape::Bezier{start,segments,..},SegmentAddress::Bezier(i))=>{
            if i>=segments.len(){return Err("Invalid Bézier segment".into());}
            (if i==0{*start}else{segments[i-1].end},&mut segments[i])
        },
        (Shape::CompoundPath{contours},SegmentAddress::Compound(j,i))=>{
            let c=contours.get_mut(j).ok_or("Invalid contour")?;
            if i>=c.segments.len(){return Err("Invalid contour segment".into());}
            (if i==0{c.start}else{c.segments[i-1].end},&mut c.segments[i])
        },
        _=>return Err("Select a cubic segment".into()),
    };
    let end=segment.end;
    segment.control1=lerp(start,end,1.0/3.0);
    segment.control2=lerp(start,end,2.0/3.0);
    Ok(())
}

/// Reverse segment order and exchange control handles without changing the
/// geometric locus of the path, including compound path contours.
pub fn reverse_path(shape:&mut Shape)->Result<(),String>{
    fn reverse(start:&mut Point,segments:&mut [CubicSegment]){
        if segments.is_empty(){return;}
        let original_start=*start;
        let mut previous=original_start;
        let mut reversed=Vec::with_capacity(segments.len());
        for segment in segments.iter(){
            reversed.push(CubicSegment{
                control1:segment.control2,
                control2:segment.control1,
                end:previous,
            });
            previous=segment.end;
        }
        *start=previous;
        for (dst,src) in segments.iter_mut().zip(reversed.into_iter().rev()){
            *dst=src;
        }
    }
    match shape{
        Shape::Bezier{start,segments,..}=>{
            reverse(start,segments);
            Ok(())
        }
        Shape::CompoundPath{contours}=>{
            for contour in contours{reverse(&mut contour.start,&mut contour.segments);}
            Ok(())
        }
        Shape::Polyline{points,..}=>{points.reverse();Ok(())}
        _=>Err("Selected object has no reversible path".into()),
    }
}
/// Open or close a contour without silently inserting another curve.
/// Compound-path contours are switched together; their paths remain editable.
pub fn set_closed(shape:&mut Shape,closed_value:bool)->Result<(),String>{
    match shape{
        Shape::Polyline{points,closed}=>{
            if closed_value && points.len()<3{return Err("Closing requires three points".into());}
            *closed=closed_value;Ok(())
        }
        Shape::Bezier{segments,closed,..}=>{
            if closed_value && segments.is_empty(){return Err("Cannot close empty path".into());}
            *closed=closed_value;Ok(())
        }
        Shape::CompoundPath{contours}=>{
            if closed_value && contours.iter().any(|c|c.segments.is_empty()){
                return Err("Cannot close empty contour".into());
            }
            for contour in contours{contour.closed=closed_value;}
            Ok(())
        }
        _=>Err("Selected object has no closable contour".into())
    }
}
/// Upgrade a polyline to editable cubic segments. Straight geometry is
/// retained exactly: control handles lie at one and two thirds of each edge.
pub fn polyline_to_bezier(shape:&mut Shape)->Result<(),String>{
    let Shape::Polyline{points,closed}=&*shape else{
        return Err("Select a polyline to convert".into());
    };
    if points.len()<2{return Err("Polyline has too few points".into());}
    let start=points[0];
    let mut segments=Vec::with_capacity(points.len()-1);
    for pair in points.windows(2){
        segments.push(CubicSegment{
            control1:lerp(pair[0],pair[1],1.0/3.0),
            control2:lerp(pair[0],pair[1],2.0/3.0),
            end:pair[1],
        });
    }
    *shape=Shape::Bezier{start,segments,closed:*closed};
    Ok(())
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
    #[test] fn nearest_bezier_parameter_is_subsample_accurate(){
        let shape=path();
        let Shape::Bezier{start,segments,..}=&shape else{unreachable!()};
        for t in [0.071_f32,0.173,0.413,0.627,0.891]{
            let target=cubic(*start,&segments[0],t);
            let (address,found)=nearest_segment(&shape,target,0.01).unwrap();
            assert_eq!(address,SegmentAddress::Bezier(0));
            let nearest=cubic(*start,&segments[0],found);
            assert!((nearest.x-target.x).hypot(nearest.y-target.y)<0.01,
                "missed curve position at t={t}: found t={found}");
        }
    }
    #[test] fn near_endpoint_bezier_insertion_uses_valid_parameter(){
        let shape=path();
        let (address,t)=nearest_segment(&shape,Point{x:0.,y:0.},1.).unwrap();
        assert_eq!(address,SegmentAddress::Bezier(0));
        assert!((0.02..=0.98).contains(&t));
        let mut updated=shape;
        insert_node(&mut updated,address,t).unwrap();
    }
    #[test] fn anchor_modes_and_straightening(){
        let mut shape=Shape::Bezier{start:Point{x:0.,y:0.},segments:vec![
            CubicSegment{control1:Point{x:2.,y:0.},control2:Point{x:8.,y:4.},end:Point{x:10.,y:0.}},
            CubicSegment{control1:Point{x:12.,y:3.},control2:Point{x:18.,y:0.},end:Point{x:20.,y:0.}}
        ],closed:false};
        set_anchor_mode(&mut shape,NodeAddress::BezierEnd(0),AnchorMode::Smooth).unwrap();
        let Shape::Bezier{segments,..}=&shape else{panic!()};
        let a=segments[0].control2;let b=segments[1].control1;
        let cross=(10.-a.x)*b.y-(0.-a.y)*(b.x-10.);
        assert!(cross.abs()<1e-4);
        set_anchor_mode(&mut shape,NodeAddress::BezierEnd(0),AnchorMode::Symmetric).unwrap();
        let Shape::Bezier{segments,..}=&shape else{panic!()};
        assert!(((segments[0].control2.x-10.).hypot(segments[0].control2.y)-
            (segments[1].control1.x-10.).hypot(segments[1].control1.y)).abs()<1e-4);
        set_anchor_mode(&mut shape,NodeAddress::BezierEnd(0),AnchorMode::Cusp).unwrap();
        let Shape::Bezier{segments,..}=&shape else{panic!()};
        assert_eq!(segments[0].control2,Point{x:10.,y:0.});
        assert_eq!(segments[1].control1,Point{x:10.,y:0.});
        straighten_segment(&mut shape,SegmentAddress::Bezier(0)).unwrap();
        let Shape::Bezier{segments,..}=&shape else{panic!()};
        assert!((segments[0].control1.y).abs()<1e-5);
        assert!((segments[0].control2.y).abs()<1e-5);
    }
    #[test] fn reverse_path_twice_restores_exact_geometry(){
        let mut original=path();
        let before=original.clone();
        reverse_path(&mut original).unwrap();
        reverse_path(&mut original).unwrap();
        assert_eq!(original,before);
        let mut doc=Document::default();
        doc.add(original,Style::default());
        doc.validate().unwrap();
    }
    #[test] fn anchor_operations_reject_invalid_indices_without_mutation(){
        let mut shape=path();
        let previous=shape.clone();
        assert!(set_anchor_mode(&mut shape,NodeAddress::BezierEnd(0),AnchorMode::Smooth).is_err());
        assert!(straighten_segment(&mut shape,SegmentAddress::Bezier(9)).is_err());
        assert_eq!(shape,previous);
    }
    #[test] fn polyline_to_bezier_preserves_vertices_and_closure(){
        let vertices=vec![Point{x:1.,y:2.},Point{x:6.,y:2.},Point{x:6.,y:9.}];
        let mut shape=Shape::Polyline{points:vertices.clone(),closed:true};
        polyline_to_bezier(&mut shape).unwrap();
        let Shape::Bezier{start,segments,closed}=&shape else{panic!()};
        assert!(*closed);
        assert_eq!(*start,vertices[0]);
        assert_eq!(segments[0].end,vertices[1]);
        assert_eq!(segments[1].end,vertices[2]);
        set_closed(&mut shape,false).unwrap();
        let Shape::Bezier{closed,..}=&shape else{panic!()};
        assert!(!closed);
        set_closed(&mut shape,true).unwrap();
        let mut doc=Document::default();
        doc.add(shape,Style::default());
        doc.validate().unwrap();
    }
    #[test] fn nearest_polyline_segment_respects_tolerance(){
        let shape=Shape::Polyline{points:vec![
            Point{x:0.,y:0.},Point{x:20.,y:0.}],closed:false};
        assert!(matches!(nearest_segment(&shape,Point{x:10.,y:0.2},1.),
            Some((SegmentAddress::Polyline(0),_))));
        assert!(nearest_segment(&shape,Point{x:10.,y:10.},1.).is_none());
    }
}