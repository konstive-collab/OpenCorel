//! Bounded, non-destructive real-font text preview.
//! The document retains editable Shape::Text; outline geometry is only cached
//! for the current canvas frame. Missing fonts fall back to egui's text view.
use opencorel_core::{Contour,Object,Shape};
use opencorel_typography::TextOutliner;
use std::{collections::HashMap,collections::hash_map::DefaultHasher,
    hash::{Hash,Hasher}};

const MAX_PREVIEW_CHARS:usize=600;
const MAX_CACHED_OBJECTS:usize=24;

#[derive(Default)]
pub(super) struct TextPreview{
    entries:HashMap<u64,PreviewEntry>,
}
struct PreviewEntry{
    signature:u64,
    contours:Option<Vec<Contour>>,
}
fn signature(object:&Object)->u64{
    let mut h=DefaultHasher::new();
    // TextFormat contains floats, so derive a stable signature from its Debug
    // representation rather than implementing Hash on externally defined types.
    format!("{:?}:{:?}",object.shape,object.text_format).hash(&mut h);
    h.finish()
}
impl TextPreview{
    pub fn clear(&mut self){self.entries.clear();}

    pub fn contours<'a>(&'a mut self,object:&Object,
        outliner:&mut TextOutliner)->Option<&'a [Contour]>{
        let Shape::Text{value,..}=&object.shape else{return None;};
        // Large documents use the lightweight text fallback. This prevents a
        // costly outline operation during pan/zoom or inspector editing.
        if value.chars().count()>MAX_PREVIEW_CHARS || value.trim().is_empty(){
            self.entries.remove(&object.id);
            return None;
        }
        let hash=signature(object);
        if self.entries.get(&object.id).map(|e|e.signature!=hash).unwrap_or(true){
            if self.entries.len()>=MAX_CACHED_OBJECTS{
                if let Some(evicted)=self.entries.keys().next().copied(){
                    self.entries.remove(&evicted);
                }
            }
            let contours=match outliner.convert_object(object){
                Ok(Shape::CompoundPath{contours})=>Some(contours),
                _=>None,
            };
            self.entries.insert(object.id,PreviewEntry{
                signature:hash,contours
            });
        }
        self.entries.get(&object.id)
            .and_then(|entry|entry.contours.as_deref())
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Document,Style};
    #[test]
    fn signature_changes_when_text_or_font_changes(){
        let mut doc=Document::default();
        doc.add(Shape::Text{x:1.0,y:2.0,value:"АБВ".into(),size_mm:10.0},
            Style::default());
        let mut obj=doc.objects[0].clone();
        let a=signature(&obj);
        obj.text_format.family="serif".into();
        assert_ne!(a,signature(&obj));
        let b=signature(&obj);
        if let Shape::Text{value,..}=&mut obj.shape{value.push('Г');}
        assert_ne!(b,signature(&obj));
    }
    #[test]
    fn empty_and_large_text_are_not_cached(){
        let mut doc=Document::default();
        doc.add(Shape::Text{x:0.0,y:0.0,value:String::new(),size_mm:10.0},
            Style::default());
        let mut obj=doc.objects[0].clone();
        let mut cache=TextPreview::default();
        let mut outliner=TextOutliner::new();
        assert!(cache.contours(&obj,&mut outliner).is_none());
        assert!(cache.entries.is_empty());
        if let Shape::Text{value,..}=&mut obj.shape{
            *value="Ж".repeat(MAX_PREVIEW_CHARS+1);
        }
        assert!(cache.contours(&obj,&mut outliner).is_none());
        assert!(cache.entries.is_empty());
    }
}
