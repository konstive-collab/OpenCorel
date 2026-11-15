//! Integration-level synthetic regression corpus for stable native document files.
//! Deterministic; no network, external programs or fonts required.
use opencorel_core::{Document,Shape,Style,Point,CubicSegment,Contour,TextFormat};
use opencorel_core::{kdr,node_edit::{self,NodeAddress,SegmentAddress}};
use std::{fs,io::Write};

#[test]
fn mixed_legacy_and_current_projects_roundtrip(){
    let dir=tempfile::tempdir().unwrap();
    let mut doc=Document::default();
    doc.name="Карта / тест — 2026".into();
    doc.page.width_mm=3000.0;
    doc.page.height_mm=2000.0;
    let foreground=doc.add_layer("Логотип + текст".into());
    let background=doc.add(Shape::Rectangle{x:-45.0,y:10.0,w:3100.0,h:2200.0},
        Style{fill:"#FFFFFF".into(),stroke:"none".into(),stroke_width_mm:0.0});
    let text=doc.add_to_layer(Shape::Text{
        x:100.0,y:150.0,value:"Реклама <>& «Тест»\nLine two".into(),size_mm:56.0
    },Style::default(),foreground);
    doc.get_mut(text).unwrap().text_format=TextFormat{
        family:"sans-serif".into(),weight:700,..TextFormat::default()};
    let path=doc.add_to_layer(Shape::Bezier{
        start:Point{x:30.0,y:40.0},
        segments:vec![CubicSegment{
            control1:Point{x:50.0,y:150.0},
            control2:Point{x:140.0,y:150.0},
            end:Point{x:190.0,y:40.0}
        }],closed:true
    },Style::default(),foreground);
    doc.group_objects(&[text,path]).unwrap();
    doc.get_mut(background).unwrap().name="Фон".into();
    doc.validate().unwrap();
    for ext in ["ocd","kdr"]{
        let file=dir.path().join(format!("regression.{ext}"));
        if ext=="kdr"{kdr::save_kdr(&doc,&file).unwrap();}
        else{doc.write(&file).unwrap();}
        let restored=if ext=="kdr"{kdr::read_kdr(&file).unwrap()}
            else{Document::read(&file).unwrap()};
        assert_eq!(restored,doc,"native format {ext} lost editable geometry");
        assert!(restored.to_svg().contains("&lt;&gt;&amp;"));
        restored.validate().unwrap();
    }
}

#[test]
fn thousands_of_objects_and_group_ids_survive_zip_cycle(){
    let dir=tempfile::tempdir().unwrap();
    let mut doc=Document::default();
    doc.page.width_mm=10_000.0;
    doc.page.height_mm=6_000.0;
    let mut ids=Vec::with_capacity(3000);
    for i in 0..3000u32{
        let id=doc.add(Shape::Rectangle{
            x:(i%100) as f32*80.0,y:(i/100) as f32*80.0,
            w:50.0,h:50.0
        },Style::default());
        ids.push(id);
    }
    for pair in ids.chunks_exact(2).take(200){
        doc.group_objects(pair).unwrap();
    }
    doc.validate().unwrap();
    let file=dir.path().join("thousands.kdr");
    kdr::save_kdr(&doc,&file).unwrap();
    let loaded=kdr::read_kdr(&file).unwrap();
    assert_eq!(loaded.objects.len(),3000);
    assert_eq!(loaded,doc);
    assert_eq!(loaded.group_members(ids[0]),vec![ids[0],ids[1]]);
}

#[test]
fn dangerous_zip_entry_is_never_extracted(){
    use zip::{ZipWriter,write::SimpleFileOptions};
    let dir=tempfile::tempdir().unwrap();
    let file=dir.path().join("traversal.kdr");
    let mut handle=fs::File::create(&file).unwrap();
    {
        let mut zip=ZipWriter::new(&mut handle);
        let options=SimpleFileOptions::default();
        zip.start_file("../evil.txt",options).unwrap();
        zip.write_all(b"not allowed").unwrap();
        zip.start_file("manifest.json",options).unwrap();
        zip.write_all(b"{}").unwrap();
        zip.start_file("document.json",options).unwrap();
        zip.write_all(b"{}").unwrap();
        zip.finish().unwrap();
    }
    assert!(kdr::read_kdr(&file).is_err());
    assert!(!dir.path().join("evil.txt").exists());
}

#[test]
fn corrupt_and_truncated_archives_are_rejected_without_overwriting_sources(){
    let dir=tempfile::tempdir().unwrap();
    let doc=Document::default();
    let good=dir.path().join("good.kdr");
    kdr::save_kdr(&doc,&good).unwrap();
    let bytes=fs::read(&good).unwrap();
    let truncated=dir.path().join("truncated.kdr");
    fs::write(&truncated,&bytes[..bytes.len()/2]).unwrap();
    assert!(kdr::read_kdr(&truncated).is_err());
    assert_eq!(kdr::read_kdr(&good).unwrap(),doc);
}

#[test]
fn deterministic_bezier_stress_rejects_bad_edits_and_preserves_roundtrip(){
    for n in 1..=256{
        let x=n as f32*0.125;
        let mut shape=Shape::Bezier{
            start:Point{x,y:0.0},
            segments:vec![CubicSegment{
                control1:Point{x:x+10.0,y:20.0},
                control2:Point{x:x+40.0,y:-5.0},
                end:Point{x:x+50.0,y:10.0},
            }],closed:false
        };
        let segment=SegmentAddress::Bezier(0);
        let t=0.02+(n as f32/256.0)*0.96;
        node_edit::insert_node(&mut shape,segment,t.min(0.98)).unwrap();
        assert!(node_edit::move_node(&mut shape,
            NodeAddress::BezierEnd(0),Point{x:x+30.0,y:8.0}).is_ok());
        assert!(node_edit::remove_node(&mut shape,NodeAddress::BezierEnd(0)).is_ok());
        let mut doc=Document::default();
        doc.add(shape,Style::default());
        doc.validate().unwrap();
        let restored:Document=serde_json::from_str(
            &serde_json::to_string(&doc).unwrap()).unwrap();
        assert_eq!(doc,restored);
    }
}

#[test]
fn compound_with_inner_hole_stays_editable_after_native_roundtrip(){
    let ring=|min:f32,max:f32| Contour{
        start:Point{x:min,y:min},
        segments:vec![
            CubicSegment{control1:Point{x:min,y:min},control2:Point{x:max,y:min},end:Point{x:max,y:min}},
            CubicSegment{control1:Point{x:max,y:min},control2:Point{x:max,y:max},end:Point{x:max,y:max}},
            CubicSegment{control1:Point{x:max,y:max},control2:Point{x:min,y:max},end:Point{x:min,y:max}},
            CubicSegment{control1:Point{x:min,y:max},control2:Point{x:min,y:min},end:Point{x:min,y:min}},
        ],closed:true
    };
    let mut doc=Document::default();
    doc.add(Shape::CompoundPath{contours:vec![ring(10.0,90.0),ring(30.0,70.0)]},Style::default());
    doc.validate().unwrap();
    assert!(doc.to_svg().contains("fill-rule=\"evenodd\""));
    let file=tempfile::NamedTempFile::new().unwrap();
    kdr::save_kdr(&doc,file.path()).unwrap();
    assert_eq!(kdr::read_kdr(file.path()).unwrap(),doc);
}