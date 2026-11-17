//! Compound path editing commands. Exact geometry, one undo checkpoint,
//! and explicit refusal of cases that would discard object-level metadata.
use super::Editor;
use opencorel_core::{compound_ops,Document,Shape};

impl Editor{
    pub(super) fn combine_selected_contours(&mut self){
        let ids=self.selection_ids();
        if ids.len()<2 || ids.len()>10000{
            self.status="Выберите от 2 до 10 000 замкнутых кривых".into();return;
        }
        let mut originals=Vec::with_capacity(ids.len());
        for id in &ids{
            let Some(obj)=self.doc.objects.iter().find(|o|o.id==*id) else{
                self.status="Выделенный объект не найден".into();return;
            };
            if !obj.visible||obj.locked||!self.doc.layer(obj.layer_id).is_some_and(|l|l.visible&&!l.locked)
                || obj.group_id.is_some(){
                self.status="Объекты должны быть на видимых незаблокированных слоях без групп".into();
                return;
            }
            if self.doc.clip_links.iter().any(|link|
                link.source_id==*id || link.target_id==*id){
                self.status="Удалите связи обтравки перед объединением контуров".into();
                return;
            }
            if !obj.node_modes.is_empty(){
                self.status="Сначала снимите режимы связанных узлов: объединение не должно терять их метаданные".into();
                return;
            }
            originals.push(obj.clone());
        }
        let first=&originals[0];
        if originals.iter().skip(1).any(|o|
            o.layer_id!=first.layer_id || o.style!=first.style ||
            self.doc.appearances.get(&o.id)!=self.doc.appearances.get(&first.id)){
            self.status="Для составного контура нужны одинаковые слой, стиль и эффекты".into();
            return;
        }
        let shapes:Vec<&Shape>=originals.iter().map(|o|&o.shape).collect();
        let result=match compound_ops::combine_exact(&shapes){
            Ok(shape)=>shape,
            Err(e)=>{self.status=format!("Нельзя объединить контуры: {e}");return;}
        };
        let mut preview=Document::default();
        preview.add(result.clone(),first.style.clone());
        if let Err(e)=preview.validate(){
            self.status=format!("Геометрия отклонена: {e}");return;
        }
        let first_id=first.id;
        self.checkpoint();
        if let Some(o)=self.doc.get_mut(first_id){o.shape=result;}
        for id in ids.into_iter().filter(|id|*id!=first_id){
            self.doc.remove(id);
        }
        self.selected=Some(first_id);
        self.multi_selected.clear();
        self.active_node=None;self.node_drag=None;self.node_drag_checkpointed=false;
        self.fill_cache.entries.clear();
        self.compound_cache.clear();self.vector_cache.clear();self.bezier_preview.clear();
        self.status="Контуры объединены в один составной объект (even-odd, не Boolean Union). Ctrl+Z — отмена".into();
    }

    pub(super) fn break_apart_selected_contour(&mut self){
        let Some(id)=self.selected else{
            self.status="Выберите составной контур".into();return;
        };
        if !self.multi_selected.is_empty(){
            self.status="Разделяйте по одному составному объекту".into();return;
        }
        let Some(original)=self.doc.objects.iter().find(|o|o.id==id).cloned() else{return;};
        if original.group_id.is_some()||!original.visible||original.locked||
            !self.doc.layer(original.layer_id).is_some_and(|l|l.visible&&!l.locked){
            self.status="Составной объект сгруппирован либо слой заблокирован".into();
            return;
        }
        if self.doc.clip_links.iter().any(|link|
            link.source_id==id || link.target_id==id){
            self.status="Сначала удалите связь обтравки у составного контура".into();
            return;
        }
        if !original.node_modes.is_empty(){
            self.status="Составной контур содержит специальные режимы узлов; разделение пока недоступно".into();
            return;
        }
        let pieces=match compound_ops::break_apart_exact(&original.shape){
            Ok(pieces)=>pieces,
            Err(e)=>{self.status=format!("Нельзя разделить контур: {e}");return;}
        };
        if self.doc.objects.len().saturating_add(pieces.len()-1)>100_000 ||
            self.doc.next_id.checked_add((pieces.len()-1) as u64)
                .is_none_or(|v|v==u64::MAX){
            self.status="Недостаточно свободных идентификаторов или мест для объектов".into();
            return;
        }
        self.checkpoint();
        if let Some(obj)=self.doc.get_mut(id){obj.shape=pieces[0].clone();}
        let mut new_ids=Vec::new();
        for shape in pieces.into_iter().skip(1){
            let new_id=self.doc.add_to_layer(shape,original.style.clone(),original.layer_id);
            if let Some(obj)=self.doc.get_mut(new_id){
                obj.text_format=original.text_format.clone();
            }
            if let Some(appearance)=self.doc.appearances.get(&id).cloned(){
                self.doc.appearances.insert(new_id,appearance);
            }
            new_ids.push(new_id);
        }
        // Keep the separated parts in the source's original z-order region.
        let mut extracted=Vec::new();
        self.doc.objects.retain(|o|{
            if new_ids.contains(&o.id){
                extracted.push(o.clone());
                false
            }else{true}
        });
        if let Some(pos)=self.doc.objects.iter().position(|o|o.id==id){
            self.doc.objects.splice(pos+1..pos+1,extracted);
        }
        self.selected=Some(id);self.multi_selected=new_ids;
        self.active_node=None;self.node_drag=None;self.node_drag_checkpointed=false;
        self.fill_cache.entries.clear();
        self.compound_cache.clear();self.vector_cache.clear();self.bezier_preview.clear();
        self.status="Контуры разделены на отдельные закрытые кривые. Пересечения теперь заливаются независимо. Ctrl+Z — отмена".into();
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{CubicSegment,Point,Style};
    fn pt(x:f32,y:f32)->Point{Point{x,y}}
    fn shape(x:f32)->Shape{
        Shape::Bezier{start:pt(x,0.),segments:vec![
            CubicSegment{control1:pt(x+2.,0.),control2:pt(x+8.,0.),end:pt(x+10.,0.)},
            CubicSegment{control1:pt(x+10.,2.),control2:pt(x+10.,8.),end:pt(x+10.,10.)},
        ],closed:true}
    }
    #[test]
    fn combine_then_break_preserves_geometry_and_undo(){
        let mut e=Editor::default();e.doc=Document::default();
        let a=e.doc.add(shape(0.),Style::default());
        let b=e.doc.add(shape(20.),Style::default());
        let before=e.doc.clone();
        e.selected=Some(a);e.multi_selected=vec![b];
        e.combine_selected_contours();
        assert_eq!(e.doc.objects.len(),1);
        assert!(matches!(e.doc.objects[0].shape,Shape::CompoundPath{..}));
        e.doc.validate().unwrap();
        e.break_apart_selected_contour();
        assert_eq!(e.doc.objects.len(),2);
        assert!(e.doc.objects.iter().all(|o|matches!(o.shape,Shape::Bezier{..})));
        e.doc.validate().unwrap();
        e.undo();
        assert_eq!(e.doc.objects.len(),1);
        e.undo();
        assert_eq!(e.doc,before);
        e.redo();e.redo();
        e.doc.validate().unwrap();
    }
    #[test]
    fn combine_rejects_style_mismatch_without_checkpoint(){
        let mut e=Editor::default();e.doc=Document::default();
        let a=e.doc.add(shape(0.),Style::default());
        let mut alternate=Style::default();alternate.fill="#112233".into();
        let b=e.doc.add(shape(10.),alternate);
        e.selected=Some(a);e.multi_selected=vec![b];
        let before=e.doc.clone();
        e.combine_selected_contours();
        assert_eq!(e.doc,before);
        assert!(e.undo.is_empty());
    }
    #[test]
    fn break_rejects_locked_layer_without_checkpoint(){
        let mut e=Editor::default();e.doc=Document::default();
        let a=shape(0.);let b=shape(20.);
        let combined=compound_ops::combine_exact(&[&a,&b]).unwrap();
        let id=e.doc.add(combined,Style::default());
        e.doc.layers[0].locked=true;e.selected=Some(id);
        let before=e.doc.clone();
        e.break_apart_selected_contour();
        assert_eq!(e.doc,before);
        assert!(e.undo.is_empty());
    }
}
