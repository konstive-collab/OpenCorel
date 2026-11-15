//! Thumbnail renderer for embedded raster photos, independent of displayed size.
//! The original PNG/JPEG remains inside the editable .kdr document.
use base64::Engine;
use eframe::egui::{self,Color32,Pos2,Rect,TextureHandle};
use std::{collections::HashMap,hash::{Hash,Hasher},collections::hash_map::DefaultHasher};

const PREVIEW_SIDE:u32=1024;
const MAX_CACHED_IMAGES:usize=6;
#[derive(Default)]
pub(super) struct RasterCache {
    entries:HashMap<u64,(u64,TextureHandle)>,
    errors:HashMap<u64,String>,
}
impl RasterCache {
    pub fn clear(&mut self){self.entries.clear();self.errors.clear();}
    pub fn paint(&mut self,ctx:&egui::Context,painter:&egui::Painter,id:u64,
        base64_data:&str,rect:Rect){
        if rect.width()<=0.0 || rect.height()<=0.0 {return;}
        let mut hasher=DefaultHasher::new();
        // Source buffers do not change during a drag; document open/undo clears cache.
        (base64_data.as_ptr() as usize).hash(&mut hasher);
        base64_data.len().hash(&mut hasher);
        let signature=hasher.finish();
        if self.entries.get(&id).map(|(h,_)|*h!=signature).unwrap_or(true){
            match decode_thumbnail(base64_data){
                Ok(image)=>{
                    if self.entries.len()>=MAX_CACHED_IMAGES{
                        if let Some(k)=self.entries.keys().copied().next(){self.entries.remove(&k);}
                    }
                    let texture=ctx.load_texture(format!("raster-{id}-{signature}"),image,
                        egui::TextureOptions::LINEAR);
                    self.entries.insert(id,(signature,texture));
                    self.errors.remove(&id);
                }
                Err(e)=>{self.entries.remove(&id);self.errors.insert(id,e);}
            }
        }
        if let Some((_,texture))=self.entries.get(&id){
            painter.image(texture.id(),rect,
                Rect::from_min_max(Pos2::ZERO,Pos2::new(1.0,1.0)),
                Color32::WHITE);
        }else{
            painter.rect_filled(rect,0.0,Color32::from_rgb(240,220,220));
            painter.text(rect.center(),egui::Align2::CENTER_CENTER,
                "Изображение повреждено или превышает лимит",
                egui::FontId::proportional(12.0),Color32::DARK_RED);
        }
    }
}
fn decode_thumbnail(data:&str)->Result<egui::ColorImage,String>{
    if data.len()>56*1024*1024{return Err("Too large".into());}
    let bytes=base64::engine::general_purpose::STANDARD.decode(data.as_bytes())
        .map_err(|e|e.to_string())?;
    if bytes.len()>40*1024*1024{return Err("Too large".into());}
    let reader=image::ImageReader::new(std::io::Cursor::new(bytes.as_slice()))
        .with_guessed_format().map_err(|e|e.to_string())?;
    let (w,h)=reader.into_dimensions().map_err(|e|e.to_string())?;
    if w==0||h==0||w>16000||h>16000 ||
        u64::from(w)*u64::from(h)>40_000_000 {
        return Err("Too many pixels".into());
    }
    let decoded=image::load_from_memory(&bytes).map_err(|e|e.to_string())?;
    let small=decoded.thumbnail(PREVIEW_SIDE,PREVIEW_SIDE).to_rgba8();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [small.width() as usize,small.height() as usize],small.as_raw()))
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn rejects_invalid_base64(){
        assert!(decode_thumbnail("not-base64").is_err());
    }
}