//! DXF R2000 export for CAD, laser and CNC hand-off.
//! Preserves visible document layers and emits validated 2D polylines.
//! This is an interchange export, not CAM toolpath generation or machine control.
use crate::hpgl::{collect_vector_paths,Scope,VectorPath};
use opencorel_core::{Document,Point};
use std::{collections::{BTreeMap,HashMap},fmt::Write as FmtWrite,io::Write,path::Path};

const MAX_DXF_BYTES:usize=128*1024*1024;
const MAX_SYMBOL_LEN:usize=31;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum DxfUnits{Millimeters,Inches}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum DxfOrigin{BottomLeft,TopLeft}
#[derive(Clone,Copy,Debug)]
pub struct DxfOptions{
    pub units:DxfUnits,
    pub origin:DxfOrigin,
    pub tolerance_mm:f32,
    pub scope:Scope,
    pub offset_x_mm:f32,
    pub offset_y_mm:f32,
}
impl Default for DxfOptions{
    fn default()->Self{
        Self{units:DxfUnits::Millimeters,origin:DxfOrigin::BottomLeft,
            tolerance_mm:0.10,scope:Scope::StrokedOnly,
            offset_x_mm:0.0,offset_y_mm:0.0}
    }
}
impl DxfOptions{
    pub fn validate(self)->Result<(),String>{
        if !self.tolerance_mm.is_finite()||!(0.01..=2.0).contains(&self.tolerance_mm)
            || !self.offset_x_mm.is_finite()||!self.offset_y_mm.is_finite()
            || !(-100_000.0..=100_000.0).contains(&self.offset_x_mm)
            || !(-100_000.0..=100_000.0).contains(&self.offset_y_mm){
            return Err("Invalid DXF tolerance or coordinate offsets".into());
        }
        Ok(())
    }
}
#[derive(Clone,Debug)]
pub struct DxfReport{
    pub path_count:usize,
    pub vertex_count:usize,
    pub layer_count:usize,
    pub cut_length_mm:f64,
    pub skipped_unstroked:usize,
}
fn clean_layer_name(id:u64,name:&str)->String{
    let mut clean=String::new();
    for c in name.chars(){
        let mapped=if c.is_ascii_alphanumeric(){c.to_ascii_uppercase()}
            else if c=='_'||c=='-'||c.is_whitespace(){'_' }else{'_'};
        if mapped!='_'||!clean.ends_with('_'){clean.push(mapped);}
    }
    let clean=clean.trim_matches('_');
    let clean=if clean.is_empty(){"LAYER"}else{clean};
    let prefix=format!("OC{id}_");
    let suffix:String=clean.chars().take(MAX_SYMBOL_LEN.saturating_sub(prefix.len())).collect();
    format!("{prefix}{suffix}")
}
fn num(value:f64)->Result<String,String>{
    if !value.is_finite()||value.abs()>1.0e9{
        return Err("DXF coordinate is outside the supported numeric range".into());
    }
    let value=if value.abs()<0.0000005{0.0}else{value};
    Ok(format!("{value:.6}"))
}
fn pair(out:&mut String,code:u16,value:&str){
    writeln!(out,"{code}\n{value}").unwrap();
}
fn pair_num(out:&mut String,code:u16,value:f64)->Result<(),String>{
    pair(out,code,&num(value)?);Ok(())
}
fn alloc_handle(next:&mut u32)->String{
    let handle=format!("{:X}",*next);
    *next+=1;
    handle
}
fn coordinate(p:Point,page_height:f32,options:DxfOptions)->Result<(f64,f64),String>{
    let factor=match options.units{DxfUnits::Millimeters=>1.0,DxfUnits::Inches=>1.0/25.4};
    let x=(p.x as f64+options.offset_x_mm as f64)*factor;
    let y=(match options.origin{
        DxfOrigin::BottomLeft=>page_height as f64-p.y as f64,
        DxfOrigin::TopLeft=>p.y as f64,
    }+options.offset_y_mm as f64)*factor;
    if !x.is_finite()||!y.is_finite()||x.abs()>1.0e9||y.abs()>1.0e9{
        return Err("DXF coordinate is outside the supported numeric range".into());
    }
    Ok((x,y))
}
fn collect(doc:&Document,options:DxfOptions)
    ->Result<(Vec<VectorPath>,DxfReport,BTreeMap<u64,String>,HashMap<u64,u64>),String>{
    options.validate()?;
    let (paths,base)=collect_vector_paths(doc,options.tolerance_mm,options.scope)?;
    let object_layers:HashMap<u64,u64>=doc.objects.iter().map(|o|(o.id,o.layer_id)).collect();
    let mut names=BTreeMap::new();
    for path in &paths{
        let layer_id=*object_layers.get(&path.source_id)
            .ok_or_else(||format!("Missing source object #{}",path.source_id))?;
        let layer=doc.layer(layer_id).ok_or_else(||format!("Missing layer #{layer_id}"))?;
        names.entry(layer_id).or_insert_with(||clean_layer_name(layer_id,&layer.name));
    }
    let report=DxfReport{path_count:base.path_count,vertex_count:base.vertex_count,
        layer_count:names.len(),cut_length_mm:base.cut_length_mm,
        skipped_unstroked:base.skipped_unstroked};
    Ok((paths,report,names,object_layers))
}
/// Validate geometry and report the manufacturing hand-off complexity.
pub fn analyze(doc:&Document,options:DxfOptions)->Result<DxfReport,String>{
    let (_,report,_,_)=collect(doc,options)?;Ok(report)
}
/// Generate ASCII DXF R2000 with one 2D POLYLINE per source contour.
pub fn dxf_bytes(doc:&Document,options:DxfOptions)->Result<Vec<u8>,String>{
    let (paths,report,layers,object_layers)=collect(doc,options)?;
    let mut out=String::with_capacity(4096usize.saturating_add(report.vertex_count.saturating_mul(100)));
    pair(&mut out,0,"SECTION");pair(&mut out,2,"HEADER");
    pair(&mut out,9,"$ACADVER");pair(&mut out,1,"AC1015");
    let handseed=0x100u64+report.vertex_count as u64
        +report.path_count as u64*2+layers.len() as u64+32;
    pair(&mut out,9,"$HANDSEED");pair(&mut out,5,&format!("{handseed:X}"));
    pair(&mut out,9,"$INSUNITS");
    pair(&mut out,70,match options.units{DxfUnits::Millimeters=>"4",DxfUnits::Inches=>"1"});
    pair(&mut out,9,"$MEASUREMENT");
    pair(&mut out,70,match options.units{DxfUnits::Millimeters=>"1",DxfUnits::Inches=>"0"});
    pair(&mut out,9,"$LIMMIN");pair_num(&mut out,10,0.0)?;pair_num(&mut out,20,0.0)?;
    pair(&mut out,9,"$LIMMAX");
    let factor=match options.units{DxfUnits::Millimeters=>1.0,DxfUnits::Inches=>1.0/25.4};
    pair_num(&mut out,10,doc.page.width_mm as f64*factor)?;
    pair_num(&mut out,20,doc.page.height_mm as f64*factor)?;
    pair(&mut out,0,"ENDSEC");

    // R2000 symbol tables carry handles and subclass markers.
    let mut next_handle=0x100u32;
    let ltype_table=alloc_handle(&mut next_handle);
    let continuous=alloc_handle(&mut next_handle);
    pair(&mut out,0,"SECTION");pair(&mut out,2,"TABLES");
    pair(&mut out,0,"TABLE");pair(&mut out,2,"LTYPE");
    pair(&mut out,5,&ltype_table);pair(&mut out,100,"AcDbSymbolTable");
    pair(&mut out,70,"1");
    pair(&mut out,0,"LTYPE");pair(&mut out,5,&continuous);
    pair(&mut out,100,"AcDbSymbolTableRecord");
    pair(&mut out,100,"AcDbLinetypeTableRecord");
    pair(&mut out,2,"CONTINUOUS");pair(&mut out,70,"0");
    pair(&mut out,3,"Solid line");pair(&mut out,72,"65");
    pair(&mut out,73,"0");pair_num(&mut out,40,0.0)?;
    pair(&mut out,0,"ENDTAB");

    let layer_table=alloc_handle(&mut next_handle);
    pair(&mut out,0,"TABLE");pair(&mut out,2,"LAYER");
    pair(&mut out,5,&layer_table);pair(&mut out,100,"AcDbSymbolTable");
    pair(&mut out,70,&(layers.len()+1).to_string());
    let layer_zero=alloc_handle(&mut next_handle);
    pair(&mut out,0,"LAYER");pair(&mut out,5,&layer_zero);
    pair(&mut out,100,"AcDbSymbolTableRecord");
    pair(&mut out,100,"AcDbLayerTableRecord");
    pair(&mut out,2,"0");pair(&mut out,70,"0");
    pair(&mut out,62,"7");pair(&mut out,6,"CONTINUOUS");
    for name in layers.values(){
        let layer_handle=alloc_handle(&mut next_handle);
        pair(&mut out,0,"LAYER");pair(&mut out,5,&layer_handle);
        pair(&mut out,100,"AcDbSymbolTableRecord");
        pair(&mut out,100,"AcDbLayerTableRecord");
        pair(&mut out,2,name);pair(&mut out,70,"0");
        pair(&mut out,62,"7");pair(&mut out,6,"CONTINUOUS");
    }
    pair(&mut out,0,"ENDTAB");

    let block_table=alloc_handle(&mut next_handle);
    let model_record=alloc_handle(&mut next_handle);
    let paper_record=alloc_handle(&mut next_handle);
    pair(&mut out,0,"TABLE");pair(&mut out,2,"BLOCK_RECORD");
    pair(&mut out,5,&block_table);pair(&mut out,100,"AcDbSymbolTable");
    pair(&mut out,70,"2");
    for (name,handle) in [("*Model_Space",&model_record),("*Paper_Space",&paper_record)]{
        pair(&mut out,0,"BLOCK_RECORD");pair(&mut out,5,handle);
        pair(&mut out,100,"AcDbSymbolTableRecord");
        pair(&mut out,100,"AcDbBlockTableRecord");
        pair(&mut out,2,name);pair(&mut out,70,"0");
    }
    pair(&mut out,0,"ENDTAB");pair(&mut out,0,"ENDSEC");

    // Define model and paper space block records for CAD readers.
    pair(&mut out,0,"SECTION");pair(&mut out,2,"BLOCKS");
    for (name,owner) in [("*Model_Space",&model_record),("*Paper_Space",&paper_record)]{
        let block_handle=alloc_handle(&mut next_handle);
        pair(&mut out,0,"BLOCK");pair(&mut out,5,&block_handle);
        pair(&mut out,330,owner);pair(&mut out,100,"AcDbEntity");
        pair(&mut out,8,"0");pair(&mut out,100,"AcDbBlockBegin");
        pair(&mut out,2,name);pair(&mut out,70,"0");
        pair_num(&mut out,10,0.0)?;pair_num(&mut out,20,0.0)?;
        pair_num(&mut out,30,0.0)?;pair(&mut out,3,name);pair(&mut out,1,"");
        let end_handle=alloc_handle(&mut next_handle);
        pair(&mut out,0,"ENDBLK");pair(&mut out,5,&end_handle);
        pair(&mut out,330,owner);pair(&mut out,100,"AcDbEntity");
        pair(&mut out,8,"0");pair(&mut out,100,"AcDbBlockEnd");
    }
    pair(&mut out,0,"ENDSEC");

    pair(&mut out,0,"SECTION");pair(&mut out,2,"ENTITIES");
    for path in &paths{
        let layer_id=*object_layers.get(&path.source_id)
            .ok_or_else(||format!("Missing source object #{}",path.source_id))?;
        let layer=layers.get(&layer_id).ok_or_else(||format!("Missing DXF layer #{layer_id}"))?;
        let polyline_handle=alloc_handle(&mut next_handle);
        pair(&mut out,0,"POLYLINE");pair(&mut out,5,&polyline_handle);
        pair(&mut out,330,&model_record);pair(&mut out,100,"AcDbEntity");
        pair(&mut out,8,layer);pair(&mut out,100,"AcDb2dPolyline");
        pair(&mut out,66,"1");pair_num(&mut out,10,0.0)?;
        pair_num(&mut out,20,0.0)?;pair_num(&mut out,30,0.0)?;
        pair(&mut out,70,if path.closed{"1"}else{"0"});
        for point in &path.points{
            let (x,y)=coordinate(*point,doc.page.height_mm,options)?;
            let vertex_handle=alloc_handle(&mut next_handle);
            pair(&mut out,0,"VERTEX");pair(&mut out,5,&vertex_handle);
            pair(&mut out,330,&polyline_handle);pair(&mut out,100,"AcDbEntity");
            pair(&mut out,8,layer);pair(&mut out,100,"AcDbVertex");
            pair(&mut out,100,"AcDb2dVertex");
            pair_num(&mut out,10,x)?;pair_num(&mut out,20,y)?;
            pair_num(&mut out,30,0.0)?;pair(&mut out,70,"0");
            if out.len()>MAX_DXF_BYTES{return Err("DXF export exceeds 128 MiB safety limit".into());}
        }
        let seqend_handle=alloc_handle(&mut next_handle);
        pair(&mut out,0,"SEQEND");pair(&mut out,5,&seqend_handle);
        pair(&mut out,330,&polyline_handle);pair(&mut out,100,"AcDbEntity");
        pair(&mut out,8,layer);pair(&mut out,100,"AcDbSequenceEnd");
    }
    pair(&mut out,0,"ENDSEC");pair(&mut out,0,"EOF");
    if out.len()>MAX_DXF_BYTES{return Err("DXF export exceeds 128 MiB safety limit".into());}
    Ok(out.into_bytes())
}
/// Atomically save a DXF hand-off file. No device is contacted.
pub fn export_dxf(doc:&Document,path:impl AsRef<Path>,options:DxfOptions)->Result<(),String>{
    let path=path.as_ref();
    let ext=path.extension().and_then(|v|v.to_str()).unwrap_or("").to_ascii_lowercase();
    if ext!="dxf"{return Err("DXF export requires a .dxf extension".into());}
    let bytes=dxf_bytes(doc,options)?;
    let dir=path.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or_else(||Path::new("."));
    let mut temp=tempfile::Builder::new().prefix(".opencorel-dxf-")
        .tempfile_in(dir).map_err(|e|e.to_string())?;
    temp.write_all(&bytes).map_err(|e|e.to_string())?;
    temp.as_file().sync_all().map_err(|e|e.to_string())?;
    temp.persist(path).map_err(|e|e.error.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Point,Shape,Style};
    fn doc()->Document{
        let mut d=Document::default();d.page.width_mm=100.;d.page.height_mm=100.;d
    }
    fn text(doc:&Document,options:DxfOptions)->String{
        String::from_utf8(dxf_bytes(doc,options).unwrap()).unwrap()
    }
    #[test] fn exports_closed_rectangle_in_mm_with_cad_y_up(){
        let mut d=doc();d.add(Shape::Rectangle{x:10.,y:20.,w:30.,h:40.},Style::default());
        let out=text(&d,DxfOptions::default());
        assert!(out.contains("1\nAC1015\n"));
        assert!(out.contains("9\n$INSUNITS\n70\n4\n"));
        assert!(out.contains("0\nPOLYLINE\n")&&out.contains("70\n1\n"));
        assert!(out.contains("10\n10.000000\n20\n80.000000\n"));
        assert!(out.ends_with("0\nEOF\n"));
        assert!(out.contains("2\nLTYPE\n")&&out.contains("2\nBLOCK_RECORD\n"));
        assert!(out.contains("100\nAcDb2dPolyline\n"));
        let mut handles=std::collections::HashSet::new();
        let lines:Vec<_>=out.lines().collect();
        for pair in lines.windows(2){
            if pair[0]=="5"{assert!(handles.insert(pair[1].to_string()),"duplicate DXF handle {}",pair[1]);}
        }
        let r=analyze(&d,DxfOptions::default()).unwrap();
        assert_eq!(r.path_count,1);assert_eq!(r.vertex_count,4);
        assert!((r.cut_length_mm-140.0).abs()<0.001);
    }
    #[test] fn inches_and_top_left_origin_are_explicit(){
        let mut d=doc();d.add(Shape::Polyline{points:vec![
            Point{x:25.4,y:25.4},Point{x:50.8,y:50.8}],closed:false},Style::default());
        let out=text(&d,DxfOptions{units:DxfUnits::Inches,origin:DxfOrigin::TopLeft,
            ..DxfOptions::default()});
        assert!(out.contains("9\n$INSUNITS\n70\n1\n"));
        assert!(out.contains("10\n1.000000\n20\n1.000000\n"));
        assert!(out.contains("70\n0\n"));
    }
    #[test] fn source_layers_are_preserved_and_safe_for_dxf(){
        let mut d=doc();let cut=d.add_layer("Cut contour / №1".into());
        d.add_to_layer(Shape::Rectangle{x:1.,y:1.,w:10.,h:10.},Style::default(),cut);
        let out=text(&d,DxfOptions::default());
        assert!(out.contains("OC2_CUT_CONTOUR_1"));
        assert!(out.contains("8\nOC2_CUT_CONTOUR_1\n"));
    }
    #[test] fn hidden_layers_are_not_exported(){
        let mut d=doc();d.add(Shape::Rectangle{x:1.,y:1.,w:10.,h:10.},Style::default());
        let hidden=d.add_layer("Hidden".into());
        d.add_to_layer(Shape::Rectangle{x:20.,y:20.,w:10.,h:10.},Style::default(),hidden);
        d.layers.iter_mut().find(|l|l.id==hidden).unwrap().visible=false;
        let out=text(&d,DxfOptions::default());
        assert!(out.contains("OC1_LAYER"));assert!(!out.contains("OC2_HIDDEN"));
        let r=analyze(&d,DxfOptions::default()).unwrap();
        assert_eq!(r.path_count,1);assert_eq!(r.layer_count,1);
    }
    #[test] fn ellipses_and_curves_are_flattened_to_polylines(){
        let mut d=doc();d.add(Shape::Ellipse{x:10.,y:10.,w:40.,h:30.},Style::default());
        let r=analyze(&d,DxfOptions::default()).unwrap();
        assert!(r.vertex_count>20);assert_eq!(r.path_count,1);assert!(r.cut_length_mm>100.0);
    }
    #[test] fn rejects_text_and_out_of_page_geometry(){
        let mut d=doc();d.add(Shape::Text{x:1.,y:1.,value:"TEXT".into(),size_mm:5.},Style::default());
        assert!(dxf_bytes(&d,DxfOptions::default()).unwrap_err().contains("Text"));
        let mut d=doc();d.add(Shape::Rectangle{x:-1.,y:1.,w:10.,h:10.},Style::default());
        assert!(dxf_bytes(&d,DxfOptions::default()).unwrap_err().contains("leaves"));
    }
    #[test] fn refuses_masked_jobs_and_invalid_options(){
        let mut d=doc();
        let a=d.add(Shape::Rectangle{x:1.,y:1.,w:20.,h:20.},Style::default());
        let b=d.add(Shape::Rectangle{x:2.,y:2.,w:10.,h:10.},Style::default());
        d.set_clip(a,b).unwrap();
        assert!(dxf_bytes(&d,DxfOptions::default()).unwrap_err().contains("Clipping"));
        let mut opt=DxfOptions::default();opt.tolerance_mm=f32::NAN;
        assert!(analyze(&doc(),opt).is_err());
    }
    #[test] fn invalid_extension_never_creates_output(){
        let mut d=doc();d.add(Shape::Rectangle{x:1.,y:1.,w:10.,h:10.},Style::default());
        let tmp=tempfile::tempdir().unwrap();let p=tmp.path().join("not-a-dxf.txt");
        assert!(export_dxf(&d,&p,DxfOptions::default()).is_err());assert!(!p.exists());
        let p=tmp.path().join("ok.dxf");export_dxf(&d,&p,DxfOptions::default()).unwrap();
        assert!(std::fs::read_to_string(p).unwrap().ends_with("0\nEOF\n"));
    }
}
