use opencorel_core::{
    appearance::{Appearance,Gradient,GradientKind,GradientStop,StrokeCap,StrokeJoin},
    Document,Shape,Style,
};
fn appearance()->Appearance{
    Appearance{
        opacity:0.6,
        gradient:Some(Gradient{
            kind:GradientKind::Linear,angle_degrees:0.0,stops:vec![
                GradientStop{offset:0.0,color:"#FF0000".into()},
                GradientStop{offset:1.0,color:"#0000FF".into()},
            ],
        }),
        dash_array:vec![2.0,1.0],
        cap:StrokeCap::Round,join:StrokeJoin::Bevel,
    }
}
#[test]
fn appearance_kdr_roundtrip_and_legacy_default(){
    let mut d=Document::default();
    let id=d.add(Shape::Rectangle{x:0.0,y:0.0,w:100.0,h:50.0},Style::default());
    d.set_appearance(id,appearance()).unwrap();
    d.validate().unwrap();
    let tmp=tempfile::tempdir().unwrap();
    let file=tmp.path().join("appearance.kdr");
    opencorel_core::kdr::save_kdr(&d,&file).unwrap();
    assert_eq!(opencorel_core::kdr::read_kdr(&file).unwrap(),d);
    let json=serde_json::to_string(&d).unwrap();
    let mut legacy:serde_json::Value=serde_json::from_str(&json).unwrap();
    legacy.as_object_mut().unwrap().remove("appearances");
    let opened:Document=serde_json::from_value(legacy).unwrap();
    opened.validate().unwrap();
    assert!(opened.appearances.is_empty());
}
#[test]
fn appearance_exports_valid_svg_with_gradient_and_stroke_properties(){
    let mut d=Document::default();
    let id=d.add(Shape::Rectangle{x:0.0,y:0.0,w:100.0,h:50.0},Style::default());
    d.set_appearance(id,appearance()).unwrap();
    let svg=d.to_svg();
    for expected in ["<linearGradient","appearance-1","stop-color=\"#FF0000\"",
        "fill=\"url(#appearance-1)\"","opacity=\"0.6\"",
        "stroke-linecap=\"round\"","stroke-linejoin=\"bevel\"",
        "stroke-dasharray=\"2,1\""]{
        assert!(svg.contains(expected),"missing {expected}");
    }
}
#[test]
fn invalid_and_orphan_appearances_are_rejected(){
    let mut d=Document::default();
    let id=d.add(Shape::Rectangle{x:0.0,y:0.0,w:10.0,h:10.0},Style::default());
    let original=d.clone();
    let mut invalid=appearance();
    invalid.opacity=f32::NAN;
    assert!(d.set_appearance(id,invalid).is_err());
    assert_eq!(d,original);
    let mut invalid=appearance();
    invalid.gradient.as_mut().unwrap().stops[1].offset=-1.0;
    assert!(d.set_appearance(id,invalid).is_err());
    assert_eq!(d,original);
    d.appearances.insert(777,appearance());
    assert!(d.validate().is_err());
}
#[test]
fn deletion_and_duplication_maintain_appearance_links(){
    let mut d=Document::default();
    let id=d.add(Shape::Rectangle{x:0.0,y:0.0,w:10.0,h:10.0},Style::default());
    d.set_appearance(id,appearance()).unwrap();
    let original=d.objects[0].clone();
    let new_id=d.copy_object(&original,1,50.0,0.0);
    assert_eq!(d.appearances.get(&id),d.appearances.get(&new_id));
    assert!(d.remove(id));
    assert!(!d.appearances.contains_key(&id));
    assert!(d.appearances.contains_key(&new_id));
    d.validate().unwrap();
}
#[test]
fn hidden_and_locked_layers_cannot_modify_appearance(){
    let mut d=Document::default();
    let layer=d.add_layer("Locked".into());
    let id=d.add_to_layer(Shape::Rectangle{x:0.0,y:0.0,w:10.0,h:10.0},Style::default(),layer);
    d.layers.iter_mut().find(|l|l.id==layer).unwrap().locked=true;
    assert!(d.set_appearance(id,appearance()).is_err());
    assert!(d.appearances.is_empty());
}
