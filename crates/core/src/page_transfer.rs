//! Transfer complete editable groups and clip-linked objects between
//! independent pages, retaining styles and allocating collision-free IDs.
use crate::{Document,Layer,ClipLink};
use std::collections::{BTreeMap,HashSet};
impl Document{
    /// Copy or move selected objects onto an existing, inactive page.
    /// All group members and both ends of any clip links must be selected.
    /// The destination receives new IDs, including group IDs; original
    /// appearances and related layer names are preserved.
    pub fn transfer_objects_to_page(&mut self,ids:&[u64],
        destination:u64,remove_original:bool)->Result<Vec<u64>,String>{
        if destination==self.active_page_id{
            return Err("Select a different destination page".into());
        }
        if !self.pages().iter().any(|p|p.id==destination){
            return Err("Destination page not found".into());
        }
        let selected:HashSet<_>=ids.iter().copied().collect();
        if selected.is_empty(){return Err("No objects selected".into());}
        let source:Vec<_>=self.objects.iter().filter(|o|selected.contains(&o.id))
            .cloned().collect();
        if source.len()!=selected.len(){return Err("Selection contains unknown objects".into());}
        if source.iter().any(|o|self.layer(o.layer_id)
            .is_none_or(|l|l.locked||!l.visible)){
            return Err("Cannot transfer locked or hidden objects".into());
        }
        for object in &source{
            if object.group_id.is_some_and(|g|self.objects.iter().any(|other|
                other.group_id==Some(g)&&!selected.contains(&other.id))){
                return Err("Select entire groups before transferring".into());
            }
        }
        if self.clip_links.iter().any(|link|
            selected.contains(&link.source_id)!=selected.contains(&link.target_id)){
            return Err("Clipped objects must be transferred with their mask".into());
        }
        if !remove_original && self.all_object_count()+source.len()>100_000{
            return Err("Document-wide object limit exceeded".into());
        }
        let groups:HashSet<_>=source.iter().filter_map(|o|o.group_id).collect();
        let needed=source.len() as u64+groups.len() as u64;
        if self.next_id.checked_add(needed).is_none_or(|n|n==u64::MAX){
            return Err("Object identifier allocation exceeded".into());
        }
        let source_layers:Vec<Layer>=self.layers.clone();
        let clipped:Vec<ClipLink>=self.clip_links.iter().filter(|l|
            selected.contains(&l.source_id)&&selected.contains(&l.target_id))
            .cloned().collect();
        let appearances:Vec<_>=self.appearances.iter().filter(|(id,_)|
            selected.contains(id)).map(|(id,appearance)|
            (*id,appearance.clone())).collect();
        let mut ids_map=BTreeMap::new();
        let mut groups_map=BTreeMap::new();
        let mut copies=Vec::with_capacity(source.len());
        let mut inserted=Vec::new();
        {
        let target=self.inactive_pages.get_mut(&destination)
            .ok_or("Destination page data missing")?;
        for mut object in source{
            let old=object.id;
            object.id=self.next_id;
            self.next_id+=1;
            ids_map.insert(old,object.id);
            if let Some(group)=object.group_id{
                let next=if let Some(id)=groups_map.get(&group){*id}else{
                    let fresh=self.next_id;self.next_id+=1;
                    groups_map.insert(group,fresh);fresh
                };
                object.group_id=Some(next);
            }
            let layer=source_layers.iter().find(|l|l.id==object.layer_id)
                .ok_or("Original layer missing")?;
            let target_id=if let Some(existing)=target.layers.iter().find(|l|
                l.name==layer.name){
                if !existing.visible||existing.locked{
                    return Err("Target layer with the same name is not editable".into());
                }
                existing.id
            }else{
                if target.next_layer_id==u64::MAX{
                    return Err("Target layer identifier limit reached".into());
                }
                let new_id=target.next_layer_id;target.next_layer_id+=1;
                target.layers.push(Layer{id:new_id,name:layer.name.clone(),
                    visible:true,locked:false});
                new_id
            };
            object.layer_id=target_id;
            copies.push(object);
        }
        inserted=copies.iter().map(|o|o.id).collect();
        target.objects.extend(copies);
        for link in clipped{
            target.clip_links.push(ClipLink{
                source_id:ids_map[&link.source_id],
                target_id:ids_map[&link.target_id]
            });
        }
        for (id,appearance) in appearances{
            target.appearances.insert(ids_map[&id],appearance);
        }
        }
        let source_page_id=self.active_page_id;
        let symbol_copies:Vec<(u64,crate::SymbolInstance)>=self.symbol_instances.iter()
            .filter(|instance|instance.page_id==source_page_id&&
                instance.member_ids.iter().all(|id|selected.contains(id)))
            .map(|instance|{
                let group_id=groups_map.get(&instance.group_id).copied()
                    .ok_or_else(||"Symbol group ID was not remapped".to_owned())?;
                let member_ids=instance.member_ids.iter().map(|old|
                    ids_map.get(old).copied().ok_or_else(||"Symbol member ID was not remapped".to_owned()))
                    .collect::<Result<Vec<_>,_>>()?;
                Ok((instance.group_id,crate::SymbolInstance{
                    symbol_id:instance.symbol_id,page_id:destination,group_id,member_ids
                }))
            }).collect::<Result<_,String>>()?;
        if remove_original{
            for (old_group,new_instance) in symbol_copies{
                if let Some(instance)=self.symbol_instances.iter_mut().find(|item|
                    item.page_id==source_page_id&&item.group_id==old_group){
                    *instance=new_instance;
                }
            }
        }else{
            self.symbol_instances.extend(symbol_copies.into_iter().map(|(_,item)|item));
        }
        if remove_original{
            self.objects.retain(|o|!selected.contains(&o.id));
            self.clip_links.retain(|link|!selected.contains(&link.source_id)
                &&!selected.contains(&link.target_id));
            self.appearances.retain(|id,_|!selected.contains(id));
        }
        self.validate()?;
        Ok(inserted)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Page,Shape,Style};
    #[test] fn page_transfer_keeps_source_and_remaps_ids(){
        let mut d=Document::default();
        let a=d.add(Shape::Rectangle{x:2.,y:2.,w:20.,h:20.},Style::default());
        let b=d.add(Shape::Rectangle{x:3.,y:3.,w:4.,h:4.},Style::default());
        d.set_clip(a,b).unwrap();
        let target=d.add_page(Page{width_mm:210.,height_mm:297.}).unwrap();
        d.switch_page(1).unwrap();
        assert!(d.transfer_objects_to_page(&[a],target,false).is_err());
        let copied=d.transfer_objects_to_page(&[a,b],target,false).unwrap();
        assert_eq!(copied.len(),2);
        assert!(d.get(a).is_some());
        d.switch_page(target).unwrap();
        assert_eq!(d.objects.len(),2);
        assert_ne!(d.objects[0].id,a);
        assert_eq!(d.clip_links.len(),1);
        d.validate().unwrap();
    }
    #[test] fn copying_and_moving_symbol_instances_preserves_links(){
        let mut d=Document::default();
        let a=d.add(Shape::Rectangle{x:1.,y:2.,w:20.,h:20.},Style::default());
        let b=d.add(Shape::Ellipse{x:3.,y:4.,w:8.,h:8.},Style::default());
        let symbol=d.create_symbol(&[a,b],"Page symbol","Test").unwrap();
        let target=d.add_page(Page{width_mm:210.,height_mm:297.}).unwrap();
        d.switch_page(1).unwrap();
        let copied=d.transfer_objects_to_page(&[a,b],target,false).unwrap();
        assert_eq!(d.symbol_instances.len(),2);
        let copied_instance=d.symbol_instances.iter()
            .find(|item|item.page_id==target).unwrap();
        assert_eq!(copied_instance.symbol_id,symbol.id);
        assert_eq!(copied_instance.member_ids,copied);
        let moved=d.transfer_objects_to_page(&[a,b],target,true).unwrap();
        assert_eq!(d.symbol_instances.len(),2);
        assert!(d.symbol_instances.iter().all(|item|item.page_id==target));
        assert!(d.symbol_instances.iter().any(|item|item.member_ids==copied));
        assert!(d.symbol_instances.iter().any(|item|item.member_ids==moved));
        d.validate().unwrap();
    }
    #[test] fn move_group_to_page_is_reversible_by_document_snapshot(){
        let mut d=Document::default();
        let ids:Vec<_>=(0..2).map(|i|d.add(Shape::Ellipse{
            x:i as f32*10.,y:0.,w:5.,h:5.},Style::default())).collect();
        d.group_objects(&ids).unwrap();
        let target=d.add_page(Page{width_mm:300.,height_mm:200.}).unwrap();
        d.switch_page(1).unwrap();
        assert!(d.transfer_objects_to_page(&[ids[0]],target,true).is_err());
        let previous=d.clone();
        d.transfer_objects_to_page(&ids,target,true).unwrap();
        assert!(d.objects.is_empty());
        d.switch_page(target).unwrap();
        assert_eq!(d.objects.len(),2);
        assert_eq!(d.objects[0].group_id,d.objects[1].group_id);
        d.validate().unwrap();
        let mut before=previous;
        before.validate().unwrap();
        assert_eq!(before.page_index(),0);
    }
}
