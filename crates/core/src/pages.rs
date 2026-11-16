//! Multi-page native KDR document navigation.
//! The visible page stays in the original Document fields for compatibility
//! with existing canvas, tools and importers. Inactive pages live in slots.
//! The active slot is a deliberately EMPTY placeholder; switching moves the
//! live geometry into its slot and restores the target page.
use crate::{Document,Page,Object,Layer,ClipLink,SCHEMA_VERSION,appearance::Appearance,guides};
use std::collections::BTreeMap;

pub const MAX_PAGES:usize=64;

pub fn default_page_title()->String{"Страница 1".to_string()}
pub fn is_page_zero(v:&usize)->bool{*v==0}

#[derive(Clone,Debug,PartialEq,serde::Serialize,serde::Deserialize)]
pub struct StoredPage{
    pub title:String,
    pub page:Page,
    pub objects:Vec<Object>,
    pub next_id:u64,
    pub layers:Vec<Layer>,
    pub next_layer_id:u64,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub clip_links:Vec<ClipLink>,
    #[serde(default,skip_serializing_if="BTreeMap::is_empty")]
    pub appearances:BTreeMap<u64,Appearance>,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub guides:Vec<guides::Guide>,
}
impl StoredPage{
    fn blank(page:Page,title:String)->Self{
        let base=Document::default();
        Self{title,page,objects:vec![],next_id:1,layers:base.layers,
            next_layer_id:base.next_layer_id,clip_links:vec![],
            appearances:BTreeMap::new(),guides:vec![]}
    }
    fn placeholder()->Self{
        Self::blank(Page{width_mm:1.,height_mm:1.},String::new())
    }
    fn as_document(&self,document_name:&str)->Document{
        let mut doc=Document::default();
        doc.schema_version=SCHEMA_VERSION;
        doc.name=document_name.to_owned();
        doc.page=self.page.clone();
        doc.objects=self.objects.clone();
        doc.next_id=self.next_id;
        doc.layers=self.layers.clone();
        doc.next_layer_id=self.next_layer_id;
        doc.clip_links=self.clip_links.clone();
        doc.appearances=self.appearances.clone();
        doc.guides=self.guides.clone();
        doc.active_page_title=self.title.clone();
        doc
    }
}
fn valid_title(s:&str)->bool{
    !s.trim().is_empty() && s.len()<=128 && s.chars().all(|c|
        !c.is_control() && c!='<' && c!='>' && c!='&')
}
impl Document{
    /// A legacy single-page document has no slots. Multi-page documents have
    /// exactly one slot per logical page, including the active placeholder.
    pub fn page_count(&self)->usize{
        if self.page_slots.is_empty(){1}else{self.page_slots.len()}
    }
    pub fn page_title(&self,index:usize)->Option<&str>{
        if index>=self.page_count(){return None;}
        if index==self.active_page_index{Some(&self.active_page_title)}
        else{self.page_slots.get(index).map(|p|p.title.as_str())}
    }
    pub fn page_dimensions(&self,index:usize)->Option<(f32,f32)>{
        if index>=self.page_count(){return None;}
        let p=if index==self.active_page_index{
            &self.page
        }else{
            &self.page_slots.get(index)?.page
        };
        Some((p.width_mm,p.height_mm))
    }
    fn snapshot(&self)->StoredPage{
        StoredPage{
            title:self.active_page_title.clone(),
            page:self.page.clone(),objects:self.objects.clone(),next_id:self.next_id,
            layers:self.layers.clone(),next_layer_id:self.next_layer_id,
            clip_links:self.clip_links.clone(),appearances:self.appearances.clone(),
            guides:self.guides.clone(),
        }
    }
    /// Move the live page out without cloning large embedded images.
    fn take_live(&mut self)->StoredPage{
        StoredPage{
            title:std::mem::take(&mut self.active_page_title),
            page:std::mem::replace(&mut self.page,Page{
                width_mm:1.,height_mm:1.}),
            objects:std::mem::take(&mut self.objects),
            next_id:std::mem::replace(&mut self.next_id,1),
            layers:std::mem::take(&mut self.layers),
            next_layer_id:std::mem::replace(&mut self.next_layer_id,2),
            clip_links:std::mem::take(&mut self.clip_links),
            appearances:std::mem::take(&mut self.appearances),
            guides:std::mem::take(&mut self.guides),
        }
    }
    fn install_live(&mut self,page:StoredPage){
        self.active_page_title=page.title;
        self.page=page.page;
        self.objects=page.objects;
        self.next_id=page.next_id;
        self.layers=page.layers;
        self.next_layer_id=page.next_layer_id;
        self.clip_links=page.clip_links;
        self.appearances=page.appearances;
        self.guides=page.guides;
    }
    /// Switch page with no geometry clones, including raster and SVG assets.
    pub fn switch_page(&mut self,index:usize)->Result<(),String>{
        if index>=self.page_count(){return Err("Page index out of range".into());}
        if index==self.active_page_index{return Ok(());}
        if self.page_slots.len()<2{return Err("No additional page".into());}
        let source=self.active_page_index;
        let old=self.take_live();
        self.page_slots[source]=old;
        let target=std::mem::replace(
            &mut self.page_slots[index],StoredPage::placeholder());
        self.install_live(target);
        self.active_page_index=index;
        Ok(())
    }
    /// Add a new blank page or duplicate the entire active page (including
    /// styles, images, clipping, guides, layers and independent object IDs).
    pub fn append_page(&mut self,duplicate:bool)->Result<usize,String>{
        self.validate()?;
        if self.page_count()>=MAX_PAGES{
            return Err(format!("Page limit exceeded (maximum {MAX_PAGES})"));
        }
        let number=self.page_count()+1;
        let new_title=if duplicate{
            let source=self.active_page_title.as_str();
            let prefix="Копия: ";
            let max_bytes=128usize.saturating_sub(prefix.len());
            let shortened=source.char_indices().take_while(|(i,c)|
                i+c.len_utf8()<=max_bytes).map(|(_,c)|c).collect::<String>();
            format!("{prefix}{shortened}")
        }else{format!("Страница {number}")};
        let next=if duplicate{
            let mut snapshot=self.snapshot();
            snapshot.title=new_title;
            snapshot
        }else{
            StoredPage::blank(self.page.clone(),new_title)
        };
        if self.page_slots.is_empty(){
            self.page_slots.push(StoredPage::placeholder());
        }
        let index=self.page_slots.len();
        self.page_slots.push(next);
        self.switch_page(index)?;
        Ok(index)
    }
    pub fn rename_page(&mut self,index:usize,title:&str)->Result<(),String>{
        if index>=self.page_count(){return Err("Page index out of range".into());}
        if !valid_title(title){return Err("Invalid page title (1–128 printable bytes)".into());}
        if index==self.active_page_index{self.active_page_title=title.into();}
        else{self.page_slots[index].title=title.into();}
        Ok(())
    }
    pub fn delete_page(&mut self,index:usize)->Result<(),String>{
        if index>=self.page_count(){return Err("Page index out of range".into());}
        if self.page_count()<=1{return Err("Cannot remove last page".into());}
        if index==self.active_page_index{
            let target=if index==0{1}else{index-1};
            self.switch_page(target)?;
        }
        self.page_slots.remove(index);
        if self.active_page_index>index{self.active_page_index-=1;}
        Ok(())
    }
    pub fn page_document(&self,index:usize)->Result<Document,String>{
        if index>=self.page_count(){return Err("Page index out of range".into());}
        let snapshot=if index==self.active_page_index{
            self.snapshot()
        }else{
            self.page_slots[index].clone()
        };
        let document=snapshot.as_document(&self.name);
        document.validate()?;
        Ok(document)
    }
    pub(crate) fn validate_pages(&self)->Result<(),String>{
        if !valid_title(&self.active_page_title){
            return Err("Invalid active page title".into());
        }
        if self.page_slots.is_empty(){
            if self.active_page_index!=0{
                return Err("Legacy document has nonzero active page index".into());
            }
            return Ok(());
        }
        if self.page_slots.len()<2||self.page_slots.len()>MAX_PAGES ||
            self.active_page_index>=self.page_slots.len(){
            return Err("Invalid page array or active page index".into());
        }
        for (index,page) in self.page_slots.iter().enumerate(){
            if index==self.active_page_index{
                // Disallow nested geometry in the unused placeholder.
                if !page.objects.is_empty()||!page.clip_links.is_empty()||
                    !page.appearances.is_empty()||!page.guides.is_empty() {
                    return Err("Active page slot must be empty".into());
                }
                continue;
            }
            if !valid_title(&page.title){
                return Err(format!("Invalid title for page {}",index+1));
            }
            page.as_document(&self.name).validate().map_err(|e|
                format!("Page {}: {e}",index+1))?;
        }
        Ok(())
    }
    pub(crate) fn inactive_size_bytes(&self)->usize{
        self.page_slots.iter().enumerate().filter(|(i,_)|
            *i!=self.active_page_index).fold(0usize,|sum,(_,page)|
            sum.saturating_add(page.as_document(&self.name).estimated_size_bytes()))
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Shape,Style,kdr};
    #[test]
    fn add_switch_edit_rename_and_delete_preserves_independent_page_contents(){
        let mut doc=Document::default();
        doc.page.width_mm=210.;doc.page.height_mm=297.;
        let first=doc.add(Shape::Rectangle{x:1.,y:2.,w:5.,h:5.},Style::default());
        let second_page=doc.append_page(false).unwrap();
        assert_eq!(second_page,1);
        assert_eq!(doc.page_count(),2);
        assert!(doc.objects.is_empty());
        assert_eq!(doc.page.width_mm,210.);
        doc.page.width_mm=90.;
        let second=doc.add(Shape::Text{x:7.,y:8.,value:"Second".into(),size_mm:7.},
            Style::default());
        assert_eq!(first,second); // Per-page IDs are independent.
        doc.rename_page(1,"Печать / оборот").unwrap();
        doc.switch_page(0).unwrap();
        assert_eq!(doc.objects.len(),1);
        assert!(matches!(doc.objects[0].shape,Shape::Rectangle{..}));
        assert_eq!(doc.page.width_mm,210.);
        doc.switch_page(1).unwrap();
        assert_eq!(doc.page.width_mm,90.);
        assert_eq!(doc.page_title(1),Some("Печать / оборот"));
        doc.validate().unwrap();
        doc.delete_page(1).unwrap();
        assert_eq!(doc.page_count(),1);
        assert_eq!(doc.objects.len(),1);
        assert!(matches!(doc.objects[0].shape,Shape::Rectangle{..}));
        assert_eq!(doc.active_page_index,0);
        assert!(doc.delete_page(0).is_err());
    }
    #[test]
    fn duplicate_is_independent_and_serialization_roundtrips(){
        let mut doc=Document::default();
        doc.add(Shape::Rectangle{x:9.,y:13.,w:15.,h:15.},Style::default());
        doc.append_page(true).unwrap();
        let original=doc.page_document(0).unwrap();
        assert_eq!(doc.objects.len(),1);
        doc.objects[0].shape.translate(30.,40.);
        let clone=doc.clone();
        let bytes=serde_json::to_vec(&doc).unwrap();
        let restored:Document=serde_json::from_slice(&bytes).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored,clone);
        assert_eq!(restored.page_document(0).unwrap(),original);
        assert_ne!(restored.page_document(0).unwrap().objects,
            restored.page_document(1).unwrap().objects);
        let dir=tempfile::tempdir().unwrap();
        let path=dir.path().join("multipage.kdr");
        kdr::save_kdr(&restored,&path).unwrap();
        assert_eq!(kdr::read_kdr(&path).unwrap(),restored);
    }
    #[test]
    fn old_one_page_json_remains_valid_and_new_pages_are_rejected_if_corrupt(){
        let mut doc=Document::default();
        let mut val=serde_json::to_value(&doc).unwrap();
        val.as_object_mut().unwrap().remove("page_slots");
        val.as_object_mut().unwrap().remove("active_page_index");
        val.as_object_mut().unwrap().remove("active_page_title");
        let legacy:Document=serde_json::from_value(val).unwrap();
        legacy.validate().unwrap();
        assert_eq!(legacy.page_count(),1);
        doc.append_page(false).unwrap();
        doc.page_slots[0].objects.push(Object{
            id:0,shape:Shape::Rectangle{x:0.,y:0.,w:1.,h:1.},
            style:Style::default(),layer_id:1,text_format:Default::default(),
            name:String::new(),group_id:None,node_modes:vec![]});
        assert!(doc.validate().is_err());
    }
    #[test]
    fn deleted_and_switched_pages_have_valid_names_and_no_nested_active_data(){
        let mut d=Document::default();
        d.append_page(false).unwrap();
        d.append_page(false).unwrap();
        d.switch_page(0).unwrap();
        d.delete_page(1).unwrap();
        d.validate().unwrap();
        assert_eq!(d.page_count(),2);
        assert!(d.rename_page(1,"\n").is_err());
        assert!(d.switch_page(2).is_err());
    }
}
