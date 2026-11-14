//! Lazy SVG preview cache for imported vector artwork.
use eframe::egui::{self,Color32,Pos2,Rect,TextureHandle,Vec2};
use std::{collections::HashMap,hash::{Hash,Hasher},collections::hash_map::DefaultHasher};

#[derive(Default)]
pub(super) struct VectorCache {
    textures:HashMap<u64,(u64,TextureHandle)>,
    errors:HashMap<u64,String>,
}
impl VectorCache {
    pub fn clear(&mut self){
        self.textures.clear();
        self.errors.clear();
    }
    pub fn paint(&mut self,ctx:&egui::Context,p:&egui::Painter,id:u64,svg:&str,rect:Rect){
        if rect.width()<=0.0 || rect.height()<=0.0{return;}
        let width=(rect.width().ceil() as u32).clamp(1,2048);
        let height=(rect.height().ceil() as u32).clamp(1,2048);
        let mut hasher=DefaultHasher::new();
        svg.hash(&mut hasher);width.hash(&mut hasher);height.hash(&mut hasher);
        let hash=hasher.finish();
        if self.textures.get(&id).map(|(h,_)|*h!=hash).unwrap_or(true){
            match rasterize(svg,width,height){
                Ok(image)=>{
                    if self.textures.len()>64{
                        if let Some(k)=self.textures.keys().copied().next(){self.textures.remove(&k);}
                    }
                    let texture=ctx.load_texture(format!("vector-{id}-{hash}"),image,egui::TextureOptions::LINEAR);
                    self.textures.insert(id,(hash,texture));
                    self.errors.remove(&id);
                }
                Err(e)=>{
                    self.textures.remove(&id);
                    self.errors.insert(id,e);
                }
            }
        }
        if let Some((_,texture))=self.textures.get(&id){
            p.image(texture.id(),rect,Rect::from_min_max(Pos2::ZERO,Pos2::new(1.0,1.0)),Color32::WHITE);
        }else{
            p.rect_filled(rect,0.0,Color32::from_rgb(255,240,240));
            p.text(rect.center(),egui::Align2::CENTER_CENTER,
                format!("Векторный объект #{}\nНет предпросмотра",id),
                egui::FontId::proportional(12.0),Color32::RED);
        }
    }
}
fn rasterize(svg:&str,width:u32,height:u32)->Result<egui::ColorImage,String>{
    // No remote/network image resolver is configured.
    let mut opt=resvg::usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    let tree=resvg::usvg::Tree::from_str(svg,&opt).map_err(|e|e.to_string())?;
    let src=tree.size();
    if src.width()<=0.0 || src.height()<=0.0{
        return Err("SVG has no visible dimensions".into());
    }
    let mut pixmap=resvg::tiny_skia::Pixmap::new(width,height)
        .ok_or_else(||"Cannot allocate vector preview".to_string())?;
    let transform=resvg::tiny_skia::Transform::from_scale(
        width as f32/src.width(),height as f32/src.height());
    resvg::render(&tree,transform,&mut pixmap.as_mut());
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [width as usize,height as usize],pixmap.data()))
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn renders_simple_svg_to_pixels(){
        let img=rasterize(r##"<svg xmlns="http://www.w3.org/2000/svg" width="50" height="50"><rect width="50" height="50" fill="#00ff00"/></svg>"##,40,40).unwrap();
        assert_eq!(img.size,[40,40]);
    }
}
