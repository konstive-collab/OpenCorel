use opencorel_core::{Document,Shape,Style};
use std::{env,process::ExitCode,time::Instant};
use opencorel_core::Point;
use opencorel_formats::{open_any,save_document};

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
        [_,op,path] if op=="inspect" => open_any(path).map(|r|{
            println!("{}: {} x {} mm; {} objects",
                r.document.name,r.document.page.width_mm,r.document.page.height_mm,r.document.objects.len());
            println!("Import quality: {:?}. {}",r.quality,r.warning);
        }),
        [_,op,src,dst] if op=="convert" => open_any(src).and_then(|r|{
            if !r.warning.is_empty(){eprintln!("NOTE: {}",r.warning);}
            save_document(&r.document,dst)
        }),
        [_,op,src,dst] if op=="import-svg" => open_any(src)
            .and_then(|r|save_document(&r.document,dst)),
        [_,op,src,dst] if op=="export-pdf" => open_any(src)
            .and_then(|r|save_document(&r.document,dst)),
        [_,op,src,dst] if op=="export-svg" => open_any(src)
            .and_then(|r|save_document(&r.document,dst)),
        _=>{eprintln!("Usage:\n  opencorel-cli benchmark COUNT\n  opencorel-cli new FILE.kdr\n  opencorel-cli inspect FILE.kdr\n  opencorel-cli convert INPUT OUTPUT\n  opencorel-cli export-svg FILE.kdr FILE.svg\n  opencorel-cli export-pdf FILE.kdr FILE.pdf\n  opencorel-cli import-svg FILE.svg FILE.kdr");
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
