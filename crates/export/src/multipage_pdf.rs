//! Multi-page vector PDF export. Individual SVG pages are converted with
//! the existing svg2pdf engine, then their page trees are nested under a new
//! root. Keeping the original resource-owning Pages nodes preserves inherited
//! MediaBox, Resources, CropBox and font references without flattening pages.
use lopdf::{dictionary,Document as PdfDocument,Object};
use opencorel_core::Document;
use std::{io::Write,path::Path};

const MAX_PDF_PAGES:usize=64;
const MAX_MERGED_BYTES:usize=192*1024*1024;

/// Produce a genuine, vector multi-page PDF. Per-page font embedding and RGB
/// SVG rendering continue to use the same converter as the legacy PDF export.
pub fn multipage_pdf_bytes(source:&Document)->Result<Vec<u8>,String>{
    source.validate()?;
    let page_count=source.page_count();
    if page_count==0||page_count>MAX_PDF_PAGES{
        return Err("PDF pages must be between 1 and 64".into());
    }
    if page_count==1{return crate::pdf_bytes(source);}
    let mut joined=PdfDocument::with_version("1.7");
    let root_pages_id=joined.new_object_id();
    let mut kids=Vec::with_capacity(page_count);
    let mut total=0usize;
    for index in 0..page_count{
        let page=source.page_document(index)?;
        let bytes=crate::pdf_bytes(&page)
            .map_err(|e|format!("Страница {}: {e}",index+1))?;
        total=total.checked_add(bytes.len())
            .ok_or("PDF byte count overflow")?;
        if total>MAX_MERGED_BYTES{
            return Err("PDF pages exceed 192 MiB safety limit".into());
        }
        let mut incoming=PdfDocument::load_mem(&bytes)
            .map_err(|e|format!("Invalid generated PDF page {}: {e}",index+1))?;
        if incoming.get_pages().len()!=1{
            return Err(format!("PDF conversion yielded unexpected page count for page {}",index+1));
        }
        incoming.renumber_objects_with(joined.max_id+1);
        // A source PDF has its own Catalog -> Pages tree. Preserve its
        // inherited page attributes by attaching the Pages root, not the Page.
        let pages_id=incoming.catalog().map_err(|e|e.to_string())?
            .get(b"Pages").map_err(|e|e.to_string())?
            .as_reference().map_err(|e|e.to_string())?;
        let root=incoming.get_object_mut(pages_id)
            .map_err(|e|e.to_string())?
            .as_dict_mut().map_err(|e|e.to_string())?;
        root.set("Parent",root_pages_id);
        kids.push(Object::Reference(pages_id));
        joined.max_id=joined.max_id.max(incoming.max_id);
        joined.objects.extend(incoming.objects);
    }
    joined.objects.insert(root_pages_id,Object::Dictionary(dictionary!{
        "Type"=>"Pages",
        "Kids"=>kids,
        "Count"=>page_count as u32
    }));
    let catalog=joined.add_object(dictionary!{
        "Type"=>"Catalog",
        "Pages"=>root_pages_id
    });
    joined.trailer.set("Root",catalog);
    let mut out=Vec::new();
    joined.save_to(&mut out).map_err(|e|format!("Could not write multipage PDF: {e}"))?;
    let verify=PdfDocument::load_mem(&out)
        .map_err(|e|format!("Multi-page PDF could not be reopened: {e}"))?;
    if verify.get_pages().len()!=page_count{
        return Err("Multi-page PDF verification failed".into());
    }
    Ok(out)
}

/// Atomic publication: failure does not replace an existing customer's file.
pub fn export_multipage_pdf(source:&Document,path:impl AsRef<Path>)->Result<(),String>{
    let bytes=multipage_pdf_bytes(source)?;
    let path=path.as_ref();
    let dir=path.parent().filter(|p|!p.as_os_str().is_empty())
        .unwrap_or_else(||Path::new("."));
    let mut tmp=tempfile::Builder::new().prefix(".opencorel-multipdf-")
        .tempfile_in(dir).map_err(|e|e.to_string())?;
    tmp.write_all(&bytes).map_err(|e|e.to_string())?;
    tmp.as_file().sync_all().map_err(|e|e.to_string())?;
    tmp.persist(path).map_err(|e|e.error.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Shape,Style};
    #[test]
    fn three_vector_pages_are_in_pdf_in_right_order_and_different_sizes(){
        let mut d=Document::default();
        d.page.width_mm=210.;d.page.height_mm=297.;
        d.add(Shape::Rectangle{x:20.,y:20.,w:40.,h:40.},Style::default());
        d.append_page(false).unwrap();
        d.page.width_mm=90.;d.page.height_mm=50.;
        d.add(Shape::Ellipse{x:1.,y:1.,w:10.,h:10.},Style::default());
        d.append_page(false).unwrap();
        d.page.width_mm=300.;d.page.height_mm=100.;
        d.add(Shape::Text{x:6.,y:7.,value:"Page three".into(),size_mm:10.},
            Style::default());
        let original=d.clone();
        let bytes=multipage_pdf_bytes(&d).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        let pdf=PdfDocument::load_mem(&bytes).unwrap();
        assert_eq!(pdf.get_pages().len(),3);
        for (i,(w,h)) in [(210.,297.),(90.,50.),(300.,100.)].iter().enumerate(){
            let page_id=pdf.get_pages()[&((i+1) as u32)];
            let dims=pdf.get_page_media_box(page_id).unwrap();
            let coordinates=dims.as_array().unwrap();
            let width=coordinates[2].as_float().unwrap_or_else(|_|
                coordinates[2].as_i64().unwrap() as f32);
            let height=coordinates[3].as_float().unwrap_or_else(|_|
                coordinates[3].as_i64().unwrap() as f32);
            assert!((width-w*72./25.4).abs()<1.0);
            assert!((height-h*72./25.4).abs()<1.0);
        }
        assert_eq!(d,original);
    }
    #[test]
    fn invalid_pdf_page_is_rejected_without_output(){
        let mut d=Document::default();
        d.append_page(false).unwrap();
        d.page.width_mm=6000.;
        assert!(multipage_pdf_bytes(&d).is_err());
    }
    #[test]
    fn one_page_uses_original_pdf_bytes(){
        let mut d=Document::default();
        d.add(Shape::Rectangle{x:2.,y:3.,w:6.,h:7.},Style::default());
        assert_eq!(multipage_pdf_bytes(&d).unwrap(),crate::pdf_bytes(&d).unwrap());
    }
    #[test]
    fn export_is_atomic_with_invalid_source(){
        let dir=tempfile::tempdir().unwrap();
        let file=dir.path().join("proof.pdf");
        std::fs::write(&file,b"keep existing proof").unwrap();
        let mut d=Document::default();
        d.append_page(false).unwrap();
        d.page.width_mm=7000.;
        assert!(export_multipage_pdf(&d,&file).is_err());
        assert_eq!(std::fs::read(&file).unwrap(),b"keep existing proof");
    }
}
