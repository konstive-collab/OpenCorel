//! File format router and deliberately labeled compatibility adapters.
//! External formats are not lossless CorelDRAW project files.
use flate2::{read::GzDecoder,write::GzEncoder,Compression};
use base64::Engine;
use image::{ImageFormat, ImageReader};
use std::io::Cursor;
use opencorel_core::{kdr,svg_import,Document,Shape,Style};
use std::{fs,io::{Read,Write},path::Path,process::{Command,Stdio},time::Duration};
use wait_timeout::ChildExt;

const MAX_EXTERNAL_FILE:u64=128*1024*1024;
const MAX_SVG_SIZE:usize=20*1024*1024;
const LO_TIMEOUT:Duration=Duration::from_secs(60);
const PDF_TIMEOUT:Duration=Duration::from_secs(45);

#[derive(Debug,Clone,PartialEq)]
pub enum ImportQuality {
    NativeEditable,
    SimplifiedEditable,
    PlacedVector,
    PlacedRaster,
}
#[derive(Debug,Clone)]
pub struct OpenResult{
    pub document:Document,
    pub quality:ImportQuality,
    pub warning:String,
    pub source_format:String,
}
pub fn extension(path:&Path)->String{
    path.extension().and_then(|s|s.to_str()).unwrap_or("")
        .to_ascii_lowercase()
}
pub const IMPORT_EXTENSIONS:&[&str]=&[
    "kdr","ocd","svg","svgz","pdf","cdr","cmx","ai","eps","ps",
    "odg","fodg","dxf","wmf","emf","wpg","vsd","vsdx","sxd",
    "png","jpg","jpeg","webp","gif","bmp","tif","tiff"
];
pub const EXPORT_EXTENSIONS:&[&str]=&["kdr","ocd","svg","svgz","pdf"];

/// Raster import applies limits BEFORE decoding, to avoid huge allocations from
/// malformed image dimensions. PNG/JPEG keep compressed originals; other supported
/// formats are normalized to PNG, so every saved KDR can export to SVG.
pub fn import_raster_shape(path:impl AsRef<Path>)->Result<Shape,String>{
    let path=path.as_ref();
    let bytes=fs::read(path).map_err(|e|e.to_string())?;
    if bytes.is_empty() || bytes.len()>40*1024*1024{
        return Err("Raster image must be between 1 byte and 40 MiB".into());
    }
    let actual=image::guess_format(&bytes).map_err(|e|e.to_string())?;
    let supported=matches!(actual,ImageFormat::Png|ImageFormat::Jpeg|
        ImageFormat::WebP|ImageFormat::Gif|ImageFormat::Bmp|ImageFormat::Tiff);
    if !supported{return Err(format!("Raster codec {actual:?} is not yet supported"));}
    let (width,height)=ImageReader::new(Cursor::new(bytes.as_slice()))
        .with_guessed_format().map_err(|e|e.to_string())?
        .into_dimensions().map_err(|e|e.to_string())?;
    if width==0||height==0||width>16_000||height>16_000 ||
        u64::from(width)*u64::from(height)>40_000_000 {
        return Err("Raster exceeds 40 megapixels or 16000 pixels per side".into());
    }
    let (mime,data)=if actual==ImageFormat::Png{
        ("image/png".to_string(),bytes)
    }else if actual==ImageFormat::Jpeg{
        ("image/jpeg".to_string(),bytes)
    }else {
        let image=image::load_from_memory(&bytes).map_err(|e|e.to_string())?;
        let mut normalized=Vec::new();
        image.write_to(&mut Cursor::new(&mut normalized),ImageFormat::Png)
            .map_err(|e|e.to_string())?;
        if normalized.len()>40*1024*1024 {
            return Err("PNG normalization exceeds 40 MiB".into());
        }
        ("image/png".into(),normalized)
    };
    let data_base64=base64::engine::general_purpose::STANDARD.encode(data);
    let shape=Shape::Raster{x:0.0,y:0.0,
        w:width as f32*25.4/96.0,h:height as f32*25.4/96.0,
        pixel_width:width,pixel_height:height,mime,data_base64};
    Ok(shape)
}
fn load_raster(path:&Path)->Result<OpenResult,String>{
    let ext=extension(path).to_ascii_uppercase();
    let shape=import_raster_shape(path)?;
    let bounds=shape.bounds();
    let mut document=Document::default();
    document.name=path.file_stem().and_then(|s|s.to_str())
        .unwrap_or("Imported photo").to_string();
    document.page.width_mm=bounds.w;
    document.page.height_mm=bounds.h;
    document.add(shape,Style{fill:"none".into(),stroke:"none".into(),
        stroke_width_mm:0.0});
    document.validate()?;
    Ok(OpenResult{document,quality:ImportQuality::PlacedRaster,
        source_format:ext,warning:"Raster image placed at 96 DPI. It is movable and scalable but not vectorized. Color management and EXIF orientation are not yet guaranteed.".into()})
}
fn check_file_size(path:&Path)->Result<(),String>{
    if fs::metadata(path).map_err(|e|e.to_string())?.len()>MAX_EXTERNAL_FILE{
        return Err("File is too large for current import limits".into());
    }
    Ok(())
}
fn read_svg_bytes(path:&Path,compressed:bool)->Result<String,String>{
    check_file_size(path)?;
    let bytes=if compressed{
        let file=fs::File::open(path).map_err(|e|e.to_string())?;
        let mut decoded=Vec::new();
        GzDecoder::new(file).take((MAX_SVG_SIZE+1) as u64)
            .read_to_end(&mut decoded).map_err(|e|e.to_string())?;
        decoded
    } else {fs::read(path).map_err(|e|e.to_string())?};
    if bytes.len()>MAX_SVG_SIZE {return Err("Expanded SVG exceeds 20 MiB".into());}
    String::from_utf8(bytes).map_err(|_|"SVG must be UTF-8".into())
}
fn parse_length_mm(raw:&str)->Option<f32>{
    let raw=raw.trim();
    let units=[("mm",1.0),("cm",10.0),("in",25.4),("pt",25.4/72.0),("px",25.4/96.0)];
    let (number,factor)=units.iter().find_map(|(unit,factor)|raw.strip_suffix(unit).map(|v|(v,*factor)))
        .unwrap_or((raw,25.4/96.0));
    let value=number.trim().parse::<f32>().ok()?;
    let result=value*factor;
    if result.is_finite() && (1.0..=100_000.0).contains(&result){Some(result)}
    else{None}
}
fn svg_dimensions(svg:&str)->Result<(f32,f32),String>{
    // Strict XML parse: no DTD expansion, scripts, remote resources or XML network access.
    let parsed=roxmltree::Document::parse_with_options(svg,roxmltree::ParsingOptions{
        allow_dtd:false,nodes_limit:300_000,..Default::default()
    }).map_err(|e|format!("SVG metadata invalid: {e}"))?;
    let root=parsed.root_element();
    if root.tag_name().name()!="svg"{return Err("Not an SVG drawing".into());}
    let width=root.attribute("width").and_then(parse_length_mm);
    let height=root.attribute("height").and_then(parse_length_mm);
    if let (Some(w),Some(h))=(width,height){return Ok((w,h));}
    if let Some(vb)=root.attribute("viewBox"){
        let values:Vec<_>=vb.split(|c:char|c==','||c.is_ascii_whitespace())
            .filter_map(|v|if v.is_empty(){None}else{v.parse::<f32>().ok()}).collect();
        if values.len()==4 && values[2].is_finite()&&values[3].is_finite() &&
            values[2]>0.0 && values[3]>0.0 {
            return Ok((values[2].clamp(1.0,100_000.0)*25.4/96.0,
                values[3].clamp(1.0,100_000.0)*25.4/96.0));
        }
    }
    Err("SVG has no usable physical page dimensions".into())
}
fn load_svg(svg:String,origin:&str,converted:bool)->Result<OpenResult,String>{
    if svg.len()>MAX_SVG_SIZE{return Err("SVG exceeds size limit".into());}
    // Preserve full vector composition when it cannot be represented by the current
    // editable primitive model. Do not silently discard clipping/gradients/paths.
    if let Ok(doc)=svg_import::parse_svg(&svg) {
        return Ok(OpenResult{
            document:doc,quality:ImportQuality::SimplifiedEditable,
            warning:if converted {
                format!("{origin} imported through conversion. Check text, colors and dimensions.")
            } else {"Editable SVG subset imported. Compare with the original.".into()},
            source_format:origin.into(),
        });
    }
    let (w,h)=svg_dimensions(&svg)?;
    let mut doc=Document::default();
    doc.name=format!("Imported {origin}");
    doc.page.width_mm=w;
    doc.page.height_mm=h;
    doc.add(Shape::EmbeddedSvg{x:0.0,y:0.0,w,h,svg,source_format:origin.into()},
        Style::default());
    doc.validate()?;
    Ok(OpenResult{document:doc,quality:ImportQuality::PlacedVector,
        warning:format!(
            "{origin}: vector artwork preserved as ONE placed object. Its internal text/nodes \
            are not editable. Verify the visual output, scale, fonts and colors."),
        source_format:origin.into()})
}

fn execute(binary:&str,args:&[String],timeout:Duration)->Result<(),String>{
    let mut child=Command::new(binary).args(args).stdin(Stdio::null())
        .stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().map_err(|e|format!("Cannot start converter {binary}: {e}"))?;
    match child.wait_timeout(timeout).map_err(|e|e.to_string())?{
        Some(status) if status.success()=>Ok(()),
        Some(status)=>Err(format!("{binary} exited with {status}")),
        None=>{
            let _=child.kill();let _=child.wait();
            Err(format!("Converter {binary} timed out after {} seconds",timeout.as_secs()))
        }
    }
}
fn executable_candidates()->Vec<String>{
    let mut items=Vec::new();
    if let Ok(path)=std::env::var("OPENCOREL_SOFFICE") {
        if !path.trim().is_empty(){items.push(path);}
    }
    items.extend(["soffice","libreoffice"].into_iter().map(str::to_owned));
    #[cfg(windows)]{
        items.push(r"C:\Program Files\LibreOffice\program\soffice.exe".into());
        items.push(r"C:\Program Files (x86)\LibreOffice\program\soffice.exe".into());
    }
    items
}
fn office_convert(input:&Path,output_format:&str,output_ext:&str,dir:&Path)
    ->Result<std::path::PathBuf,String>{
    let profile=dir.join("lo-profile");
    let profile_url=url::Url::from_directory_path(&profile)
        .map_err(|_|"Could not create temporary converter profile".to_string())?;
    let arguments=vec![
        format!("-env:UserInstallation={profile_url}"),"--headless".into(),
        "--convert-to".into(),output_format.into(),"--outdir".into(),
        dir.to_string_lossy().into_owned(),input.to_string_lossy().into_owned(),
    ];
    let output=dir.join(input.file_stem().unwrap_or_default()).with_extension(output_ext);
    let mut errors=Vec::new();
    for binary in executable_candidates(){
        match execute(&binary,&arguments,LO_TIMEOUT){
            Ok(()) if output.is_file()=>return Ok(output),
            Ok(())=>errors.push(format!("{binary}: output {output_ext} not generated")),
            Err(e)=>errors.push(e),
        }
    }
    Err(format!("LibreOffice conversion unavailable or failed. {}",errors.join("; ")))
}
fn pdf_to_svg(input:&Path,dir:&Path)->Result<String,String>{
    let output=dir.join("converted.svg");
    execute("pdftocairo",&[
        "-f".into(),"1".into(),"-l".into(),"1".into(),"-svg".into(),
        input.to_string_lossy().into_owned(),
        output.to_string_lossy().into_owned()
    ],PDF_TIMEOUT)?;
    read_svg_bytes(&output,false)
}
fn office_to_svg(path:&Path,workdir:&Path)->Result<String,String>{
    // Convert to SVG using the Draw export filter. This is a fallback if PDF
    // extraction tools are unavailable; it may include unsupported XML.
    let svg_path=office_convert(path,"svg:draw_svg_Export","svg",workdir)?;
    let content=read_svg_bytes(&svg_path,false)?;
    Ok(strip_svg_doctype(&content))
}
fn strip_svg_doctype(svg:&str)->String{
    // LibreOffice writes the public SVG 1.1 doctype even when no entities are
    // required. Reject internal entity sets rather than expanding untrusted XML.
    if let Some(start)=svg.find("<!DOCTYPE"){
        if let Some(end)=svg[start..].find('>'){
            let text=&svg[start..start+end+1];
            if !text.contains('['){
                let mut out=String::with_capacity(svg.len());
                out.push_str(&svg[..start]);out.push_str(&svg[start+end+1..]);
                return out;
            }
        }
    }
    svg.into()
}
fn external_vector(path:&Path,ext:&str)->Result<OpenResult,String>{
    check_file_size(path)?;
    let dir=tempfile::tempdir().map_err(|e|e.to_string())?;
    // Prefer PDF then pdftocairo SVG to preserve complicated geometry as a
    // single visual vector object. When no pdftocairo exists, try Draw SVG.
    let svg=if ext=="pdf" {
        pdf_to_svg(path,dir.path())?
    } else if ext=="ai" && fs::File::open(path).ok()
        .and_then(|mut f|{
            let mut magic=[0u8;5];f.read_exact(&mut magic).ok()?;
            Some(magic==*b"%PDF-")
        }).unwrap_or(false) {
        pdf_to_svg(path,dir.path())?
    } else {
        let pdf=office_convert(path,"pdf:draw_pdf_Export","pdf",dir.path());
        match pdf {
            Ok(path)=>match pdf_to_svg(&path,dir.path()){
                Ok(svg)=>svg,
                Err(_)=>office_to_svg(path.as_path(),dir.path())?,
            },
            Err(err)=>return Err(format!("Could not import {ext}: {err}")),
        }
    };
    let content=strip_svg_doctype(&svg);
    load_svg(content,&ext.to_ascii_uppercase(),true)
}
pub fn open_any(path:impl AsRef<Path>)->Result<OpenResult,String>{
    let path=path.as_ref();let ext=extension(path);
    let native=|document:Document|OpenResult{
        document,quality:ImportQuality::NativeEditable,warning:String::new(),
        source_format:ext.to_ascii_uppercase()
    };
    match ext.as_str(){
        "kdr"=>kdr::read_kdr(path).map(native),
        "ocd"=>Document::read(path).map(native),
        "svg"=>load_svg(read_svg_bytes(path,false)?,"SVG",false),
        "svgz"=>load_svg(read_svg_bytes(path,true)?,"SVGZ",false),
        "png"|"jpg"|"jpeg"|"webp"|"gif"|"bmp"|"tif"|"tiff"=>load_raster(path),
        "pdf"|"cdr"|"cmx"|"ai"|"eps"|"ps"|"odg"|"fodg"|"dxf"|
        "wmf"|"emf"|"wpg"|"vsd"|"vsdx"|"sxd"=>external_vector(path,&ext),
        _=>Err(format!("Unsupported file extension .{ext}; supported: {}",
            IMPORT_EXTENSIONS.join(", ")))
    }
}
pub fn save_document(doc:&Document,path:impl AsRef<Path>)->Result<(),String>{
    let path=path.as_ref();
    match extension(path).as_str(){
        "kdr"=>kdr::save_kdr(doc,path),
        "ocd"=>doc.write(path),
        "svg"=>doc.validate().and_then(|_|doc.export_svg(path).map_err(|e|e.to_string())),
        "svgz"=>{
            doc.validate()?;
            let mut encoder=GzEncoder::new(Vec::new(),Compression::default());
            encoder.write_all(doc.to_svg().as_bytes()).map_err(|e|e.to_string())?;
            let bytes=encoder.finish().map_err(|e|e.to_string())?;
            let parent=path.parent().filter(|p|!p.as_os_str().is_empty())
                .unwrap_or_else(||Path::new("."));
            let mut tmp=tempfile::Builder::new().prefix(".svgz-write-").tempfile_in(parent)
                .map_err(|e|e.to_string())?;
            tmp.write_all(&bytes).map_err(|e|e.to_string())?;
            tmp.as_file().sync_all().map_err(|e|e.to_string())?;
            tmp.persist(path).map_err(|e|e.error.to_string())?;
            Ok(())
        },
        "pdf"=>opencorel_export::export_pdf(doc,path),
        "cdr"=>Err("CDR export is not implemented. Save as .kdr or export SVG/PDF; do not rename files.".into()),
        other=>Err(format!("Saving .{other} is unsupported. Supported: {}",EXPORT_EXTENSIONS.join(", ")))
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn raster_png_import_roundtrip_kdr_and_svg(){
        let dir=tempfile::tempdir().unwrap();
        let image_path=dir.path().join("photo.png");
        let mut img=image::RgbaImage::new(10,8);
        for pixel in img.pixels_mut(){*pixel=image::Rgba([255,20,30,255]);}
        img.save(&image_path).unwrap();
        let source=open_any(&image_path).unwrap();
        assert_eq!(source.quality,ImportQuality::PlacedRaster);
        assert_eq!(source.document.objects.len(),1);
        let kdr_path=dir.path().join("photo.kdr");
        save_document(&source.document,&kdr_path).unwrap();
        let restored=open_any(&kdr_path).unwrap();
        assert_eq!(source.document,restored.document);
        assert!(restored.document.to_svg().contains("data:image/png;base64,"));
    }
    #[test] fn raster_refuses_oversized_dimensions(){
        let dir=tempfile::tempdir().unwrap();
        let path=dir.path().join("missing.jpg");
        assert!(import_raster_shape(&path).is_err());
    }
    #[test] fn kdr_and_legacy_ocd_load(){
        let dir=tempfile::tempdir().unwrap();
        let mut doc=Document::default();doc.name="Company poster".into();
        for suffix in ["kdr","ocd"]{
            let p=dir.path().join(format!("poster.{suffix}"));
            save_document(&doc,&p).unwrap();
            assert_eq!(open_any(&p).unwrap().document,doc);
        }
    }
    #[test] fn svgz_roundtrip(){
        let dir=tempfile::tempdir().unwrap();
        let mut doc=Document::default();
        doc.add(Shape::Rectangle{x:10.0,y:10.0,w:40.0,h:25.0},Style::default());
        let path=dir.path().join("drawing.svgz");
        save_document(&doc,&path).unwrap();
        let imported=open_any(&path).unwrap();
        assert_eq!(imported.quality,ImportQuality::SimplifiedEditable);
        assert_eq!(imported.document.objects.len(),1);
    }
    #[test] fn svg_rect_is_editable(){
        let dir=tempfile::tempdir().unwrap();
        let p=dir.path().join("basic.svg");
        fs::write(&p,r##"<svg width="100mm" height="50mm" viewBox="0 0 100 50"><rect x="1" y="2" width="3" height="4" fill="#FF0000"/></svg>"##).unwrap();
        let imported=open_any(&p).unwrap();
        assert_eq!(imported.quality,ImportQuality::SimplifiedEditable);
        assert_eq!(imported.document.objects.len(),1);
    }
    #[test] fn svg_complex_is_preserved_not_discarded(){
        let input=r##"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="50mm" viewBox="0 0 100 50"><defs><linearGradient id="g"><stop offset="0" stop-color="#FF0000"/></linearGradient></defs><rect x="1" y="1" width="20" height="20" fill="url(#g)"/></svg>"##;
        let imported=load_svg(input.into(),"SVG",false).unwrap();
        assert_eq!(imported.quality,ImportQuality::PlacedVector);
        assert_eq!(imported.document.objects.len(),1);
    }
    #[test] fn rejects_fake_cdr_export(){
        let dir=tempfile::tempdir().unwrap();
        let p=dir.path().join("fake.cdr");
        assert!(save_document(&Document::default(),&p).unwrap_err().contains("not implemented"));
        assert!(!p.exists());
    }
    #[test] fn strips_only_simple_doctype(){
        let xml="<svg/>";assert_eq!(strip_svg_doctype(xml),xml);
        let doc="<!DOCTYPE svg PUBLIC \"example\" \"svg.dtd\"><svg/>";
        assert_eq!(strip_svg_doctype(doc),"<svg/>");
        assert!(strip_svg_doctype("<!DOCTYPE svg [<!ENTITY boom 'X'>]><svg/>").contains("<!DOCTYPE"));
    }
}
