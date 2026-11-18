use opencorel_core::{Document,Shape,Style};
use std::{env,process::ExitCode,time::Instant};
use opencorel_core::Point;
use opencorel_formats::{open_any,save_document};
use opencorel_typography::TextOutliner;

fn main()->ExitCode{
    let args:Vec<_>=env::args().collect();
    let outcome=match args.as_slice(){
        [_,op,n] if op=="benchmark" => benchmark(n),
        [_,op,path] if op=="new" => {
            let mut d=Document::default();
            d.add(Shape::Text{x:40.0,y:40.0,value:"OpenCorel".into(),size_mm:30.0},
                Style::default());
            save_document(&d,path)
        }
        [_,op,path] if op=="preflight" => open_any(path).and_then(|r|{
            let issues=opencorel_core::preflight::check(&r.document,Default::default());
            for issue in &issues{
                println!("{:?} [{}] #{}: {}",issue.severity,issue.code,
                    issue.object_id.map(|v|v.to_string()).unwrap_or_else(||"-".into()),
                    issue.message);
            }
            if issues.iter().any(|i|i.severity==opencorel_core::preflight::Severity::Error){
                Err("The document contains preflight errors".into())
            }else{Ok(())}
        }),
        [_,op,path] if op=="inspect" => open_any(path).map(|r|{
            println!("{}: {} x {} mm; {} objects",
                r.document.name,r.document.page.width_mm,r.document.page.height_mm,r.document.objects.len());
            println!("Import quality: {:?}. {}",r.quality,r.warning);
        }),
        [_,op,src,dst,dpi] if op=="export-bitmap" =>{
            let dpi=dpi.parse::<u32>().map_err(|_|"DPI must be a number".to_string());
            dpi.and_then(|dpi|open_any(src)
                .and_then(|r|opencorel_export::raster_export::export_raster(&r.document,dst,dpi)))
        },
        [_,op,src,dst] if op=="outline-text" => {
            if src==dst {
                Err("Use different input and output paths to preserve original editable text".into())
            }else{
                open_any(src).and_then(|r|{
                    let mut doc=r.document;
                    let mut outliner=TextOutliner::new();
                    let mut replacements=Vec::new();
                    for object in &doc.objects{
                        if matches!(object.shape,Shape::Text{..}){
                            let shape=outliner.convert_object(object)
                                .map_err(|e|format!("Text #{}: {e}",object.id))?;
                            replacements.push((object.id,shape));
                        }
                    }
                    if replacements.is_empty(){
                        return Err("Document contains no editable text to outline".into());
                    }
                    let count=replacements.len();
                    for (id,shape) in replacements{
                        if let Some(object)=doc.get_mut(id){object.shape=shape;}
                    }
                    save_document(&doc,dst)?;
                    println!("Outlined {count} text object(s) to true vector curves; source unchanged.");
                    Ok(())
                })
            }
        },
        [_,op,src,dst] if op=="convert" => open_any(src).and_then(|r|{
            if !r.warning.is_empty(){eprintln!("NOTE: {}",r.warning);}
            save_document(&r.document,dst)
        }),
        [_,op,src,dst] if op=="import-svg" => open_any(src)
            .and_then(|r|save_document(&r.document,dst)),
        [_,op,src] if op=="plotter-preflight" =>open_any(src).and_then(|r|{
            let report=opencorel_export::hpgl::analyze(&r.document,Default::default())?;
            println!("PLT preflight: {} paths, {} vertices, {:.1}mm cut, {:.1}mm travel; {} unstroked objects skipped",
                report.path_count,report.vertex_count,report.cut_length_mm,
                report.travel_length_mm,report.skipped_unstroked);
            Ok(())
        }),
        [_,op,src,dst] if op=="export-plt" => {
            if src==dst{Err("Input and plotter output files must be different".into())}
            else{open_any(src).and_then(|r|opencorel_export::hpgl::export_hpgl(
                &r.document,dst,Default::default()))}
        },
        [_,op,src,dst] if op=="export-pdf" => open_any(src)
            .and_then(|r|save_document(&r.document,dst)),
        [_,op,src,dst] if op=="export-svg" => open_any(src)
            .and_then(|r|save_document(&r.document,dst)),
        _=>{eprintln!("Usage:\n  opencorel-cli benchmark COUNT\n  opencorel-cli new FILE.kdr\n  opencorel-cli inspect FILE.kdr\n  opencorel-cli preflight FILE.kdr\n  opencorel-cli convert INPUT OUTPUT\n  opencorel-cli outline-text INPUT.kdr OUTPUT.kdr\n  opencorel-cli export-bitmap FILE.kdr FILE.png DPI\n  opencorel-cli export-svg FILE.kdr FILE.svg\n  opencorel-cli export-pdf FILE.kdr FILE.pdf\n  opencorel-cli plotter-preflight FILE.kdr\n  opencorel-cli export-plt FILE.kdr FILE.plt\n  opencorel-cli import-svg FILE.svg FILE.kdr");
            return ExitCode::FAILURE;}
    };
    match outcome{
        Ok(())=>ExitCode::SUCCESS,
        Err(e)=>{eprintln!("Error: {e}");ExitCode::FAILURE}
    }
}

fn benchmark(number:&str)->Result<(),String>{
    let count:usize=number.parse().map_err(|_|"COUNT must be an integer".to_string())?;
    if count==0 || count>100_000{return Err("COUNT must be between 1 and 100000".into());}
    let started=Instant::now();
    let mut doc=Document::default();
    doc.page.width_mm=3000.0;
    doc.page.height_mm=2000.0;
    let style=Style::default();
    for i in 0..count {
        let x=(i%100) as f32*25.0;
        let y=((i/100)%75) as f32*25.0;
        doc.add(Shape::Rectangle{x,y,w:20.0,h:20.0},style.clone());
    }
    let build_ms=started.elapsed().as_millis();
    let t=Instant::now();
    doc.validate()?;
    let validate_ms=t.elapsed().as_millis();
    let t=Instant::now();
    let _hit=doc.hit_test(Point{x:500.0,y:500.0},1.0);
    let hit_us=t.elapsed().as_micros();
    let t=Instant::now();
    let svg=doc.to_svg();
    let svg_ms=t.elapsed().as_millis();
    println!("Objects: {count} | build: {build_ms} ms | validate: {validate_ms} ms | hit test: {hit_us} us | SVG: {svg_ms} ms | SVG bytes: {}",svg.len());
    println!("This measures the document engine, not UI frame time or RAM peak.");
    Ok(())
}
