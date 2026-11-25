use opencorel_core::{Document,Shape,Style,Page};
use std::{env,process::ExitCode,time::Instant};
use opencorel_core::Point;
use opencorel_formats::{open_any,save_document};
use opencorel_typography::TextOutliner;

fn main()->ExitCode{
    let args:Vec<_>=env::args().collect();
    let outcome=match args.as_slice(){
        [_,op,src,dst,page_id,object_id,settings] if op=="raster-adjust"||op=="raster-batch" => raster_adjust(src,dst,page_id,object_id,settings),
        [_,op,src,dst,page_id,object_id,kind] if op=="raster-extract" => raster_extract(src,dst,page_id,object_id,kind),
        [_,op,path,phrase] if op=="find-objects" =>open_any(path).and_then(|r|{
            let hits=r.document.find_objects(&opencorel_core::object_search::ObjectQuery{
                phrase:phrase.to_string(),..Default::default()
            })?;
            for hit in &hits{
                println!("PAGE {} OBJECT {} {} [{}] {}",
                    hit.page_id,hit.object_id,hit.kind,hit.layer_name,hit.name);
            }
            println!("Found {} objects across pages",hits.len());
            Ok(())
        }),
        [_,op,src,dst,from,to] if op=="recolor-rgb" =>{
            if src==dst{Err("Input and output files must differ".into())}
            else{open_any(src).and_then(|r|{
                let mut doc=r.document;
                let report=doc.recolor_rgb(from,to,
                    opencorel_core::recolor::ColorChannels::default(),true)?;
                if report.total()==0{return Err("No matching RGB slots".into());}
                save_document(&doc,dst)?;
                println!("Recolored {} RGB slots on {} pages",
                    report.total(),report.pages);
                Ok(())
            })}
        },
        [_,op,src,dst,payload,size] if op=="qr-code" =>{
            if src==dst{Err("Input and output files must differ".into())}
            else{size.parse::<f32>()
                .map_err(|_|"Invalid QR size in millimetres".to_string())
                .and_then(|size_mm|open_any(src).and_then(|r|{
                    let mut doc=r.document;
                    let spec=opencorel_core::qr_vector::QrSpec{
                        payload:payload.to_string(),size_mm,
                        origin:Point{
                            x:(doc.page.width_mm-size_mm)*0.5,
                            y:(doc.page.height_mm-size_mm)*0.5
                        },
                        ..Default::default()
                    };
                    let ids=doc.create_qr(&spec,1)?;
                    save_document(&doc,dst)?;
                    println!("Created native QR compound contours #{}, background #{}",
                        ids[1],ids[0]);
                    Ok(())
                }))}
        },
        [_,op,path] if op=="inventory" =>open_any(path).map(|r|{
            let inv=r.document.inventory();
            println!("PAGES {}",inv.pages);
            println!("OBJECTS {} VECTORS {} TEXT {} RASTER {} EMBEDDED {}",
                inv.objects,inv.editable_vectors,inv.editable_text,
                inv.raster_images,inv.embedded_svg);
            println!("GRADIENTS {} GROUP MEMBERS {} CLIP TARGETS {}",
                inv.gradients,inv.grouped_members,inv.clipped_targets);
            println!("ESTIMATED IMAGE BYTES {}",inv.estimated_image_bytes);
            for (font,count) in inv.font_families{
                println!("FONT {} {}",font,count);
            }
            for (color,count) in inv.solid_colors{
                println!("COLOR {} {}",color,count);
            }
        }),
        [_,op,path,needle] if op=="find-text" =>open_any(path).and_then(|r|{
            let results=r.document.find_text(needle,false,true)?;
            for hit in &results{
                println!("PAGE {} OBJECT {} HITS {} {}",
                    hit.page_id,hit.object_id,hit.occurrences,
                    hit.preview.replace('\n'," ↵ "));
            }
            println!("Total: {} hits in {} objects",
                results.iter().map(|hit|hit.occurrences).sum::<usize>(),
                results.len());
            Ok(())
        }),
        [_,op,src,dst,needle,replacement] if op=="replace-text" =>{
            if src==dst{Err("Source and destination must differ".into())}
            else{open_any(src).and_then(|r|{
                let mut doc=r.document;
                let count=doc.replace_text(needle,replacement,false,true)?;
                if count==0{return Err("No matching text to replace".into());}
                save_document(&doc,dst)?;
                println!("Replaced {count} Unicode-insensitive matches across all pages");
                Ok(())
            })}
        },
        [_,op,src,dst,object_id,preset] if op=="gradient-preset" =>{
            if src==dst{Err("Source and destination must differ".into())}
            else{open_any(src).and_then(|r|{
                use opencorel_core::appearance::GradientPreset;
                let id=object_id.parse::<u64>()
                    .map_err(|_|"Invalid object ID".to_string())?;
                let gradient=match preset.to_ascii_lowercase().as_str(){
                    "sunrise"=>Some(GradientPreset::Sunrise),
                    "ocean"=>Some(GradientPreset::Ocean),
                    "forest"=>Some(GradientPreset::Forest),
                    "sunset"=>Some(GradientPreset::Sunset),
                    "steel"=>Some(GradientPreset::Steel),
                    "candy"=>Some(GradientPreset::Candy),
                    "monochrome"=>Some(GradientPreset::Monochrome),
                    _=>None,
                }.ok_or("Unknown RGB gradient preset")?;
                let mut doc=r.document;
                let mut appearance=doc.appearances.get(&id)
                    .cloned().unwrap_or_default();
                appearance.gradient=Some(gradient.create());
                doc.set_appearance(id,appearance)?;
                save_document(&doc,dst)?;
                println!("Applied RGB preset {preset} to object #{id}");
                Ok(())
            })}
        },
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
        [_,op,path] if op=="pages" => open_any(path).map(|r|{
            for (i,page) in r.document.pages().iter().enumerate(){
                let isolated=r.document.page_document(page.id).expect("validated page");
                println!("{} | id={} | {} | {:.0}x{:.0} mm | {} objects",
                    i+1,page.id,page.name,isolated.page.width_mm,
                    isolated.page.height_mm,isolated.objects.len());
            }
        }),
        [_,op,src,dst,w,h] if op=="add-page" =>{
            if src==dst{Err("Use a different output path".into())}
            else{
                let parsed=w.parse::<f32>().map_err(|_|"Invalid width".to_string())
                    .and_then(|width|h.parse::<f32>()
                        .map(|height|(width,height)).map_err(|_|"Invalid height".to_string()));
                parsed.and_then(|(width,height)|open_any(src).and_then(|r|{
                    let mut d=r.document;
                    d.add_page(Page{width_mm:width,height_mm:height})?;
                    save_document(&d,dst)
                }))
            }
        },
        [_,op,src,dst] if op=="duplicate-page" =>{
            if src==dst{Err("Use a different output path".into())}
            else{open_any(src).and_then(|r|{
                let mut d=r.document;d.duplicate_page()?;save_document(&d,dst)
            })}
        },
        [_,op,src,dst] if op=="export-trim-svg"||op=="export-trim-pdf" =>{
            if src==dst{Err("Input and output must differ".into())}
            else{open_any(src).and_then(|r|{
                let path=std::path::Path::new(dst);
                let ext=if op=="export-trim-svg"{"svg"}else{"pdf"};
                if path.extension().and_then(|e|e.to_str())!=Some(ext){
                    return Err(format!("Destination must have a .{ext} extension"));
                }
                opencorel_export::trim_marks::export_trim(&r.document,path,Default::default())
            })}
        },
        [_,op,src,path] if op=="export-pdf-book" =>{
            if src==path{Err("Choose a different PDF destination".into())}
            else{open_any(src).and_then(|r|
                opencorel_export::pdf_book::export_pdf_book(&r.document,path,&[]))}
        },
        [_,op,src,folder] if op=="batch-svg" || op=="batch-pdf" =>{
            let kind=if op=="batch-svg"{
                opencorel_export::batch::PageExportFormat::Svg
            }else{opencorel_export::batch::PageExportFormat::Pdf};
            open_any(src).and_then(|r|{
                let paths=opencorel_export::batch::export_pages(
                    &r.document,std::path::Path::new(folder),kind,&[],96)?;
                println!("Exported {} pages",paths.len());Ok(())
            })
        },
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
        [_,op,src] if op=="dxf-preflight" =>open_any(src).and_then(|r|{
            let report=opencorel_export::dxf::analyze(&r.document,Default::default())?;
            println!("DXF preflight: {} polylines, {} vertices, {} layers, {:.1}mm total geometry; {} unstroked objects skipped",
                report.path_count,report.vertex_count,report.layer_count,
                report.cut_length_mm,report.skipped_unstroked);
            Ok(())
        }),
        [_,op,src,dst] if op=="export-dxf" => {
            if src==dst{Err("Input and DXF output files must be different".into())}
            else{open_any(src).and_then(|r|opencorel_export::dxf::export_dxf(
                &r.document,dst,Default::default()))}
        },
        [_,op,src,dst] if op=="export-plt" => {
            if src==dst{Err("Input and plotter output files must be different".into())}
            else{open_any(src).and_then(|r|opencorel_export::hpgl::export_hpgl(
                &r.document,dst,Default::default()))}
        },
        [_,op,src,dst] if op=="export-pdf" => open_any(src)
            .and_then(|r|save_document(&r.document,dst)),
        [_,op,src,dst] if op=="export-svg" => open_any(src)
            .and_then(|r|save_document(&r.document,dst)),
        _=>{eprintln!("Usage:\n  opencorel-cli raster-adjust IN.kdr OUT.kdr PAGE_ID OBJECT_ID SETTINGS.json\n  opencorel-cli raster-batch IN.kdr OUT.kdr PAGE_ID ID1,ID2 SETTINGS.json\n  opencorel-cli raster-extract IN.kdr OUT.png PAGE_ID OBJECT_ID original|adjusted\n  opencorel-cli benchmark COUNT\n  opencorel-cli new FILE.kdr\n  opencorel-cli inventory FILE.kdr\n  opencorel-cli qr-code IN.kdr OUT.kdr URL_OR_TEXT SIZE_MM\n  opencorel-cli recolor-rgb IN.kdr OUT.kdr #RRGGBB #RRGGBB\n  opencorel-cli find-objects FILE.kdr QUERY\n  opencorel-cli find-text FILE.kdr QUERY\n  opencorel-cli replace-text IN.kdr OUT.kdr QUERY REPLACEMENT\n  opencorel-cli gradient-preset IN.kdr OUT.kdr OBJECT_ID sunrise|ocean|forest|sunset|steel|candy|monochrome\n  opencorel-cli inspect FILE.kdr\n  opencorel-cli pages FILE.kdr\n  opencorel-cli add-page IN.kdr OUT.kdr WIDTH_MM HEIGHT_MM\n  opencorel-cli duplicate-page IN.kdr OUT.kdr\n  opencorel-cli batch-svg FILE.kdr DIRECTORY\n  opencorel-cli batch-pdf FILE.kdr DIRECTORY\n  opencorel-cli export-pdf-book FILE.kdr DOCUMENT.pdf\n  opencorel-cli export-trim-pdf FILE.kdr TRIM.pdf\n  opencorel-cli export-trim-svg FILE.kdr TRIM.svg\n  opencorel-cli preflight FILE.kdr\n  opencorel-cli convert INPUT OUTPUT\n  opencorel-cli outline-text INPUT.kdr OUTPUT.kdr\n  opencorel-cli export-bitmap FILE.kdr FILE.png DPI\n  opencorel-cli export-svg FILE.kdr FILE.svg\n  opencorel-cli export-pdf FILE.kdr FILE.pdf\n  opencorel-cli plotter-preflight FILE.kdr\n  opencorel-cli export-plt FILE.kdr FILE.plt\n  opencorel-cli dxf-preflight FILE.kdr\n  opencorel-cli export-dxf FILE.kdr FILE.dxf\n  opencorel-cli import-svg FILE.svg FILE.kdr");
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

fn raster_adjust(src:&str,dst:&str,page:&str,object:&str,settings:&str)->Result<(),String>{
    if src==dst||std::path::Path::new(dst).exists(){return Err("Choose a new output file".into());}
    if !matches!(opencorel_formats::extension(std::path::Path::new(dst)).as_str(),"kdr"|"ocd"){
        return Err("Raster adjustment output must be KDR or OCD to preserve the original source".into());
    }
    let page_id=page.parse::<u64>().map_err(|_|"Invalid page ID")?;
    let ids=object.split(',').map(|v|v.parse::<u64>().map_err(|_|"Invalid object ID"))
        .collect::<Result<Vec<_>,_>>()?;
    if std::fs::metadata(settings).map_err(|e|e.to_string())?.len()>16384{
        return Err("Raster settings file exceeds 16 KiB".into());
    }
    let params:opencorel_core::raster_edit::RasterSettings=
        serde_json::from_slice(&std::fs::read(settings).map_err(|e|e.to_string())?)
            .map_err(|e|format!("Invalid raster settings: {e}"))?;
    let mut doc=open_any(src)?.document;
    doc.switch_page(page_id)?;
    doc.apply_raster_batch(&ids,params)?;
    save_document(&doc,dst)?;
    println!("Adjusted {} raster object(s) on page #{page_id}; originals retained.",ids.len());
    Ok(())
}
fn raster_extract(src:&str,dst:&str,page:&str,object:&str,kind:&str)->Result<(),String>{
    if src==dst{return Err("Choose a different output path".into());}
    let mut doc=open_any(src)?.document;
    doc.switch_page(page.parse::<u64>().map_err(|_|"Invalid page ID")?)?;
    let id=object.parse::<u64>().map_err(|_|"Invalid object ID")?;
    let shape=&doc.get(id).ok_or("Object not found")?.shape;
    opencorel_core::raster_edit::extract_raster(shape,kind,std::path::Path::new(dst))?;
    println!("Extracted {kind} raster #{id}.");
    Ok(())
}
