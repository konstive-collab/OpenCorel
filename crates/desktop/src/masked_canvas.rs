//! Bounded and cache-aware SVG preview for documents containing clip paths.
//! The same SVG rendering engine is used for raster output, avoiding a second
//! incompatible clipping implementation in the immediate-mode egui painter.
use eframe::egui::{self,Color32,Pos2,Rect,TextureHandle};
use opencorel_core::{Document,Point,Shape};
use std::hash::{Hash,Hasher};
use std::collections::hash_map::DefaultHasher;
const MAX_SIDE:u32=2048;
const MAX_PIXELS:u32=4_194_304;
const MAX_SVG_BYTES:usize=42*1024*1024;

#[derive(Default)]
pub(super) struct MaskedCanvasCache{
    cached:Option<(u64,TextureHandle)>,
    error:Option<String>,
}
impl MaskedCanvasCache{
    pub fn paint(&mut self,ctx:&egui::Context,p:&egui::Painter,doc:&Document,rect:Rect){
        if rect.width()<=0.0||rect.height()<=0.0{return;}
        let (w,h)=preview_size(rect);
        let fingerprint=document_fingerprint(doc,w,h);
        if self.cached.as_ref().is_none_or(|(hash,_)|*hash!=fingerprint){
            self.cached=None;
            let result=render_document(doc,w,h);
            match result{
                Ok(img)=>{
                    self.cached=Some((fingerprint,ctx.load_texture(
                        "opencorel-clipped-canvas",img,egui::TextureOptions::LINEAR)));
                    self.error=None;
                }
                Err(err)=>self.error=Some(err),
            }
        }
        if let Some((_,texture))=&self.cached{
            p.image(texture.id(),rect,
                Rect::from_min_max(Pos2::ZERO,Pos2::new(1.0,1.0)),Color32::WHITE);
        }else{
            p.text(rect.center(),egui::Align2::CENTER_CENTER,
                format!("Предпросмотр маски недоступен: {}",self.error.as_deref().unwrap_or("ошибка")),
                egui::FontId::proportional(13.0),Color32::RED);
        }
    }
}
fn preview_size(rect:Rect)->(u32,u32){
    let mut width=rect.width().ceil().max(1.0) as u32;
    let mut height=rect.height().ceil().max(1.0) as u32;
    let scale=(MAX_SIDE as f64/width.max(height) as f64)
        .min((MAX_PIXELS as f64/(width as f64*height as f64)).sqrt())
        .min(1.0);
    width=((width as f64*scale).round() as u32).clamp(1,MAX_SIDE);
    height=((height as f64*scale).round() as u32).clamp(1,MAX_SIDE);
    (width,height)
}
fn render_document(doc:&Document,w:u32,h:u32)->Result<egui::ColorImage,String>{
    let svg=doc.to_svg();
    if svg.len()>MAX_SVG_BYTES{
        return Err("объём встроенных изображений превышает лимит предпросмотра".into());
    }
    let mut options=resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree=resvg::usvg::Tree::from_str(&svg,&options)
        .map_err(|e|format!("SVG: {e}"))?;
    let dimensions=tree.size();
    if dimensions.width()<=0.0||dimensions.height()<=0.0 {
        return Err("нулевые размеры страницы".into());
    }
    let mut pixmap=resvg::tiny_skia::Pixmap::new(w,h)
        .ok_or("не удалось выделить память под предпросмотр")?;
    let transform=resvg::tiny_skia::Transform::from_scale(
        w as f32/dimensions.width(),h as f32/dimensions.height());
    resvg::render(&tree,transform,&mut pixmap.as_mut());
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [w as usize,h as usize],pixmap.data()))
}
fn hash_point(p:Point,h:&mut impl Hasher){p.x.to_bits().hash(h);p.y.to_bits().hash(h);}
fn hash_float(v:f32,h:&mut impl Hasher){v.to_bits().hash(h);}
fn large_string_signature(s:&String,h:&mut impl Hasher){
    s.len().hash(h);
    (s.as_ptr() as usize).hash(h);
    s.as_bytes().iter().take(16).for_each(|b|b.hash(h));
    s.as_bytes().iter().rev().take(16).for_each(|b|b.hash(h));
}
/// Compute a compact frame signature without re-encoding megabytes of photo
/// assets on every UI repaint. Geometry, styles, masks and visibility are hashed.
fn document_fingerprint(doc:&Document,width:u32,height:u32)->u64{
    let mut h=DefaultHasher::new();
    width.hash(&mut h);height.hash(&mut h);
    hash_float(doc.page.width_mm,&mut h);hash_float(doc.page.height_mm,&mut h);
    doc.name.hash(&mut h);
    for layer in &doc.layers{
        layer.id.hash(&mut h);layer.visible.hash(&mut h);layer.locked.hash(&mut h);
    }
    for clip in &doc.clip_links{
        clip.source_id.hash(&mut h);clip.target_id.hash(&mut h);
    }
    for (id,appearance) in &doc.appearances{
        id.hash(&mut h);
        hash_float(appearance.opacity,&mut h);
        format!("{:?}",appearance.cap).hash(&mut h);
        format!("{:?}",appearance.join).hash(&mut h);
        for value in &appearance.dash_array{hash_float(*value,&mut h);}
        if let Some(gradient)=&appearance.gradient{
            format!("{:?}",gradient.kind).hash(&mut h);
            hash_float(gradient.angle_degrees,&mut h);
            for stop in &gradient.stops{
                hash_float(stop.offset,&mut h);stop.color.hash(&mut h);
            }
        }
    }
    for obj in &doc.objects{
        obj.id.hash(&mut h);obj.layer_id.hash(&mut h);obj.group_id.hash(&mut h);
        obj.style.fill.hash(&mut h);obj.style.stroke.hash(&mut h);
        hash_float(obj.style.stroke_width_mm,&mut h);
        obj.text_format.family.hash(&mut h);
        obj.text_format.weight.hash(&mut h);
        obj.text_format.italic.hash(&mut h);
        format!("{:?}",obj.text_format.alignment).hash(&mut h);
        hash_float(obj.text_format.letter_spacing_mm,&mut h);
        hash_float(obj.text_format.line_height,&mut h);
        std::mem::discriminant(&obj.shape).hash(&mut h);
        match &obj.shape{
            Shape::Rectangle{x,y,w,h:height}|Shape::Ellipse{x,y,w,h:height}=>{
                for value in [x,y,w,height]{hash_float(*value,&mut h);}
            }
            Shape::Text{x,y,value,size_mm}=>{
                hash_float(*x,&mut h);hash_float(*y,&mut h);
                hash_float(*size_mm,&mut h);value.hash(&mut h);
            }
            Shape::Polyline{points,closed}=>{
                closed.hash(&mut h);points.len().hash(&mut h);
                for p in points{hash_point(*p,&mut h);}
            }
            Shape::Bezier{start,segments,closed}=>{
                closed.hash(&mut h);hash_point(*start,&mut h);
                for segment in segments{
                    hash_point(segment.control1,&mut h);
                    hash_point(segment.control2,&mut h);
                    hash_point(segment.end,&mut h);
                }
            }
            Shape::CompoundPath{contours}=>{
                for contour in contours{
                    contour.closed.hash(&mut h);hash_point(contour.start,&mut h);
                    for segment in &contour.segments{
                        hash_point(segment.control1,&mut h);
                        hash_point(segment.control2,&mut h);
                        hash_point(segment.end,&mut h);
                    }
                }
            }
            Shape::EmbeddedSvg{x,y,w,h:height,svg,source_format}=>{
                for value in [x,y,w,height]{hash_float(*value,&mut h);}
                large_string_signature(svg,&mut h);source_format.hash(&mut h);
            }
            Shape::Raster{x,y,w,h:height,pixel_width,pixel_height,mime,data_base64,..}=>{
                for value in [x,y,w,height]{hash_float(*value,&mut h);}
                pixel_width.hash(&mut h);pixel_height.hash(&mut h);
                mime.hash(&mut h);large_string_signature(data_base64,&mut h);
            }
        }
    }
    h.finish()
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Style,Shape};
    #[test] fn clipped_svg_renders_pixels(){
        let mut doc=Document::default();
        doc.page.width_mm=100.0;doc.page.height_mm=100.0;
        let mask=doc.add(Shape::Rectangle{x:10.0,y:10.0,w:20.0,h:20.0},Style::default());
        let target=doc.add(Shape::Rectangle{x:0.0,y:0.0,w:100.0,h:100.0},
            Style{fill:"#FF0000".into(),stroke:"none".into(),stroke_width_mm:0.0});
        doc.set_clip(mask,target).unwrap();
        let image=render_document(&doc,100,100).unwrap();
        assert_eq!(image.size,[100,100]);
        // Transparent outside the clip, opaque inside.
        assert_eq!(image.pixels[5*100+5].a(),0);
        assert_eq!(image.pixels[20*100+20].a(),255);
    }
    #[test] fn geometry_changes_invalidate_fingerprint(){
        let mut doc=Document::default();
        let id=doc.add(Shape::Rectangle{x:10.0,y:10.0,w:20.0,h:20.0},Style::default());
        let before=document_fingerprint(&doc,200,200);
        doc.get_mut(id).unwrap().shape.translate(1.0,0.0);
        assert_ne!(before,document_fingerprint(&doc,200,200));
    }
    #[test] fn preview_is_bounded(){
        let rect=Rect::from_min_size(Pos2::ZERO,egui::vec2(60000.0,60000.0));
        let (w,h)=preview_size(rect);
        assert!(w<=MAX_SIDE && h<=MAX_SIDE && w*h<=MAX_PIXELS);
    }
}
