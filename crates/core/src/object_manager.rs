//! Persistent layer and object-manager operations.
//!
//! All multi-object changes validate completely before mutating the document.
//! IDs are never reused; group and clipping relationships are remapped only
//! when every member is part of the duplicated layer.
use crate::{ClipLink,Document,Layer,Object};
use std::collections::{HashMap,HashSet};

const MAX_LAYERS:usize=1024;
const MAX_OBJECTS:usize=100_000;
const MAX_CLIPS:usize=10_000;
const MAX_LABEL_BYTES:usize=256;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ObjectStackMove{Forward,Backward,Front,Back}

fn bounded_copy_name(name:&str)->String{
    let mut value=format!("Копия — {name}");
    if value.len()>MAX_LABEL_BYTES{
        let mut end=MAX_LABEL_BYTES;
        while !value.is_char_boundary(end){end-=1;}
        value.truncate(end);
    }
    value
}

impl Document{
    pub fn object_is_visible(&self,object:&Object)->bool{
        object.visible&&self.layer(object.layer_id).is_some_and(|l|l.visible)
    }
    pub fn is_object_visible(&self,id:u64)->bool{
        self.objects.iter().find(|o|o.id==id)
            .is_some_and(|o|self.object_is_visible(o))
    }
    pub fn object_is_editable(&self,object:&Object)->bool{
        object.visible&&!object.locked&&self.layer(object.layer_id)
            .is_some_and(|l|l.visible&&!l.locked)
    }
    pub fn is_object_editable(&self,id:u64)->bool{
        self.objects.iter().find(|o|o.id==id)
            .is_some_and(|o|self.object_is_editable(o))
    }
    /// Return IDs that can be edited by the current canvas selection model.
    pub fn editable_object_ids(&self)->HashSet<u64>{
        let editable_layers:HashSet<u64>=self.layers.iter()
            .filter(|layer|layer.visible&&!layer.locked)
            .map(|layer|layer.id).collect();
        self.objects.iter().filter(|o|
            o.visible&&!o.locked&&editable_layers.contains(&o.layer_id))
            .map(|o|o.id).collect()
    }
    /// Reorder selected objects inside one layer. Selected objects retain
    /// their relative order; whole groups move together. Layer stack order
    /// remains the primary cross-layer stacking mechanism.
    pub fn reorder_objects_in_layer(&mut self,ids:&[u64],movement:ObjectStackMove)
        ->Result<usize,String>{
        let wanted:HashSet<u64>=ids.iter().copied().collect();
        if wanted.is_empty(){return Err("Нет выбранных объектов".into());}
        let chosen:Vec<&Object>=self.objects.iter()
            .filter(|o|wanted.contains(&o.id)).collect();
        if chosen.len()!=wanted.len(){
            return Err("Выделение содержит отсутствующие объекты".into());
        }
        if chosen.iter().any(|o|!self.object_is_editable(o)){
            return Err("Порядок нельзя менять у скрытых или заблокированных объектов".into());
        }
        let layer_id=chosen[0].layer_id;
        if chosen.iter().any(|o|o.layer_id!=layer_id){
            return Err("Сначала переместите выделение в один слой".into());
        }
        let groups:HashSet<u64>=chosen.iter().filter_map(|o|o.group_id).collect();
        for group in groups{
            if self.objects.iter().filter(|o|o.group_id==Some(group))
                .any(|o|!wanted.contains(&o.id)){
                return Err("Для изменения порядка выберите группу целиком".into());
            }
        }
        let mut order:Vec<u64>=self.objects.iter().filter(|o|o.layer_id==layer_id)
            .map(|o|o.id).collect();
        let before=order.clone();
        match movement{
            ObjectStackMove::Front=>{
                let mut selected:Vec<u64>=order.iter().copied()
                    .filter(|id|wanted.contains(id)).collect();
                order.retain(|id|!wanted.contains(id));
                order.append(&mut selected);
            }
            ObjectStackMove::Back=>{
                let mut selected:Vec<u64>=order.iter().copied()
                    .filter(|id|wanted.contains(id)).collect();
                order.retain(|id|!wanted.contains(id));
                selected.extend(order);
                order=selected;
            }
            ObjectStackMove::Forward=>{
                if order.len()>1{
                    for i in (0..order.len()-1).rev(){
                        if wanted.contains(&order[i])&&!wanted.contains(&order[i+1]){
                            order.swap(i,i+1);
                        }
                    }
                }
            }
            ObjectStackMove::Backward=>{
                for i in 1..order.len(){
                    if wanted.contains(&order[i])&&!wanted.contains(&order[i-1]){
                        order.swap(i,i-1);
                    }
                }
            }
        }
        if order==before{return Ok(0);}
        let mut layer_orders:HashMap<u64,Vec<u64>>=HashMap::new();
        layer_orders.insert(layer_id,order);
        for layer in &self.layers{
            if layer.id==layer_id{continue;}
            layer_orders.insert(layer.id,self.objects.iter()
                .filter(|o|o.layer_id==layer.id).map(|o|o.id).collect());
        }
        let mut by_id:HashMap<u64,Object>=std::mem::take(&mut self.objects)
            .into_iter().map(|o|(o.id,o)).collect();
        let mut reordered=Vec::with_capacity(by_id.len());
        for layer in &self.layers{
            if let Some(ids)=layer_orders.get(&layer.id){
                for id in ids{
                    if let Some(object)=by_id.remove(id){reordered.push(object);}
                }
            }
        }
        reordered.extend(by_id.into_values());
        self.objects=reordered;
        Ok(wanted.len())
    }
    /// Rename a layer without changing object references or z-order.
    pub fn rename_layer(&mut self,id:u64,name:&str)->Result<(),String>{
        if name.trim().is_empty()||name.len()>MAX_LABEL_BYTES||
            name.chars().any(char::is_control)||!crate::valid_xml_text(name){
            return Err("Название слоя должно содержать 1–256 печатных байт".into());
        }
        let layer=self.layers.iter_mut().find(|l|l.id==id)
            .ok_or("Слой не найден")?;
        if layer.locked{return Err("Сначала разблокируйте слой".into());}
        if layer.name==name{return Ok(());}
        layer.name=name.to_owned();
        Ok(())
    }
    pub fn move_layer_to(&mut self,id:u64,to:usize)->Result<(),String>{
        if to>=self.layers.len(){return Err("Позиция слоя вне диапазона".into());}
        let from=self.layers.iter().position(|l|l.id==id)
            .ok_or("Слой не найден")?;
        if self.layers[from].locked{return Err("Сначала разблокируйте слой".into());}
        if from==to{return Ok(());}
        let layer=self.layers.remove(from);
        self.layers.insert(to,layer);
        Ok(())
    }
    pub fn delete_layer(&mut self,id:u64)->Result<u64,String>{
        if id==1{return Err("Основной слой нельзя удалить".into());}
        if self.layers.len()<=1{return Err("Нельзя удалить последний слой".into());}
        let index=self.layers.iter().position(|l|l.id==id)
            .ok_or("Слой не найден")?;
        if self.layers[index].locked{return Err("Сначала разблокируйте слой".into());}
        let fallback=self.layers.iter()
            .find(|l|l.id!=id&&l.visible&&!l.locked)
            .or_else(||self.layers.iter().find(|l|l.id!=id&&!l.locked))
            .map(|l|l.id)
            .ok_or("Нужен хотя бы один разблокированный слой для переноса объектов")?;
        for object in &mut self.objects{
            if object.layer_id==id{object.layer_id=fallback;}
        }
        self.layers.remove(index);
        Ok(fallback)
    }
    pub fn duplicate_layer(&mut self,id:u64)->Result<u64,String>{
        let source=self.layer(id).cloned().ok_or("Слой не найден")?;
        if source.locked{return Err("Нельзя дублировать заблокированный слой".into());}
        if self.layers.len()>=MAX_LAYERS||self.next_layer_id==u64::MAX{
            return Err("Достигнут лимит слоёв".into());
        }
        let originals:Vec<Object>=self.objects.iter()
            .filter(|o|o.layer_id==id).cloned().collect();
        if self.objects.len().checked_add(originals.len())
            .is_none_or(|n|n>MAX_OBJECTS){
            return Err("Дублирование превысит лимит 100 000 объектов".into());
        }
        let source_groups:HashSet<u64>=originals.iter()
            .filter_map(|o|o.group_id).collect();
        let complete_groups:HashSet<u64>=source_groups.into_iter().filter(|group|
            self.objects.iter().filter(|o|o.group_id==Some(*group))
                .all(|o|o.layer_id==id)).collect();
        let internal_clips:Vec<ClipLink>=self.clip_links.iter().filter_map(|link|{
            let source_inside=originals.iter().any(|o|o.id==link.source_id);
            let target_inside=originals.iter().any(|o|o.id==link.target_id);
            (source_inside&&target_inside).then_some(link.clone())
        }).collect();
        if self.clip_links.len().checked_add(internal_clips.len())
            .is_none_or(|n|n>MAX_CLIPS){
            return Err("Дублирование превысит лимит связей обтравки".into());
        }
        let needed=originals.len().checked_add(complete_groups.len())
            .ok_or("Переполнение счётчика объектов")?;
        let end=self.next_id.checked_add(needed as u64)
            .filter(|n|*n<u64::MAX)
            .ok_or("Недостаточно свободных идентификаторов объектов")?;
        let new_layer_id=self.next_layer_id;
        let mut new_layer=Layer{
            id:new_layer_id,name:bounded_copy_name(&source.name),
            visible:true,locked:false,
        };
        if new_layer.name==source.name{new_layer.name="Копия слоя".into();}
        let mut cursor=self.next_id;
        let mut id_map=HashMap::<u64,u64>::with_capacity(originals.len());
        let mut copies=Vec::with_capacity(originals.len());
        for original in &originals{
            let new_id=cursor;cursor+=1;
            let mut copy=original.clone();
            copy.id=new_id;copy.layer_id=new_layer_id;
            if !copy.group_id.is_some_and(|g|complete_groups.contains(&g)){
                copy.group_id=None;
            }
            id_map.insert(original.id,new_id);
            copies.push((original.id,copy));
        }
        let mut group_map=HashMap::<u64,u64>::with_capacity(complete_groups.len());
        for group in complete_groups{
            group_map.insert(group,cursor);cursor+=1;
        }
        for (_,copy) in &mut copies{
            if let Some(group)=copy.group_id{copy.group_id=group_map.get(&group).copied();}
        }
        let copied_appearances:Vec<_>=copies.iter().filter_map(|(old_id,new_obj)|
            self.appearances.get(old_id).map(|style|(new_obj.id,style.clone()))
        ).collect();
        let copied_clips:Vec<ClipLink>=internal_clips.iter().filter_map(|link|{
            Some(ClipLink{
                source_id:*id_map.get(&link.source_id)?,
                target_id:*id_map.get(&link.target_id)?,
            })
        }).collect();
        self.layers.push(new_layer);
        self.next_layer_id+=1;
        self.next_id=end;
        self.objects.extend(copies.into_iter().map(|(_,object)|object));
        self.appearances.extend(copied_appearances);
        self.clip_links.extend(copied_clips);
        Ok(new_layer_id)
    }
    pub fn move_objects_to_layer(&mut self,ids:&[u64],target:u64)
        ->Result<usize,String>{
        let wanted:HashSet<u64>=ids.iter().copied().collect();
        if wanted.is_empty(){return Err("Нет выбранных объектов".into());}
        let layer=self.layer(target).ok_or("Целевой слой не найден")?;
        if !layer.visible||layer.locked{
            return Err("Целевой слой должен быть видимым и разблокированным".into());
        }
        let chosen:Vec<&Object>=self.objects.iter()
            .filter(|o|wanted.contains(&o.id)).collect();
        if chosen.len()!=wanted.len(){
            return Err("Выделение содержит отсутствующие объекты".into());
        }
        if chosen.iter().any(|o|!self.is_object_editable(o.id)){
            return Err("Выделение содержит скрытый или заблокированный объект/слой".into());
        }
        let groups:HashSet<u64>=chosen.iter().filter_map(|o|o.group_id).collect();
        for group in groups{
            if self.objects.iter().filter(|o|o.group_id==Some(group))
                .any(|o|!wanted.contains(&o.id)){
                return Err("Перемещение группы требует выделить все её элементы".into());
            }
        }
        let count=chosen.len();
        for object in &mut self.objects{
            if wanted.contains(&object.id){object.layer_id=target;}
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Shape,Style,Point};
    #[test]
    fn layer_stack_controls_paint_order_and_hit_testing(){
        let mut doc=Document::default();
        let base=doc.add(Shape::Rectangle{x:0.,y:0.,w:30.,h:30.},Style::default());
        let top_layer=doc.add_layer("Top".into());
        let top=doc.add_to_layer(Shape::Ellipse{x:0.,y:0.,w:30.,h:30.},Style::default(),top_layer);
        assert_eq!(doc.hit_test(Point{x:15.,y:15.},0.),Some(top));
        assert_eq!(doc.visible_objects().map(|o|o.id).collect::<Vec<_>>(),vec![base,top]);
        doc.move_layer_to(top_layer,0).unwrap();
        assert_eq!(doc.hit_test(Point{x:15.,y:15.},0.),Some(base));
        assert_eq!(doc.visible_objects().map(|o|o.id).collect::<Vec<_>>(),vec![top,base]);
    }
    #[test]
    fn object_stack_moves_one_step_and_preserves_selection_order(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Rectangle{x:0.,y:0.,w:5.,h:5.},Style::default());
        let b=doc.add(Shape::Rectangle{x:10.,y:0.,w:5.,h:5.},Style::default());
        let c=doc.add(Shape::Rectangle{x:20.,y:0.,w:5.,h:5.},Style::default());
        assert_eq!(doc.reorder_objects_in_layer(&[a],ObjectStackMove::Forward).unwrap(),1);
        assert_eq!(doc.objects.iter().map(|o|o.id).collect::<Vec<_>>(),vec![b,a,c]);
        doc.reorder_objects_in_layer(&[a,c],ObjectStackMove::Back).unwrap();
        assert_eq!(doc.objects.iter().map(|o|o.id).collect::<Vec<_>>(),vec![a,c,b]);
        doc.validate().unwrap();
    }
    #[test]
    fn legacy_documents_default_object_flags_to_visible_and_unlocked(){
        let mut doc=Document::default();
        let id=doc.add(Shape::Rectangle{x:1.,y:2.,w:10.,h:10.},Style::default());
        let mut value=serde_json::to_value(&doc).unwrap();
        let object=value["objects"].as_array_mut().unwrap().iter_mut()
            .find(|o|o["id"]==id).unwrap();
        object.as_object_mut().unwrap().remove("visible");
        object.as_object_mut().unwrap().remove("locked");
        let restored:Document=serde_json::from_value(value).unwrap();
        let item=restored.objects.iter().find(|o|o.id==id).unwrap();
        assert!(item.visible&&!item.locked);
        assert!(restored.is_object_editable(id));
    }
    #[test]
    fn hidden_objects_do_not_render_export_or_hit_test_but_can_be_restored(){
        let mut doc=Document::default();
        let id=doc.add(Shape::Rectangle{x:0.,y:0.,w:20.,h:20.},Style::default());
        assert_eq!(doc.hit_test(Point{x:10.,y:10.},0.),Some(id));
        doc.get_mut(id).unwrap().visible=false;
        assert_eq!(doc.hit_test(Point{x:10.,y:10.},0.),None);
        assert!(!doc.to_svg().contains("<rect"));
        assert!(!doc.is_object_visible(id));
        doc.get_mut(id).unwrap().visible=true;
        doc.get_mut(id).unwrap().locked=true;
        assert_eq!(doc.hit_test(Point{x:10.,y:10.},0.),None);
        assert!(doc.to_svg().contains("<rect"));
        assert!(!doc.is_object_editable(id));
        doc.validate().unwrap();
    }
    #[test]
    fn group_and_layout_commands_reject_object_level_locks(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Rectangle{x:0.,y:0.,w:10.,h:10.},Style::default());
        let b=doc.add(Shape::Rectangle{x:20.,y:0.,w:10.,h:10.},Style::default());
        doc.get_mut(b).unwrap().locked=true;
        assert!(doc.group_objects(&[a,b]).is_err());
        assert_eq!(doc.align_objects(&[a,b],crate::Alignment::Left),0);
        assert_eq!(doc.distribute_objects(&[a,b,a],crate::Distribution::HorizontalCenters),0);
    }
    #[test]
    fn layer_duplicate_preserves_groups_objects_and_internal_clipping(){
        let mut doc=Document::default();
        let layer=doc.add_layer("Artwork".into());
        let source=doc.add_to_layer(Shape::Rectangle{x:0.,y:0.,w:20.,h:20.},Style::default(),layer);
        let target=doc.add_to_layer(Shape::Ellipse{x:0.,y:0.,w:20.,h:20.},Style::default(),layer);
        doc.set_clip(source,target).unwrap();
        let copy_layer=doc.duplicate_layer(layer).unwrap();
        let copied:Vec<_>=doc.objects.iter().filter(|o|o.layer_id==copy_layer).collect();
        assert_eq!(copied.len(),2);
        assert_ne!(copied[0].id,source);
        assert_ne!(copied[1].id,target);
        assert_eq!(doc.clip_links.len(),2);
        let copied_source=copied.iter().find(|o|matches!(o.shape,Shape::Rectangle{..})).unwrap().id;
        let copied_target=copied.iter().find(|o|matches!(o.shape,Shape::Ellipse{..})).unwrap().id;
        assert!(doc.clip_links.iter().any(|c|c.source_id==copied_source&&c.target_id==copied_target));
        doc.validate().unwrap();
        assert!(doc.layers.iter().any(|l|l.id==copy_layer));
    }
    #[test]
    fn duplicate_layer_remaps_only_complete_groups(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Rectangle{x:0.,y:0.,w:10.,h:10.},Style::default());
        let b=doc.add(Shape::Ellipse{x:20.,y:0.,w:10.,h:10.},Style::default());
        let group=doc.group_objects(&[a,b]).unwrap();
        let copy=doc.duplicate_layer(1).unwrap();
        let copies:Vec<_>=doc.objects.iter().filter(|o|o.layer_id==copy).collect();
        assert_eq!(copies.len(),2);
        assert!(copies.iter().all(|o|o.group_id.is_some()));
        assert_ne!(copies[0].group_id,Some(group));
        doc.validate().unwrap();
    }
    #[test]
    fn move_selection_requires_full_groups_and_preserves_object_ids(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Rectangle{x:0.,y:0.,w:10.,h:10.},Style::default());
        let b=doc.add(Shape::Ellipse{x:20.,y:0.,w:10.,h:10.},Style::default());
        doc.group_objects(&[a,b]).unwrap();
        let target=doc.add_layer("Destination".into());
        assert!(doc.move_objects_to_layer(&[a],target).is_err());
        assert_eq!(doc.objects.iter().find(|o|o.id==a).unwrap().layer_id,1);
        assert_eq!(doc.move_objects_to_layer(&[a,b],target).unwrap(),2);
        assert!(doc.objects.iter().filter(|o|o.id==a||o.id==b)
            .all(|o|o.layer_id==target));
        doc.validate().unwrap();
    }
    #[test]
    fn layer_rename_reorder_and_delete_have_safe_guards(){
        let mut doc=Document::default();
        let a=doc.add_layer("Second".into());
        let b=doc.add_layer("Third".into());
        assert!(doc.rename_layer(a,"  ").is_err());
        doc.rename_layer(a,"Слой логотипа").unwrap();
        doc.move_layer_to(b,0).unwrap();
        assert_eq!(doc.layers[0].id,b);
        let id=doc.add_to_layer(Shape::Rectangle{x:1.,y:1.,w:2.,h:2.},Style::default(),a);
        assert!(doc.delete_layer(1).is_err());
        assert_eq!(doc.delete_layer(a).unwrap(),1);
        assert_eq!(doc.objects.iter().find(|o|o.id==id).unwrap().layer_id,1);
        assert!(doc.layer(a).is_none());
        doc.validate().unwrap();
    }
    #[test]
    fn object_visibility_and_lock_flags_survive_native_kdr_roundtrip(){
        let mut doc=Document::default();
        let id=doc.add(Shape::Rectangle{x:1.,y:2.,w:10.,h:10.},Style::default());
        let object=doc.get_mut(id).unwrap();
        object.visible=false;object.locked=true;
        let dir=tempfile::tempdir().unwrap();
        let path=dir.path().join("object-state.kdr");
        doc.write(&path).unwrap();
        let restored=Document::read(&path).unwrap();
        let object=restored.objects.iter().find(|o|o.id==id).unwrap();
        assert!(!object.visible&&object.locked);
        restored.validate().unwrap();
    }
    #[test]
    fn deleting_layer_uses_unlocked_fallback_if_primary_is_locked(){
        let mut doc=Document::default();
        let source=doc.add_layer("Source".into());
        let fallback=doc.add_layer("Fallback".into());
        let object=doc.add_to_layer(Shape::Rectangle{x:1.,y:1.,w:5.,h:5.},Style::default(),source);
        doc.layers.iter_mut().find(|l|l.id==1).unwrap().locked=true;
        assert_eq!(doc.delete_layer(source).unwrap(),fallback);
        assert_eq!(doc.objects.iter().find(|o|o.id==object).unwrap().layer_id,fallback);
        doc.validate().unwrap();
    }
    #[test]
    fn layer_mutations_reject_locked_layers_without_partial_changes(){
        let mut doc=Document::default();
        let id=doc.add_layer("Locked".into());
        doc.layers.iter_mut().find(|l|l.id==id).unwrap().locked=true;
        let before=doc.clone();
        assert!(doc.rename_layer(id,"New").is_err());
        assert!(doc.move_layer_to(id,0).is_err());
        assert!(doc.delete_layer(id).is_err());
        assert!(doc.duplicate_layer(id).is_err());
        assert_eq!(doc,before);
    }
}
