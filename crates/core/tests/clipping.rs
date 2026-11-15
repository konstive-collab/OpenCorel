use opencorel_core::{Document,Shape,Style,ClipLink};

fn rect(x:f32,y:f32,w:f32,h:f32)->Shape{
    Shape::Rectangle{x,y,w,h}
}
#[test]
fn clip_persists_in_old_schema_and_kdr(){
    let mut doc=Document::default();
    let source=doc.add(rect(0.0,0.0,30.0,30.0),Style::default());
    let target=doc.add(rect(5.0,5.0,40.0,40.0),Style::default());
    let original=doc.objects[1].shape.clone();
    doc.set_clip(source,target).unwrap();
    assert_eq!(doc.objects[1].shape,original);
    assert!(doc.to_svg().contains("clip-path=\"url(#clip-target-2)\""));
    assert!(doc.to_svg().contains("<clipPath id=\"clip-target-2\""));
    assert!(!doc.to_svg().contains("<g id=\"object-1\">"));
    let json=serde_json::to_string(&doc).unwrap();
    let restored:Document=serde_json::from_str(&json).unwrap();
    assert_eq!(restored,doc);
    restored.validate().unwrap();
    let tmp=tempfile::tempdir().unwrap();
    let path=tmp.path().join("masked.kdr");
    opencorel_core::kdr::save_kdr(&doc,&path).unwrap();
    assert_eq!(opencorel_core::kdr::read_kdr(&path).unwrap(),doc);
    let old_json=serde_json::to_string(&doc).unwrap();
    let mut old:serde_json::Value=serde_json::from_str(&old_json).unwrap();
    old.as_object_mut().unwrap().remove("clip_links");
    let legacy:Document=serde_json::from_value(old).unwrap();
    assert!(legacy.clip_links.is_empty());
}
#[test]
fn invalid_and_locked_clips_are_atomic(){
    let mut doc=Document::default();
    let source=doc.add(rect(0.0,0.0,30.0,30.0),Style::default());
    let target=doc.add(rect(5.0,5.0,40.0,40.0),Style::default());
    assert!(doc.set_clip(target,target).is_err());
    assert!(doc.clip_links.is_empty());
    let open=doc.add(Shape::Polyline{points:vec![
        opencorel_core::Point{x:0.0,y:0.0},
        opencorel_core::Point{x:1.0,y:1.0}
    ],closed:false},Style::default());
    assert!(doc.set_clip(open,target).is_err());
    assert!(doc.clip_links.is_empty());
    doc.layers[0].locked=true;
    assert!(doc.set_clip(source,target).is_err());
    doc.layers[0].locked=false;
    doc.set_clip(source,target).unwrap();
    assert!(doc.set_clip(target,source).is_err()); // cycles disallowed
    assert_eq!(doc.clip_links,vec![ClipLink{source_id:source,target_id:target}]);
    assert!(doc.remove(source));
    assert!(doc.clip_links.is_empty());
    doc.validate().unwrap();
}
#[test]
fn corrupted_links_are_rejected(){
    let mut doc=Document::default();
    let source=doc.add(rect(0.0,0.0,30.0,30.0),Style::default());
    let target=doc.add(rect(5.0,5.0,40.0,40.0),Style::default());
    doc.clip_links.push(ClipLink{source_id:source,target_id:target});
    doc.clip_links.push(ClipLink{source_id:source,target_id:target});
    assert!(doc.validate().is_err());
    doc.clip_links.pop();
    doc.clip_links.push(ClipLink{source_id:target,target_id:source});
    assert!(doc.validate().is_err());
}
#[test]
fn raster_can_be_clipped_but_not_used_as_source(){
    let mut doc=Document::default();
    let mask=doc.add(Shape::Ellipse{x:0.0,y:0.0,w:100.0,h:100.0},Style::default());
    let bitmap=doc.add(Shape::Raster{
        x:0.0,y:0.0,w:50.0,h:50.0,pixel_width:1,pixel_height:1,
        mime:"image/png".into(),
        data_base64:"iVBORw0KGgo=".into(),
    },Style::default());
    assert!(doc.set_clip(bitmap,mask).is_err());
    doc.set_clip(mask,bitmap).unwrap();
    assert!(doc.to_svg().contains("clip-path=\"url(#clip-target-2)\""));
}

#[test]
fn masked_hit_test_obeys_visible_region(){
    let mut doc=Document::default();
    let mask=doc.add(rect(10.0,10.0,20.0,20.0),Style::default());
    let target=doc.add(rect(0.0,0.0,100.0,100.0),Style::default());
    doc.set_clip(mask,target).unwrap();
    assert_eq!(doc.hit_test(opencorel_core::Point{x:20.0,y:20.0},0.0),Some(target));
    assert_eq!(doc.hit_test(opencorel_core::Point{x:70.0,y:70.0},0.0),None);
    assert_eq!(doc.hit_test(opencorel_core::Point{x:5.0,y:5.0},0.0),None);
}
#[test]
fn nested_compound_hole_not_clickable(){
    use opencorel_core::{Point,CubicSegment,Contour};
    fn p(x:f32,y:f32)->Point{Point{x,y}}
    fn square(x:f32,y:f32,w:f32)->Contour{
        let start=p(x,y);
        let pts=[p(x+w,y),p(x+w,y+w),p(x,y+w),start];
        let mut prev=start;
        let segments=pts.iter().map(|end|{
            let seg=CubicSegment{control1:prev,control2:*end,end:*end};
            prev=*end;
            seg
        }).collect();
        Contour{start,segments,closed:true}
    }
    let shape=Shape::CompoundPath{contours:vec![square(0.0,0.0,100.0),square(25.0,25.0,50.0)]};
    let mut doc=Document::default();
    let mask=doc.add(shape,Style::default());
    let target=doc.add(rect(0.0,0.0,100.0,100.0),Style::default());
    doc.set_clip(mask,target).unwrap();
    assert_eq!(doc.hit_test(p(10.0,10.0),0.0),Some(target));
    assert_eq!(doc.hit_test(p(50.0,50.0),0.0),None);
}
