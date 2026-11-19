//! Native editable measurement annotations, in physical millimetres.
//! No separate proprietary dimension objects: each measurement is four
//! regular KDR vector objects grouped for repositioning/undo/export.
use crate::{Document,Point,Shape,Style};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum DimensionAxis{Width,Height}
impl Document{
    pub fn annotate_dimension(&mut self,object:u64,axis:DimensionAxis,
        offset_mm:f32)->Result<Vec<u64>,String>{
        if !offset_mm.is_finite()||!(2.0..=500.0).contains(&offset_mm){
            return Err("Dimension offset must be 2–500 mm".into());
        }
        if self.objects.len()>99_995||self.next_id>u64::MAX-7{
            return Err("Document object limit reached".into());
        }
        let original=self.get(object).ok_or("Selected object not found")?;
        let layer=original.layer_id;
        if self.layer(layer).is_none_or(|l|l.locked||!l.visible){
            return Err("Cannot measure an object on a locked/hidden layer".into());
        }
        let b=original.bounds();
        if b.w<0.01||b.h<0.01{
            return Err("Object has no measurable size".into());
        }
        let center=match axis{
            DimensionAxis::Width=>Point{x:b.x+b.w*0.5,y:b.y+b.h+offset_mm},
            DimensionAxis::Height=>Point{x:b.x+b.w+offset_mm,y:b.y+b.h*0.5}
        };
        let (start,end,tick_start,tick_end,label)=match axis{
            DimensionAxis::Width=>{
                let y=center.y;
                (Point{x:b.x,y},Point{x:b.x+b.w,y},
                (Point{x:b.x,y:y-2.},Point{x:b.x,y:y+2.}),
                (Point{x:b.x+b.w,y:y-2.},Point{x:b.x+b.w,y:y+2.}),
                format!("W {:.2} mm",b.w))
            }
            DimensionAxis::Height=>{
                let x=center.x;
                (Point{x,y:b.y},Point{x,y:b.y+b.h},
                (Point{x:x-2.,y:b.y},Point{x:x+2.,y:b.y}),
                (Point{x:x-2.,y:b.y+b.h},Point{x:x+2.,y:b.y+b.h}),
                format!("H {:.2} mm",b.h))
            }
        };
        let line_style=Style{fill:"none".into(),
            stroke:"#0C83A5".into(),stroke_width_mm:0.3};
        let mut ids=Vec::with_capacity(4);
        for (start,end) in [(start,end),tick_start,tick_end]{
            ids.push(self.add_to_layer(Shape::Polyline{
                points:vec![start,end],closed:false},line_style.clone(),layer));
        }
        let (tx,ty)=match axis{
            DimensionAxis::Width=>(center.x-4.0,center.y-8.0),
            DimensionAxis::Height=>(center.x+4.0,center.y-4.0)
        };
        ids.push(self.add_to_layer(Shape::Text{
            x:tx,y:ty,value:label,size_mm:4.0},
            Style{fill:"#0C83A5".into(),stroke:"none".into(),
                stroke_width_mm:0.0},layer));
        let _=self.group_objects(&ids)?;
        for (index,id) in ids.iter().enumerate(){
            if let Some(obj)=self.get_mut(*id){
                obj.name=if index==3{"Размер (мм)".into()}
                    else{"Размерная линия".into()};
            }
        }
        Ok(ids)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::Shape;
    #[test] fn annotations_are_grouped_natively_and_undoable_by_snapshot(){
        let mut d=Document::default();
        let id=d.add(Shape::Rectangle{x:10.,y:20.,w:50.,h:25.},Style::default());
        let before=d.clone();
        let ids=d.annotate_dimension(id,DimensionAxis::Width,10.).unwrap();
        assert_eq!(ids.len(),4);
        assert!(ids.iter().all(|x|d.get(*x).unwrap().group_id.is_some()));
        let group=d.get(ids[0]).unwrap().group_id;
        assert!(ids.iter().all(|x|d.get(*x).unwrap().group_id==group));
        d.validate().unwrap();
        assert_eq!(before.objects.len()+4,d.objects.len());
    }
    #[test] fn annotation_rejects_locked_object_without_partial_changes(){
        let mut d=Document::default();
        let id=d.add(Shape::Ellipse{x:5.,y:5.,w:20.,h:15.},Style::default());
        d.layers[0].locked=true;
        let before=d.clone();
        assert!(d.annotate_dimension(id,DimensionAxis::Height,12.).is_err());
        assert_eq!(before,d);
    }
}
