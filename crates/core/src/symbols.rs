//! Reusable, linked vector symbols and portable local libraries.
//! Symbol instances are ordinary editable document objects grouped by a stable
//! group ID. This deliberately avoids a new Shape variant, so existing SVG,
//! PDF, raster previews and older renderers continue to work unchanged.
use crate::{appearance::Appearance,Bounds,Document,Object,Point,Shape,Style,TextFormat};
use serde::{Deserialize,Serialize};
use std::{
    collections::{BTreeMap,HashSet},
    env,fs,
    io::Write,
    path::{Path,PathBuf},
    sync::atomic::{AtomicU64,Ordering},
    time::{SystemTime,UNIX_EPOCH},
};

pub const SYMBOL_LIBRARY_FORMAT:&str="opencorel-symbol-library";
pub const SYMBOL_LIBRARY_VERSION:u32=1;
pub const MAX_LIBRARY_BYTES:usize=64*1024*1024;
pub const MAX_LIBRARY_SYMBOLS:usize=512;
pub const MAX_SYMBOL_OBJECTS:usize=500;
pub const MAX_SYMBOL_BYTES:usize=8*1024*1024;
pub const MAX_DOCUMENT_SYMBOL_BYTES:usize=32*1024*1024;
pub const MAX_SYMBOL_INSTANCES:usize=10_000;

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
pub struct SymbolObject{
    pub shape:Shape,
    pub style:Style,
    #[serde(default)]
    pub text_format:TextFormat,
    #[serde(default)]
    pub name:String,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub appearance:Option<Appearance>,
}
impl SymbolObject{
    fn from_object(object:&Object,appearance:Option<Appearance>)->Self{
        Self{shape:object.shape.clone(),style:object.style.clone(),
            text_format:object.text_format.clone(),name:object.name.clone(),appearance}
    }
    fn bounds(&self)->Bounds{
        Object{id:1,shape:self.shape.clone(),style:self.style.clone(),layer_id:1,
            text_format:self.text_format.clone(),name:self.name.clone(),group_id:None}.bounds()
    }
}

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
pub struct SymbolDefinition{
    /// Document-local identifier. A library key remains stable across documents.
    pub id:u64,
    pub library_key:String,
    pub name:String,
    #[serde(default)]
    pub category:String,
    pub objects:Vec<SymbolObject>,
}
impl SymbolDefinition{
    pub fn bounds(&self)->Bounds{union_symbol_bounds(&self.objects)}
    pub fn validate(&self)->Result<(),String>{
        if self.id==0{return Err("Invalid symbol identifier".into());}
        if self.library_key.len()<8||self.library_key.len()>128||
            !self.library_key.chars().all(|c|c.is_ascii_alphanumeric()||c=='-'||c=='_'){
            return Err("Invalid symbol library key".into());
        }
        if self.name.trim().is_empty()||self.name.len()>128||
            self.name.chars().any(char::is_control){
            return Err("Symbol name must contain 1–128 printable bytes".into());
        }
        if self.category.len()>64||self.category.chars().any(char::is_control){
            return Err("Symbol category exceeds 64 printable bytes".into());
        }
        if self.objects.is_empty()||self.objects.len()>MAX_SYMBOL_OBJECTS{
            return Err("A symbol must contain 1–500 objects".into());
        }
        let bytes=self.objects.iter().fold(0usize,|sum,item|
            sum.saturating_add(symbol_object_bytes(item)));
        if bytes>MAX_SYMBOL_BYTES{
            return Err("A symbol exceeds the 8 MiB embedded-resource budget".into());
        }
        let mut draft=Document::default();
        for item in &self.objects{
            let id=draft.add(item.shape.clone(),item.style.clone());
            let object=draft.get_mut(id).ok_or("Symbol object allocation failed")?;
            object.text_format=item.text_format.clone();
            object.name=item.name.clone();
            if let Some(appearance)=&item.appearance{
                draft.appearances.insert(id,appearance.clone());
            }
        }
        draft.validate().map_err(|e|format!("Invalid symbol artwork: {e}"))?;
        let bounds=self.bounds();
        if ![bounds.x,bounds.y,bounds.w,bounds.h].iter().all(|v|v.is_finite())||
            bounds.x.abs()>100_000.||bounds.y.abs()>100_000.||
            (bounds.x+bounds.w).abs()>100_000.||
            (bounds.y+bounds.h).abs()>100_000.{
            return Err("Symbol geometry exceeds the supported coordinate range".into());
        }
        Ok(())
    }
}

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
pub struct SymbolInstance{
    pub symbol_id:u64,
    pub page_id:u64,
    pub group_id:u64,
    /// Ordered one-to-one mapping to SymbolDefinition.objects.
    pub member_ids:Vec<u64>,
}

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SymbolLibrary{
    pub format:String,
    pub version:u32,
    pub symbols:Vec<SymbolDefinition>,
}
impl Default for SymbolLibrary{
    fn default()->Self{
        Self{format:SYMBOL_LIBRARY_FORMAT.into(),version:SYMBOL_LIBRARY_VERSION,
            symbols:Vec::new()}
    }
}
impl SymbolLibrary{
    pub fn validate(&self)->Result<(),String>{
        if self.format!=SYMBOL_LIBRARY_FORMAT||self.version!=SYMBOL_LIBRARY_VERSION{
            return Err("Unsupported OpenCorel symbol library format/version".into());
        }
        if self.symbols.len()>MAX_LIBRARY_SYMBOLS{
            return Err("Symbol library exceeds the 512-symbol limit".into());
        }
        let mut keys=HashSet::new();
        for symbol in &self.symbols{
            symbol.validate()?;
            if !keys.insert(symbol.library_key.as_str()){
                return Err("Duplicate symbol key in library".into());
            }
        }
        Ok(())
    }
    pub fn upsert(&mut self,symbol:SymbolDefinition)->Result<(),String>{
        symbol.validate()?;
        if let Some(existing)=self.symbols.iter_mut()
            .find(|item|item.library_key==symbol.library_key){
            *existing=symbol;
        }else{
            if self.symbols.len()>=MAX_LIBRARY_SYMBOLS{
                return Err("Symbol library limit (512) reached".into());
            }
            self.symbols.push(symbol);
        }
        self.symbols.sort_by(|a,b|a.category.cmp(&b.category)
            .then_with(||a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        Ok(())
    }
    pub fn remove(&mut self,key:&str)->bool{
        let len=self.symbols.len();
        self.symbols.retain(|item|item.library_key!=key);
        self.symbols.len()!=len
    }
    pub fn read(path:impl AsRef<Path>)->Result<Self,String>{
        let path=path.as_ref();
        let size=fs::metadata(path).map_err(|e|e.to_string())?.len();
        if size>MAX_LIBRARY_BYTES as u64{
            return Err("Symbol library exceeds the 64 MiB safety limit".into());
        }
        let bytes=fs::read(path).map_err(|e|e.to_string())?;
        let library:Self=serde_json::from_slice(&bytes)
            .map_err(|e|format!("Invalid symbol library JSON: {e}"))?;
        library.validate()?;
        Ok(library)
    }
    pub fn save_atomic(&self,path:impl AsRef<Path>)->Result<(),String>{
        self.validate()?;
        let bytes=serde_json::to_vec_pretty(self).map_err(|e|e.to_string())?;
        if bytes.len()>MAX_LIBRARY_BYTES{
            return Err("Symbol library exceeds the 64 MiB safety limit".into());
        }
        let path=path.as_ref();
        let parent=path.parent().filter(|p|!p.as_os_str().is_empty())
            .unwrap_or_else(||Path::new("."));
        fs::create_dir_all(parent).map_err(|e|format!("Cannot create library folder: {e}"))?;
        let mut temp=tempfile::Builder::new().prefix(".oclib-write-")
            .tempfile_in(parent).map_err(|e|e.to_string())?;
        temp.write_all(&bytes).map_err(|e|e.to_string())?;
        temp.as_file().sync_all().map_err(|e|e.to_string())?;
        temp.persist(path).map_err(|e|e.error.to_string())?;
        Ok(())
    }
    pub fn default_path()->PathBuf{
        #[cfg(target_os="windows")]
        let root=env::var_os("LOCALAPPDATA").or_else(||env::var_os("APPDATA"))
            .map(PathBuf::from).unwrap_or_else(env::temp_dir);
        #[cfg(target_os="macos")]
        let root=env::var_os("HOME").map(PathBuf::from)
            .unwrap_or_else(env::temp_dir).join("Library").join("Application Support");
        #[cfg(not(any(target_os="windows",target_os="macos")))]
        let root=env::var_os("XDG_DATA_HOME").map(PathBuf::from)
            .or_else(||env::var_os("HOME").map(|h|PathBuf::from(h).join(".local").join("share")))
            .unwrap_or_else(env::temp_dir);
        root.join("OpenCorel").join("Libraries").join("symbols.oclib")
    }
}

static KEY_COUNTER:AtomicU64=AtomicU64::new(1);
pub fn new_library_key()->String{
    let stamp=SystemTime::now().duration_since(UNIX_EPOCH)
        .unwrap_or_default().as_nanos();
    let count=KEY_COUNTER.fetch_add(1,Ordering::Relaxed);
    format!("{stamp:x}-{:x}-{count:x}",std::process::id())
}

fn symbol_object_bytes(object:&SymbolObject)->usize{
    std::mem::size_of::<SymbolObject>()+object.name.len()
        +object.text_format.family.len()+object.style.fill.len()+object.style.stroke.len()
        +match &object.shape{
            Shape::Text{value,..}=>value.len(),
            Shape::Polyline{points,..}=>points.len()*std::mem::size_of::<Point>(),
            Shape::Bezier{segments,..}=>segments.len()*std::mem::size_of::<crate::CubicSegment>(),
            Shape::CompoundPath{contours}=>contours.iter().map(|c|
                std::mem::size_of::<crate::Contour>()+
                c.segments.len()*std::mem::size_of::<crate::CubicSegment>()).sum(),
            Shape::EmbeddedSvg{svg,..}=>svg.len(),
            Shape::Raster{data_base64,..}=>data_base64.len(),
            _=>0,
        }
}
fn union_symbol_bounds(objects:&[SymbolObject])->Bounds{
    let mut left=f32::INFINITY;
    let mut top=f32::INFINITY;
    let mut right=f32::NEG_INFINITY;
    let mut bottom=f32::NEG_INFINITY;
    for object in objects{
        let b=object.bounds();
        left=left.min(b.x);top=top.min(b.y);
        right=right.max(b.x+b.w);bottom=bottom.max(b.y+b.h);
    }
    if objects.is_empty(){Bounds{x:0.,y:0.,w:0.,h:0.}}
    else{Bounds{x:left,y:top,w:(right-left).max(0.),h:(bottom-top).max(0.)}}
}
fn union_object_bounds(objects:&[Object])->Bounds{
    let mut left=f32::INFINITY;
    let mut top=f32::INFINITY;
    let mut right=f32::NEG_INFINITY;
    let mut bottom=f32::NEG_INFINITY;
    for object in objects{
        let b=object.bounds();
        left=left.min(b.x);top=top.min(b.y);
        right=right.max(b.x+b.w);bottom=bottom.max(b.y+b.h);
    }
    Bounds{x:left,y:top,w:(right-left).max(0.),h:(bottom-top).max(0.)}
}
fn page_objects(doc:&Document,page_id:u64)->Option<(&[Object],&BTreeMap<u64,Appearance>)>{
    if page_id==doc.active_page_id{Some((doc.objects.as_slice(),&doc.appearances))}
    else{doc.inactive_pages.get(&page_id).map(|p|(p.objects.as_slice(),&p.appearances))}
}
fn page_layer_editable(doc:&Document,page_id:u64,layer_id:u64)->bool{
    if page_id==doc.active_page_id{
        doc.layer(layer_id).is_some_and(|layer|layer.visible&&!layer.locked)
    }else{
        doc.inactive_pages.get(&page_id)
            .and_then(|page|page.layers.iter().find(|layer|layer.id==layer_id))
            .is_some_and(|layer|layer.visible&&!layer.locked)
    }
}
fn page_objects_mut(doc:&mut Document,page_id:u64)
    ->Option<(&mut Vec<Object>,&mut BTreeMap<u64,Appearance>)>{
    if page_id==doc.active_page_id{Some((&mut doc.objects,&mut doc.appearances))}
    else{doc.inactive_pages.get_mut(&page_id).map(|p|(&mut p.objects,&mut p.appearances))}
}
fn map_point(point:&mut Point,source:Bounds,target:Bounds,sx:f32,sy:f32){
    point.x=target.x+(point.x-source.x)*sx;
    point.y=target.y+(point.y-source.y)*sy;
}
fn map_shape(shape:&mut Shape,source:Bounds,target:Bounds)->Result<(),String>{
    let sx=if source.w.abs()>0.0001{target.w/source.w}else{1.0};
    let sy=if source.h.abs()>0.0001{target.h/source.h}else{1.0};
    if !sx.is_finite()||!sy.is_finite()||sx<=0.0||sy<=0.0||
        sx>10_000.0||sy>10_000.0{
        return Err("Symbol instance scale is outside the supported range".into());
    }
    match shape{
        Shape::Rectangle{x,y,w,h}|Shape::Ellipse{x,y,w,h}|
        Shape::EmbeddedSvg{x,y,w,h,..}|Shape::Raster{x,y,w,h,..}=>{
            let mut p=Point{x:*x,y:*y};
            map_point(&mut p,source,target,sx,sy);
            *x=p.x;*y=p.y;*w*=sx;*h*=sy;
        }
        Shape::Text{x,y,size_mm,..}=>{
            let mut p=Point{x:*x,y:*y};
            map_point(&mut p,source,target,sx,sy);
            *x=p.x;*y=p.y;*size_mm*=sy;
        }
        Shape::Polyline{points,..}=>{
            for point in points{map_point(point,source,target,sx,sy);}
        }
        Shape::Bezier{start,segments,..}=>{
            map_point(start,source,target,sx,sy);
            for segment in segments{
                map_point(&mut segment.control1,source,target,sx,sy);
                map_point(&mut segment.control2,source,target,sx,sy);
                map_point(&mut segment.end,source,target,sx,sy);
            }
        }
        Shape::CompoundPath{contours}=>{
            for contour in contours{
                map_point(&mut contour.start,source,target,sx,sy);
                for segment in &mut contour.segments{
                    map_point(&mut segment.control1,source,target,sx,sy);
                    map_point(&mut segment.control2,source,target,sx,sy);
                    map_point(&mut segment.end,source,target,sx,sy);
                }
            }
        }
    }
    let b=shape.bounds();
    if ![b.x,b.y,b.w,b.h].iter().all(|v|v.is_finite())||
        b.x.abs()>100_000.||b.y.abs()>100_000.||
        (b.x+b.w).abs()>100_000.||(b.y+b.h).abs()>100_000.{
        return Err("Updated symbol geometry exceeds the document coordinate range".into());
    }
    Ok(())
}
fn transformed_object(template:&SymbolObject,source:Bounds,target:Bounds,
    id:u64,layer_id:u64,group_id:u64)->Result<Object,String>{
    let mut shape=template.shape.clone();
    map_shape(&mut shape,source,target)?;
    let sx=if source.w.abs()>0.0001{target.w/source.w}else{1.0};
    let mut text_format=template.text_format.clone();
    text_format.letter_spacing_mm*=sx;
    Ok(Object{id,shape,style:template.style.clone(),layer_id,text_format,
        name:template.name.clone(),group_id:Some(group_id)})
}

impl Document{
    pub fn symbol_definition(&self,id:u64)->Option<&SymbolDefinition>{
        self.symbols.iter().find(|symbol|symbol.id==id)
    }
    pub fn symbol_instance_for_group(&self,group_id:u64)->Option<&SymbolInstance>{
        self.symbol_instances.iter().find(|item|item.group_id==group_id)
    }
    pub fn symbol_instance_for_object(&self,object_id:u64)->Option<&SymbolInstance>{
        self.symbol_instances.iter().find(|item|item.member_ids.contains(&object_id))
    }
    pub fn import_symbol_definition(&mut self,definition:&SymbolDefinition)->Result<u64,String>{
        definition.validate()?;
        if let Some(existing)=self.symbols.iter()
            .find(|item|item.library_key==definition.library_key).cloned(){
            let mut replacement=definition.clone();
            replacement.id=existing.id;
            if replacement!=existing{
                self.replace_symbol_definition(existing.id,replacement)?;
            }
            return Ok(existing.id);
        }
        if self.symbols.len()>=MAX_LIBRARY_SYMBOLS||self.next_symbol_id>=u64::MAX-1{
            return Err("Document symbol limit reached".into());
        }
        let mut imported=definition.clone();
        imported.id=self.next_symbol_id;
        let mut draft=self.clone();
        draft.next_symbol_id+=1;
        draft.symbols.push(imported.clone());
        draft.validate()?;
        *self=draft;
        Ok(imported.id)
    }
    pub fn create_symbol(&mut self,ids:&[u64],name:&str,category:&str)
        ->Result<SymbolDefinition,String>{
        let wanted:HashSet<u64>=ids.iter().copied().collect();
        if wanted.is_empty()||wanted.len()!=ids.len(){
            return Err("Select one or more unique objects".into());
        }
        if wanted.len()>MAX_SYMBOL_OBJECTS{
            return Err("A symbol may contain at most 500 objects".into());
        }
        if self.symbols.len()>=MAX_LIBRARY_SYMBOLS||
            self.symbol_instances.len()>=MAX_SYMBOL_INSTANCES||
            self.next_symbol_id>=u64::MAX-1||self.next_id>=u64::MAX-1{
            return Err("Document symbol capacity reached".into());
        }
        let editable:HashSet<u64>=self.layers.iter()
            .filter(|layer|layer.visible&&!layer.locked).map(|layer|layer.id).collect();
        let chosen:Vec<_>=self.objects.iter().filter(|object|wanted.contains(&object.id)).collect();
        if chosen.len()!=wanted.len(){return Err("Selection contains an unknown object".into());}
        if chosen.iter().any(|object|!editable.contains(&object.layer_id)||
            object.group_id.is_some()){
            return Err("Symbols require visible, unlocked, ungrouped objects".into());
        }
        if self.clip_links.iter().any(|link|
            wanted.contains(&link.source_id)||wanted.contains(&link.target_id)){
            return Err("Clipped objects must be detached before creating a symbol".into());
        }
        let templates=chosen.iter().map(|object|SymbolObject::from_object(
            object,self.appearances.get(&object.id).cloned())).collect();
        let definition=SymbolDefinition{id:self.next_symbol_id,
            library_key:new_library_key(),name:name.trim().to_owned(),
            category:category.trim().to_owned(),objects:templates};
        definition.validate()?;
        let mut draft=self.clone();
        let group_id=draft.next_id;
        draft.next_id+=1;
        for object in &mut draft.objects{
            if wanted.contains(&object.id){object.group_id=Some(group_id);}
        }
        draft.next_symbol_id+=1;
        draft.symbols.push(definition.clone());
        draft.symbol_instances.push(SymbolInstance{symbol_id:definition.id,
            page_id:draft.active_page_id,group_id,member_ids:chosen.iter().map(|o|o.id).collect()});
        draft.validate()?;
        *self=draft;
        Ok(definition)
    }
    pub fn insert_symbol(&mut self,symbol_id:u64,layer_id:u64,x:f32,y:f32)
        ->Result<Vec<u64>,String>{
        if !x.is_finite()||!y.is_finite()||x.abs()>100_000.||y.abs()>100_000.{
            return Err("Invalid symbol insertion position".into());
        }
        if !self.layer(layer_id).is_some_and(|l|l.visible&&!l.locked){
            return Err("Target layer is hidden or locked".into());
        }
        if self.symbol_instances.len()>=MAX_SYMBOL_INSTANCES{
            return Err("Symbol instance limit reached".into());
        }
        let definition=self.symbol_definition(symbol_id)
            .ok_or("Symbol definition not found")?.clone();
        if self.all_object_count()+definition.objects.len()>100_000{
            return Err("Whole document object limit exceeded".into());
        }
        let source=definition.bounds();
        let needed=definition.objects.len() as u64+1;
        if self.next_id.checked_add(needed).is_none_or(|next|next>=u64::MAX){
            return Err("Object identifier limit reached".into());
        }
        let mut draft=self.clone();
        let mut ids=Vec::with_capacity(definition.objects.len());
        for template in &definition.objects{
            let mut shape=template.shape.clone();
            map_shape(&mut shape,source,Bounds{x,y,w:source.w,h:source.h})?;
            let id=draft.add_to_layer(shape,template.style.clone(),layer_id);
            let object=draft.get_mut(id).ok_or("Inserted symbol object missing")?;
            object.text_format=template.text_format.clone();
            object.name=template.name.clone();
            if let Some(appearance)=&template.appearance{
                draft.appearances.insert(id,appearance.clone());
            }
            ids.push(id);
        }
        if draft.next_id>=u64::MAX-1{return Err("Object identifier limit reached".into());}
        let group_id=draft.next_id;draft.next_id+=1;
        for id in &ids{draft.get_mut(*id).ok_or("Inserted object missing")?.group_id=Some(group_id);}
        draft.symbol_instances.push(SymbolInstance{symbol_id,page_id:draft.active_page_id,
            group_id,member_ids:ids.clone()});
        draft.validate()?;
        *self=draft;
        Ok(ids)
    }
    pub fn update_symbol_from_group(&mut self,group_id:u64)
        ->Result<SymbolDefinition,String>{
        let instance=self.symbol_instance_for_group(group_id)
            .ok_or("Selected group is not a linked symbol instance")?.clone();
        let (objects,appearances)=page_objects(self,instance.page_id)
            .ok_or("Symbol instance page is missing")?;
        let mut source=Vec::with_capacity(instance.member_ids.len());
        for id in &instance.member_ids{
            let object=objects.iter().find(|item|item.id==*id)
                .ok_or("Symbol instance member is missing")?;
            if object.group_id!=Some(group_id)||
                !page_layer_editable(self,instance.page_id,object.layer_id){
                return Err("Symbol source must be on visible, unlocked layers".into());
            }
            source.push(SymbolObject::from_object(object,appearances.get(id).cloned()));
        }
        let old=self.symbol_definition(instance.symbol_id)
            .ok_or("Symbol definition is missing")?;
        let definition=SymbolDefinition{id:old.id,library_key:old.library_key.clone(),
            name:old.name.clone(),category:old.category.clone(),objects:source};
        self.replace_symbol_definition(definition.id,definition.clone())?;
        Ok(definition)
    }
    pub fn replace_symbol_definition(&mut self,symbol_id:u64,
        mut definition:SymbolDefinition)->Result<(),String>{
        let old=self.symbol_definition(symbol_id)
            .ok_or("Symbol definition not found")?.clone();
        definition.id=symbol_id;
        if definition.library_key!=old.library_key{
            return Err("A symbol update cannot change its library key".into());
        }
        definition.validate()?;
        let mut draft=self.clone();
        let index=draft.symbols.iter().position(|item|item.id==symbol_id)
            .ok_or("Symbol definition not found")?;
        draft.symbols[index]=definition.clone();
        let instances:Vec<_>=draft.symbol_instances.iter()
            .filter(|item|item.symbol_id==symbol_id).cloned().collect();
        for instance in instances{
            apply_definition_to_instance(&mut draft,&definition,&instance)?;
        }
        draft.validate()?;
        *self=draft;
        Ok(())
    }
    pub fn detach_symbol_instance(&mut self,group_id:u64)->Result<usize,String>{
        let index=self.symbol_instances.iter().position(|item|item.group_id==group_id)
            .ok_or("Linked symbol instance not found")?;
        let instance=self.symbol_instances[index].clone();
        let mut draft=self.clone();
        let (objects,_)=page_objects_mut(&mut draft,instance.page_id)
            .ok_or("Symbol instance page is missing")?;
        let mut count=0;
        for object in objects.iter_mut().filter(|item|instance.member_ids.contains(&item.id)){
            if object.group_id==Some(group_id){object.group_id=None;count+=1;}
        }
        if count!=instance.member_ids.len(){
            return Err("Symbol instance membership is inconsistent".into());
        }
        draft.symbol_instances.remove(index);
        draft.validate()?;
        *self=draft;
        Ok(count)
    }
    /// Recreate linked symbol metadata for a copied set of objects.
    /// Missing definitions (for example, clipboard content pasted into another
    /// document) are intentionally left as ordinary grouped artwork.
    pub fn clone_symbol_instances(&mut self,source:&[SymbolInstance],
        id_map:&BTreeMap<u64,u64>)->Result<usize,String>{
        let mut draft=self.clone();
        let mut created=0usize;
        for instance in source{
            if draft.symbol_definition(instance.symbol_id).is_none(){continue;}
            let Some(member_ids)=instance.member_ids.iter()
                .map(|old|id_map.get(old).copied()).collect::<Option<Vec<_>>>() else{
                continue;
            };
            if member_ids.is_empty()||draft.symbol_instances.len()>=MAX_SYMBOL_INSTANCES{
                return Err("Cannot duplicate symbol instance: capacity reached".into());
            }
            let objects:Vec<_>=member_ids.iter().map(|id|
                draft.get(*id).cloned().ok_or("Copied symbol object is missing"))
                .collect::<Result<_,_>>()?;
            let group_ids:HashSet<_>=objects.iter().filter_map(|object|object.group_id).collect();
            if group_ids.len()>1{
                return Err("Copied symbol members have inconsistent groups".into());
            }
            let group_id=if let Some(group)=group_ids.iter().next().copied(){
                group
            }else{
                if draft.next_id>=u64::MAX-1{return Err("Group identifier limit reached".into());}
                let group=draft.next_id;draft.next_id+=1;group
            };
            for id in &member_ids{
                let object=draft.get_mut(*id).ok_or("Copied symbol object is missing")?;
                if object.group_id.is_some_and(|group|group!=group_id){
                    return Err("Copied symbol members have inconsistent groups".into());
                }
                object.group_id=Some(group_id);
            }
            draft.symbol_instances.push(SymbolInstance{
                symbol_id:instance.symbol_id,page_id:draft.active_page_id,
                group_id,member_ids
            });
            created+=1;
        }
        if created>0{
            draft.validate()?;
            *self=draft;
        }
        Ok(created)
    }
    pub fn validate_symbols(&self)->Result<(),String>{
        if self.next_symbol_id==0||self.next_symbol_id==u64::MAX||
            self.symbols.len()>MAX_LIBRARY_SYMBOLS||
            self.symbol_instances.len()>MAX_SYMBOL_INSTANCES{
            return Err("Invalid symbol allocation state".into());
        }
        let mut symbol_ids=HashSet::new();
        let mut keys=HashSet::new();
        let mut symbol_bytes=0usize;
        for symbol in &self.symbols{
            symbol_bytes=symbol_bytes.saturating_add(symbol.objects.iter()
                .map(symbol_object_bytes).sum::<usize>());
            if symbol_bytes>MAX_DOCUMENT_SYMBOL_BYTES{
                return Err("Document symbol definitions exceed the 32 MiB resource budget".into());
            }
            symbol.validate()?;
            if symbol.id>=self.next_symbol_id||!symbol_ids.insert(symbol.id)||
                !keys.insert(symbol.library_key.as_str()){
                return Err("Duplicate or out-of-range symbol definition".into());
            }
        }
        let pages=self.pages();
        let mut group_ids=HashSet::new();
        let mut member_ids=HashSet::new();
        let mut all_object_ids:HashSet<u64>=self.objects.iter().map(|o|o.id).collect();
        for page in self.inactive_pages.values(){
            all_object_ids.extend(page.objects.iter().map(|o|o.id));
        }
        for instance in &self.symbol_instances{
            let definition=self.symbol_definition(instance.symbol_id)
                .ok_or("Symbol instance references an unknown definition")?;
            if instance.group_id==0||instance.group_id>=self.next_id||
                all_object_ids.contains(&instance.group_id)||
                !group_ids.insert(instance.group_id)||
                !pages.iter().any(|page|page.id==instance.page_id)||
                instance.member_ids.is_empty()||
                instance.member_ids.len()!=definition.objects.len()||
                instance.member_ids.len()>MAX_SYMBOL_OBJECTS{
                return Err("Invalid symbol instance metadata".into());
            }
            let (objects,_)=page_objects(self,instance.page_id)
                .ok_or("Symbol instance references a missing page")?;
            let ids:HashSet<_>=instance.member_ids.iter().copied().collect();
            if ids.len()!=instance.member_ids.len()||
                instance.member_ids.iter().any(|id|!member_ids.insert(*id)){
                return Err("Duplicate symbol instance member".into());
            }
            let members:Vec<_>=objects.iter().filter(|object|
                object.group_id==Some(instance.group_id)).collect();
            if members.len()!=instance.member_ids.len()||
                members.iter().any(|object|!ids.contains(&object.id))||
                instance.member_ids.iter().any(|id|
                    !objects.iter().any(|object|object.id==*id&&
                        object.group_id==Some(instance.group_id))){
                return Err("Symbol instance group is incomplete".into());
            }
        }
        Ok(())
    }
}

fn apply_definition_to_instance(doc:&mut Document,definition:&SymbolDefinition,
    instance:&SymbolInstance)->Result<(),String>{
    let (existing,_) = page_objects(doc,instance.page_id)
        .ok_or("Symbol instance page is missing")?;
    let old_objects:Vec<Object>=instance.member_ids.iter().map(|id|
        existing.iter().find(|object|object.id==*id).cloned()
            .ok_or("Symbol instance member is missing")).collect::<Result<_,_>>()?;
    if old_objects.iter().any(|object|object.group_id!=Some(instance.group_id)){
        return Err("Symbol instance group is inconsistent".into());
    }
    let target=union_object_bounds(&old_objects);
    let source=definition.bounds();
    let fallback_layer=old_objects.first().map(|object|object.layer_id).unwrap_or(1);
    let mut next_ids=Vec::with_capacity(definition.objects.len());
    let mut replacements=Vec::with_capacity(definition.objects.len());
    for (index,template) in definition.objects.iter().enumerate(){
        let id=if let Some(old)=old_objects.get(index){old.id}else{
            if doc.next_id>=u64::MAX-1{return Err("Object identifier limit reached".into());}
            let id=doc.next_id;doc.next_id+=1;id
        };
        let layer=old_objects.get(index).map(|object|object.layer_id).unwrap_or(fallback_layer);
        let object=transformed_object(template,source,target,id,layer,instance.group_id)?;
        next_ids.push(id);
        replacements.push(object);
    }
    let (objects,appearances)=page_objects_mut(doc,instance.page_id)
        .ok_or("Symbol instance page is missing")?;
    let retained:HashSet<u64>=next_ids.iter().copied().collect();
    for old in &old_objects{
        if !retained.contains(&old.id){appearances.remove(&old.id);}
    }
    objects.retain(|object|!instance.member_ids.contains(&object.id)||retained.contains(&object.id));
    for replacement in replacements{
        if let Some(existing)=objects.iter_mut().find(|object|object.id==replacement.id){
            *existing=replacement;
        }else{
            objects.push(replacement);
        }
    }
    for (id,template) in next_ids.iter().zip(definition.objects.iter()){
        if let Some(appearance)=&template.appearance{
            appearances.insert(*id,appearance.clone());
        }else{
            appearances.remove(id);
        }
    }
    if let Some(item)=doc.symbol_instances.iter_mut().find(|item|
        item.group_id==instance.group_id&&item.page_id==instance.page_id){
        item.member_ids=next_ids;
    }
    Ok(())
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Page,Shape,Style};

    fn rect(x:f32,y:f32)->Shape{Shape::Rectangle{x,y,w:20.,h:10.}}
    fn add_pair(doc:&mut Document)->Vec<u64>{
        vec![
            doc.add(rect(10.,10.),Style::default()),
            doc.add(Shape::Text{x:35.,y:10.,value:"Logo".into(),size_mm:8.},Style::default())
        ]
    }
    #[test]
    fn create_insert_and_roundtrip_linked_instances(){
        let mut doc=Document::default();
        let source=add_pair(&mut doc);
        let symbol=doc.create_symbol(&source,"Brand mark","Identity").unwrap();
        assert_eq!(doc.symbol_instances.len(),1);
        let inserted=doc.insert_symbol(symbol.id,1,100.,80.).unwrap();
        assert_eq!(inserted.len(),2);
        assert_ne!(inserted[0],source[0]);
        doc.validate().unwrap();
        let json=serde_json::to_vec(&doc).unwrap();
        let decoded:Document=serde_json::from_slice(&json).unwrap();
        decoded.validate().unwrap();
        assert_eq!(decoded.symbol_instances.len(),2);
    }
    #[test]
    fn definition_update_propagates_and_preserves_each_instance_position(){
        let mut doc=Document::default();
        let source=add_pair(&mut doc);
        let symbol=doc.create_symbol(&source,"Logo","Marks").unwrap();
        let second=doc.insert_symbol(symbol.id,1,100.,100.).unwrap();
        let second_before_objects:Vec<_>=second.iter()
            .map(|id|doc.get(*id).unwrap().clone()).collect();
        let second_before=union_object_bounds(&second_before_objects);
        let group=doc.symbol_instance_for_object(source[0]).unwrap().group_id;
        if let Shape::Text{value,..}=&mut doc.get_mut(source[1]).unwrap().shape{
            *value="Updated".into();
        }
        doc.get_mut(source[0]).unwrap().shape=Shape::Rectangle{x:10.,y:10.,w:40.,h:20.};
        let updated=doc.update_symbol_from_group(group).unwrap();
        assert_eq!(updated.objects.len(),2);
        let second_after_objects:Vec<_>=second.iter()
            .map(|id|doc.get(*id).unwrap().clone()).collect();
        let second_after=union_object_bounds(&second_after_objects);
        assert!((second_after.x-second_before.x).abs()<0.001);
        assert!((second_after.y-second_before.y).abs()<0.001);
        assert!((second_after.w-second_before.w).abs()<0.001);
        assert!((second_after.h-second_before.h).abs()<0.001);
        assert!(matches!(&doc.get(second[1]).unwrap().shape,
            Shape::Text{value,..} if value=="Updated"));
        assert!(matches!(&doc.get(source[1]).unwrap().shape,
            Shape::Text{value,..} if value=="Updated"));
        doc.validate().unwrap();
    }
    #[test]
    fn detach_breaks_link_without_destroying_group_or_geometry(){
        let mut doc=Document::default();
        let ids=add_pair(&mut doc);
        let symbol=doc.create_symbol(&ids,"Badge","UI").unwrap();
        let group=doc.symbol_instance_for_object(ids[0]).unwrap().group_id;
        let before=doc.get(ids[0]).unwrap().clone();
        assert_eq!(doc.detach_symbol_instance(group).unwrap(),2);
        assert!(doc.symbol_instance_for_group(group).is_none());
        assert!(doc.get(ids[0]).unwrap().group_id.is_none());
        assert_eq!(doc.get(ids[0]).unwrap().shape,before.shape);
        assert!(doc.symbol_definition(symbol.id).is_some());
        doc.validate().unwrap();
    }
    #[test]
    fn update_can_add_and_remove_members_atomically(){
        let mut doc=Document::default();
        let ids=add_pair(&mut doc);
        let symbol=doc.create_symbol(&ids,"Two","Test").unwrap();
        let copy=doc.insert_symbol(symbol.id,1,100.,100.).unwrap();
        let group=doc.symbol_instance_for_object(ids[0]).unwrap().group_id;
        doc.objects.retain(|object|object.id!=ids[1]);
        doc.symbol_instances.iter_mut().find(|item|item.group_id==group)
            .unwrap().member_ids.retain(|id|*id!=ids[1]);
        doc.objects.push(Object{id:doc.next_id,shape:Shape::Ellipse{x:40.,y:10.,w:10.,h:10.},
            style:Style::default(),layer_id:1,text_format:TextFormat::default(),
            name:"New".into(),group_id:Some(group)});
        doc.next_id+=1;
        doc.symbol_instances.iter_mut().find(|item|item.group_id==group)
            .unwrap().member_ids.push(doc.next_id-1);
        let updated=doc.update_symbol_from_group(group).unwrap();
        assert_eq!(updated.objects.len(),2);
        assert_eq!(doc.symbol_instance_for_object(copy[0]).unwrap().member_ids.len(),2);
        doc.validate().unwrap();
    }
    #[test]
    fn old_document_json_without_symbol_fields_remains_readable(){
        let json=r#"{"schema_version":1,"name":"Legacy","page":{"width_mm":50.0,"height_mm":50.0},"objects":[],"next_id":1}"#;
        let doc:Document=serde_json::from_str(json).unwrap();
        assert!(doc.symbols.is_empty());
        assert!(doc.symbol_instances.is_empty());
        doc.validate().unwrap();
    }
    #[test]
    fn symbol_library_is_atomic_and_rejects_corruption(){
        let dir=tempfile::tempdir().unwrap();
        let path=dir.path().join("symbols.oclib");
        let mut doc=Document::default();
        let ids=add_pair(&mut doc);
        let symbol=doc.create_symbol(&ids,"Library item","Shapes").unwrap();
        let mut library=SymbolLibrary::default();
        library.upsert(symbol).unwrap();
        library.save_atomic(&path).unwrap();
        let loaded=SymbolLibrary::read(&path).unwrap();
        assert_eq!(loaded.symbols.len(),1);
        let mut changed=loaded.clone();
        changed.symbols[0].name="Updated library item".into();
        changed.save_atomic(&path).unwrap();
        assert_eq!(SymbolLibrary::read(&path).unwrap().symbols[0].name,
            "Updated library item");
        fs::write(&path,b"{broken").unwrap();
        assert!(SymbolLibrary::read(&path).is_err());
    }
    #[test]
    fn duplicate_page_duplicates_linked_instances_with_new_ids(){
        let mut doc=Document::default();
        let ids=add_pair(&mut doc);
        let symbol=doc.create_symbol(&ids,"Page logo","Marks").unwrap();
        let old_page=doc.active_page_id;
        let new_page=doc.duplicate_page().unwrap();
        assert_eq!(doc.symbol_instances.len(),2);
        let copy=doc.symbol_instances.iter().find(|item|item.page_id==new_page).unwrap();
        assert_eq!(copy.symbol_id,symbol.id);
        assert!(copy.member_ids.iter().all(|id|!ids.contains(id)));
        doc.switch_page(old_page).unwrap();
        doc.validate().unwrap();
    }
    #[test]
    fn importing_a_library_key_reuses_and_updates_document_definition(){
        let mut source_doc=Document::default();
        let ids=add_pair(&mut source_doc);
        let original=source_doc.create_symbol(&ids,"Portable mark","Brand").unwrap();
        let mut destination=Document::default();
        let local_id=destination.import_symbol_definition(&original).unwrap();
        assert_eq!(destination.import_symbol_definition(&original).unwrap(),local_id);
        let instance=destination.insert_symbol(local_id,1,40.,50.).unwrap();
        let mut updated=original.clone();
        if let Shape::Rectangle{w,..}=&mut updated.objects[0].shape{*w=60.;}
        destination.import_symbol_definition(&updated).unwrap();
        assert!(destination.get(instance[0]).unwrap().bounds().w>20.);
        assert_eq!(destination.symbols.len(),1);
        destination.validate().unwrap();
    }
    #[test]
    fn invalid_symbol_update_leaves_all_instances_unchanged(){
        let mut doc=Document::default();
        let ids=add_pair(&mut doc);
        let symbol=doc.create_symbol(&ids,"Stable","Test").unwrap();
        let second=doc.insert_symbol(symbol.id,1,100.,100.).unwrap();
        let before=doc.clone();
        let mut invalid=symbol.clone();
        invalid.objects.clear();
        assert!(doc.replace_symbol_definition(symbol.id,invalid).is_err());
        assert_eq!(doc,before);
        assert!(doc.get(second[0]).is_some());
    }
    #[test]
    fn deleting_or_ungrouping_a_linked_member_detaches_the_instance(){
        let mut deleted=Document::default();
        let ids=add_pair(&mut deleted);
        deleted.create_symbol(&ids,"Delete test","Test").unwrap();
        assert!(deleted.remove(ids[0]));
        assert!(deleted.symbol_instances.is_empty());
        assert!(deleted.get(ids[1]).unwrap().group_id.is_none());
        deleted.validate().unwrap();

        let mut ungrouped=Document::default();
        let ids=add_pair(&mut ungrouped);
        ungrouped.create_symbol(&ids,"Ungroup test","Test").unwrap();
        assert_eq!(ungrouped.ungroup_objects(&ids).unwrap(),2);
        assert!(ungrouped.symbol_instances.is_empty());
        assert!(ids.iter().all(|id|ungrouped.get(*id).unwrap().group_id.is_none()));
        ungrouped.validate().unwrap();
    }
    #[test]
    fn single_object_symbol_can_be_copied_without_losing_its_link(){
        let mut doc=Document::default();
        let id=doc.add(rect(10.,10.),Style::default());
        let symbol=doc.create_symbol(&[id],"Single","Test").unwrap();
        let instance=doc.symbol_instances[0].clone();
        let original=doc.get(id).unwrap().clone();
        let copy=doc.copy_object(&original,1,30.,0.);
        let map=BTreeMap::from([(id,copy)]);
        assert_eq!(doc.clone_symbol_instances(&[instance],&map).unwrap(),1);
        assert_eq!(doc.symbol_instances.len(),2);
        assert_eq!(doc.get(copy).unwrap().group_id,Some(doc.symbol_instances[1].group_id));
        assert_eq!(doc.symbols[0].id,symbol.id);
        doc.validate().unwrap();
    }
    #[test]
    fn page_deletion_removes_instances_but_keeps_library_definition(){
        let mut doc=Document::default();
        let ids=add_pair(&mut doc);
        let symbol=doc.create_symbol(&ids,"Page logo","Marks").unwrap();
        let page=doc.add_page(Page{width_mm:100.,height_mm:100.}).unwrap();
        doc.insert_symbol(symbol.id,1,10.,10.).unwrap();
        doc.switch_page(1).unwrap();
        doc.delete_page(page).unwrap();
        assert_eq!(doc.symbol_instances.len(),1);
        doc.validate().unwrap();
    }
}
