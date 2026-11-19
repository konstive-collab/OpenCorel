//! Assemble one valid multi-page RGB PDF from the independent vector pages.
//! A real PDF page tree is built with unique renumbered indirect objects:
//! resources, embedded fonts/images and page contents cannot collide.
use opencorel_core::Document;
use lopdf::{Document as PdfDoc,Dictionary,Object,ObjectId};
use std::{io::Write,path::Path};

fn materialize_page(doc:&PdfDoc,id:ObjectId)->Result<Dictionary,String>{
    let mut page=doc.get_object(id).map_err(|e|e.to_string())?
        .as_dict().map_err(|e|e.to_string())?.clone();
    // Resource and media-box inheritance is permitted by the PDF spec.
    // Copy those values directly onto the leaf before detaching parent trees.
    for key in [b"Resources".as_slice(),b"MediaBox".as_slice(),
        b"CropBox".as_slice(),b"Rotate".as_slice()]{
        if page.get(key).is_ok(){continue;}
        let mut node=id;
        for _ in 0..16{
            let source=doc.get_object(node).map_err(|e|e.to_string())?
                .as_dict().map_err(|e|e.to_string())?;
            if let Ok(value)=source.get(key){
                page.set(key.to_vec(),value.clone());
                break;
            }
            if let Ok(parent)=source.get(b"Parent").and_then(Object::as_reference){
                node=parent;
            }else{break;}
        }
    }
    if page.get(b"MediaBox").is_err(){
        return Err("PDF page has no MediaBox".into());
    }
    Ok(page)
}
/// Generate a combined book. Empty selected list exports every page in order.
/// RGB proofing only — this is not PDF/X or color-managed CMYK printing.
pub fn pdf_book_bytes(doc:&Document,selected:&[u64])->Result<Vec<u8>,String>{
    doc.validate()?;
    let pages=doc.pages();
    if selected.iter().any(|id|!pages.iter().any(|p|p.id==*id)){
        return Err("PDF selection contains an unknown page".into());
    }
    let chosen:Vec<_>=pages.iter()
        .filter(|p|selected.is_empty()||selected.contains(&p.id)).collect();
    if chosen.is_empty(){return Err("No pages selected".into());}
    let mut out=PdfDoc::with_version("1.7");
    let pages_id=out.new_object_id();
    let catalog_id=out.new_object_id();
    let mut children=Vec::with_capacity(chosen.len());
    for tab in chosen{
        let page=doc.page_document(tab.id)?;
        let source=crate::pdf_bytes(&page)?;
        if source.len()>64*1024*1024{
            return Err("A PDF page exceeds the 64 MiB limit".into());
        }
        let mut part=PdfDoc::load_mem(&source)
            .map_err(|e|format!("Cannot read generated page PDF: {e}"))?;
        let leaf=*part.get_pages().values().next()
            .ok_or("Generated PDF has no page")?;
        let page_dict=materialize_page(&part,leaf)?;
        part.objects.insert(leaf,Object::Dictionary(page_dict));
        part.renumber_objects_with(out.max_id.saturating_add(1));
        let leaf=*part.get_pages().values().next()
            .ok_or("PDF page lost after object renumbering")?;
        let mut page_dict=part.get_object(leaf)
            .map_err(|e|e.to_string())?
            .as_dict().map_err(|e|e.to_string())?.clone();
        page_dict.set("Parent",pages_id);
        part.objects.insert(leaf,Object::Dictionary(page_dict));
        out.max_id=out.max_id.max(part.max_id);
        for (id,obj) in part.objects{
            match obj.type_name().unwrap_or(b""){
                b"Catalog"|b"Pages"|b"Outlines"|b"Outline"=>{},
                _=>{out.objects.insert(id,obj);}
            }
        }
        children.push(Object::Reference(leaf));
    }
    let mut tree=Dictionary::new();
    tree.set("Type",Object::Name(b"Pages".to_vec()));
    tree.set("Kids",Object::Array(children));
    tree.set("Count",Object::Integer(pages.iter()
        .filter(|p|selected.is_empty()||selected.contains(&p.id)).count() as i64));
    out.objects.insert(pages_id,Object::Dictionary(tree));
    let mut catalog=Dictionary::new();
    catalog.set("Type",Object::Name(b"Catalog".to_vec()));
    catalog.set("Pages",pages_id);
    out.objects.insert(catalog_id,Object::Dictionary(catalog));
    out.trailer.set("Root",catalog_id);
    out.compress();
    let mut bytes=Vec::new();
    out.save_to(&mut bytes).map_err(|e|format!("Cannot encode PDF book: {e}"))?;
    let reloaded=PdfDoc::load_mem(&bytes).map_err(|e|
        format!("Combined PDF failed validation: {e}"))?;
    let count=reloaded.get_pages().len();
    let desired=pages.iter().filter(|p|selected.is_empty()||selected.contains(&p.id)).count();
    if count!=desired{
        return Err(format!("Combined PDF page count mismatch: {count}/{desired}"));
    }
    Ok(bytes)
}
pub fn export_pdf_book(doc:&Document,path:impl AsRef<Path>,
    selected:&[u64])->Result<(),String>{
    let bytes=pdf_book_bytes(doc,selected)?;
    let path=path.as_ref();
    let dir=path.parent().filter(|p|!p.as_os_str().is_empty())
        .unwrap_or_else(||Path::new("."));
    let mut tmp=tempfile::Builder::new().prefix(".opencorel-book-")
        .tempfile_in(dir).map_err(|e|e.to_string())?;
    tmp.write_all(&bytes).map_err(|e|e.to_string())?;
    tmp.as_file().sync_all().map_err(|e|e.to_string())?;
    tmp.persist(path).map_err(|e|e.error.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Page,Shape,Style};
    #[test] fn book_contains_all_pages_in_order(){
        let mut d=Document::default();
        d.page=Page{width_mm:210.,height_mm:297.};
        d.add(Shape::Rectangle{x:5.,y:5.,w:30.,h:20.},Style::default());
        let other=d.add_page(Page{width_mm:297.,height_mm:210.}).unwrap();
        d.add(Shape::Ellipse{x:20.,y:20.,w:50.,h:45.},Style::default());
        let bytes=pdf_book_bytes(&d,&[]).unwrap();
        let parsed=PdfDoc::load_mem(&bytes).unwrap();
        assert_eq!(parsed.get_pages().len(),2);
        assert!(pdf_book_bytes(&d,&[999]).is_err());
        let subset=pdf_book_bytes(&d,&[other]).unwrap();
        assert_eq!(PdfDoc::load_mem(&subset).unwrap().get_pages().len(),1);
    }
}
