//! Persistent multipage document support compatible with legacy one-page JSON.
//! Active content remains in Document.page/objects for existing tools; all
//! inactive pages live here with independent art, clipping and layers.
use crate::{Document,Page,Object,Layer,ClipLink,Shape,Point,CubicSegment,
    default_layers,default_next_layer_id,valid_xml_text};
use crate::appearance::Appearance;
use crate::guides::Guide;
use serde::{Serialize,Deserialize};
use std::collections::{BTreeMap,HashSet};
pub fn initial_page_id()->u64{1}
pub fn initial_next_page_id()->u64{2}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageTab{
    pub id:u64,
    pub name:String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageContent{
    pub page:Page,
    pub objects:Vec<Object>,
    pub layers:Vec<Layer>,
    pub next_layer_id:u64,
    pub clip_links:Vec<ClipLink>,
    pub appearances:BTreeMap<u64,Appearance>,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub guides:Vec<Guide>,
}
impl Document{
    fn capture_page(&self)->PageContent{
        PageContent{page:self.page.clone(),objects:self.objects.clone(),
            layers:self.layers.clone(),next_layer_id:self.next_layer_id,
            clip_links:self.clip_links.clone(),appearances:self.appearances.clone(),
            guides:self.guides.clone()}
    }
    fn restore_page(&mut self,state:PageContent){
        self.page=state.page;
        self.objects=state.objects;
        self.layers=state.layers;
        self.next_layer_id=state.next_layer_id;
        self.clip_links=state.clip_links;
        self.appearances=state.appearances;
        self.guides=state.guides;
    }
    fn initialize_page_tabs(&mut self){
        if self.page_tabs.is_empty(){
            self.page_tabs.push(PageTab{id:1,name:"Страница 1".into()});
            self.active_page_id=1;
            self.next_page_id=self.next_page_id.max(2);
        }
    }
    pub fn pages(&self)->Vec<PageTab>{
        if self.page_tabs.is_empty(){
            vec![PageTab{id:1,name:"Страница 1".into()}]
        }else{self.page_tabs.clone()}
    }
    pub fn page_index(&self)->usize{
        self.pages().iter().position(|p|p.id==self.active_page_id).unwrap_or(0)
    }
    pub fn page_object_count(&self,id:u64)->Option<usize>{
        if id==self.active_page_id{return Some(self.objects.len());}
        self.inactive_pages.get(&id).map(|p|p.objects.len())
    }
    /// Switch between complete page editing environments without disturbing
    /// object IDs or losing appearances / clipping / layer names.
    pub fn switch_page(&mut self,id:u64)->Result<(),String>{
        if id==self.active_page_id{return Ok(());}
        self.initialize_page_tabs();
        if !self.page_tabs.iter().any(|p|p.id==id){
            return Err("Unknown page".into());
        }
        let target=self.inactive_pages.remove(&id)
            .ok_or("Missing page content")?;
        let previous=PageContent{
            page:std::mem::replace(&mut self.page,target.page),
            objects:std::mem::replace(&mut self.objects,target.objects),
            layers:std::mem::replace(&mut self.layers,target.layers),
            next_layer_id:std::mem::replace(&mut self.next_layer_id,target.next_layer_id),
            clip_links:std::mem::replace(&mut self.clip_links,target.clip_links),
            appearances:std::mem::replace(&mut self.appearances,target.appearances),
            guides:std::mem::replace(&mut self.guides,target.guides),
        };
        self.inactive_pages.insert(self.active_page_id,previous);
        self.active_page_id=id;
        Ok(())
    }
    pub fn add_page(&mut self,size:Page)->Result<u64,String>{
        if self.pages().len()>=64{return Err("Document page limit (64) reached".into());}
        if ![size.width_mm,size.height_mm].iter().all(|v|
            v.is_finite()&&(1.0..=100_000.0).contains(v)){
            return Err("Invalid new page dimensions".into());
        }
        self.initialize_page_tabs();
        if self.next_page_id==u64::MAX{return Err("Page identifier limit reached".into());}
        let id=self.next_page_id;
        self.next_page_id+=1;
        let previous=PageContent{
            page:std::mem::replace(&mut self.page,size),
            objects:std::mem::take(&mut self.objects),
            layers:std::mem::replace(&mut self.layers,default_layers()),
            next_layer_id:std::mem::replace(&mut self.next_layer_id,default_next_layer_id()),
            clip_links:std::mem::take(&mut self.clip_links),
            appearances:std::mem::take(&mut self.appearances),
            guides:std::mem::take(&mut self.guides),
        };
        self.inactive_pages.insert(self.active_page_id,previous);
        self.page_tabs.push(PageTab{id,name:format!("Страница {}",self.page_tabs.len()+1)});
        self.active_page_id=id;
        Ok(id)
    }
    /// Duplicate the current page with fresh object/group IDs and valid
    /// clipped pairs. The source retains its geometry and all metadata.
    pub fn duplicate_page(&mut self)->Result<u64,String>{
        let source_page_id=self.active_page_id;
        let source_instances:Vec<_>=self.symbol_instances.iter()
            .filter(|item|item.page_id==source_page_id).cloned().collect();
        let original=self.capture_page();
        let source_name=self.pages()[self.page_index()].name.clone();
        let groups:HashSet<_>=original.objects.iter().filter_map(|o|o.group_id).collect();
        let needed=original.objects.len() as u64+groups.len() as u64;
        if self.next_id.checked_add(needed).is_none_or(|n|n==u64::MAX){
            return Err("Object identifier limit reached".into());
        }
        if original.objects.len()+self.all_object_count()>100_000{
            return Err("Whole document object limit exceeded".into());
        }
        let id=self.add_page(original.page.clone())?;
        self.layers=original.layers;
        self.next_layer_id=original.next_layer_id;
        let mut ids=BTreeMap::new();
        let mut group_ids=BTreeMap::new();
        for mut obj in original.objects{
            let old_id=obj.id;
            obj.id=self.next_id;
            self.next_id+=1;
            ids.insert(old_id,obj.id);
            if let Some(old_group)=obj.group_id{
                let new_group=if let Some(value)=group_ids.get(&old_group){
                    *value
                }else{
                    let v=self.next_id;self.next_id+=1;
                    group_ids.insert(old_group,v);
                    v
                };
                obj.group_id=Some(new_group);
            }
            self.objects.push(obj);
        }
        self.clip_links=original.clip_links.into_iter().map(|link|ClipLink{
            source_id:ids[&link.source_id],target_id:ids[&link.target_id]
        }).collect();
        self.appearances=original.appearances.into_iter().map(|(key,value)|
            (ids[&key],value)).collect();
        self.guides=original.guides;
        for instance in source_instances{
            let Some(&group_id)=group_ids.get(&instance.group_id) else{
                return Err("Cannot duplicate an incomplete symbol instance".into());
            };
            let member_ids=instance.member_ids.iter().map(|old|
                ids.get(old).copied().ok_or("Missing duplicated symbol member"))
                .collect::<Result<Vec<_>,_>>()?;
            self.symbol_instances.push(crate::SymbolInstance{
                symbol_id:instance.symbol_id,page_id:id,group_id,member_ids
            });
        }
        if let Some(tab)=self.page_tabs.iter_mut().find(|p|p.id==id){
            tab.name=format!("{source_name} — копия");
        }
        Ok(id)
    }
    /// Reorder pages while keeping the active sheet and every layer intact.
    pub fn move_page(&mut self,id:u64,to_index:usize)->Result<(),String>{
        self.initialize_page_tabs();
        if to_index>=self.page_tabs.len(){return Err("Invalid target page position".into());}
        let from=self.page_tabs.iter().position(|tab|tab.id==id)
            .ok_or("Unknown page")?;
        if from==to_index{return Ok(());}
        let tab=self.page_tabs.remove(from);
        self.page_tabs.insert(to_index,tab);
        Ok(())
    }
    pub fn rename_page(&mut self,id:u64,name:&str)->Result<(),String>{
        if name.is_empty()||name.len()>120||!valid_xml_text(name){
            return Err("Page title must be 1–120 valid characters".into());
        }
        self.initialize_page_tabs();
        let tab=self.page_tabs.iter_mut().find(|p|p.id==id)
            .ok_or("Unknown page")?;
        tab.name=name.to_owned();
        Ok(())
    }
    pub fn delete_page(&mut self,id:u64)->Result<(),String>{
        self.initialize_page_tabs();
        if self.page_tabs.len()<=1{return Err("Cannot delete the last page".into());}
        let index=self.page_tabs.iter().position(|p|p.id==id)
            .ok_or("Unknown page")?;
        if id==self.active_page_id{
            let next=if index+1<self.page_tabs.len(){
                self.page_tabs[index+1].id
            }else{self.page_tabs[index-1].id};
            let state=self.inactive_pages.remove(&next)
                .ok_or("Missing replacement page")?;
            self.restore_page(state);
            self.active_page_id=next;
        }else{self.inactive_pages.remove(&id);}
        self.page_tabs.remove(index);
        self.symbol_instances.retain(|item|item.page_id!=id);
        Ok(())
    }
    /// Render a detached document containing only the requested page. Existing
    /// exporters remain single-page, while callers can export every page.
    pub fn page_document(&self,id:u64)->Result<Document,String>{
        if !self.pages().iter().any(|p|p.id==id){return Err("Unknown page".into());}
        // Important for 1000+ object projects: clone only the requested
        // page, not all other pages in the document for each export.
        let state=if id==self.active_page_id{
            self.capture_page()
        }else{self.inactive_pages.get(&id).ok_or("Missing page content")?.clone()};
        let mut doc=Document{
            name:self.name.clone(),
            schema_version:self.schema_version,
            next_id:self.next_id,
            ..Document::default()
        };
        doc.restore_page(state);
        Ok(doc)
    }
    pub fn all_object_count(&self)->usize{
        self.objects.len()+self.inactive_pages.values()
            .map(|p|p.objects.len()).sum::<usize>()
    }
    pub(crate) fn inactive_pages_memory_bytes(&self)->usize{
        let objects=self.inactive_pages.values().flat_map(|p|p.objects.iter())
            .map(|o|{
                let geometry=match &o.shape{
                    Shape::Raster{data_base64,..}=>data_base64.len(),
                    Shape::EmbeddedSvg{svg,..}=>svg.len(),
                    Shape::Text{value,..}=>value.len(),
                    Shape::Polyline{points,..}=>points.len()*std::mem::size_of::<Point>(),
                    Shape::Bezier{segments,..}=>segments.len()*std::mem::size_of::<CubicSegment>(),
                    Shape::CompoundPath{contours}=>contours.iter()
                        .map(|c|c.segments.len()*std::mem::size_of::<CubicSegment>()).sum(),
                    _=>64
                };
                std::mem::size_of::<Object>()+o.name.len()
                    +o.text_format.family.len()+geometry
            }).sum::<usize>();
        objects+self.page_tabs.iter().map(|p|p.name.len()).sum::<usize>()
            +self.guides.len()*std::mem::size_of::<Guide>()
            +self.inactive_pages.values().map(|p|p.guides.len()*std::mem::size_of::<Guide>()).sum::<usize>()
    }
    pub(crate) fn validate_all_pages(&self)->Result<(),String>{
        if self.next_page_id<=self.active_page_id||self.next_page_id==u64::MAX{
            return Err("Invalid page identifier allocation".into());
        }
        if self.page_tabs.is_empty(){
            if self.active_page_id!=1||!self.inactive_pages.is_empty(){
                return Err("Invalid legacy page layout".into());
            }
            return Ok(());
        }
        if self.page_tabs.len()>64{return Err("Document page limit exceeded".into());}
        let mut seen=HashSet::new();
        for tab in &self.page_tabs{
            if tab.id==0||tab.id>=self.next_page_id
                || !seen.insert(tab.id)||tab.name.is_empty()
                || tab.name.len()>120||!valid_xml_text(&tab.name){
                return Err("Invalid page entry".into());
            }
        }
        if !seen.contains(&self.active_page_id)
            || self.inactive_pages.len()+1!=self.page_tabs.len(){
            return Err("Inconsistent active/inactive page states".into());
        }
        if self.all_object_count()>100_000{
            return Err("Whole document object limit exceeded".into());
        }
        let mut global_ids:HashSet<u64>=self.objects.iter().map(|o|o.id).collect();
        for (id,state) in &self.inactive_pages{
            if *id==self.active_page_id||!seen.contains(id){
                return Err("Unknown inactive page".into());
            }
            for object in &state.objects{
                if !global_ids.insert(object.id){
                    return Err("Duplicate object identifier across pages".into());
                }
            }
            // Validate only the target page, never cloning every other page
            // on each call (critical for large posters and 64-page projects).
            let mut isolated=Document{
                name:self.name.clone(),schema_version:self.schema_version,
                next_id:self.next_id,..Document::default()
            };
            isolated.restore_page(state.clone());
            isolated.validate().map_err(|e|format!("Invalid page #{id}: {e}"))?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Shape,Style};
    #[test] fn switch_and_save_independent_pages(){
        let mut d=Document::default();
        let first=d.add(Shape::Rectangle{x:1.,y:2.,w:10.,h:15.},Style::default());
        let second=d.add_page(Page{width_mm:210.,height_mm:297.}).unwrap();
        assert!(d.objects.is_empty());
        let another=d.add(Shape::Ellipse{x:2.,y:3.,w:20.,h:22.},Style::default());
        d.switch_page(1).unwrap();
        assert_eq!(d.objects[0].id,first);
        assert_eq!(d.page.width_mm,841.);
        d.switch_page(second).unwrap();
        assert_eq!(d.objects[0].id,another);
        d.validate().unwrap();
        let encoded=serde_json::to_vec(&d).unwrap();
        let mut decoded:Document=serde_json::from_slice(&encoded).unwrap();
        decoded.validate().unwrap();
        decoded.switch_page(1).unwrap();
        assert_eq!(decoded.objects[0].id,first);
    }
    #[test] fn duplicate_rekeys_groups_and_clips(){
        let mut d=Document::default();
        let a=d.add(Shape::Rectangle{x:1.,y:1.,w:20.,h:20.},Style::default());
        let b=d.add(Shape::Ellipse{x:2.,y:2.,w:4.,h:4.},Style::default());
        let c=d.add(Shape::Text{x:1.,y:1.,value:"Hello".into(),size_mm:5.},Style::default());
        d.group_objects(&[b,c]).unwrap();
        let m=d.add(Shape::Rectangle{x:3.,y:3.,w:15.,h:15.},Style::default());
        let t=d.add(Shape::Rectangle{x:4.,y:4.,w:5.,h:5.},Style::default());
        d.set_clip(m,t).unwrap();
        let group=d.get(b).unwrap().group_id;
        let new_id=d.duplicate_page().unwrap();
        assert_eq!(d.objects.len(),5);
        assert_ne!(d.objects[0].id,a);
        assert_ne!(d.objects[1].group_id,group);
        assert_eq!(d.clip_links.len(),1);
        assert_ne!(d.clip_links[0].source_id,m);
        assert_eq!(d.active_page_id,new_id);
        d.validate().unwrap();
        d.switch_page(1).unwrap();
        assert_eq!(d.get(a).unwrap().id,a);
    }
    #[test] fn page_order_is_persistent_and_does_not_change_geometry(){
        let mut d=Document::default();
        let first=d.add(Shape::Rectangle{x:1.,y:1.,w:10.,h:10.},Style::default());
        let second=d.add_page(Page{width_mm:300.,height_mm:200.}).unwrap();
        let other=d.add(Shape::Ellipse{x:5.,y:5.,w:20.,h:20.},Style::default());
        d.move_page(second,0).unwrap();
        assert_eq!(d.pages()[0].id,second);
        d.switch_page(1).unwrap();
        assert_eq!(d.objects[0].id,first);
        d.switch_page(second).unwrap();
        assert_eq!(d.objects[0].id,other);
        d.validate().unwrap();
    }
    #[test] fn deleting_page_preserves_last_page(){
        let mut d=Document::default();
        assert!(d.delete_page(1).is_err());
        let id=d.add_page(Page{width_mm:100.,height_mm:80.}).unwrap();
        assert!(d.rename_page(id,"").is_err());
        d.rename_page(id,"Обложка").unwrap();
        d.delete_page(1).unwrap();
        assert_eq!(d.pages().len(),1);
        assert_eq!(d.active_page_id,id);
        d.validate().unwrap();
    }
    #[test] fn legacy_document_json_can_be_upgraded(){
        let mut d=Document::default();
        d.page_tabs.clear();
        let mut old=serde_json::to_value(&d).unwrap();
        let object=old.as_object_mut().unwrap();
        object.remove("active_page_id");object.remove("next_page_id");
        let mut loaded:Document=serde_json::from_value(old).unwrap();
        loaded.validate().unwrap();
        loaded.add_page(Page{width_mm:100.,height_mm:100.}).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded.pages().len(),2);
    }
    #[test] fn invalid_or_duplicate_page_data_is_rejected(){
        let mut d=Document::default();
        let id=d.add(Shape::Rectangle{x:1.,y:1.,w:3.,h:3.},Style::default());
        d.add_page(Page{width_mm:200.,height_mm:100.}).unwrap();
        d.objects.push(d.inactive_pages.get(&1).unwrap().objects[0].clone());
        assert!(d.validate().unwrap_err().contains("Duplicate object"));
        d.objects.clear();
        d.inactive_pages.get_mut(&1).unwrap().page.width_mm=0.;
        assert!(d.validate().unwrap_err().contains("Invalid page"));
        assert!(id>0);
    }
}
