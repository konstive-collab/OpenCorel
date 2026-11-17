//! Persistent object/layer management for the OpenCorel Studio workspace.
//! All batch operations validate their complete target set before mutation.
use crate::{Document,Object,ClipLink};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum StackOrder{RaiseOne,LowerOne,Front,Back}

fn selected_set(ids:&[u64])->Result<std::collections::HashSet<u64>,String>{
    let wanted:std::collections::HashSet<u64>=ids.iter().copied().collect();
    if wanted.is_empty(){return Err("Select at least one object".into());}
    if wanted.len()!=ids.len(){return Err("Selection contains duplicate object IDs".into());}
    Ok(wanted)
}
fn ensure_group_complete(doc:&Document,wanted:&std::collections::HashSet<u64>)
    ->Result<(),String>{
    let groups:std::collections::HashSet<u64>=doc.objects.iter()
        .filter(|o|wanted.contains(&o.id)).filter_map(|o|o.group_id).collect();
    if groups.iter().any(|group|doc.objects.iter().any(|o|
        o.group_id==Some(*group)&&!wanted.contains(&o.id))){
        return Err("Select the complete group before changing its layer or duplicating it".into());
    }
    Ok(())
}
fn ensure_editable(doc:&Document,wanted:&std::collections::HashSet<u64>)
    ->Result<(),String>{
    for id in wanted{
        let object=doc.objects.iter().find(|o|o.id==*id)
            .ok_or_else(||format!("Object #{id} no longer exists"))?;
        if !object.visible||object.locked{
            return Err(format!("Object #{id} is hidden or locked"));
        }
        if !doc.layer(object.layer_id)
            .is_some_and(|l|l.visible&&!l.locked){
            return Err(format!("Object #{id} is on a hidden or locked layer"));
        }
    }
    Ok(())
}
impl Document{
    pub fn object_is_editable(&self,id:u64)->bool{
        self.objects.iter().find(|o|o.id==id).is_some_and(|o|
            o.visible&&!o.locked&&self.layer(o.layer_id)
                .is_some_and(|l|l.visible&&!l.locked))
    }
    pub fn set_object_visible(&mut self,id:u64,visible:bool)->Result<bool,String>{
        let obj=self.objects.iter_mut().find(|o|o.id==id)
            .ok_or_else(||format!("Object #{id} does not exist"))?;
        if obj.visible==visible{return Ok(false);}
        obj.visible=visible;Ok(true)
    }
    pub fn set_object_locked(&mut self,id:u64,locked:bool)->Result<bool,String>{
        let obj=self.objects.iter_mut().find(|o|o.id==id)
            .ok_or_else(||format!("Object #{id} does not exist"))?;
        if obj.locked==locked{return Ok(false);}
        obj.locked=locked;Ok(true)
    }
    pub fn rename_layer(&mut self,id:u64,name:&str)->Result<bool,String>{
        if name.trim().is_empty()||name.len()>256||
            name.chars().any(|c|c.is_control()){
            return Err("Layer name must contain 1–256 printable bytes".into());
        }
        let layer=self.layers.iter_mut().find(|l|l.id==id)
            .ok_or_else(||"Layer does not exist".to_string())?;
        if layer.name==name{return Ok(false);}
        layer.name=name.to_owned();Ok(true)
    }
    pub fn move_layer(&mut self,id:u64,to_index:usize)->Result<bool,String>{
        let from=self.layers.iter().position(|l|l.id==id)
            .ok_or_else(||"Layer does not exist".to_string())?;
        if to_index>=self.layers.len(){return Err("Layer index is out of range".into());}
        if from==to_index{return Ok(false);}
        let layer=self.layers.remove(from);
        self.layers.insert(to_index,layer);
        Ok(true)
    }
    pub fn remove_layer_into(&mut self,id:u64,target_id:u64)->Result<usize,String>{
        if id==target_id{return Err("Choose a different destination layer".into());}
        if self.layers.len()<=1{return Err("Cannot remove the last layer".into());}
        let target=self.layer(target_id).ok_or("Destination layer does not exist")?;
        if !target.visible||target.locked{
            return Err("Destination layer must be visible and unlocked".into());
        }
        if self.layer(id).is_none(){return Err("Layer does not exist".into());}
        let count=self.objects.iter().filter(|o|o.layer_id==id).count();
        for obj in &mut self.objects{
            if obj.layer_id==id{obj.layer_id=target_id;}
        }
        self.layers.retain(|l|l.id!=id);
        Ok(count)
    }
    pub fn move_objects_to_layer(&mut self,ids:&[u64],target_id:u64)
        ->Result<usize,String>{
        let wanted=selected_set(ids)?;
        let target=self.layer(target_id).ok_or("Destination layer does not exist")?;
        if !target.visible||target.locked{
            return Err("Destination layer must be visible and unlocked".into());
        }
        ensure_editable(self,&wanted)?;
        ensure_group_complete(self,&wanted)?;
        let mut changed=0;
        for obj in &mut self.objects{
            if wanted.contains(&obj.id)&&obj.layer_id!=target_id{
                obj.layer_id=target_id;changed+=1;
            }
        }
        Ok(changed)
    }
    /// Stable z-order transforms. Relative order within the selection and
    /// within the unselected artwork is always preserved.
    pub fn reorder_objects(&mut self,ids:&[u64],operation:StackOrder)
        ->Result<usize,String>{
        let wanted=selected_set(ids)?;
        if self.objects.iter().filter(|o|wanted.contains(&o.id)).count()!=wanted.len(){
            return Err("Selection contains an object that no longer exists".into());
        }
        let before:Vec<u64>=self.objects.iter().map(|o|o.id).collect();
        match operation{
            StackOrder::Front|StackOrder::Back=>{
                let mut chosen=Vec::with_capacity(wanted.len());
                let mut rest=Vec::with_capacity(self.objects.len()-wanted.len());
                for obj in self.objects.drain(..){
                    if wanted.contains(&obj.id){chosen.push(obj);}
                    else{rest.push(obj);}
                }
                if operation==StackOrder::Front{
                    rest.extend(chosen);self.objects=rest;
                }else{
                    chosen.extend(rest);self.objects=chosen;
                }
            }
            StackOrder::RaiseOne=>{
                if self.objects.len()>1{
                    for i in (0..self.objects.len()-1).rev(){
                        if wanted.contains(&self.objects[i].id)&&
                            !wanted.contains(&self.objects[i+1].id){
                            self.objects.swap(i,i+1);
                        }
                    }
                }
            }
            StackOrder::LowerOne=>{
                for i in 1..self.objects.len(){
                    if wanted.contains(&self.objects[i].id)&&
                        !wanted.contains(&self.objects[i-1].id){
                        self.objects.swap(i,i-1);
                    }
                }
            }
        }
        Ok(before.iter().zip(self.objects.iter()).filter(|(a,b)|**a!=b.id).count())
    }
    /// Duplicate a complete editable selection with its text, style, layer,
    /// appearance, group topology and internal clipping relationships intact.
    /// The operation builds and validates a candidate document before commit.
    pub fn duplicate_objects(&mut self,ids:&[u64],dx:f32,dy:f32)
        ->Result<Vec<u64>,String>{
        let wanted=selected_set(ids)?;
        ensure_editable(self,&wanted)?;
        ensure_group_complete(self,&wanted)?;
        if !dx.is_finite()||!dy.is_finite()||dx.abs()>10_000.||dy.abs()>10_000.{
            return Err("Duplicate offset must be finite and within ±10,000 mm".into());
        }
        // Clipping is a relationship, not a paint attribute. Do not create
        // visually different duplicates when only one end of a clip pair is
        // selected; require both ends so the relation can be reproduced.
        for link in &self.clip_links{
            if wanted.contains(&link.source_id)!=wanted.contains(&link.target_id){
                return Err("Duplicate the clipping source and target together".into());
            }
        }
        let originals:Vec<Object>=self.objects.iter()
            .filter(|o|wanted.contains(&o.id)).cloned().collect();
        let groups:std::collections::HashSet<u64>=originals.iter()
            .filter_map(|o|o.group_id).collect();
        let needed=originals.len().checked_add(groups.len())
            .ok_or("Identifier allocation overflow")?;
        let end=self.next_id.checked_add(needed as u64)
            .filter(|next|*next<u64::MAX)
            .ok_or("Document object identifier limit reached")?;
        let mut candidate=self.clone();
        let mut id_map=std::collections::HashMap::with_capacity(originals.len());
        let mut new_ids=Vec::with_capacity(originals.len());
        for original in &originals{
            let id=candidate.next_id;
            candidate.next_id+=1;
            id_map.insert(original.id,id);
            new_ids.push(id);
        }
        let mut group_map=std::collections::HashMap::new();
        for group in groups{
            let id=candidate.next_id;
            candidate.next_id+=1;
            group_map.insert(group,id);
        }
        let mut copies=Vec::with_capacity(originals.len());
        for original in originals{
            let mut copy=original.clone();
            let id=id_map[&original.id];
            copy.id=id;
            copy.shape.translate(dx,dy);
            copy.group_id=copy.group_id.map(|g|group_map[&g]);
            let b=copy.bounds();
            if ![b.x,b.y,b.w,b.h].iter().all(|v|v.is_finite())||
                b.x< -100_000.||b.y< -100_000.||
                b.x+b.w>100_000.||b.y+b.h>100_000.{
                return Err(format!("Duplicate of object #{} exceeds document bounds",original.id));
            }
            if let Some(appearance)=self.appearances.get(&original.id){
                candidate.appearances.insert(id,appearance.clone());
            }
            copies.push(copy);
        }
        for link in &self.clip_links{
            if let (Some(source_id),Some(target_id))=
                (id_map.get(&link.source_id),id_map.get(&link.target_id)){
                candidate.clip_links.push(ClipLink{
                    source_id:*source_id,target_id:*target_id});
            }
        }
        candidate.objects.extend(copies);
        candidate.next_id=end;
        candidate.validate()?;
        *self=candidate;
        Ok(new_ids)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Appearance,Document,Shape,Style,Point};
    fn rect(doc:&mut Document,x:f32)->u64{
        doc.add(Shape::Rectangle{x,y:2.,w:10.,h:10.},Style::default())
    }
    #[test]
    fn legacy_objects_default_visible_and_unlocked(){
        let mut d=Document::default();
        let id=rect(&mut d,1.);
        let mut json=serde_json::to_value(&d).unwrap();
        json["objects"][0].as_object_mut().unwrap().remove("visible");
        json["objects"][0].as_object_mut().unwrap().remove("locked");
        let old:Document=serde_json::from_value(json).unwrap();
        let obj=old.objects.iter().find(|o|o.id==id).unwrap();
        assert!(obj.visible&&!obj.locked);
        assert_eq!(old.visible_objects().count(),1);
        old.validate().unwrap();
    }
    #[test]
    fn visibility_and_lock_are_persistent_and_independent(){
        let mut d=Document::default();
        let id=rect(&mut d,1.);
        assert!(d.set_object_locked(id,true).unwrap());
        assert!(d.object_is_editable(id)==false);
        assert!(d.set_object_visible(id,false).unwrap());
        assert_eq!(d.visible_objects().count(),0);
        assert!(d.set_object_locked(id,false).unwrap());
        assert_eq!(d.visible_objects().count(),0);
        assert!(d.set_object_visible(id,true).unwrap());
        assert!(d.object_is_editable(id));
        d.validate().unwrap();
    }
    #[test]
    fn stack_operations_preserve_relative_order_and_selection(){
        let mut d=Document::default();
        let a=rect(&mut d,0.);let b=rect(&mut d,20.);
        let c=rect(&mut d,40.);let e=rect(&mut d,60.);
        d.reorder_objects(&[b,e],StackOrder::RaiseOne).unwrap();
        assert_eq!(d.objects.iter().map(|o|o.id).collect::<Vec<_>>(),vec![a,c,b,e]);
        d.reorder_objects(&[b,e],StackOrder::Back).unwrap();
        assert_eq!(d.objects.iter().map(|o|o.id).collect::<Vec<_>>(),vec![b,e,a,c]);
        d.reorder_objects(&[b,e],StackOrder::Front).unwrap();
        assert_eq!(d.objects.iter().map(|o|o.id).collect::<Vec<_>>(),vec![a,c,b,e]);
        d.reorder_objects(&[b,e],StackOrder::LowerOne).unwrap();
        assert_eq!(d.objects.iter().map(|o|o.id).collect::<Vec<_>>(),vec![a,b,e,c]);
    }
    #[test]
    fn duplicate_preserves_text_style_appearance_groups_and_undoable_document(){
        let mut d=Document::default();
        let a=rect(&mut d,1.);
        let b=d.add(Shape::Text{x:20.,y:4.,value:"Ёжик".into(),size_mm:8.},
            Style{fill:"#AA1133".into(),stroke:"none".into(),stroke_width_mm:0.});
        d.group_objects(&[a,b]).unwrap();
        let appearance=Appearance{opacity:0.75,..Appearance::default()};
        d.set_appearance(a,appearance.clone()).unwrap();
        let before=d.objects.len();
        let ids=d.duplicate_objects(&[a,b],5.,7.).unwrap();
        assert_eq!(ids.len(),2);assert_eq!(d.objects.len(),before+2);
        let x=d.objects.iter().find(|o|o.id==ids[0]).unwrap();
        let y=d.objects.iter().find(|o|o.id==ids[1]).unwrap();
        assert_eq!(x.group_id,y.group_id);assert!(x.group_id.is_some());
        assert_eq!(x.bounds().x,6.);assert_eq!(y.bounds().y,11.);
        assert_eq!(y.style.fill,"#AA1133");
        assert_eq!(d.appearances.get(&ids[0]),Some(&appearance));
        d.validate().unwrap();
    }
    #[test]
    fn duplicate_rejects_partial_groups_clip_pairs_locked_and_invalid_offset_atomically(){
        let mut d=Document::default();
        let a=rect(&mut d,1.);let b=rect(&mut d,20.);
        d.group_objects(&[a,b]).unwrap();
        let before=d.clone();
        assert!(d.duplicate_objects(&[a],2.,2.).is_err());
        assert_eq!(d,before);
        d.objects[1].group_id=None;
        d.clip_links.push(ClipLink{source_id:a,target_id:b});
        let before=d.clone();
        assert!(d.duplicate_objects(&[a],2.,2.).is_err());
        assert_eq!(d,before);
        assert!(d.duplicate_objects(&[a,b],f32::NAN,0.).is_err());
        assert_eq!(d,before);
        d.clip_links.clear();d.objects[0].locked=true;
        let before=d.clone();
        assert!(d.duplicate_objects(&[a],2.,2.).is_err());
        assert_eq!(d,before);
    }
    #[test]
    fn moving_objects_and_layers_is_atomic_and_validated(){
        let mut d=Document::default();
        let a=rect(&mut d,1.);let b=rect(&mut d,20.);
        let layer=d.add_layer("Overlay".into());
        assert_eq!(d.move_objects_to_layer(&[a,b],layer).unwrap(),2);
        assert!(d.objects.iter().all(|o|o.layer_id==layer));
        d.rename_layer(layer,"Верхний слой").unwrap();
        assert_eq!(d.layer(layer).unwrap().name,"Верхний слой");
        assert_eq!(d.remove_layer_into(layer,1).unwrap(),2);
        assert!(d.objects.iter().all(|o|o.layer_id==1));
        assert_eq!(d.layers.len(),1);
        d.validate().unwrap();
    }
    #[test]
    fn hidden_locked_objects_cannot_be_hit_or_grouped(){
        let mut d=Document::default();
        let a=rect(&mut d,1.);let b=rect(&mut d,20.);
        d.set_object_locked(b,true).unwrap();
        assert!(d.group_objects(&[a,b]).is_err());
        assert_eq!(d.hit_test(Point{x:25.,y:5.},0.0),None);
        d.set_object_locked(b,false).unwrap();
        d.set_object_visible(b,false).unwrap();
        assert_eq!(d.hit_test(Point{x:25.,y:5.},0.0),None);
        d.validate().unwrap();
    }
}
