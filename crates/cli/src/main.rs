use opencorel_core::{Document,Shape,Style};
use std::{env,process::ExitCode};

fn main()->ExitCode{
    let args:Vec<_>=env::args().collect();
    let outcome=match args.as_slice(){
        [_,op,path] if op=="new" => {
            let mut d=Document::default();
            d.add(Shape::Text{x:40.0,y:40.0,value:"OpenCorel".into(),size_mm:30.0},
                Style::default());
            d.write(path)
        }
        [_,op,path] if op=="inspect" => Document::read(path).map(|d|{
            println!("{}: {} x {} mm; {} objects",d.name,d.page.width_mm,d.page.height_mm,d.objects.len());
        }),
        [_,op,src,dst] if op=="import-svg" => std::fs::read_to_string(src)
            .map_err(|e|e.to_string())
            .and_then(|svg|opencorel_core::svg_import::parse_svg(&svg))
            .and_then(|doc|doc.write(dst)),
        [_,op,src,dst] if op=="export-pdf" => Document::read(src)
            .and_then(|d|opencorel_export::export_pdf(&d,dst)),
        [_,op,src,dst] if op=="export-svg" => Document::read(src)
            .and_then(|d|d.export_svg(dst).map_err(|e|e.to_string())),
        _=>{eprintln!("Usage:\n  opencorel-cli new FILE.ocd\n  opencorel-cli inspect FILE.ocd\n  opencorel-cli export-svg FILE.ocd FILE.svg\n  opencorel-cli export-pdf FILE.ocd FILE.pdf\n  opencorel-cli import-svg FILE.svg FILE.ocd");
            return ExitCode::FAILURE;}
    };
    match outcome{
        Ok(())=>ExitCode::SUCCESS,
        Err(e)=>{eprintln!("Error: {e}");ExitCode::FAILURE}
    }
}
