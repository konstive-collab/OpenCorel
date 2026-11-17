//! Object Studio command layer. Mutations are atomic, bounded and undoable.
use super::Editor;
use opencorel_core::object_studio::StackOrder;
use std::collections::HashSet;
impl Editor{
    pub(super) fn object_visibility(&mut self,id:u64,visible:bool){
        let Some(obj)=self.doc.objects.iter().find(|o|o.id==id)else{return;};
        if obj.visible==visible{return;}
        self.checkpoint();
        if let Err(e)=self.doc.set_object_visible(id,visible){
            self.undo.pop();self.status=e;return;
        }
        if !visible{
            self.selected=None;self.multi_selected.clear();
            self.active_node=None;self.node_drag=None;
        }
        self.clear_object_studio_caches();
        self.status=if visible{"Объект показан"}else{"Объект скрыт"}.into();
    }
    pub(super) fn object_lock(&mut self,id:u64,locked:bool){
        let Some(obj)=self.doc.objects.iter().find(|o|o.id==id)else{return;};
        if obj.locked==locked{return;}
        self.checkpoint();
        if let Err(e)=self.doc.set_object_locked(id,locked){
            self.undo.pop();self.status=e;return;
        }
        if locked&&self.selected==Some(id){
            self.active_node=None;self.node_drag=None;
        }
        self.status=if locked{"Объект заблокирован"}else{"Объект разблокирован"}.into();
    }
    pub(super) fn object_duplicate_selection(&mut self){
        let ids=self.selection_ids();
        if ids.is_empty(){self.status="Выберите объекты для дублирования".into();return;}
        let mut candidate=self.doc.clone();
        match candidate.duplicate_objects(&ids,5.0,5.0){
            Ok(new_ids)=>{
                self.checkpoint();
                self.doc=candidate;
                self.selected=new_ids.first().copied();
                self.multi_selected=new_ids.into_iter().skip(1).collect();
                self.active_node=None;self.node_drag=None;
                self.clear_object_studio_caches();
                self.status=format!("Создано копий: {}. Сохранены группы, стили и оформление. Ctrl+Z — отмена.",
                    self.selection_ids().len());
            },
            Err(e)=>self.status=format!("Дублирование отменено: {e}"),
        }
    }
    pub(super) fn object_reorder_selection(&mut self,order:StackOrder){
        let ids=self.selection_ids();
        if ids.is_empty(){return;}
        let mut candidate=self.doc.clone();
        match candidate.reorder_objects(&ids,order){
            Ok(changed) if candidate.objects!=self.doc.objects=>{
                self.checkpoint();self.doc=candidate;
                self.status=format!("Порядок объектов обновлён ({changed} позиций)");
            },
            Ok(_)=>self.status="Выбранные объекты уже находятся в этом порядке".into(),
            Err(e)=>self.status=format!("Порядок не изменён: {e}"),
        }
    }
    pub(super) fn object_move_to_active_layer(&mut self){
        let ids=self.selection_ids();
        if ids.is_empty(){return;}
        let mut candidate=self.doc.clone();
        match candidate.move_objects_to_layer(&ids,self.active_layer){
            Ok(0)=>self.status="Объекты уже находятся на активном слое".into(),
            Ok(count)=>{
                self.checkpoint();self.doc=candidate;
                self.status=format!("Перемещено объектов на активный слой: {count}");
            },
            Err(e)=>self.status=format!("Перемещение на слой отменено: {e}"),
        }
    }
    pub(super) fn object_delete_selection(&mut self){
        let ids:HashSet<u64>=self.selection_ids().into_iter().collect();
        if ids.is_empty(){return;}
        let mut candidate=self.doc.clone();
        if candidate.objects.iter().any(|o|ids.contains(&o.id)&&
            (!o.visible||o.locked||!candidate.layer(o.layer_id)
                .is_some_and(|l|l.visible&&!l.locked))){
            self.status="Удаление отменено: в выделении есть скрытый или заблокированный объект".into();
            return;
        }
        candidate.objects.retain(|o|!ids.contains(&o.id));
        candidate.clip_links.retain(|c|
            !ids.contains(&c.source_id)&&!ids.contains(&c.target_id));
        candidate.appearances.retain(|id,_|!ids.contains(id));
        if let Err(e)=candidate.validate(){
            self.status=format!("Удаление отменено: {e}");return;
        }
        self.checkpoint();self.doc=candidate;
        self.selected=None;self.multi_selected.clear();
        self.active_node=None;self.node_drag=None;
        self.clear_object_studio_caches();
        self.status=format!("Удалено объектов: {}",ids.len());
    }
    pub(super) fn object_select_same_type(&mut self){
        let Some(id)=self.selected else{return;};
        let Some(source)=self.doc.objects.iter().find(|o|o.id==id)else{return;};
        let same=|a:&opencorel_core::Shape,b:&opencorel_core::Shape|{
            use opencorel_core::Shape::*;
            matches!((a,b),
                (Rectangle{..},Rectangle{..})|(Ellipse{..},Ellipse{..})|
                (Text{..},Text{..})|(Polyline{..},Polyline{..})|
                (Bezier{..},Bezier{..})|(CompoundPath{..},CompoundPath{..})|
                (EmbeddedSvg{..},EmbeddedSvg{..})|(Raster{..},Raster{..}))
        };
        let ids:Vec<u64>=self.doc.objects.iter().filter(|o|
            o.visible&&!o.locked&&self.doc.layer(o.layer_id)
                .is_some_and(|l|l.visible&&!l.locked)&&same(&source.shape,&o.shape))
            .map(|o|o.id).collect();
        self.selected=ids.first().copied();
        self.multi_selected=ids.into_iter().skip(1).collect();
        self.status=format!("Выбраны объекты того же типа: {}",self.selection_ids().len());
    }
    pub(super) fn object_rename_layer(&mut self,id:u64,name:&str){
        if self.doc.layer(id).is_none(){return;}
        if self.doc.layer(id).is_some_and(|l|l.name==name){return;}
        let mut candidate=self.doc.clone();
        match candidate.rename_layer(id,name){
            Ok(true)=>{
                self.checkpoint();self.doc=candidate;
                self.status="Слой переименован".into();
            },
            Ok(false)=>{},
            Err(e)=>self.status=format!("Имя слоя не изменено: {e}"),
        }
    }
    pub(super) fn object_move_layer(&mut self,id:u64,to:usize){
        let mut candidate=self.doc.clone();
        match candidate.move_layer(id,to){
            Ok(true)=>{self.checkpoint();self.doc=candidate;},
            Ok(false)=>{},
            Err(e)=>self.status=format!("Порядок слоёв не изменён: {e}"),
        }
    }
    pub(super) fn object_remove_layer(&mut self,id:u64){
        if id==self.active_layer{
            self.status="Выберите другой активный слой для переноса объектов".into();
            return;
        }
        let mut candidate=self.doc.clone();
        match candidate.remove_layer_into(id,self.active_layer){
            Ok(count)=>{
                self.checkpoint();self.doc=candidate;
                self.status=format!("Слой удалён; перенесено объектов: {count}");
            },
            Err(e)=>self.status=format!("Слой не удалён: {e}"),
        }
    }
    fn clear_object_studio_caches(&mut self){
        self.fill_cache.entries.clear();
        self.vector_cache.clear();self.compound_cache.clear();
        self.bezier_preview.clear();self.raster_cache.clear();
        self.text_preview.clear();
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Document,Shape,Style,object_studio::StackOrder};
    #[test]
    fn duplicate_is_one_atomic_undo_step_with_selection_on_copies(){
        let mut e=Editor::default();e.doc=Document::default();
        let a=e.doc.add(Shape::Rectangle{x:0.,y:0.,w:10.,h:10.},Style::default());
        let b=e.doc.add(Shape::Ellipse{x:20.,y:0.,w:10.,h:10.},Style::default());
        e.selected=Some(a);e.multi_selected=vec![b];
        e.object_duplicate_selection();
        assert_eq!(e.doc.objects.len(),4);
        assert_eq!(e.undo.len(),1);
        assert_eq!(e.selection_ids().len(),2);
        assert!(e.doc.objects[2].bounds().x>=5.);
        e.undo();assert_eq!(e.doc.objects.len(),2);
    }
    #[test]
    fn lock_visibility_and_order_use_document_history(){
        let mut e=Editor::default();e.doc=Document::default();
        let a=e.doc.add(Shape::Rectangle{x:0.,y:0.,w:10.,h:10.},Style::default());
        let b=e.doc.add(Shape::Ellipse{x:20.,y:0.,w:10.,h:10.},Style::default());
        e.object_lock(a,true);
        assert!(e.doc.get_mut(a).unwrap().locked);
        assert_eq!(e.undo.len(),1);
        e.object_visibility(b,false);
        assert!(!e.doc.get_mut(b).unwrap().visible);
        assert_eq!(e.undo.len(),2);
        e.selected=Some(a);e.multi_selected.clear();
        e.object_reorder_selection(StackOrder::Back);
        assert_eq!(e.undo.len(),2);
        e.undo();assert!(e.doc.get_mut(b).unwrap().visible);
    }
    #[test]
    fn move_selection_to_layer_and_delete_are_atomic(){
        let mut e=Editor::default();e.doc=Document::default();
        let a=e.doc.add(Shape::Rectangle{x:0.,y:0.,w:10.,h:10.},Style::default());
        let layer=e.doc.add_layer("Second".into());
        e.active_layer=layer;e.selected=Some(a);
        e.object_move_to_active_layer();
        assert_eq!(e.doc.get_mut(a).unwrap().layer_id,layer);
        assert_eq!(e.undo.len(),1);
        e.object_delete_selection();
        assert!(e.doc.objects.is_empty());
        assert_eq!(e.undo.len(),2);
        e.undo();assert!(e.doc.get_mut(a).is_some());
    }
}
