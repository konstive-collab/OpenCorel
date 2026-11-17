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
    pub fn is_object_visible(&self,id:u64)->bool{
        self.objects.iter().find(|o|o.id==id).is_some_and(|o|
            o.visible&&self.layer(o.layer_id).is_some_and(|l|l.visible))
    }
    pub fn is_object_editable(&self,id:u64)->bool{
        self.objects.iter().find(|o|o.id==id).is_some_and(|o|
            o.visible&&!o.locked&&self.layer(o.layer_id)
                .is_some_and(|l|l.visible&&!l.locked))
    }
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
        if !self.layers.iter().any(|l|l.id==1){
            return Err("Основной слой отсутствует".into());
        }
        for object in &mut self.objects{
            if object.layer_id==id{object.layer_id=1;}
        }
        self.layers.remove(index);
        Ok(1)
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
