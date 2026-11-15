//! Bounded, non-destructive real-font text preview.
//! The document retains editable Shape::Text; outline geometry is only cached
//! for the current canvas frame. Missing fonts fall back to egui's text view.
use opencorel_core::{Bounds,Contour,Object,Shape};
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
    ink_bounds:Option<Bounds>,
}
fn signature(object:&Object)->u64{
    let mut h=DefaultHasher::new();
    // TextFormat contains floats, so derive a stable signature from its Debug
    // representation rather than implementing Hash on externally defined types.
    format!("{:?}:{:?}",object.shape,object.text_format).hash(&mut h);
    h.finish()
}
/// Conservative control-hull envelope; Bezier segments never leave this box.
fn contour_bounds(contours:&[Contour])->Option<Bounds>{
    let mut min_x=f32::INFINITY;
    let mut min_y=f32::INFINITY;
    let mut max_x=f32::NEG_INFINITY;
    let mut max_y=f32::NEG_INFINITY;
    let mut found=false;
    for contour in contours{
        for p in std::iter::once(&contour.start).chain(contour.segments.iter()
            .flat_map(|segment|[&segment.control1,&segment.control2,&segment.end])){
            if !p.x.is_finite() || !p.y.is_finite(){return None;}
            min_x=min_x.min(p.x);min_y=min_y.min(p.y);
            max_x=max_x.max(p.x);max_y=max_y.max(p.y);
            found=true;
        }
    }
    if !found{return None;}
    Some(Bounds{x:min_x,y:min_y,w:max_x-min_x,h:max_y-min_y})
}
impl TextPreview{
    /// Returns bounds only for fresh cached contours (never stale positions).
    pub fn ink_bounds(&self,object:&Object)->Option<Bounds>{
        let entry=self.entries.get(&object.id)?;
        if entry.signature!=signature(object){return None;}
        entry.ink_bounds
    }

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
            let ink_bounds=contours.as_deref().and_then(contour_bounds);
            self.entries.insert(object.id,PreviewEntry{
                signature:hash,contours,ink_bounds
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
    fn glyph_control_hull_and_invalid_coordinates(){
        use opencorel_core::{Point,CubicSegment};
        let contour=Contour{
            start:Point{x:1.0,y:2.0},
            segments:vec![CubicSegment{
                control1:Point{x:-4.0,y:6.0},
                control2:Point{x:7.0,y:-3.0},
                end:Point{x:4.0,y:5.0},
            }],
            closed:true,
        };
        let b=contour_bounds(&[contour.clone()]).unwrap();
        assert_eq!((b.x,b.y,b.w,b.h),(-4.0,-3.0,11.0,9.0));
        assert!(contour_bounds(&[]).is_none());
        let mut bad=contour;
        bad.start.x=f32::NAN;
        assert!(contour_bounds(&[bad]).is_none());
    }
    #[test]
    fn stale_text_bounds_are_not_used(){
        let mut doc=Document::default();
        doc.add(Shape::Text{x:1.0,y:2.0,value:"Test".into(),size_mm:10.0},
            Style::default());
        let mut obj=doc.objects[0].clone();
        let mut preview=TextPreview::default();
        let mut outliner=TextOutliner::new();
        let _=preview.contours(&obj,&mut outliner);
        if let Shape::Text{x,..}=&mut obj.shape{*x+=5.0;}
        assert!(preview.ink_bounds(&obj).is_none());
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
