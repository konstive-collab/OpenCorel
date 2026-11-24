//! Safe vector geometry maintenance: convert primitives into editable cubic
//! Béziers, simplify open polylines and join coincident open endpoints.
//! All methods preflight the entire selected set before modifying the KDR.
use crate::{CubicSegment,Document,Point,Shape};
use std::collections::HashSet;

fn lerp(a:Point,b:Point,t:f32)->Point{
    Point{x:a.x+(b.x-a.x)*t,y:a.y+(b.y-a.y)*t}
}
fn segment(a:Point,b:Point)->CubicSegment{
    CubicSegment{control1:lerp(a,b,1.0/3.0),
        control2:lerp(a,b,2.0/3.0),end:b}
}
pub fn to_editable_bezier(shape:&Shape)->Result<Shape,String>{
    match shape{
        Shape::Bezier{..}=>Ok(shape.clone()),
        Shape::Rectangle{x,y,w,h}=>{
            if !w.is_finite()||!h.is_finite()||*w<=0.0||*h<=0.0{
                return Err("Invalid rectangle dimensions".into());
            }
            let corners=[
                Point{x:*x,y:*y},Point{x:x+w,y:*y},
                Point{x:x+w,y:y+h},Point{x:*x,y:y+h},
            ];
            Ok(Shape::Bezier{start:corners[0],
                segments:(0..4).map(|i|segment(corners[i],corners[(i+1)%4])).collect(),
                closed:true})
        }
        Shape::Ellipse{x,y,w,h}=>{
            if !w.is_finite()||!h.is_finite()||*w<=0.0||*h<=0.0{
                return Err("Invalid ellipse dimensions".into());
            }
            let (cx,cy)=(x+w*0.5,y+h*0.5);
            let (rx,ry)=(w*0.5,h*0.5);
            let k=0.552_284_8_f32;
            let start=Point{x:cx+rx,y:cy};
            let segments=vec![
                CubicSegment{control1:Point{x:cx+rx,y:cy+k*ry},
                    control2:Point{x:cx+k*rx,y:cy+ry},end:Point{x:cx,y:cy+ry}},
                CubicSegment{control1:Point{x:cx-k*rx,y:cy+ry},
                    control2:Point{x:cx-rx,y:cy+k*ry},end:Point{x:cx-rx,y:cy}},
                CubicSegment{control1:Point{x:cx-rx,y:cy-k*ry},
                    control2:Point{x:cx-k*rx,y:cy-ry},end:Point{x:cx,y:cy-ry}},
                CubicSegment{control1:Point{x:cx+k*rx,y:cy-ry},
                    control2:Point{x:cx+rx,y:cy-k*ry},end:start},
            ];
            Ok(Shape::Bezier{start,segments,closed:true})
        }
        Shape::Polyline{points,closed}=>{
            if points.len()<2{return Err("Polyline requires at least two nodes".into());}
            let mut segments:Vec<_>=points.windows(2).map(|p|segment(p[0],p[1])).collect();
            if *closed && points.last()!=points.first(){
                segments.push(segment(*points.last().unwrap(),points[0]));
            }
            Ok(Shape::Bezier{start:points[0],segments,closed:*closed})
        }
        Shape::CompoundPath{..}=>Err("Compound paths already have editable cubic contours".into()),
        Shape::Text{..}=>Err("Convert text to outlines with the typography tool first".into()),
        Shape::Raster{..}|Shape::EmbeddedSvg{..}=>Err(
            "Embedded artwork cannot be converted into independent editable curves".into()),
    }
}
impl Document{
    fn verify_path_selection(&self,ids:&[u64])->Result<HashSet<u64>,String>{
        let selected:HashSet<_>=ids.iter().copied().collect();
        if selected.is_empty(){return Err("Select vector objects first".into());}
        if self.objects.iter().filter(|o|selected.contains(&o.id)).count()
            !=selected.len(){return Err("Unknown object selected".into());}
        for obj in self.objects.iter().filter(|o|selected.contains(&o.id)){
            if self.layer(obj.layer_id).is_none_or(|l|!l.visible||l.locked){
                return Err("Cannot edit a hidden or locked layer".into());
            }
            if obj.group_id.is_some_and(|group|
                self.objects.iter().any(|other|
                    other.group_id==Some(group)&&!selected.contains(&other.id))){
                return Err("Select every member of the group".into());
            }
        }
        Ok(selected)
    }
    /// Convert all chosen rectangles, ellipses and paths in a single action.
    /// IDs, fills, gradients, group IDs, layer and clipping links are retained.
    pub fn convert_selection_to_bezier(&mut self,ids:&[u64])
        ->Result<usize,String>{
        let selection=self.verify_path_selection(ids)?;
        let mut converted=Vec::new();
        for obj in self.objects.iter().filter(|o|selection.contains(&o.id)){
            let shape=to_editable_bezier(&obj.shape)?;
            if shape!=obj.shape{converted.push((obj.id,shape));}
        }
        let count=converted.len();
        for (id,shape) in converted{
            self.get_mut(id).expect("preflighted").shape=shape;
        }
        Ok(count)
    }
    /// Losslessly retained input stroke endpoints; removes interior vertices
    /// using a bounded geometric tolerance. Closed paths are rejected because
    /// simplifying their final-to-first seam can change fill topology.
    pub fn simplify_selected_polylines(&mut self,ids:&[u64],
        tolerance_mm:f32)->Result<usize,String>{
        let selection=self.verify_path_selection(ids)?;
        let mut planned=Vec::new();
        let mut removed=0usize;
        for obj in self.objects.iter().filter(|o|selection.contains(&o.id)){
            let Shape::Polyline{points,closed:false}=&obj.shape else{
                return Err("Simplify requires open polylines only".into());
            };
            let next=crate::drawing::simplify_stroke(points,tolerance_mm)?;
            if next.len()<2{
                return Err("Simplification produced fewer than two nodes".into());
            }
            removed+=points.len()-next.len();
            planned.push((obj.id,next));
        }
        for (id,points) in planned{
            self.get_mut(id).expect("preflighted").shape=Shape::Polyline{
                points,closed:false
            };
        }
        Ok(removed)
    }
    /// Join exactly two open paths. Polyline pairs retain their node type;
    /// mixed line/Bézier pairs become fully editable cubic paths. Reverse
    /// orientation is automatic and the first object's ID/style survives.
    /// Preflight the entire geometry before modifying the document.
    pub fn join_open_paths(&mut self,ids:&[u64],
        tolerance_mm:f32)->Result<u64,String>{
        fn endpoints(shape:&Shape)->Result<(Point,Point,usize),String>{
            match shape{
                Shape::Polyline{points,closed} if !*closed && points.len()>=2=>
                    Ok((points[0],*points.last().unwrap(),points.len())),
                Shape::Bezier{start,segments,closed} if !*closed && !segments.is_empty()=>
                    Ok((*start,segments.last().unwrap().end,segments.len())),
                _=>Err("Choose two nonempty open polylines or Bézier curves".into()),
            }
        }
        fn safe(point:Point)->bool{
            point.x.is_finite()&&point.y.is_finite()&&
                point.x.abs()<=100_000.&&point.y.abs()<=100_000.
        }
        if ids.len()!=2||ids[0]==ids[1]{
            return Err("Select exactly two different open paths".into());
        }
        if !tolerance_mm.is_finite()||!(0.01..=20.0).contains(&tolerance_mm){
            return Err("Endpoint tolerance must be 0.01–20 mm".into());
        }
        self.verify_path_selection(ids)?;
        let a=self.get(ids[0]).expect("preflighted").clone();
        let b=self.get(ids[1]).expect("preflighted").clone();
        if a.group_id.is_some()||b.group_id.is_some()||
            a.layer_id!=b.layer_id||a.style!=b.style||
            self.appearances.get(&a.id)!=self.appearances.get(&b.id){
            return Err("Paths must be ungrouped and share layer and styling".into());
        }
        if self.clip_links.iter().any(|link|
            [a.id,b.id].contains(&link.source_id)||
            [a.id,b.id].contains(&link.target_id)){
            return Err("Detach clipping masks before joining paths".into());
        }
        let (a_start,a_end,a_count)=endpoints(&a.shape)?;
        let (b_start,b_end,b_count)=endpoints(&b.shape)?;
        let pure_poly=matches!((&a.shape,&b.shape),
            (Shape::Polyline{..},Shape::Polyline{..}));
        let limit=if pure_poly{100_000}else{60_000};
        if a_count+b_count>limit{
            return Err("Joined path exceeds the safe node count".into());
        }
        if ![a_start,a_end,b_start,b_end].iter().all(|p|safe(*p)){
            return Err("Path endpoints are outside safe bounds".into());
        }
        let length=|p:Point,q:Point|(p.x-q.x).hypot(p.y-q.y);
        let candidates=[
            length(a_end,b_start),length(a_end,b_end),
            length(a_start,b_end),length(a_start,b_start)
        ];
        let (mode,distance)=candidates.iter().copied().enumerate()
            .min_by(|a,b|a.1.total_cmp(&b.1)).expect("4 endpoints");
        if !distance.is_finite()||distance>tolerance_mm{
            return Err(format!("Endpoints are {:.3} mm apart, tolerance {:.3} mm",
                distance,tolerance_mm));
        }
        let joined=if pure_poly{
            let (Shape::Polyline{points:ap,..},
                Shape::Polyline{points:bp,..})=(&a.shape,&b.shape) else{unreachable!()};
            let mut first=ap.clone();
            let mut second=bp.clone();
            match mode{
                1=>second.reverse(),
                2=>std::mem::swap(&mut first,&mut second),
                3=>first.reverse(),
                _=>{}
            }
            let last=first.len()-1;
            let (p,q)=(first[last],second[0]);
            first[last]=Point{x:p.x+(q.x-p.x)*0.5,y:p.y+(q.y-p.y)*0.5};
            first.extend(second.into_iter().skip(1));
            Shape::Polyline{points:first,closed:false}
        }else{
            let mut first=to_editable_bezier(&a.shape)?;
            let mut second=to_editable_bezier(&b.shape)?;
            match mode{
                1=>crate::node_edit::reverse_path(&mut second)?,
                2=>std::mem::swap(&mut first,&mut second),
                3=>crate::node_edit::reverse_path(&mut first)?,
                _=>{}
            }
            let (Shape::Bezier{segments:left,..},
                Shape::Bezier{start:right_start,segments:right,..})=
                (&mut first,&mut second) else{unreachable!()};
            let tail=left.last_mut().ok_or("Empty left Bézier")?;
            let origin=tail.end;
            let join=Point{x:origin.x+(right_start.x-origin.x)*0.5,
                y:origin.y+(right_start.y-origin.y)*0.5};
            let left_control=Point{
                x:tail.control2.x+(join.x-origin.x),
                y:tail.control2.y+(join.y-origin.y),
            };
            let first_right=right.first().ok_or("Empty right Bézier")?;
            let right_control=Point{
                x:first_right.control1.x+(join.x-right_start.x),
                y:first_right.control1.y+(join.y-right_start.y),
            };
            if ![join,left_control,right_control].iter().all(|p|safe(*p)){
                return Err("Joining would move a control handle outside safe bounds".into());
            }
            tail.end=join;
            tail.control2=left_control;
            right[0].control1=right_control;
            *right_start=join;
            left.append(right);
            first
        };
        self.get_mut(a.id).expect("preflighted").shape=joined;
        self.remove(b.id);
        Ok(a.id)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Shape,Style};
    #[test] fn converts_rectangle_and_ellipse_to_closed_cubics(){
        let r=Shape::Rectangle{x:1.,y:2.,w:80.,h:40.};
        let e=Shape::Ellipse{x:1.,y:2.,w:80.,h:40.};
        for original in [r,e]{
            let Shape::Bezier{segments,closed,..}=
                to_editable_bezier(&original).unwrap() else{panic!()};
            assert_eq!(segments.len(),4);
            assert!(closed);
        }
    }
    #[test] fn conversion_preserves_native_id_gradient_and_undo_snapshot(){
        let mut doc=Document::default();
        let id=doc.add(Shape::Ellipse{x:10.,y:10.,w:20.,h:25.},Style::default());
        doc.set_appearance(id,crate::appearance::Appearance{
            gradient:Some(crate::appearance::GradientPreset::Sunrise.create()),
            ..Default::default()
        }).unwrap();
        let before=doc.clone();
        assert_eq!(doc.convert_selection_to_bezier(&[id]).unwrap(),1);
        assert!(matches!(doc.get(id).unwrap().shape,Shape::Bezier{..}));
        assert_eq!(doc.appearances.get(&id),before.appearances.get(&id));
        doc.validate().unwrap();
    }
    #[test] fn join_rejects_incompatible_paths_atomically(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Polyline{points:vec![
            Point{x:0.,y:0.},Point{x:10.,y:0.}],closed:false},Style::default());
        let b=doc.add(Shape::Polyline{points:vec![
            Point{x:30.,y:0.},Point{x:50.,y:0.}],closed:false},Style::default());
        let before=doc.clone();
        assert!(doc.join_open_paths(&[a,b],1.0).is_err());
        assert_eq!(doc,before);
    }
    #[test] fn join_paths_matches_reversed_endpoint_and_keeps_first_id(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Polyline{points:vec![
            Point{x:0.,y:0.},Point{x:10.,y:0.}],closed:false},Style::default());
        let b=doc.add(Shape::Polyline{points:vec![
            Point{x:20.,y:0.},Point{x:10.05,y:0.}],closed:false},Style::default());
        let result=doc.join_open_paths(&[a,b],0.1).unwrap();
        assert_eq!(result,a);
        let Shape::Polyline{points,..}=&doc.get(a).unwrap().shape else{panic!()};
        assert_eq!(points.len(),3);
        assert!(doc.get(b).is_none());
        doc.validate().unwrap();
    }
    #[test] fn joins_two_beziers_without_losing_cubic_controls(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Bezier{start:Point{x:0.,y:0.},segments:vec![
            CubicSegment{control1:Point{x:2.,y:5.},control2:Point{x:8.,y:5.},
                end:Point{x:10.,y:0.}}],closed:false},Style::default());
        let b=doc.add(Shape::Bezier{start:Point{x:10.1,y:0.},segments:vec![
            CubicSegment{control1:Point{x:12.,y:-5.},control2:Point{x:18.,y:-5.},
                end:Point{x:20.,y:0.}}],closed:false},Style::default());
        let result=doc.join_open_paths(&[a,b],0.2).unwrap();
        assert_eq!(result,a);
        assert!(doc.get(b).is_none());
        let Shape::Bezier{start,segments,closed}=&doc.get(a).unwrap().shape
            else{panic!()};
        assert_eq!(*start,Point{x:0.,y:0.});
        assert!(!closed);
        assert_eq!(segments.len(),2);
        assert!((segments[0].end.x-10.05).abs()<1e-5);
        assert!((segments[1].control1.x-11.95).abs()<1e-4);
        doc.validate().unwrap();
    }
    #[test] fn joins_mixed_polyline_and_bezier_reversing_start_endpoint(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Polyline{points:vec![
            Point{x:10.,y:0.},Point{x:0.,y:0.}],closed:false},Style::default());
        let b=doc.add(Shape::Bezier{start:Point{x:10.05,y:0.},segments:vec![
            CubicSegment{control1:Point{x:12.,y:5.},control2:Point{x:18.,y:5.},
                end:Point{x:20.,y:0.}}],closed:false},Style::default());
        doc.join_open_paths(&[a,b],0.1).unwrap();
        let Shape::Bezier{start,segments,closed}=&doc.get(a).unwrap().shape
            else{panic!()};
        assert_eq!(*start,Point{x:0.,y:0.});
        assert!(!closed);
        assert_eq!(segments.len(),2);
        assert_eq!(segments.last().unwrap().end,Point{x:20.,y:0.});
        doc.validate().unwrap();
    }
    #[test] fn bezier_join_mismatch_is_atomic(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Bezier{start:Point{x:0.,y:0.},segments:vec![
            segment(Point{x:0.,y:0.},Point{x:10.,y:0.})
        ],closed:false},Style::default());
        let b=doc.add(Shape::Bezier{start:Point{x:50.,y:0.},segments:vec![
            segment(Point{x:50.,y:0.},Point{x:60.,y:0.})
        ],closed:false},Style::default());
        let before=doc.clone();
        assert!(doc.join_open_paths(&[a,b],0.2).is_err());
        assert_eq!(doc,before);
    }
    #[test] fn simplify_path_keeps_endpoints_and_rejects_closed(){
        let mut doc=Document::default();
        let id=doc.add(Shape::Polyline{points:(0..100).map(|i|
            Point{x:i as f32*0.25,y:0.}).collect(),closed:false},Style::default());
        let removed=doc.simplify_selected_polylines(&[id],0.2).unwrap();
        assert_eq!(removed,98);
        doc.validate().unwrap();
    }
}
