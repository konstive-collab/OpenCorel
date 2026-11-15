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
pub fn handles(shape:&Shape,limit:usize)->Vec<(NodeAddress,Point)>{
    let mut nodes=Vec::new();
    if limit==0{return nodes;}
    let mut push=|a,p|{
        if nodes.len()<limit{nodes.push((a,p));}
    };
    match shape{
        Shape::Polyline{points,..}=>{
            for (i,p) in points.iter().enumerate().take(limit){push(NodeAddress::Polyline(i),*p);}
        }
        Shape::Bezier{start,segments,..}=>{
            push(NodeAddress::BezierStart,*start);
            for (i,s) in segments.iter().enumerate().take(limit){
                push(NodeAddress::BezierEnd(i),s.end);
                push(NodeAddress::BezierControl1(i),s.control1);
                push(NodeAddress::BezierControl2(i),s.control2);
            }
        }
        Shape::CompoundPath{contours}=>{
            for (j,c) in contours.iter().enumerate(){
                push(NodeAddress::CompoundStart(j),c.start);
                for (i,s) in c.segments.iter().enumerate(){
                    push(NodeAddress::CompoundEnd(j,i),s.end);
                    push(NodeAddress::CompoundControl1(j,i),s.control1);
                    push(NodeAddress::CompoundControl2(j,i),s.control2);
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
/// Return nearest path segment and parameter for double-click insertion.
/// Segment search is deliberately capped for heavy compound glyph outlines.
pub fn nearest_segment(shape:&Shape,p:Point,tolerance:f32)
    ->Option<(SegmentAddress,f32)>{
    let mut result=None;
    let mut best=tolerance*tolerance;
    let mut tested=0usize;
    let mut examine=|address,start:Point,segment:&CubicSegment|{
        if tested>=4000{return;}
        tested+=1;
        for sample in 1..=24{
            let t=sample as f32/25.0;
            let q=cubic(start,segment,t);
            let d=(p.x-q.x).powi(2)+(p.y-q.y).powi(2);
            if d<best{best=d;result=Some((address,t));}
        }
    };
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
                examine(SegmentAddress::Bezier(i),prev,s);
                prev=s.end;
                if tested>=4000{break;}
            }
        }
        Shape::CompoundPath{contours}=>{
            'outer:for (j,c) in contours.iter().enumerate(){
                let mut prev=c.start;
                for (i,s) in c.segments.iter().enumerate(){
                    examine(SegmentAddress::Compound(j,i),prev,s);
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