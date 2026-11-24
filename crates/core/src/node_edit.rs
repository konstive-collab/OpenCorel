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

    // Always expose anchors before control points. Previously every cubic
    // consumed three of the visible slots, so a long path stopped exposing
    // anchors after only limit / 3 segments.
    match shape{
        Shape::Polyline{points,..}=>{
            for (i,p) in points.iter().enumerate().take(limit){
                push_handle(&mut nodes,limit,NodeAddress::Polyline(i),*p);
            }
        }
        Shape::Bezier{start,segments,..}=>{
            push_handle(&mut nodes,limit,NodeAddress::BezierStart,*start);
            for (i,s) in segments.iter().enumerate(){
                if nodes.len()>=limit{break;}
                push_handle(&mut nodes,limit,NodeAddress::BezierEnd(i),s.end);
            }
            for (i,s) in segments.iter().enumerate(){
                if nodes.len()>=limit{break;}
                push_handle(&mut nodes,limit,NodeAddress::BezierControl1(i),s.control1);
                push_handle(&mut nodes,limit,NodeAddress::BezierControl2(i),s.control2);
            }
        }
        Shape::CompoundPath{contours}=>{
            'anchors:for (j,c) in contours.iter().enumerate(){
                if nodes.len()>=limit{break;}
                push_handle(&mut nodes,limit,NodeAddress::CompoundStart(j),c.start);
                for (i,s) in c.segments.iter().enumerate(){
                    if nodes.len()>=limit{break 'anchors;}
                    push_handle(&mut nodes,limit,NodeAddress::CompoundEnd(j,i),s.end);
                }
            }
            'controls:for (j,c) in contours.iter().enumerate(){
                for (i,s) in c.segments.iter().enumerate(){
                    if nodes.len()>=limit{break 'controls;}
                    push_handle(&mut nodes,limit,NodeAddress::CompoundControl1(j,i),s.control1);
                    push_handle(&mut nodes,limit,NodeAddress::CompoundControl2(j,i),s.control2);
                }
            }
        }
        _=>{}
    }
    nodes
}
/// Pick the nearest editable anchor/handle across the ENTIRE path, not
/// just the first MAX_VISIBLE_NODES used for rendering. Anchors win exact
/// ties over controls, making dense curves predictable to select.
pub fn nearest_handle(shape:&Shape,target:Point,tolerance_mm:f32)
    ->Option<(NodeAddress,Point)>{
    if !valid(target)||!tolerance_mm.is_finite()||tolerance_mm<=0.{
        return None;
    }
    let mut best=tolerance_mm*tolerance_mm;
    let mut found=None;
    let mut check=|address:NodeAddress,point:Point|{
        if !valid(point){return;}
        let dx=point.x-target.x;
        let dy=point.y-target.y;
        let dist=dx*dx+dy*dy;
        if dist<best{best=dist;found=Some((address,point));}
    };
    match shape{
        Shape::Polyline{points,..}=>{
            for (i,p) in points.iter().enumerate(){
                check(NodeAddress::Polyline(i),*p);
            }
        }
        Shape::Bezier{start,segments,..}=>{
            check(NodeAddress::BezierStart,*start);
            for (i,segment) in segments.iter().enumerate(){
                check(NodeAddress::BezierEnd(i),segment.end);
            }
            for (i,segment) in segments.iter().enumerate(){
                check(NodeAddress::BezierControl1(i),segment.control1);
                check(NodeAddress::BezierControl2(i),segment.control2);
            }
        }
        Shape::CompoundPath{contours}=>{
            for (j,c) in contours.iter().enumerate(){
                check(NodeAddress::CompoundStart(j),c.start);
                for (i,segment) in c.segments.iter().enumerate(){
                    check(NodeAddress::CompoundEnd(j,i),segment.end);
                }
            }
            for (j,c) in contours.iter().enumerate(){
                for (i,segment) in c.segments.iter().enumerate(){
                    check(NodeAddress::CompoundControl1(j,i),segment.control1);
                    check(NodeAddress::CompoundControl2(j,i),segment.control2);
                }
            }
        }
        _=>{}
    }
    found
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
/// How the opposite control handle reacts while dragging a cubic tangent.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum HandleMode{Independent,Smooth,Symmetric}

/// Locate a tangent paired across a continuous Bézier anchor. The seam is
/// linked only when the closed contour has an explicit final cubic ending at
/// the start; an implicit straight closing edge is not a smooth tangent.
fn adjacent_tangent(segments:&[CubicSegment],start:Point,closed:bool,
    index:usize,first:bool)->Option<(Point,usize,bool,Point)>{
    let current=segments.get(index)?;
    if first{
        if index>0{
            let previous=&segments[index-1];
            Some((previous.end,index-1,false,previous.control2))
        }else if closed && segments.last()?.end==start{
            let previous=segments.last()?;
            Some((start,segments.len()-1,false,previous.control2))
        }else{None}
    }else if index+1<segments.len(){
        let following=&segments[index+1];
        Some((current.end,index+1,true,following.control1))
    }else if closed && current.end==start{
        Some((start,0,true,segments.first()?.control1))
    }else{None}
}

/// Move a cubic control handle with optional C1 continuity. On failure the
/// path is unchanged; symmetric mode mirrors the moved handle, while smooth
/// mode preserves the opposite handle's length.
pub fn move_handle(shape:&mut Shape,address:NodeAddress,to:Point,
    mode:HandleMode)->Result<(),String>{
    if mode==HandleMode::Independent || !address.is_handle(){
        return move_node(shape,address,to);
    }
    if !valid(to){return Err("Handle coordinates must stay within safe bounds".into());}
    let pairing=match (&*shape,address){
        (Shape::Bezier{start,segments,closed},NodeAddress::BezierControl1(i))=>
            adjacent_tangent(segments,*start,*closed,i,true)
                .map(|(anchor,j,first,old)|(anchor,
                    if first{NodeAddress::BezierControl1(j)}
                    else{NodeAddress::BezierControl2(j)},old)),
        (Shape::Bezier{start,segments,closed},NodeAddress::BezierControl2(i))=>
            adjacent_tangent(segments,*start,*closed,i,false)
                .map(|(anchor,j,first,old)|(anchor,
                    if first{NodeAddress::BezierControl1(j)}
                    else{NodeAddress::BezierControl2(j)},old)),
        (Shape::CompoundPath{contours},NodeAddress::CompoundControl1(j,i))=>
            contours.get(j).and_then(|c|
                adjacent_tangent(&c.segments,c.start,c.closed,i,true))
                .map(|(anchor,k,first,old)|(anchor,
                    if first{NodeAddress::CompoundControl1(j,k)}
                    else{NodeAddress::CompoundControl2(j,k)},old)),
        (Shape::CompoundPath{contours},NodeAddress::CompoundControl2(j,i))=>
            contours.get(j).and_then(|c|
                adjacent_tangent(&c.segments,c.start,c.closed,i,false))
                .map(|(anchor,k,first,old)|(anchor,
                    if first{NodeAddress::CompoundControl1(j,k)}
                    else{NodeAddress::CompoundControl2(j,k)},old)),
        _=>None,
    };
    let opposite=if let Some((anchor,other,old))=pairing{
        let dx=to.x-anchor.x;
        let dy=to.y-anchor.y;
        let length=dx.hypot(dy);
        let other_length=if mode==HandleMode::Smooth{
            (old.x-anchor.x).hypot(old.y-anchor.y)
        }else{length};
        let point=if length<=1e-6{
            // Avoid a singular direction when a tangent collapses.
            anchor
        }else{
            Point{x:anchor.x-dx/length*other_length,
                y:anchor.y-dy/length*other_length}
        };
        if !valid(point){
            return Err("Linked control handle would exceed safe bounds".into());
        }
        Some((other,point))
    }else{None};
    move_node(shape,address,to)?;
    if let Some((other,point))=opposite{
        // A control handle has no attached dependents, so this cannot fail
        // after coordinate and address preflight.
        move_node(shape,other,point)?;
    }
    Ok(())
}

/// Open a closed contour at the chosen anchor, retaining every cubic segment.
/// If closure was implicit, materialize its straight edge before reordering.
/// The result starts and ends at the split point and is editable as an open
/// path, including non-selected contours in a compound path.
/// Split an open single-contour path at an interior anchor into two
/// independent editable shapes. The shared anchor is retained at both ends,
/// so no original segment or control coordinate is modified.
pub fn split_open_at_node(shape:&Shape,address:NodeAddress)
    ->Result<(Shape,Shape),String>{
    match (shape,address){
        (Shape::Polyline{points,closed:false},NodeAddress::Polyline(i))
            if i>0 && i+1<points.len()=>{
            Ok((
                Shape::Polyline{points:points[..=i].to_vec(),closed:false},
                Shape::Polyline{points:points[i..].to_vec(),closed:false}
            ))
        }
        (Shape::Bezier{start,segments,closed:false},NodeAddress::BezierEnd(i))
            if i+1<segments.len()=>{
            Ok((
                Shape::Bezier{start:*start,segments:segments[..=i].to_vec(),closed:false},
                Shape::Bezier{start:segments[i].end,
                    segments:segments[i+1..].to_vec(),closed:false}
            ))
        }
        _=>Err("Select an interior anchor of an open polyline or Bézier curve".into()),
    }
}

pub fn break_closed_at_node(shape:&mut Shape,address:NodeAddress)
    ->Result<(),String>{
    fn break_cubic(start:&mut Point,segments:&mut Vec<CubicSegment>,
        closed:&mut bool,index:Option<usize>)->Result<(),String>{
        if !*closed{return Err("Contour is already open".into());}
        if segments.is_empty(){return Err("Cannot break an empty contour".into());}
        if index.is_some_and(|i|i>=segments.len()){
            return Err("Invalid anchor index".into());
        }
        let original_start=*start;
        let origin=if let Some(i)=index{segments[i].end}else{original_start};
        if segments.last().expect("nonempty").end!=original_start{
            if segments.len()>=60_000{return Err("Contour exceeds safe node limit".into());}
            let last=segments.last().expect("nonempty").end;
            segments.push(CubicSegment{
                control1:lerp(last,original_start,1.0/3.0),
                control2:lerp(last,original_start,2.0/3.0),
                end:original_start,
            });
        }
        let offset=index.map_or(0,|i|i+1);
        let count=segments.len();
        segments.rotate_left(offset%count);
        *start=origin;
        *closed=false;
        Ok(())
    }
    match (shape,address){
        (Shape::Polyline{points,closed},NodeAddress::Polyline(i))=>{
            if !*closed{return Err("Polyline is already open".into());}
            if i>=points.len(){return Err("Invalid polyline anchor".into());}
            if points.len()>=60_000{return Err("Polyline exceeds safe node limit".into());}
            points.rotate_left(i);
            points.push(points[0]);
            *closed=false;
            Ok(())
        }
        (Shape::Bezier{start,segments,closed},NodeAddress::BezierStart)=>
            break_cubic(start,segments,closed,None),
        (Shape::Bezier{start,segments,closed},NodeAddress::BezierEnd(i))=>
            break_cubic(start,segments,closed,Some(i)),
        (Shape::CompoundPath{contours},NodeAddress::CompoundStart(j))=>{
            let c=contours.get_mut(j).ok_or("Invalid contour")?;
            break_cubic(&mut c.start,&mut c.segments,&mut c.closed,None)
        }
        (Shape::CompoundPath{contours},NodeAddress::CompoundEnd(j,i))=>{
            let c=contours.get_mut(j).ok_or("Invalid contour")?;
            break_cubic(&mut c.start,&mut c.segments,&mut c.closed,Some(i))
        }
        _=>Err("Select a closed path anchor to break at".into()),
    }
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
    #[test] fn visible_handles_prioritize_anchors_on_long_bezier(){
        let segment=CubicSegment{
            control1:Point{x:1.,y:1.},control2:Point{x:2.,y:2.},
            end:Point{x:3.,y:3.},
        };
        let shape=Shape::Bezier{
            start:Point{x:0.,y:0.},segments:vec![segment;1000],closed:false,
        };
        let picked=handles(&shape,750);
        assert_eq!(picked.len(),750);
        assert!(picked.iter().all(|(addr,_)|!addr.is_handle()));
        assert_eq!(picked[0].0,NodeAddress::BezierStart);
        assert_eq!(picked[749].0,NodeAddress::BezierEnd(748));
    }
    #[test] fn visible_handles_keep_controls_on_small_paths(){
        let picked=handles(&path(),4);
        assert_eq!(picked.len(),4);
        assert_eq!(picked[0].0,NodeAddress::BezierStart);
        assert_eq!(picked[1].0,NodeAddress::BezierEnd(0));
        assert_eq!(picked[2].0,NodeAddress::BezierControl1(0));
        assert_eq!(picked[3].0,NodeAddress::BezierControl2(0));
    }
    #[test] fn compound_visible_handles_prioritize_contour_anchors(){
        let segment=CubicSegment{
            control1:Point{x:1.,y:1.},control2:Point{x:2.,y:2.},
            end:Point{x:3.,y:3.},
        };
        let shape=Shape::CompoundPath{contours:vec![
            Contour{start:Point{x:0.,y:0.},segments:vec![segment;10],closed:true},
            Contour{start:Point{x:5.,y:5.},segments:vec![segment;10],closed:true},
        ]};
        let picked=handles(&shape,15);
        assert!(picked.iter().all(|(addr,_)|!addr.is_handle()));
        assert!(picked.iter().any(|(addr,_)|*addr==NodeAddress::CompoundStart(1)));
    }
    #[test] fn nearest_handle_reaches_nodes_beyond_visible_750_limit(){
        let segments=(0..1500).map(|i|{
            let x=(i+1) as f32*2.0;
            CubicSegment{
                control1:Point{x:x-1.5,y:5.},
                control2:Point{x:x-0.5,y:5.},
                end:Point{x,y:0.},
            }
        }).collect();
        let shape=Shape::Bezier{start:Point{x:0.,y:0.},segments,closed:false};
        assert!(!handles(&shape,750).iter().any(|(a,_)|
            *a==NodeAddress::BezierEnd(1450)));
        let target=Point{x:2902.,y:0.};
        let (address,position)=nearest_handle(&shape,target,0.5).unwrap();
        assert_eq!(address,NodeAddress::BezierEnd(1450));
        assert_eq!(position,target);
    }
    #[test] fn nearest_handle_prefers_anchor_on_coincident_controls(){
        let shape=Shape::Bezier{start:Point{x:0.,y:0.},segments:vec![
            CubicSegment{control1:Point{x:0.,y:0.},control2:Point{x:10.,y:0.},
                end:Point{x:10.,y:0.}}
        ],closed:false};
        let (address,_)=nearest_handle(&shape,Point{x:0.,y:0.},1.).unwrap();
        assert_eq!(address,NodeAddress::BezierStart);
    }
    #[test] fn split_open_cubic_retains_every_control_and_shared_anchor(){
        let original=Shape::Bezier{start:Point{x:0.,y:0.},segments:vec![
            CubicSegment{control1:Point{x:2.,y:2.},control2:Point{x:8.,y:2.},
                end:Point{x:10.,y:0.}},
            CubicSegment{control1:Point{x:12.,y:4.},control2:Point{x:18.,y:4.},
                end:Point{x:20.,y:0.}},
            CubicSegment{control1:Point{x:22.,y:6.},control2:Point{x:28.,y:6.},
                end:Point{x:30.,y:0.}},
        ],closed:false};
        let (left,right)=split_open_at_node(&original,NodeAddress::BezierEnd(0)).unwrap();
        let Shape::Bezier{start:a,segments:first,..}=left else{panic!()};
        let Shape::Bezier{start:b,segments:second,..}=right else{panic!()};
        assert_eq!(a,Point{x:0.,y:0.});
        assert_eq!(b,first.last().unwrap().end);
        assert_eq!(first.len(),1);
        assert_eq!(second.len(),2);
        assert_eq!(second[0].control1,Point{x:12.,y:4.});
    }
    #[test] fn split_open_polyline_keeps_common_vertex(){
        let original=Shape::Polyline{points:vec![
            Point{x:0.,y:0.},Point{x:2.,y:3.},Point{x:5.,y:6.},
            Point{x:8.,y:9.}],closed:false};
        let (left,right)=split_open_at_node(&original,NodeAddress::Polyline(2)).unwrap();
        let Shape::Polyline{points:a,..}=left else{panic!()};
        let Shape::Polyline{points:b,..}=right else{panic!()};
        assert_eq!(a.len(),3);
        assert_eq!(b.len(),2);
        assert_eq!(a.last(),b.first());
    }
    #[test] fn split_open_rejects_endpoint_and_closed_paths(){
        let original=path();
        assert!(split_open_at_node(&original,NodeAddress::BezierEnd(0)).is_err());
        let poly=Shape::Polyline{points:vec![
            Point{x:0.,y:0.},Point{x:10.,y:0.},Point{x:10.,y:10.}
        ],closed:true};
        assert!(split_open_at_node(&poly,NodeAddress::Polyline(1)).is_err());
    }
    #[test] fn smooth_handle_drag_preserves_opposite_length_and_collinearity(){
        let mut shape=Shape::Bezier{start:Point{x:0.,y:0.},segments:vec![
            CubicSegment{control1:Point{x:2.,y:0.},control2:Point{x:8.,y:0.},
                end:Point{x:10.,y:0.}},
            CubicSegment{control1:Point{x:14.,y:0.},control2:Point{x:18.,y:2.},
                end:Point{x:20.,y:0.}},
        ],closed:false};
        move_handle(&mut shape,NodeAddress::BezierControl1(1),
            Point{x:10.,y:4.},HandleMode::Smooth).unwrap();
        let Shape::Bezier{segments,..}=&shape else{panic!()};
        assert!((segments[0].control2.x-10.).abs()<1e-5);
        assert!((segments[0].control2.y+2.).abs()<1e-5);
        assert_eq!(segments[1].control1,Point{x:10.,y:4.});
    }
    #[test] fn symmetric_handle_drag_mirrors_opposite(){
        let mut shape=Shape::Bezier{start:Point{x:0.,y:0.},segments:vec![
            CubicSegment{control1:Point{x:1.,y:2.},control2:Point{x:8.,y:0.},
                end:Point{x:10.,y:0.}},
            CubicSegment{control1:Point{x:12.,y:3.},control2:Point{x:18.,y:0.},
                end:Point{x:20.,y:0.}},
        ],closed:false};
        move_handle(&mut shape,NodeAddress::BezierControl2(0),
            Point{x:7.,y:-4.},HandleMode::Symmetric).unwrap();
        let Shape::Bezier{segments,..}=&shape else{panic!()};
        assert_eq!(segments[1].control1,Point{x:13.,y:4.});
        assert_eq!(segments[0].control2,Point{x:7.,y:-4.});
    }
    #[test] fn linked_handle_drag_rejects_out_of_bounds_atomically(){
        let mut shape=Shape::Bezier{start:Point{x:0.,y:0.},segments:vec![
            CubicSegment{control1:Point{x:0.,y:0.},control2:Point{x:-99_999.,y:0.},
                end:Point{x:90_000.,y:0.}},
            CubicSegment{control1:Point{x:90_001.,y:0.},control2:Point{x:90_003.,y:0.},
                end:Point{x:90_004.,y:0.}}
        ],closed:false};
        let before=shape.clone();
        assert!(move_handle(&mut shape,NodeAddress::BezierControl1(1),
            Point{x:90_000.,y:20.},HandleMode::Smooth).is_err());
        assert_eq!(shape,before);
    }
    #[test] fn split_closed_bezier_at_middle_anchor_keeps_segments(){
        let mut shape=Shape::Bezier{start:Point{x:0.,y:0.},segments:vec![
            CubicSegment{control1:Point{x:2.,y:0.},control2:Point{x:8.,y:0.},
                end:Point{x:10.,y:0.}},
            CubicSegment{control1:Point{x:10.,y:2.},control2:Point{x:0.,y:8.},
                end:Point{x:0.,y:0.}},
        ],closed:true};
        break_closed_at_node(&mut shape,NodeAddress::BezierEnd(0)).unwrap();
        let Shape::Bezier{start,segments,closed}=shape else{panic!()};
        assert!(!closed);
        assert_eq!(start,Point{x:10.,y:0.});
        assert_eq!(segments.len(),2);
        assert_eq!(segments[0].end,Point{x:0.,y:0.});
        assert_eq!(segments[1].end,start);
    }
    #[test] fn split_implicit_closure_materializes_straight_segment(){
        let mut shape=Shape::Bezier{start:Point{x:0.,y:0.},segments:vec![
            CubicSegment{control1:Point{x:2.,y:0.},control2:Point{x:8.,y:0.},
                end:Point{x:10.,y:0.}}
        ],closed:true};
        break_closed_at_node(&mut shape,NodeAddress::BezierStart).unwrap();
        let Shape::Bezier{start,segments,closed}=shape else{panic!()};
        assert!(!closed);
        assert_eq!(segments.len(),2);
        assert_eq!(segments[1].end,start);
        assert_eq!(segments[1].control1,Point{x:10.0/1.0-10.0/3.0,y:0.});
    }
    #[test] fn break_closed_polyline_duplicates_seam_without_losing_edges(){
        let mut shape=Shape::Polyline{points:vec![
            Point{x:0.,y:0.},Point{x:5.,y:0.},Point{x:5.,y:5.}],closed:true};
        break_closed_at_node(&mut shape,NodeAddress::Polyline(1)).unwrap();
        let Shape::Polyline{points,closed}=shape else{panic!()};
        assert!(!closed);
        assert_eq!(points,vec![Point{x:5.,y:0.},Point{x:5.,y:5.},
            Point{x:0.,y:0.},Point{x:5.,y:0.}]);
    }
    #[test] fn break_invalid_or_open_path_preserves_geometry(){
        let mut shape=path();
        let before=shape.clone();
        assert!(break_closed_at_node(&mut shape,NodeAddress::BezierEnd(0)).is_err());
        assert_eq!(shape,before);
        let Shape::Bezier{closed,..}=&mut shape else{panic!()};
        *closed=true;
        let before=shape.clone();
        assert!(break_closed_at_node(&mut shape,NodeAddress::BezierEnd(9)).is_err());
        assert_eq!(shape,before);
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