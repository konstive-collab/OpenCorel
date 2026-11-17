//! Preliminary vector PDF export for RGB proofing and customer review.
//! NOT a PDF/X prepress exporter: no CMYK, output intents, trapping, or bleed.
pub mod raster_export;
pub mod multipage_pdf;
use opencorel_core::Document;
use std::{fs, io::Write, path::Path};

/// Produce a single-page PDF with document geometry.
pub fn pdf_bytes(doc:&Document)->Result<Vec<u8>,String>{
    doc.validate()?;
    // PDF without /UserUnit is usually limited to 14,400 points (5,080 mm).
    // Reject rather than quietly deliver an incorrectly scaled large poster.
    if doc.page.width_mm>5000.0 || doc.page.height_mm>5000.0 {
        return Err("PDF preview currently supports pages up to 5000 mm. Export SVG for larger sheets.".into());
    }
    let svg=doc.to_svg();
    let mut options=svg2pdf::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree=svg2pdf::usvg::Tree::from_str(&svg,&options)
        .map_err(|e|format!("Could not parse document SVG for PDF export: {e}"))?;
    let pdf=svg2pdf::to_pdf(&tree,
        svg2pdf::ConversionOptions::default(),
        // SVG/CSS pixel geometry is based on 96 DPI. Explicitly convert
        // those pixels to PDF points (72 per inch) to preserve millimetres.
        // The svg2pdf default of 72 DPI enlarges physical pages by 4/3.
        svg2pdf::PageOptions{dpi:96.0})
        .map_err(|e|format!("PDF conversion failed: {e:?}"))?;
    if !pdf.starts_with(b"%PDF-"){
        return Err("PDF exporter returned invalid output".into());
    }
    Ok(pdf)
}

/// Write a temporary PDF in the destination directory and then publish it.
pub fn export_pdf(doc:&Document,path:impl AsRef<Path>)->Result<(),String>{
    let bytes=if doc.page_count()>1{
        multipage_pdf::multipage_pdf_bytes(doc)?
    }else{pdf_bytes(doc)?};
    let path=path.as_ref();
    let dir=path.parent().filter(|p|!p.as_os_str().is_empty())
        .unwrap_or_else(||Path::new("."));
    let mut tmp=tempfile::Builder::new().prefix(".opencorel-pdf-").tempfile_in(dir)
        .map_err(|e|e.to_string())?;
    tmp.write_all(&bytes).map_err(|e|e.to_string())?;
    tmp.as_file().sync_all().map_err(|e|e.to_string())?;
    tmp.persist(path).map_err(|e|e.error.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Shape,Style};
    #[test] fn exports_valid_pdf_header(){
        let mut doc=Document::default();
        doc.add(Shape::Rectangle{x:20.0,y:20.0,w:100.0,h:70.0},Style::default());
        let pdf=pdf_bytes(&doc).unwrap();
        assert!(pdf.len()>100);
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf.windows(5).any(|v|v==b"%%EOF"));
    }
    #[test] fn pdf_media_box_matches_physical_millimetres(){
        let mut doc=Document::default();
        doc.page.width_mm=210.0;
        doc.page.height_mm=297.0;
        doc.add(Shape::Rectangle{x:10.0,y:12.0,w:30.0,h:40.0},
            Style::default());
        let bytes=pdf_bytes(&doc).unwrap();
        let pdf=lopdf::Document::load_mem(&bytes).unwrap();
        let page_id=*pdf.get_pages().get(&1).unwrap();
        let page=pdf.get_object(page_id).unwrap().as_dict().unwrap();
        let media_box=page.get(b"MediaBox")
            .or_else(|_|{
                let parent=page.get(b"Parent")?.as_reference()?;
                pdf.get_object(parent)?.as_dict()?.get(b"MediaBox")
            }).unwrap().as_array().unwrap();
        let value=|entry:&lopdf::Object|entry.as_float().unwrap_or_else(|_|
            entry.as_i64().unwrap() as f32);
        let width=value(&media_box[2])-value(&media_box[0]);
        let height=value(&media_box[3])-value(&media_box[1]);
        assert!((width-210.0*72.0/25.4).abs()<1.0,
            "A4 width in PDF points: {width}");
        assert!((height-297.0*72.0/25.4).abs()<1.0,
            "A4 height in PDF points: {height}");
    }
    #[test] fn clipped_document_exports_pdf(){
        let mut doc=Document::default();
        let mask=doc.add(Shape::Ellipse{x:10.0,y:10.0,w:40.0,h:40.0},Style::default());
        let object=doc.add(Shape::Rectangle{x:0.0,y:0.0,w:100.0,h:100.0},Style::default());
        doc.set_clip(mask,object).unwrap();
        let svg=doc.to_svg();
        assert!(svg.contains("clipPath"));
        let pdf=pdf_bytes(&doc).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf.windows(5).any(|bytes|bytes==b"%%EOF"));
    }
    #[test] fn oversized_pdf_is_explicit_error(){
        let mut doc=Document::default();
        doc.page.width_mm=6000.0;
        assert!(pdf_bytes(&doc).is_err());
    }
}
