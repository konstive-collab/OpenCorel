//! Conservative HP-GL/1 (PLT) production export for pen plotters and vinyl
//! cutters. This module creates files; it NEVER sends commands to hardware.
//! Always dry-run on the target device and verify the physical origin.
use opencorel_core::{CubicSegment,Document,Point,Shape};
use std::{collections::HashSet,fmt::Write as FmtWrite,io::Write,path::Path};

const MAX_VERTICES:usize=250_000;
const MAX_PATHS:usize=100_000;
const MAX_RECURSION:u8=16;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Origin{BottomLeft,TopLeft}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Scope{StrokedOnly,AllVectorOutlines}
#[derive(Clone,Copy,Debug)]
pub struct PlotterOptions{
    /// Common HP-GL uses 40 plotter units per millimetre (0.025mm/unit).
    pub units_per_mm:u32,
    /// Maximum permitted cubic-flattening deviation in millimetres.
    pub tolerance_mm:f32,
    pub origin:Origin,
    pub scope:Scope,
    pub pen:u8,
    pub offset_x_mm:f32,
    pub offset_y_mm:f32,
}
impl Default for PlotterOptions{
    fn default()->Self{
        Self{units_per_mm:40,tolerance_mm:0.15,origin:Origin::BottomLeft,
            scope:Scope::StrokedOnly,pen:1,offset_x_mm:0.0,offset_y_mm:0.0}
    }
}
#[derive(Clone,Debug)]
pub struct PlotterReport{
    pub path_count:usize,
    pub vertex_count:usize,
    pub cut_length_mm:f64,
    pub travel_length_mm:f64,
    pub skipped_unstroked:usize,
}
impl PlotterOptions{
    pub fn validate(self)->Result<(),String>{
        if !(1..=1000).contains(&self.units_per_mm)
            || !(1..=8).contains(&self.pen)
            || !self.tolerance_mm.is_finite()
            || !(0.01..=2.0).contains(&self.tolerance_mm)
            || !self.offset_x_mm.is_finite()||!self.offset_y_mm.is_finite()
            || !(0.0..=5000.0).contains(&self.offset_x_mm)
            || !(0.0..=5000.0).contains(&self.offset_y_mm){
            return Err("Invalid plotter units, pen, tolerance, or offsets".into());
        }
        Ok(())
    }
}
#[derive(Clone)]
pub struct VectorPath{
    points:Vec<Point>,
    closed:bool,
    source_id:u64,
}
fn distance(a:Point,b:Point)->f64{
    ((a.x as f64-b.x as f64).powi(2)+(a.y as f64-b.y as f64).powi(2)).sqrt()
}
fn halfway(a:Point,b:Point)->Point{
    Point{x:(a.x+b.x)*0.5,y:(a.y+b.y)*0.5}
}
fn dist_to_chord_sq(p:Point,a:Point,b:Point)->f32{
    let (dx,dy)=(b.x-a.x,b.y-a.y);
    let length2=dx*dx+dy*dy;
    if length2<=1e-14{return (p.x-a.x).powi(2)+(p.y-a.y).powi(2);}
    // Distance to a finite chord, not its infinite line: collinear control
    // handles outside the endpoints can still cause severe cubic overshoot.
    let t=(((p.x-a.x)*dx+(p.y-a.y)*dy)/length2).clamp(0.0,1.0);
    let (qx,qy)=(a.x+t*dx,a.y+t*dy);
    (p.x-qx).powi(2)+(p.y-qy).powi(2)
}
fn flatten_cubic(points:&mut Vec<Point>,start:Point,seg:&CubicSegment,
    tolerance2:f32,depth:u8)->Result<(),String>{
    let flat=dist_to_chord_sq(seg.control1,start,seg.end).max(
        dist_to_chord_sq(seg.control2,start,seg.end));
    if flat<=tolerance2{
        points.push(seg.end);
    }else{
        if depth>=MAX_RECURSION{
            return Err("Curve requires more subdivisions than the safe plotter limit".into());
        }
        let a=halfway(start,seg.control1);
        let b=halfway(seg.control1,seg.control2);
        let c=halfway(seg.control2,seg.end);
        let d=halfway(a,b);
        let e=halfway(b,c);
        let mid=halfway(d,e);
        let left=CubicSegment{control1:a,control2:d,end:mid};
        let right=CubicSegment{control1:e,control2:c,end:seg.end};
        flatten_cubic(points,start,&left,tolerance2,depth+1)?;
        flatten_cubic(points,mid,&right,tolerance2,depth+1)?;
    }
    if points.len()>MAX_VERTICES{
        return Err("Cut trajectory exceeds safe vertex budget".into());
    }
    Ok(())
}
fn flatten_segments(start:Point,segments:&[CubicSegment],closed:bool,
    tolerance:f32)->Result<VectorPath,String>{
    let mut points=Vec::with_capacity(segments.len().min(256)+1);
    points.push(start);
    let mut prev=start;
    for segment in segments{
        flatten_cubic(&mut points,prev,segment,tolerance*tolerance,0)?;
        prev=segment.end;
    }
    Ok(VectorPath{points,closed,source_id:0})
}
fn ellipse(x:f32,y:f32,w:f32,h:f32,tolerance:f32)->Result<Vec<Point>,String>{
    if w<=0.0||h<=0.0{return Err("Degenerate ellipse is not cuttable".into());}
    let r=(w.max(h)*0.5) as f64;
    let delta=(tolerance as f64/r).min(1.0);
    // Sagitta bound for a circular arc is conservative for ellipse axes.
    let step=(1.0-delta).acos();
    let count=if step==0.0{MAX_VERTICES+1}
        else{(std::f64::consts::PI/step).ceil() as usize};
    let count=count.max(16);
    if count>MAX_VERTICES{return Err("Ellipse exceeds safe plotter resolution".into());}
    Ok((0..count).map(|i|{
        let angle=std::f64::consts::TAU*i as f64/count as f64;
        Point{x:x+w*0.5+(w as f64*0.5*angle.cos()) as f32,
            y:y+h*0.5+(h as f64*0.5*angle.sin()) as f32}
    }).collect())
}
fn object_paths(id:u64,shape:&Shape,tolerance:f32)->Result<Vec<VectorPath>,String>{
    let mut paths=match shape{
        Shape::Rectangle{x,y,w,h}=>{
            if *w<=0.0||*h<=0.0{return Err("Degenerate rectangle".into());}
            vec![VectorPath{points:vec![
                Point{x:*x,y:*y},Point{x:*x+*w,y:*y},
                Point{x:*x+*w,y:*y+*h},Point{x:*x,y:*y+*h},
            ],closed:true,source_id:id}]
        }
        Shape::Ellipse{x,y,w,h}=>vec![VectorPath{points:ellipse(*x,*y,*w,*h,tolerance)?,
            closed:true,source_id:id}],
        Shape::Polyline{points,closed}=>vec![VectorPath{
            points:points.clone(),closed:*closed,source_id:id}],
        Shape::Bezier{start,segments,closed}=>vec![
            flatten_segments(*start,segments,*closed,tolerance)?],
        Shape::CompoundPath{contours}=>{
            let mut output=Vec::with_capacity(contours.len());
            for contour in contours{
                output.push(flatten_segments(
                    contour.start,&contour.segments,contour.closed,tolerance)?);
            }
            output
        }
        Shape::Text{..}=>return Err(format!(
            "Text object #{id} is not a cut path: convert text to curves first")),
        Shape::Raster{..}|Shape::EmbeddedSvg{..}=>return Err(format!(
            "Object #{id} is placed artwork: convert it to editable vector contours first")),
    };
    for p in &mut paths{
        p.source_id=id;
        if p.points.len()<2 || (p.closed && p.points.len()<3){
            return Err(format!("Object #{id} contains an incomplete cut path"));
        }
    }
    Ok(paths)
}
fn collect_paths(doc:&Document,options:PlotterOptions)->Result<(Vec<VectorPath>,PlotterReport),String>{
    doc.validate()?;
    options.validate()?;
    if !doc.clip_links.is_empty(){
        return Err("Clipping masks are not supported by HP-GL: remove or flatten them first".into());
    }
    let visible:HashSet<u64>=doc.layers.iter().filter(|l|l.visible).map(|l|l.id).collect();
    let mut paths=Vec::new();
    let mut report=PlotterReport{path_count:0,vertex_count:0,
        cut_length_mm:0.0,travel_length_mm:0.0,skipped_unstroked:0};
    let mut previous_end=None;
    for object in &doc.objects{
        if !visible.contains(&object.layer_id){continue;}
        if options.scope==Scope::StrokedOnly
            && (object.style.stroke=="none"||object.style.stroke_width_mm==0.0){
            report.skipped_unstroked+=1;continue;
        }
        let next=object_paths(object.id,&object.shape,options.tolerance_mm)?;
        for p in next{
            let remaining=MAX_VERTICES.saturating_sub(report.vertex_count);
            if p.points.len()>remaining || report.path_count>=MAX_PATHS{
                return Err("Plotter job exceeds safe number of paths or vertices".into());
            }
            for &point in &p.points{
                if point.x< -0.001||point.y< -0.001
                    || point.x>doc.page.width_mm+0.001
                    || point.y>doc.page.height_mm+0.001{
                    return Err(format!("Object #{} leaves the page/cutting area",p.source_id));
                }
            }
            for segment in p.points.windows(2){
                report.cut_length_mm+=distance(segment[0],segment[1]);
            }
            if p.closed{
                report.cut_length_mm+=distance(*p.points.last().unwrap(),p.points[0]);
            }
            if let Some(prev)=previous_end{
                report.travel_length_mm+=distance(prev,p.points[0]);
            }
            previous_end=Some(if p.closed{p.points[0]}else{*p.points.last().unwrap()});
            report.vertex_count+=p.points.len();
            report.path_count+=1;
            paths.push(p);
        }
    }
    if paths.is_empty(){return Err("No plotter paths to export; add vector strokes".into());}
    Ok((paths,report))
}
/// Preview production characteristics before writing a plotter file.
pub fn analyze(doc:&Document,options:PlotterOptions)->Result<PlotterReport,String>{
    let (_,report)=collect_paths(doc,options)?;
    Ok(report)
}

/// Shared, validated geometry extraction for manufacturing interchange formats.
pub fn collect_vector_paths(doc:&Document,tolerance_mm:f32,scope:Scope)
    ->Result<(Vec<VectorPath>,PlotterReport),String>{
    collect_paths(doc,PlotterOptions{
        tolerance_mm,scope,..PlotterOptions::default()
    })
}
fn machine_point(point:Point,height_mm:f32,options:PlotterOptions)
    ->Result<(i64,i64),String>{
    let x=point.x as f64+options.offset_x_mm as f64;
    let y=match options.origin{
        Origin::TopLeft=>point.y as f64,
        Origin::BottomLeft=>height_mm as f64-point.y as f64,
    }+options.offset_y_mm as f64;
    let scalar=options.units_per_mm as f64;
    let (x,y)=((x*scalar).round(),(y*scalar).round());
    if !x.is_finite()||!y.is_finite()
        || !(0.0..=2_000_000_000.0).contains(&x)
        || !(0.0..=2_000_000_000.0).contains(&y){
        return Err("Plotter coordinates exceed safe machine range".into());
    }
    Ok((x as i64,y as i64))
}
/// Generate HP-GL/1 commands in absolute units with pen-up between paths.
/// This is NOT an HP-GL/2 or model-specific cutter driver.
pub fn hpgl_bytes(doc:&Document,options:PlotterOptions)->Result<Vec<u8>,String>{
    let (paths,_)=collect_paths(doc,options)?;
    let mut result=String::with_capacity(256+paths.iter()
        .map(|p|p.points.len()).sum::<usize>().saturating_mul(20));
    write!(&mut result,"IN;SP{};PA;\n",options.pen).unwrap();
    for p in paths{
        let first=machine_point(p.points[0],doc.page.height_mm,options)?;
        write!(&mut result,"PU{},{};",first.0,first.1).unwrap();
        let mut prev=first;
        for point in p.points.iter().skip(1){
            let q=machine_point(*point,doc.page.height_mm,options)?;
            if q!=prev{
                write!(&mut result,"PD{},{};",q.0,q.1).unwrap();
                prev=q;
            }
        }
        if p.closed && prev!=first{
            write!(&mut result,"PD{},{};",first.0,first.1).unwrap();
        }
        result.push_str("PU;\n");
    }
    result.push_str("SP0;\n");
    Ok(result.into_bytes())
}
/// Save a fully validated .plt file atomically. Never sends data to the device.
pub fn export_hpgl(doc:&Document,path:impl AsRef<Path>,
    options:PlotterOptions)->Result<(),String>{
    let path=path.as_ref();
    let suffix=path.extension().and_then(|s|s.to_str()).unwrap_or("").to_ascii_lowercase();
    if suffix!="plt" && suffix!="hpgl"{
        return Err("Plotter export requires .plt or .hpgl extension".into());
    }
    let data=hpgl_bytes(doc,options)?;
    let dir=path.parent().filter(|p|!p.as_os_str().is_empty())
        .unwrap_or_else(||Path::new("."));
    let mut temp=tempfile::Builder::new().prefix(".opencorel-plotter-")
        .tempfile_in(dir).map_err(|e|e.to_string())?;
    temp.write_all(&data).map_err(|e|e.to_string())?;
    temp.as_file().sync_all().map_err(|e|e.to_string())?;
    temp.persist(path).map_err(|e|e.error.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Contour,Style};
    fn doc()->Document{
        let mut d=Document::default();
        d.page.width_mm=100.;
        d.page.height_mm=100.;
        d
    }
    #[test] fn rectangle_scales_to_hpgl_units_and_flips_y(){
        let mut d=doc();
        d.add(Shape::Rectangle{x:10.,y:20.,w:30.,h:20.},Style::default());
        let text=String::from_utf8(hpgl_bytes(&d,PlotterOptions::default()).unwrap()).unwrap();
        assert!(text.starts_with("IN;SP1;PA;\nPU400,3200;"));
        assert!(text.contains("PD1600,3200;PD1600,2400;PD400,2400;PD400,3200;"));
        assert!(text.ends_with("PU;\nSP0;\n"));
        let report=analyze(&d,PlotterOptions::default()).unwrap();
        assert_eq!(report.path_count,1);
        assert!((report.cut_length_mm-100.0).abs()<0.001);
    }
    #[test] fn open_path_is_not_automatically_closed(){
        let mut d=doc();
        d.add(Shape::Polyline{points:vec![Point{x:1.,y:1.},Point{x:2.,y:2.}],
            closed:false},Style::default());
        let text=String::from_utf8(hpgl_bytes(&d,PlotterOptions{
            origin:Origin::TopLeft,..PlotterOptions::default()
        }).unwrap()).unwrap();
        assert!(text.contains("PU40,40;PD80,80;PU;"));
        assert!(!text.contains("PD40,40;"));
    }
    #[test] fn smooth_ellipse_uses_multiple_short_segments(){
        let mut d=doc();
        d.add(Shape::Ellipse{x:10.,y:10.,w:50.,h:50.},Style::default());
        let report=analyze(&d,PlotterOptions::default()).unwrap();
        assert!(report.vertex_count>=24);
        assert!(report.cut_length_mm>150.0 && report.cut_length_mm<160.0);
        assert_eq!(report.path_count,1);
    }
    #[test] fn compound_ring_has_separate_pen_up_paths(){
        let mut d=doc();
        let contour=|x:f32|Contour{start:Point{x,y:x},
            segments:vec![
                CubicSegment{control1:Point{x:x+5.,y:x},control2:Point{x:x+10.,y:x},end:Point{x:x+15.,y:x}},
                CubicSegment{control1:Point{x:x+15.,y:x+5.},control2:Point{x:x+15.,y:x+10.},end:Point{x:x+15.,y:x+15.}},
                CubicSegment{control1:Point{x:x+10.,y:x+15.},control2:Point{x:x+5.,y:x+15.},end:Point{x,y:x+15.}},
                CubicSegment{control1:Point{x,y:x+10.},control2:Point{x,y:x+5.},end:Point{x,y:x}}
            ],closed:true};
        d.add(Shape::CompoundPath{contours:vec![contour(10.),contour(40.)]},
            Style::default());
        let bytes=hpgl_bytes(&d,PlotterOptions::default()).unwrap();
        let text=String::from_utf8(bytes).unwrap();
        assert_eq!(text.matches("PU;\n").count(),2);
        assert_eq!(analyze(&d,PlotterOptions::default()).unwrap().path_count,2);
    }
    #[test] fn unsupported_text_causes_explicit_failure(){
        let mut d=doc();
        d.add(Shape::Text{x:0.,y:1.,value:"CUT".into(),size_mm:5.},Style::default());
        assert!(hpgl_bytes(&d,PlotterOptions::default()).unwrap_err().contains("Text"));
    }
    #[test] fn fill_only_objects_skip_in_stroke_mode(){
        let mut d=doc();
        d.add(Shape::Text{x:1.,y:1.,value:"EXCLUDED".into(),size_mm:4.},
            Style{stroke:"none".into(),stroke_width_mm:0.0,fill:"#FFFFFF".into()});
        d.add(Shape::Rectangle{x:1.,y:1.,w:10.,h:10.},Style::default());
        let report=analyze(&d,PlotterOptions::default()).unwrap();
        assert_eq!(report.skipped_unstroked,1);
        assert_eq!(report.path_count,1);
        assert!(analyze(&d,PlotterOptions{scope:Scope::AllVectorOutlines,
            ..PlotterOptions::default()}).is_err());
    }
    #[test] fn locked_layers_are_visible_but_hidden_layers_are_not(){
        let mut d=doc();
        d.add(Shape::Rectangle{x:2.,y:2.,w:10.,h:10.},Style::default());
        let hidden=d.add_layer("hidden".into());
        d.add_to_layer(Shape::Rectangle{x:0.,y:0.,w:20.,h:20.},
            Style::default(),hidden);
        d.layers.iter_mut().find(|l|l.id==hidden).unwrap().visible=false;
        d.layers[0].locked=true;
        assert_eq!(analyze(&d,PlotterOptions::default()).unwrap().path_count,1);
    }
    #[test] fn refuses_out_of_bounds_and_masked_jobs(){
        let mut d=doc();
        let a=d.add(Shape::Rectangle{x:-2.,y:10.,w:20.,h:10.},Style::default());
        assert!(hpgl_bytes(&d,PlotterOptions::default()).unwrap_err().contains("leaves"));
        d.get_mut(a).unwrap().shape=Shape::Rectangle{x:2.,y:10.,w:20.,h:10.};
        let b=d.add(Shape::Rectangle{x:3.,y:10.,w:15.,h:10.},Style::default());
        d.set_clip(a,b).unwrap();
        assert!(hpgl_bytes(&d,PlotterOptions::default()).unwrap_err().contains("Clipping"));
    }
    #[test] fn collinear_but_overshooting_cubic_is_not_silently_shortcut(){
        let mut d=doc();
        d.add(Shape::Bezier{start:Point{x:20.,y:30.},segments:vec![
            CubicSegment{control1:Point{x:300.,y:30.},
                control2:Point{x:-200.,y:30.},end:Point{x:80.,y:30.}}
        ],closed:false},Style::default());
        let err=hpgl_bytes(&d,PlotterOptions::default()).unwrap_err();
        assert!(err.contains("leaves"),"unexpected error: {err}");
    }
    #[test] fn invalid_settings_rejected_and_no_output_written(){
        let mut d=doc();
        d.add(Shape::Rectangle{x:0.,y:0.,w:10.,h:10.},Style::default());
        let mut options=PlotterOptions::default();
        options.units_per_mm=0;
        assert!(hpgl_bytes(&d,options).is_err());
        let temp=tempfile::tempdir().unwrap();
        let destination=temp.path().join("test.plt");
        assert!(export_hpgl(&d,&destination,options).is_err());
        assert!(!destination.exists());
        export_hpgl(&d,&destination,PlotterOptions::default()).unwrap();
        assert!(std::fs::read(&destination).unwrap().starts_with(b"IN;SP1;PA;"));
    }
}
