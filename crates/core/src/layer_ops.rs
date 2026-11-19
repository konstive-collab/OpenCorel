//! Safe layer and z-order editing, preserving object identity and edit locks.
use crate::Document;
use std::collections::HashSet;
impl Document {
    pub fn rename_layer(&mut self,id:u64,name:&str)->Result<(),String>{
        if name.trim().is_empty()||name.len()>128||
            name.chars().any(|c|c.is_control()){
            return Err("Layer name must be 1–128 printable bytes".into());
        }
        let layer=self.layers.iter_mut().find(|l|l.id==id)
            .ok_or("Unknown layer")?;
        layer.name=name.into();
        Ok(())
    }
    pub fn delete_empty_layer(&mut self,id:u64)->Result<(),String>{
        if id==1{return Err("Primary layer cannot be deleted".into());}
        if self.objects.iter().any(|o|o.layer_id==id){
            return Err("Cannot delete a non-empty layer".into());
        }
        let index=self.layers.iter().position(|l|l.id==id)
            .ok_or("Unknown layer")?;
        self.layers.remove(index);
        Ok(())
    }
    pub fn move_to_layer(&mut self,ids:&[u64],target:u64)->Result<usize,String>{
        let new_layer=self.layer(target).ok_or("Destination layer missing")?;
        if new_layer.locked||!new_layer.visible{
            return Err("Destination layer must be visible and unlocked".into());
        }
        let chosen:HashSet<_>=ids.iter().copied().collect();
        if chosen.is_empty(){return Err("Nothing selected".into());}
        let source:Vec<_>=self.objects.iter().filter(|o|chosen.contains(&o.id)).collect();
        if source.len()!=chosen.len()||source.iter().any(|o|self.layer(o.layer_id)
            .is_none_or(|l|l.locked||!l.visible)){
            return Err("Selection includes missing or locked objects".into());
        }
        if source.iter().any(|o|o.group_id.is_some_and(|group|
            self.objects.iter().any(|other|
                other.group_id==Some(group)&&!chosen.contains(&other.id)))){
            return Err("Select every member of a group before moving it".into());
        }
        let changed=source.iter().filter(|o|o.layer_id!=target).count();
        for obj in &mut self.objects{
            if chosen.contains(&obj.id){obj.layer_id=target;}
        }
        Ok(changed)
    }
    /// Shift selected objects by one z-position while retaining the relative
    /// order of a contiguous selection. The caller handles Undo checkpoints.
    pub fn step_z_order(&mut self,ids:&[u64],forward:bool)->Result<usize,String>{
        let selected:HashSet<_>=ids.iter().copied().collect();
        if selected.is_empty(){return Err("Nothing selected".into());}
        if self.objects.iter().filter(|o|selected.contains(&o.id)).count()!=selected.len()
            || self.objects.iter().any(|o|selected.contains(&o.id)
                &&self.layer(o.layer_id).is_none_or(|l|l.locked||!l.visible)){
            return Err("Selection contains missing or locked objects".into());
        }
        let mut moves=0;
        if forward {
            for i in (1..self.objects.len()).rev(){
                if selected.contains(&self.objects[i-1].id)
                    &&!selected.contains(&self.objects[i].id){
                    self.objects.swap(i-1,i);moves+=1;
                }
            }
        }else{
            for i in 1..self.objects.len(){
                if selected.contains(&self.objects[i].id)
                    &&!selected.contains(&self.objects[i-1].id){
                    self.objects.swap(i-1,i);moves+=1;
                }
            }
        }
        Ok(moves)
    }
    pub fn reorder_layer(&mut self,id:u64,up:bool)->Result<bool,String>{
        let index=self.layers.iter().position(|l|l.id==id).ok_or("Unknown layer")?;
        let target=if up{index.saturating_add(1)}else{index.saturating_sub(1)};
        if target>=self.layers.len()||target==index{return Ok(false);}
        self.layers.swap(index,target);
        // Arrange visually by layer order. Stable sort retains each layer's
        // internal stacking order, never changes object IDs or group links.
        let rank:std::collections::HashMap<_,_>=self.layers.iter().enumerate()
            .map(|(i,l)|(l.id,i)).collect();
        self.objects.sort_by_key(|o|rank.get(&o.layer_id).copied().unwrap_or(0));
        Ok(true)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Shape,Style};
    #[test] fn layer_lifecycle_and_group_membership(){
        let mut d=Document::default();
        let a=d.add(Shape::Rectangle{x:1.,y:1.,w:2.,h:2.},Style::default());
        let b=d.add(Shape::Rectangle{x:3.,y:3.,w:2.,h:2.},Style::default());
        let group=d.group_objects(&[a,b]).unwrap();
        let layer=d.add_layer("Foreground".into());
        assert_eq!(d.move_to_layer(&[a],layer).unwrap_err(),
            "Select every member of a group before moving it");
        assert_eq!(d.move_to_layer(&[a,b],layer).unwrap(),2);
        assert_eq!(d.get(a).unwrap().group_id,Some(group));
        assert!(d.delete_empty_layer(layer).is_err());
        assert!(d.rename_layer(layer,"").is_err());
        d.rename_layer(layer,"Artwork").unwrap();
        assert!(d.reorder_layer(layer,false).unwrap());
        d.validate().unwrap();
        assert_eq!(d.layers[0].id,layer);
    }
    #[test] fn z_order_steps_keep_selection_and_ids(){
        let mut d=Document::default();
        let ids:Vec<_>=(0..4).map(|i|d.add(Shape::Rectangle{
            x:i as f32,y:0.,w:2.,h:2.},Style::default())).collect();
        assert_eq!(d.step_z_order(&ids[1..3],true).unwrap(),2);
        assert_eq!(d.objects.iter().map(|o|o.id).collect::<Vec<_>>(),
            vec![ids[0],ids[3],ids[1],ids[2]]);
        assert_eq!(d.step_z_order(&[ids[1],ids[2]],false).unwrap(),2);
        assert_eq!(d.objects.iter().map(|o|o.id).collect::<Vec<_>>(),ids);
        d.validate().unwrap();
    }
}
