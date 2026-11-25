//! Raster RGB proof exports for customer approval.
//! These files are NOT color-managed production masters or CMYK separations.
use opencorel_core::Document;
use std::{io::{Cursor,Write},path::Path};
use image::{DynamicImage,ImageFormat};

const MAX_PIXELS:u64=24_000_000;
const MAX_SIDE:u32=10_000;

pub fn raster_bytes(doc:&Document,dpi:u32,format:ImageFormat)->Result<Vec<u8>,String>{
    doc.validate()?;
    if !(36..=300).contains(&dpi){return Err("DPI must be between 36 and 300".into());}
    if !matches!(format,ImageFormat::Png|ImageFormat::Jpeg){
        return Err("Only PNG and JPEG proof exports are supported".into());
    }
    let width=(doc.page.width_mm as f64*dpi as f64/25.4).ceil();
    let height=(doc.page.height_mm as f64*dpi as f64/25.4).ceil();
    if width<1.0 || height<1.0 ||width>MAX_SIDE as f64 ||height>MAX_SIDE as f64 ||
        width*height>MAX_PIXELS as f64 {
        return Err(format!(
            "Output too large ({width:.0}×{height:.0}px at {dpi}dpi). Lower DPI or export SVG/PDF. Limit: 24 megapixels / 10k per side."
        ));
    }
    let (w,h)=(width as u32,height as u32);
    let svg=doc.to_svg();
    let mut options=resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree=resvg::usvg::Tree::from_str(&svg,&options)
        .map_err(|e|format!("Cannot render document: {e}"))?;
    let original=tree.size();
    if original.width()<=0.0 || original.height()<=0.0 {
        return Err("Invalid rendering dimensions".into());
    }
    let mut pixmap=resvg::tiny_skia::Pixmap::new(w,h)
        .ok_or("Cannot allocate bitmap proof")?;
    pixmap.fill(resvg::tiny_skia::Color::WHITE);
    let transform=resvg::tiny_skia::Transform::from_scale(
        w as f32/original.width(),h as f32/original.height());
    resvg::render(&tree,transform,&mut pixmap.as_mut());

    let rgba=image::RgbaImage::from_raw(w,h,pixmap.data().to_vec())
        .ok_or("Unexpected raster buffer size")?;
    let converted=if format==ImageFormat::Jpeg{
        DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(rgba).to_rgb8())
    }else{
        DynamicImage::ImageRgba8(rgba)
    };
    let mut out=Vec::new();
    converted.write_to(&mut Cursor::new(&mut out),format)
        .map_err(|e|format!("Cannot encode raster proof: {e}"))?;
    Ok(out)
}
pub fn export_raster(doc:&Document,path:impl AsRef<Path>,dpi:u32)->Result<(),String>{
    let path=path.as_ref();
    let kind=match path.extension().and_then(|e|e.to_str()).unwrap_or("").to_ascii_lowercase().as_str(){
        "png"=>ImageFormat::Png,
        "jpg"|"jpeg"=>ImageFormat::Jpeg,
        _=>return Err("Raster export supports only .png, .jpg or .jpeg".into())
    };
    let bytes=raster_bytes(doc,dpi,kind)?;
    let parent=path.parent().filter(|p|!p.as_os_str().is_empty())
        .unwrap_or_else(||Path::new("."));
    let mut tmp=tempfile::Builder::new().prefix(".opencorel-preview-")
        .tempfile_in(parent).map_err(|e|e.to_string())?;
    tmp.write_all(&bytes).map_err(|e|e.to_string())?;
    tmp.as_file().sync_all().map_err(|e|e.to_string())?;
    tmp.persist(path).map_err(|e|e.error.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Shape,Style};
    #[test] fn gradient_preview_preserves_color_across_page(){
        use opencorel_core::appearance::{Appearance,Gradient,GradientKind,GradientStop};
        let mut d=Document::default();
        d.page.width_mm=25.4;d.page.height_mm=12.7;
        let id=d.add(Shape::Rectangle{x:0.0,y:0.0,w:25.4,h:12.7},
            Style{fill:"#FFFFFF".into(),stroke:"none".into(),stroke_width_mm:0.0});
        let gradient=Gradient{kind:GradientKind::Linear,angle_degrees:0.0,stops:vec![
            GradientStop{offset:0.0,color:"#FF0000".into()},
            GradientStop{offset:1.0,color:"#0000FF".into()},
        ]};
        d.set_appearance(id,Appearance{gradient:Some(gradient),..Appearance::default()}).unwrap();
        let bytes=raster_bytes(&d,96,ImageFormat::Png).unwrap();
        let rgba=image::load_from_memory(&bytes).unwrap().to_rgba8();
        let left=rgba.get_pixel(10,24).0;
        let right=rgba.get_pixel(85,24).0;
        assert!(left[0]>left[2],"left edge should be red-dominant");
        assert!(right[2]>right[0],"right edge should be blue-dominant");
    }
    #[test] fn edited_raster_matches_png_proof_and_embeds_in_pdf(){
        use base64::Engine;
        use opencorel_core::raster_edit::RasterSettings;
        let mut png=Cursor::new(Vec::new());
        let input=image::RgbaImage::from_pixel(2,2,image::Rgba([255,0,0,255]));
        DynamicImage::ImageRgba8(input).write_to(&mut png,ImageFormat::Png).unwrap();
        let mut d=Document::default();
        d.page.width_mm=25.4;d.page.height_mm=25.4;
        let id=d.add(Shape::Raster{x:0.,y:0.,w:25.4,h:25.4,pixel_width:2,pixel_height:2,
            mime:"image/png".into(),
            data_base64:base64::engine::general_purpose::STANDARD.encode(png.into_inner()),
            edit:None},Style::default());
        d.apply_raster_edit(id,RasterSettings{invert:true,..Default::default()}).unwrap();
        let bytes=raster_bytes(&d,96,ImageFormat::Png).unwrap();
        let output=image::load_from_memory(&bytes).unwrap().to_rgba8();
        let p=output.get_pixel(48,48).0;
        assert!(p[0]<10&&p[1]>245&&p[2]>245,"expected cyan, got {p:?}");
        let pdf=crate::pdf_bytes(&d).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        let parsed=lopdf::Document::load_mem(&pdf).unwrap();
        assert!(parsed.objects.values().any(|object|
            object.as_stream().ok().and_then(|stream|stream.dict.get(b"Subtype").ok())
                .and_then(|kind|kind.as_name().ok())==Some(b"Image".as_slice())));
        d.apply_raster_edit(id,RasterSettings{key_enabled:true,key_color:[255,0,0],
            key_tolerance:0,key_softness:0,..Default::default()}).unwrap();
        let bytes=raster_bytes(&d,96,ImageFormat::Png).unwrap();
        let p=image::load_from_memory(&bytes).unwrap().to_rgba8().get_pixel(48,48).0;
        assert!(p[0]>245&&p[1]>245&&p[2]>245,"transparent image must reveal white proof page");
    }
    #[test] fn jpeg_png_proofs_have_expected_dimensions(){
        let mut d=Document::default();
        d.page.width_mm=25.4;d.page.height_mm=12.7;
        d.add(Shape::Rectangle{x:1.0,y:1.0,w:10.0,h:5.0},Style::default());
        for format in [ImageFormat::Png,ImageFormat::Jpeg]{
            let encoded=raster_bytes(&d,96,format).unwrap();
            let decoded=image::load_from_memory(&encoded).unwrap();
            assert_eq!((decoded.width(),decoded.height()),(96,48));
        }
    }
    #[test] fn big_banner_requires_reduced_dpi(){
        let mut doc=Document::default();
        doc.page.width_mm=3000.0;doc.page.height_mm=2000.0;
        assert!(raster_bytes(&doc,96,ImageFormat::Png).is_err());
    }
}
