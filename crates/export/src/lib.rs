//! Preliminary vector PDF export for RGB proofing and customer review.
//! NOT a PDF/X prepress exporter: no CMYK, output intents, trapping, or bleed.
pub mod raster_export;
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
        svg2pdf::PageOptions::default())
        .map_err(|e|format!("PDF conversion failed: {e:?}"))?;
    if !pdf.starts_with(b"%PDF-"){
        return Err("PDF exporter returned invalid output".into());
    }
    Ok(pdf)
}

/// Write a temporary PDF in the destination directory and then publish it.
pub fn export_pdf(doc:&Document,path:impl AsRef<Path>)->Result<(),String>{
    let bytes=pdf_bytes(doc)?;
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
    #[test] fn oversized_pdf_is_explicit_error(){
        let mut doc=Document::default();
        doc.page.width_mm=6000.0;
        assert!(pdf_bytes(&doc).is_err());
    }
}
