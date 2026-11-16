//! Regression tests for persistent cubic node types, KDR compatibility,
 //! endpoint-remapping and exact direction reversal.
use opencorel_core::{CubicSegment,Document,Point,Shape,Style,Contour};
use opencorel_core::node_edit::{
    self,NodeAddress,SegmentAddress,TangentOperation as Mode,
    NodeModeBinding,
};

fn p(x:f32,y:f32)->Point{Point{x,y}}
fn curve()->Shape{
    Shape::Bezier{start:p(0.,0.),segments:vec![
        CubicSegment{control1:p(3.,5.),control2:p(8.,6.),end:p(10.,10.)},
        CubicSegment{control1:p(13.,14.),control2:p(18.,15.),end:p(30.,10.)},
        CubicSegment{control1:p(40.,10.),control2:p(47.,7.),end:p(50.,0.)},
    ],closed:false}
}
fn sample(a:Point,s:&CubicSegment,t:f32)->Point{
    let u=1.0-t;
    p(u*u*u*a.x+3.0*u*u*t*s.control1.x+3.0*u*t*t*s.control2.x+t*t*t*s.end.x,
      u*u*u*a.y+3.0*u*u*t*s.control1.y+3.0*u*t*t*s.control2.y+t*t*t*s.end.y)
}
fn length(a:Point,b:Point)->f32{(a.x-b.x).hypot(a.y-b.y)}
fn assert_close(a:Point,b:Point){
    assert!((a.x-b.x).abs()<0.0005,"x {} != {}",a.x,b.x);
    assert!((a.y-b.y).abs()<0.0005,"y {} != {}",a.y,b.y);
}
#[test]
fn legacy_document_without_modes_is_readable_and_identical(){
    let mut doc=Document::default();
    let id=doc.add(curve(),Style::default());
    let json=serde_json::to_string(&doc).unwrap();
    assert!(!json.contains("node_modes"));
    let restored:Document=serde_json::from_str(&json).unwrap();
    restored.validate().unwrap();
    assert!(restored.objects.iter().find(|o|o.id==id).unwrap().node_modes.is_empty());
    assert_eq!(restored,doc);
}
#[test]
fn saved_symmetric_mode_couples_handles_after_roundtrip(){
    let mut doc=Document::default();
    let id=doc.add(curve(),Style::default());
    let obj=doc.get_mut(id).unwrap();
    let anchor=NodeAddress::BezierEnd(0);
    node_edit::set_tangent(&mut obj.shape,anchor,Mode::Symmetric).unwrap();
    assert!(node_edit::set_mode(&mut obj.node_modes,anchor,Mode::Symmetric));
    doc.validate().unwrap();
    let json=serde_json::to_string(&doc).unwrap();
    assert!(json.contains("node_modes"));
    let mut reloaded:Document=serde_json::from_str(&json).unwrap();
    reloaded.validate().unwrap();
    assert_eq!(doc,reloaded);
    let obj=reloaded.get_mut(id).unwrap();
    let target=p(21.,22.);
    node_edit::move_node_with_modes(&mut obj.shape,NodeAddress::BezierControl1(1),
        target,&obj.node_modes).unwrap();
    let Shape::Bezier{segments,..}=&obj.shape else{panic!()};
    let a=segments[0].end;
    assert_eq!(segments[1].control1,target);
    assert_close(segments[0].control2,p(a.x-(target.x-a.x),a.y-(target.y-a.y)));
    reloaded.validate().unwrap();
}
#[test]
fn smooth_mode_preserves_other_handle_length(){
    let mut shape=curve();
    let anchor=NodeAddress::BezierEnd(0);
    node_edit::set_tangent(&mut shape,anchor,Mode::Smooth).unwrap();
    let modes=[NodeModeBinding{address:anchor,mode:Mode::Smooth}];
    let Shape::Bezier{segments,..}=&shape else{panic!()};
    let a=segments[0].end;
    let old_len=length(a,segments[0].control2);
    let new=p(15.,25.);
    node_edit::move_node_with_modes(&mut shape,NodeAddress::BezierControl1(1),new,&modes).unwrap();
    let Shape::Bezier{segments,..}=&shape else{panic!()};
    assert!((length(a,segments[0].control2)-old_len).abs()<0.0005);
    let a_to_new=p(new.x-a.x,new.y-a.y);
    let in_dir=p(a.x-segments[0].control2.x,a.y-segments[0].control2.y);
    assert!((a_to_new.x*in_dir.y-a_to_new.y*in_dir.x).abs()<0.001);
}
#[test]
fn corner_mode_keeps_control_handles_independent(){
    let mut shape=curve();
    let anchor=NodeAddress::BezierEnd(0);
    node_edit::set_tangent(&mut shape,anchor,Mode::Corner).unwrap();
    let modes=[NodeModeBinding{address:anchor,mode:Mode::Corner}];
    node_edit::move_node_with_modes(&mut shape,NodeAddress::BezierControl1(1),
        p(19.,21.),&modes).unwrap();
    let Shape::Bezier{segments,..}=&shape else{panic!()};
    assert_eq!(segments[0].control2,segments[0].end);
    assert_eq!(segments[1].control1,p(19.,21.));
}
#[test]
fn insert_and_delete_keep_mode_on_correct_anchor(){
    let mut shape=curve();
    let mut modes=vec![NodeModeBinding{
        address:NodeAddress::BezierEnd(0),mode:Mode::Smooth}];
    node_edit::insert_node(&mut shape,SegmentAddress::Bezier(0),0.5).unwrap();
    node_edit::modes_after_insert(&mut modes,SegmentAddress::Bezier(0));
    assert_eq!(modes[0].address,NodeAddress::BezierEnd(1));
    node_edit::validate_modes(&shape,&modes).unwrap();
    node_edit::remove_node(&mut shape,NodeAddress::BezierEnd(0)).unwrap();
    node_edit::modes_after_remove(&mut modes,NodeAddress::BezierEnd(0));
    assert_eq!(modes[0].address,NodeAddress::BezierEnd(0));
    node_edit::validate_modes(&shape,&modes).unwrap();
    node_edit::remove_node(&mut shape,NodeAddress::BezierStart).unwrap();
    node_edit::modes_after_remove(&mut modes,NodeAddress::BezierStart);
    assert!(modes.is_empty(),"old first interior anchor became new start");
    node_edit::validate_modes(&shape,&modes).unwrap();
}
#[test]
fn compound_insert_remap_is_confined_to_its_own_contour(){
    let Shape::Bezier{start,segments,..}=curve() else{panic!()};
    let mut shape=Shape::CompoundPath{contours:vec![
        Contour{start,segments:segments.clone(),closed:true},
        Contour{start,segments,closed:false},
    ]};
    let mut modes=vec![
        NodeModeBinding{address:NodeAddress::CompoundEnd(0,0),mode:Mode::Smooth},
        NodeModeBinding{address:NodeAddress::CompoundEnd(1,0),mode:Mode::Symmetric},
    ];
    node_edit::insert_node(&mut shape,SegmentAddress::Compound(0,0),0.35).unwrap();
    node_edit::modes_after_insert(&mut modes,SegmentAddress::Compound(0,0));
    assert_eq!(modes[0].address,NodeAddress::CompoundEnd(0,1));
    assert_eq!(modes[1].address,NodeAddress::CompoundEnd(1,0));
    node_edit::validate_modes(&shape,&modes).unwrap();
}
#[test]
fn reverse_preserves_cubic_trace_and_remaps_modes(){
    let mut shape=curve();
    let original=shape.clone();
    let mut modes=vec![NodeModeBinding{
        address:NodeAddress::BezierEnd(0),mode:Mode::Smooth}];
    node_edit::reverse_path(&mut shape).unwrap();
    node_edit::modes_after_reverse(&shape,&mut modes);
    assert_eq!(modes[0].address,NodeAddress::BezierEnd(1));
    node_edit::validate_modes(&shape,&modes).unwrap();
    let Shape::Bezier{start:a,segments:as_,..}=original else{panic!()};
    let Shape::Bezier{start:b,segments:bs,..}=&shape else{panic!()};
    for idx in 0..as_.len(){
        let old_start=if idx==0{a}else{as_[idx-1].end};
        let rev_idx=as_.len()-1-idx;
        let new_start=if rev_idx==0{*b}else{bs[rev_idx-1].end};
        for k in 0..=20{
            let t=k as f32/20.0;
            assert_close(sample(old_start,&as_[idx],t),
                sample(new_start,&bs[rev_idx],1.0-t));
        }
    }
    node_edit::reverse_path(&mut shape).unwrap();
    node_edit::modes_after_reverse(&shape,&mut modes);
    assert_eq!(shape,Shape::Bezier{start:a,segments:as_,closed:false});
    assert_eq!(modes[0].address,NodeAddress::BezierEnd(0));
}
#[test]
fn closure_and_reversal_are_reversible_without_losing_nodes(){
    let mut shape=curve();
    assert!(node_edit::set_closed(&mut shape,true).unwrap());
    assert!(!node_edit::set_closed(&mut shape,true).unwrap());
    node_edit::reverse_path(&mut shape).unwrap();
    assert!(node_edit::set_closed(&mut shape,false).unwrap());
    let Shape::Bezier{closed,segments,..}=shape else{panic!()};
    assert!(!closed);
    assert_eq!(segments.len(),3);
}
#[test]
fn invalid_modes_rejected_on_document_load(){
    let mut doc=Document::default();
    let id=doc.add(curve(),Style::default());
    let binding=NodeModeBinding{address:NodeAddress::BezierEnd(0),mode:Mode::Smooth};
    doc.get_mut(id).unwrap().node_modes=vec![binding,binding];
    assert!(doc.validate().is_err());
    doc.get_mut(id).unwrap().node_modes=vec![
        NodeModeBinding{address:NodeAddress::BezierEnd(usize::MAX),mode:Mode::Corner}];
    assert!(doc.validate().is_err());
    doc.get_mut(id).unwrap().node_modes=vec![
        NodeModeBinding{address:NodeAddress::BezierEnd(0),mode:Mode::Corner}];
    doc.validate().unwrap();
    doc.get_mut(id).unwrap().shape=Shape::Rectangle{x:0.,y:0.,w:10.,h:10.};
    assert!(doc.validate().is_err());
}
