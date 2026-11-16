use opencorel_core::{Bounds,Document,Point,Shape,Style};

#[test]
fn measured_bounds_override_approximate_text_geometry(){
    let mut doc=Document::default();
    let id=doc.add(Shape::Text{
        x:10.0,y:20.0,value:"Ж".into(),size_mm:10.0,
    },Style::default());
    let inside_measured=Point{x:51.0,y:51.0};
    assert_eq!(doc.hit_test(inside_measured,0.0),None);
    assert_eq!(doc.hit_test_with_bounds(inside_measured,0.0,|o|{
        if o.id==id{Bounds{x:40.0,y:40.0,w:20.0,h:20.0}}
        else{o.bounds()}
    }),Some(id));
    assert_eq!(doc.hit_test_with_bounds(Point{x:12.0,y:22.0},0.0,|o|{
        if o.id==id{Bounds{x:40.0,y:40.0,w:20.0,h:20.0}}
        else{o.bounds()}
    }),None);
}

#[test]
fn measured_bounds_respect_locked_hidden_and_invalid_queries(){
    let mut doc=Document::default();
    let id=doc.add(Shape::Rectangle{x:5.0,y:5.0,w:5.0,h:5.0},
        Style::default());
    let query=Point{x:7.0,y:7.0};
    assert_eq!(doc.hit_test_with_bounds(query,0.0,|o|o.bounds()),Some(id));
    assert_eq!(doc.hit_test_with_bounds(query,-1.0,|o|o.bounds()),None);
    assert_eq!(doc.hit_test_with_bounds(
        Point{x:f32::NAN,y:7.0},0.0,|o|o.bounds()),None);
    doc.layers[0].locked=true;
    assert_eq!(doc.hit_test_with_bounds(query,0.0,|o|o.bounds()),None);
    doc.layers[0].locked=false;
    doc.layers[0].visible=false;
    assert_eq!(doc.hit_test_with_bounds(query,0.0,|o|o.bounds()),None);
}

#[test]
fn topmost_object_wins_with_custom_bounds(){
    let mut doc=Document::default();
    let _bottom=doc.add(Shape::Rectangle{x:0.0,y:0.0,w:30.0,h:30.0},
        Style::default());
    let top=doc.add(Shape::Rectangle{x:0.0,y:0.0,w:30.0,h:30.0},
        Style::default());
    assert_eq!(doc.hit_test_with_bounds(Point{x:5.0,y:5.0},0.0,|o|o.bounds()),
        Some(top));
}
