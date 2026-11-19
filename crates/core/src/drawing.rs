//! Interactive freehand geometry and Corel-style window/crossing selection.
//! All coordinates are real document millimetres, independent of display DPI.
use crate::{Bounds,Document,Point};
use std::collections::HashSet;

fn distance_segment_sq(p:Point,a:Point,b:Point)->f32{
    let vx=b.x-a.x;
    let vy=b.y-a.y;
    let len=vx*vx+vy*vy;
    let t=if len<1.0e-10{0.0}else{
        ((p.x-a.x)*vx+(p.y-a.y)*vy)/len
    }.clamp(0.0,1.0);
    (p.x-a.x-t*vx).powi(2)+(p.y-a.y-t*vy).powi(2)
}
/// Bounded Ramer–Douglas–Peucker simplification for pressure-free mouse or
/// pen strokes. Keeps the endpoints exactly and never mutates the input.
pub fn simplify_stroke(points:&[Point],tolerance_mm:f32)->Result<Vec<Point>,String>{
    if points.len()>30_000{return Err("Freehand stroke is too long".into());}
    if !tolerance_mm.is_finite()||!(0.01..=20.0).contains(&tolerance_mm){
        return Err("Invalid smoothing tolerance".into());
    }
    if points.iter().any(|p|!p.x.is_finite()||!p.y.is_finite()
        ||p.x.abs()>100_000.0||p.y.abs()>100_000.0){
        return Err("Invalid freehand coordinates".into());
    }
    let mut vertices=Vec::with_capacity(points.len());
    for &p in points{
        if vertices.last().is_none_or(|old:&Point|
            (p.x-old.x).hypot(p.y-old.y)>=0.02){
            vertices.push(p);
        }
    }
    if vertices.len()<3{return Ok(vertices);}
    let mut retained=vec![false;vertices.len()];
    retained[0]=true;
    retained[vertices.len()-1]=true;
    let mut stack=vec![(0,vertices.len()-1)];
    let threshold=tolerance_mm*tolerance_mm;
    while let Some((start,end))=stack.pop(){
        if end-start<2{continue;}
        let mut best=(0.0,start);
        for i in start+1..end{
            let distance=distance_segment_sq(vertices[i],vertices[start],vertices[end]);
            if distance>best.0{best=(distance,i);}
        }
        if best.0>threshold{
            retained[best.1]=true;
            stack.push((start,best.1));
            stack.push((best.1,end));
        }
    }
    Ok(vertices.into_iter().zip(retained)
        .filter_map(|(p,keep)|keep.then_some(p)).collect())
}
fn contains(outer:Bounds,inner:Bounds)->bool{
    inner.x>=outer.x&&inner.y>=outer.y
        &&inner.x+inner.w<=outer.x+outer.w
        &&inner.y+inner.h<=outer.y+outer.h
}
fn intersects(a:Bounds,b:Bounds)->bool{
    a.x<=b.x+b.w&&a.x+a.w>=b.x
        &&a.y<=b.y+b.h&&a.y+a.h>=b.y
}
impl Document{
    /// Drag rightward = all objects must fit entirely in the window.
    /// Drag leftward = crossing selection, any overlap counts.
    /// Hidden and locked layers, and invisible clipping-mask sources are
    /// excluded just as for ordinary pointer hit testing.
    pub fn marquee_select(&self,rect:Bounds,crossing:bool)->Vec<u64>{
        if ![rect.x,rect.y,rect.w,rect.h].iter().all(|v|v.is_finite())
            ||rect.w<0.0||rect.h<0.0{return Vec::new();}
        let editable:HashSet<_>=self.layers.iter().filter(|l|
            l.visible&&!l.locked).map(|l|l.id).collect();
        let masks:HashSet<_>=self.clip_links.iter().map(|l|l.source_id).collect();
        let mut ids=HashSet::new();
        let mut groups=HashSet::new();
        for obj in &self.objects{
            if !editable.contains(&obj.layer_id)||masks.contains(&obj.id){continue;}
            let bounds=obj.bounds();
            if if crossing{intersects(rect,bounds)}else{contains(rect,bounds)}{
                ids.insert(obj.id);
                if let Some(group)=obj.group_id{groups.insert(group);}
            }
        }
        // Group selection behaves exactly like a click: whole groups are
        // selected together, except locked/hidden members stay untouched.
        for obj in &self.objects{
            if obj.group_id.is_some_and(|g|groups.contains(&g))
                && editable.contains(&obj.layer_id)&&!masks.contains(&obj.id){
                ids.insert(obj.id);
            }
        }
        self.objects.iter().filter_map(|o|ids.contains(&o.id).then_some(o.id))
            .collect()
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Shape,Style};
    #[test] fn simplifies_dense_strokes_without_distorting_endpoints(){
        let points:Vec<_>=(0..200).map(|i|Point{
            x:i as f32*0.5,y:(i as f32*0.03).sin()*0.02
        }).collect();
        let simplified=simplify_stroke(&points,0.1).unwrap();
        assert!(simplified.len()<10);
        assert_eq!(simplified[0],points[0]);
        assert_eq!(*simplified.last().unwrap(),*points.last().unwrap());
        assert_eq!(points.len(),200);
    }
    #[test] fn preserves_polyline_corners_and_rejects_invalid_points(){
        let p=[Point{x:0.,y:0.},Point{x:10.,y:0.},
               Point{x:10.,y:20.},Point{x:30.,y:20.}];
        assert!(simplify_stroke(&p,0.1).unwrap().len()>=4);
        assert!(simplify_stroke(&[Point{x:f32::NAN,y:0.}],1.).is_err());
    }
    #[test] fn window_and_crossing_selection_ignore_hidden_layers(){
        let mut d=Document::default();
        let a=d.add(Shape::Rectangle{x:10.,y:10.,w:10.,h:10.},Style::default());
        let b=d.add(Shape::Rectangle{x:22.,y:10.,w:10.,h:10.},Style::default());
        let r=Bounds{x:5.,y:5.,w:20.,h:20.};
        assert_eq!(d.marquee_select(r,false),vec![a]);
        assert_eq!(d.marquee_select(r,true),vec![a,b]);
        d.layers[0].locked=true;
        assert!(d.marquee_select(r,true).is_empty());
    }
    #[test] fn picking_one_group_member_selects_all_members(){
        let mut d=Document::default();
        let a=d.add(Shape::Rectangle{x:10.,y:10.,w:5.,h:5.},Style::default());
        let b=d.add(Shape::Rectangle{x:80.,y:10.,w:5.,h:5.},Style::default());
        d.group_objects(&[a,b]).unwrap();
        let selected=d.marquee_select(Bounds{x:9.,y:9.,w:8.,h:8.},false);
        assert_eq!(selected,vec![a,b]);
    }
}
