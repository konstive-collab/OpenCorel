//! Page batch export. Existing SVG/PDF/raster backends are reused without
//! duplicating document rendering. All selected pages are rendered to a
//! staging directory before publication; output file collisions are rejected.
use opencorel_core::Document;
use std::{fs,path::{Path,PathBuf}};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum PageExportFormat{Svg,Pdf,Png,Jpeg}
impl PageExportFormat{
    pub fn extension(self)->&'static str{
        match self{Self::Svg=>"svg",Self::Pdf=>"pdf",Self::Png=>"png",Self::Jpeg=>"jpg"}
    }
    pub fn label(self)->&'static str{
        match self{Self::Svg=>"SVG (вектор)",Self::Pdf=>"PDF (RGB)",
            Self::Png=>"PNG (растр)",Self::Jpeg=>"JPEG (растр)"}
    }
}
fn safe_name(value:&str)->String{
    let clean:String=value.chars().filter_map(|c|
        if c.is_alphanumeric(){Some(c)}
        else if c==' '||c=='_'||c=='-'{Some('_')}
        else{None}
    ).take(35).collect();
    let normalized=clean.trim_matches('_');
    if normalized.is_empty(){"page".into()}else{normalized.into()}
}
/// Exports arbitrary selected pages (or all pages if selection is empty).
/// Unmodified source page names and globally unique IDs are preserved in .kdr.
pub fn export_pages(doc:&Document,directory:&Path,format:PageExportFormat,
    selected:&[u64],dpi:u32)->Result<Vec<PathBuf>,String>{
    if !(36..=600).contains(&dpi){return Err("DPI must be between 36 and 600".into());}
    doc.validate()?;
    if !directory.is_dir(){return Err("Select an existing output directory".into());}
    let pages=doc.pages();
    let targets:Vec<_>=pages.iter().enumerate().filter(|(_,p)|
        selected.is_empty()||selected.contains(&p.id)).collect();
    if targets.is_empty(){return Err("No pages selected for export".into());}
    if selected.iter().any(|id|!pages.iter().any(|p|p.id==*id)){
        return Err("Page selection contains unknown identifiers".into());
    }
    let slug=safe_name(&doc.name);
    let mut outputs=Vec::with_capacity(targets.len());
    for (index,tab) in &targets{
        let path=directory.join(format!("{}_p{:02}_{}.{}",slug,index+1,
            safe_name(&tab.name),format.extension()));
        if path.exists(){return Err(format!("Output already exists: {}",path.display()));}
        outputs.push(path);
    }
    let stage=tempfile::Builder::new().prefix(".opencorel-batch-")
        .tempdir_in(directory).map_err(|e|e.to_string())?;
    for ((_,page),dest) in targets.iter().zip(outputs.iter()){
        let sheet=doc.page_document(page.id)?;
        sheet.validate()?;
        let file=stage.path().join(dest.file_name().ok_or("Invalid export filename")?);
        match format{
            PageExportFormat::Svg=>sheet.export_svg(&file).map_err(|e|e.to_string())?,
            PageExportFormat::Pdf=>crate::export_pdf(&sheet,&file)?,
            PageExportFormat::Png|PageExportFormat::Jpeg=>
                crate::raster_export::export_raster(&sheet,&file,dpi)?
        }
    }
    let mut published=Vec::with_capacity(outputs.len());
    for dest in &outputs{
        let src=stage.path().join(dest.file_name().ok_or("Invalid export filename")?);
        if let Err(error)=fs::rename(&src,dest){
            for path in &published{let _=fs::remove_file(path);}
            return Err(format!("Cannot publish page export: {error}"));
        }
        published.push(dest.clone());
    }
    Ok(published)
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Page,Shape,Style};
    #[test] fn exports_all_pages_to_independent_svg_files(){
        let mut d=Document::default();
        d.name="Catalog".into();
        d.add(Shape::Rectangle{x:5.,y:6.,w:7.,h:8.},Style::default());
        let second=d.add_page(Page{width_mm:210.,height_mm:297.}).unwrap();
        d.add(Shape::Ellipse{x:10.,y:20.,w:30.,h:40.},Style::default());
        let temp=tempfile::tempdir().unwrap();
        let files=export_pages(&d,temp.path(),PageExportFormat::Svg,&[],96).unwrap();
        assert_eq!(files.len(),2);
        assert!(fs::read_to_string(&files[0]).unwrap().contains("<rect "));
        assert!(fs::read_to_string(&files[1]).unwrap().contains("<ellipse "));
        assert!(export_pages(&d,temp.path(),PageExportFormat::Svg,&[],96).is_err());
        let selected=tempfile::tempdir().unwrap();
        assert_eq!(export_pages(&d,selected.path(),PageExportFormat::Svg,&[second],96)
            .unwrap().len(),1);
        assert_eq!(d.active_page_id,second);
    }
    #[test] fn rejected_page_selection_is_atomic(){
        let d=Document::default();
        let temp=tempfile::tempdir().unwrap();
        assert!(export_pages(&d,temp.path(),PageExportFormat::Svg,&[777],96).is_err());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(),0);
    }
}
