//! RGB client proofs with vector crop marks. Not CMYK or PDF/X.
//! Does not create bleed or change the source document.
use opencorel_core::Document;
use std::{io::Write,path::Path};
#[derive(Clone,Copy,Debug)]
pub struct TrimOptions{pub margin_mm:f32,pub line_mm:f32}
impl Default for TrimOptions{
    fn default()->Self{Self{margin_mm:12.0,line_mm:0.16}}
}
pub fn trim_svg(doc:&Document,opts:TrimOptions)->Result<String,String>{
    doc.validate()?;
    if !opts.margin_mm.is_finite()||!(8.0..=40.0).contains(&opts.margin_mm)
        ||!opts.line_mm.is_finite()||!(0.05..=0.5).contains(&opts.line_mm){
        return Err("Invalid crop mark settings".into());
    }
    let (w,h,m)=(doc.page.width_mm,doc.page.height_mm,opts.margin_mm);
    let (paper_w,paper_h)=(w+2.0*m,h+2.0*m);
    let (x0,y0,x1,y1)=(m,m,m+w,m+h);
    let mut marks=String::new();
    let mut line=|x1:f32,y1:f32,x2:f32,y2:f32|{
        marks.push_str(&format!(
            "<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\"/>\n"));
    };
    for x in [x0,x1]{
        line(x,0.5,x,y0-2.0);
        line(x,y1+2.0,x,paper_h-0.5);
    }
    for y in [y0,y1]{
        line(0.5,y,x0-2.0,y);
        line(x1+2.0,y,paper_w-0.5,y);
    }
    // The existing editable document stays as an SVG subdocument; the
    // outer sheet merely adds trim marks without changing any IDs.
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{paper_w}mm\" height=\"{paper_h}mm\" viewBox=\"0 0 {paper_w} {paper_h}\">\n\
        <title>OpenCorel RGB trim proof, not PDF/X</title>\n\
        <g transform=\"translate({m},{m})\">{}</g>\n\
        <g fill=\"none\" stroke=\"#000000\" stroke-width=\"{}\">{marks}</g></svg>\n",
        doc.to_svg(),opts.line_mm
    ))
}
pub fn trim_pdf_bytes(doc:&Document,opts:TrimOptions)->Result<Vec<u8>,String>{
    let svg=trim_svg(doc,opts)?;
    if doc.page.width_mm+2.0*opts.margin_mm>5000.0
        ||doc.page.height_mm+2.0*opts.margin_mm>5000.0{
        return Err("PDF with crop marks supports sheets up to 5000 mm".into());
    }
    let mut options=svg2pdf::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree=svg2pdf::usvg::Tree::from_str(&svg,&options)
        .map_err(|e|format!("Trim SVG parse: {e}"))?;
    let pdf=svg2pdf::to_pdf(&tree,svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default())
        .map_err(|e|format!("Trim PDF generation: {e:?}"))?;
    if !pdf.starts_with(b"%PDF-"){return Err("Invalid PDF proof".into());}
    Ok(pdf)
}
pub fn export_trim(doc:&Document,path:impl AsRef<Path>,
    opts:TrimOptions)->Result<(),String>{
    let path=path.as_ref();
    let data=match path.extension().and_then(|e|e.to_str())
        .unwrap_or("").to_ascii_lowercase().as_str(){
        "svg"=>trim_svg(doc,opts)?.into_bytes(),
        "pdf"=>trim_pdf_bytes(doc,opts)?,
        _=>return Err("Output must have a .svg or .pdf extension".into()),
    };
    let dir=path.parent().filter(|p|!p.as_os_str().is_empty())
        .unwrap_or_else(||Path::new("."));
    let mut tmp=tempfile::Builder::new().prefix(".opencorel-trim-")
        .tempfile_in(dir).map_err(|e|e.to_string())?;
    tmp.write_all(&data).map_err(|e|e.to_string())?;
    tmp.as_file().sync_all().map_err(|e|e.to_string())?;
    tmp.persist(path).map_err(|e|e.error.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Page,Shape,Style};
    #[test] fn trim_svg_contains_eight_marks_and_art_is_unchanged(){
        let mut d=Document::default();
        d.page=Page{width_mm:210.,height_mm:297.};
        d.add(Shape::Rectangle{x:10.,y:10.,w:50.,h:50.},Style::default());
        let before=d.clone();
        let svg=trim_svg(&d,TrimOptions::default()).unwrap();
        assert!(svg.contains("width=\"234mm\""));
        assert_eq!(svg.matches("<line ").count(),8);
        assert!(svg.contains("translate(12,12)"));
        assert_eq!(d,before);
        assert!(trim_svg(&d,TrimOptions{margin_mm:0.,..Default::default()}).is_err());
    }
    #[test] fn trim_pdf_loads_with_independent_pdf_parser(){
        let mut d=Document::default();
        d.page=Page{width_mm:210.,height_mm:297.};
        let pdf=trim_pdf_bytes(&d,TrimOptions::default()).unwrap();
        assert_eq!(lopdf::Document::load_mem(&pdf).unwrap().get_pages().len(),1);
    }
}
