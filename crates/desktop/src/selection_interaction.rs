//! Selection ergonomics: crossing/window marquee and atomic object movement.
//! Mouse interactions mutate the document only after meaningful movement;
//! clicks, cancelled drags and empty marquee selections do not affect Undo.
use super::{Editor,Point,View};
use eframe::egui::{self,Color32,Pos2,Rect,Stroke};
use std::collections::HashSet;

#[derive(Clone,Copy,Debug)]
pub(super) struct MarqueeDrag{
    pub start:Point,
    pub current:Point,
    pub additive:bool,
}
impl MarqueeDrag{
    fn is_crossing(&self)->bool{self.current.x<self.start.x}
    fn rect(&self)->(f32,f32,f32,f32){
        (self.start.x.min(self.current.x),
         self.start.y.min(self.current.y),
         self.start.x.max(self.current.x),
         self.start.y.max(self.current.y))
    }
    fn contains(&self,b:opencorel_core::Bounds)->bool{
        let (x1,y1,x2,y2)=self.rect();
        let ax=b.x.min(b.x+b.w);let ay=b.y.min(b.y+b.h);
        let bx=b.x.max(b.x+b.w);let by=b.y.max(b.y+b.h);
        if self.is_crossing(){
            ax<=x2 && bx>=x1 && ay<=y2 && by>=y1
        }else{
            ax>=x1 && bx<=x2 && ay>=y1 && by<=y2
        }
    }
}
impl Editor{
    pub(super) fn begin_move_selection(&mut self,id:u64,p:Point){
        self.drag_start=Some((p,id));
        self.selection_drag_checkpointed=false;
        self.marquee_drag=None;
    }
    pub(super) fn move_selection_drag(&mut self,p:Point){
        let Some((previous,id))=self.drag_start else{return;};
        if !p.x.is_finite()||!p.y.is_finite()||
            p.x.abs()>100_000.0||p.y.abs()>100_000.0{
            self.status="Недопустимая координата перемещения".into();
            return;
        }
        let dx=p.x-previous.x;let dy=p.y-previous.y;
        if dx.abs()<=1e-6 && dy.abs()<=1e-6{return;}
        let ids:HashSet<u64>=self.selection_ids().into_iter().collect();
        if !ids.contains(&id) || ids.is_empty(){return;}
        let mut valid_count=0;
        for o in &self.doc.objects{
            if !ids.contains(&o.id){continue;}
            if !self.doc.layer(o.layer_id)
                .is_some_and(|layer|layer.visible&&!layer.locked){
                self.status="Перемещение невозможно: выделение содержит заблокированный слой".into();
                return;
            }
            let bounds=o.bounds();
            if [bounds.x+dx,bounds.y+dy,bounds.x+bounds.w+dx,
                bounds.y+bounds.h+dy].iter().any(|v|
                    !v.is_finite() || v.abs()>100_000.0){
                self.status="Перемещение выходит за безопасные координаты".into();
                return;
            }
            valid_count+=1;
        }
        if valid_count==0{return;}
        if !self.selection_drag_checkpointed{
            self.checkpoint();
            self.selection_drag_checkpointed=true;
        }
        for obj in &mut self.doc.objects{
            if ids.contains(&obj.id){obj.shape.translate(dx,dy);}
        }
        self.drag_start=Some((p,id));
        // Vector/compound tessellation is often keyed only by object ID.
        // Invalidate it whenever native geometry changes.
        self.fill_cache.entries.clear();
        self.vector_cache.clear();
        self.compound_cache.clear();
        self.bezier_preview.clear();
        self.raster_cache.clear();
    }
    pub(super) fn begin_marquee(&mut self,p:Point,additive:bool){
        self.drag_start=None;
        self.selection_drag_checkpointed=false;
        self.marquee_drag=Some(MarqueeDrag{
            start:p,current:p,additive,
        });
    }
    pub(super) fn update_marquee(&mut self,p:Point){
        if !p.x.is_finite()||!p.y.is_finite(){return;}
        if let Some(drag)=&mut self.marquee_drag{
            drag.current=p;
        }
    }
    pub(super) fn finish_marquee(&mut self){
        let Some(drag)=self.marquee_drag.take() else{return;};
        let (x1,y1,x2,y2)=drag.rect();
        let meaningful=(x2-x1).abs()>0.02 || (y2-y1).abs()>0.02;
        if !meaningful{return;}
        let clip_sources:HashSet<u64>=self.doc.clip_links
            .iter().map(|c|c.source_id).collect();
        // Two linear passes rather than group_members() inside each object
        // (which was quadratic on documents with thousands of objects).
        let editable=|o:&opencorel_core::Object|{
            self.doc.layer(o.layer_id)
                .is_some_and(|layer|layer.visible&&!layer.locked) &&
            !clip_sources.contains(&o.id)
        };
        let mut direct=HashSet::new();
        let mut groups=HashSet::new();
        let mut blocked_groups=HashSet::new();
        for o in &self.doc.objects{
            if !editable(o){
                if let Some(group)=o.group_id{blocked_groups.insert(group);}
                continue;
            }
            let bounds=self.text_preview.ink_bounds(o)
                .unwrap_or_else(||o.bounds());
            if drag.contains(bounds){
                direct.insert(o.id);
                if let Some(group)=o.group_id{groups.insert(group);}
            }
        }
        let mut matched:Vec<u64>=Vec::new();
        for o in &self.doc.objects{
            if !editable(o){continue;}
            if let Some(group)=o.group_id{
                if groups.contains(&group)&&!blocked_groups.contains(&group){
                    matched.push(o.id);
                }
            }else if direct.contains(&o.id){
                matched.push(o.id);
            }
        }
        let mut next=if drag.additive{
            self.selection_ids()
        }else{
            Vec::new()
        };
        for id in matched{
            if !next.contains(&id){next.push(id);}
        }
        self.selected=next.first().copied();
        self.multi_selected=next.into_iter().skip(1).collect();
        self.active_node=None;
        self.status=format!("Рамка выделения: {} объектов ({})",
            self.selection_ids().len(),
            if drag.is_crossing(){"пересечение"}else{"внутри"});
    }
    pub(super) fn paint_marquee(&self,painter:&egui::Painter,v:View){
        let Some(drag)=self.marquee_drag else{return;};
        let a=v.screen(drag.start);
        let b=v.screen(drag.current);
        if !a.x.is_finite()||!a.y.is_finite()||!b.x.is_finite()||!b.y.is_finite(){
            return;
        }
        let rect=Rect::from_two_pos(a,b);
        let crossing=drag.is_crossing();
        let tint=if crossing{
            Color32::from_rgb(234,155,58)
        }else{
            Color32::from_rgb(32,163,211)
        };
        painter.rect_filled(rect,0.0,Color32::from_rgba_unmultiplied(
            tint.r(),tint.g(),tint.b(),26));
        let points=[
            rect.left_top(),rect.right_top(),rect.right_bottom(),
            rect.left_bottom(),rect.left_top(),
        ].to_vec();
        painter.add(egui::Shape::line(points,Stroke::new(1.0,tint)));
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Document,Shape,Style,Bounds};
    fn p(x:f32,y:f32)->Point{Point{x,y}}
    #[test]
    fn object_drag_has_lazy_one_step_undo_even_after_many_updates(){
        let mut e=Editor::default();
        e.doc=Document::default();
        let id=e.doc.add(Shape::Rectangle{x:0.,y:0.,w:20.,h:10.},
            Style::default());
        e.selected=Some(id);
        let before=e.doc.clone();
        e.begin_move_selection(id,p(5.,5.));
        e.move_selection_drag(p(5.,5.));
        assert!(e.undo.is_empty());
        for i in 1..30{
            e.move_selection_drag(p(i as f32+5.,5.));
        }
        assert_eq!(e.undo.len(),1);
        let b=e.doc.get_mut(id).unwrap().bounds();
        assert_eq!(b.x,29.);
        e.doc.validate().unwrap();
        e.undo();
        assert_eq!(e.doc,before);
    }
    #[test]
    fn locked_selection_prevents_partial_multi_move_and_history(){
        let mut e=Editor::default();
        e.doc=Document::default();
        let id=e.doc.add(Shape::Rectangle{x:1.,y:1.,w:5.,h:5.},Style::default());
        let layer=e.doc.add_layer("Locked".into());
        let another=e.doc.add_to_layer(Shape::Rectangle{x:20.,y:1.,w:5.,h:5.},
            Style::default(),layer);
        e.doc.layers.iter_mut().find(|l|l.id==layer).unwrap().locked=true;
        e.selected=Some(id);e.multi_selected=vec![another];
        let original=e.doc.clone();
        e.begin_move_selection(id,p(2.,2.));
        e.move_selection_drag(p(12.,12.));
        assert_eq!(e.doc,original);
        assert!(e.undo.is_empty());
    }
    #[test]
    fn marquee_left_to_right_encloses_but_right_to_left_crosses(){
        let mut e=Editor::default();
        e.doc=Document::default();
        let a=e.doc.add(Shape::Rectangle{x:10.,y:10.,w:10.,h:10.},Style::default());
        let b=e.doc.add(Shape::Rectangle{x:25.,y:10.,w:20.,h:10.},Style::default());
        e.begin_marquee(p(5.,5.),false);
        e.update_marquee(p(35.,25.));
        e.finish_marquee();
        assert_eq!(e.selection_ids(),vec![a]);
        e.begin_marquee(p(35.,25.),false);
        e.update_marquee(p(5.,5.));
        e.finish_marquee();
        assert_eq!(e.selection_ids(),vec![a,b]);
        assert!(e.undo.is_empty(),"Selection only cannot alter document history");
    }
    #[test]
    fn shift_marquee_adds_to_selection_without_duplicates(){
        let mut e=Editor::default();
        e.doc=Document::default();
        let a=e.doc.add(Shape::Rectangle{x:5.,y:5.,w:5.,h:5.},Style::default());
        let b=e.doc.add(Shape::Rectangle{x:25.,y:5.,w:5.,h:5.},Style::default());
        e.selected=Some(a);
        e.begin_marquee(p(22.,2.),true);
        e.update_marquee(p(33.,13.));
        e.finish_marquee();
        assert_eq!(e.selection_ids(),vec![a,b]);
        e.begin_marquee(p(22.,2.),true);
        e.update_marquee(p(33.,13.));
        e.finish_marquee();
        assert_eq!(e.selection_ids(),vec![a,b]);
    }
    #[test]
    fn locked_or_hidden_objects_cannot_enter_marquee_selection(){
        let mut e=Editor::default();
        e.doc=Document::default();
        let id=e.doc.add(Shape::Rectangle{x:5.,y:5.,w:5.,h:5.},Style::default());
        e.doc.layers[0].locked=true;
        e.begin_marquee(p(0.,0.),false);
        e.update_marquee(p(15.,15.));
        e.finish_marquee();
        assert!(e.selection_ids().is_empty());
        e.doc.layers[0].locked=false;e.doc.layers[0].visible=false;
        e.begin_marquee(p(0.,0.),false);
        e.update_marquee(p(15.,15.));
        e.finish_marquee();
        assert!(e.selection_ids().is_empty());
        e.doc.layers[0].visible=true;
        e.begin_marquee(p(0.,0.),false);
        e.update_marquee(p(15.,15.));
        e.finish_marquee();
        assert_eq!(e.selected,Some(id));
    }
    #[test]
    fn resize_corner_is_lazy_and_preserves_original_aspect_ratio(){
        let mut e=Editor::default();
        e.doc=Document::default();
        let id=e.doc.add(Shape::Rectangle{x:0.,y:0.,w:20.,h:10.},
            Style::default());
        e.selected=Some(id);
        let original=e.doc.clone();
        e.resize_drag=Some((id,super::super::ResizeCorner::BottomRight));
        e.resize_drag_base=Some(e.doc.get_mut(id).unwrap().bounds());
        e.resize_from_corner(id,super::super::ResizeCorner::BottomRight,p(20.,10.),false);
        assert!(e.undo.is_empty(),"Clicking a resize handle is not an edit");
        e.resize_from_corner(id,super::super::ResizeCorner::BottomRight,p(25.,12.5),true);
        e.resize_from_corner(id,super::super::ResizeCorner::BottomRight,p(40.,20.),true);
        assert_eq!(e.undo.len(),1,"Whole drag is one Undo operation");
        let b=e.doc.get_mut(id).unwrap().bounds();
        assert_eq!(b.x,0.);
        assert_eq!(b.y,0.);
        assert_eq!(b.w,40.);
        assert_eq!(b.h,20.);
        e.doc.validate().unwrap();
        e.undo();
        assert_eq!(e.doc,original);
    }
    #[test]
    fn resize_drag_from_top_left_keeps_opposite_anchor_fixed(){
        let mut e=Editor::default();
        e.doc=Document::default();
        let id=e.doc.add(Shape::Rectangle{x:10.,y:20.,w:30.,h:40.},
            Style::default());
        e.selected=Some(id);
        let corner=super::super::ResizeCorner::TopLeft;
        e.resize_drag=Some((id,corner));
        e.resize_drag_base=Some(e.doc.get_mut(id).unwrap().bounds());
        e.resize_from_corner(id,corner,p(0.,0.),false);
        e.resize_from_corner(id,corner,p(15.,22.),false);
        let b=e.doc.get_mut(id).unwrap().bounds();
        assert_eq!((b.x,b.y,b.w,b.h),(15.,22.,25.,38.));
        assert_eq!(e.undo.len(),1);
    }
    #[test]
    fn group_marquee_skips_groups_partially_locked_on_other_layers(){
        let mut e=Editor::default();
        e.doc=Document::default();
        let a=e.doc.add(Shape::Rectangle{x:5.,y:5.,w:5.,h:5.},Style::default());
        let layer=e.doc.add_layer("Locked".into());
        let b=e.doc.add_to_layer(Shape::Rectangle{x:30.,y:30.,w:5.,h:5.},
            Style::default(),layer);
        let _=e.doc.group_objects(&[a,b]);
        e.doc.layers.iter_mut().find(|l|l.id==layer).unwrap().locked=true;
        e.begin_marquee(p(0.,0.),false);
        e.update_marquee(p(15.,15.));
        e.finish_marquee();
        assert!(e.selection_ids().is_empty(),
            "Never select only half a group on an uneditable layer");
        assert!(e.undo.is_empty());
    }
    #[test]
    fn precise_pick_falls_through_empty_curve_bbox_to_object_below(){
        use opencorel_core::CubicSegment;
        let mut e=Editor::default();
        e.doc=Document::default();
        let back=e.doc.add(Shape::Rectangle{x:0.,y:0.,w:100.,h:100.},
            Style::default());
        let front=e.doc.add(Shape::Bezier{start:p(0.,0.),closed:false,
            segments:vec![CubicSegment{control1:p(0.,100.),
                control2:p(100.,100.),end:p(100.,0.)}]},
            Style{fill:"none".into(),stroke:"#000000".into(),
                stroke_width_mm:0.3});
        assert_eq!(e.hit_test_at(p(50.,20.),1.),Some(back),
            "A curve's empty bounding rectangle must not cover artwork below");
        assert_eq!(e.hit_test_at(p(50.,75.),1.),Some(front));
    }
    #[test]
    fn pointer_selection_tolerance_is_constant_across_zoom(){
        use eframe::egui::Vec2;
        let mut e=Editor::default();
        e.doc=Document::default();
        let viewport=Rect::from_min_size(Pos2::ZERO,
            Vec2::new(1200.,800.));
        let v=View::new(viewport,&e.doc,1.0,Vec2::ZERO);
        let magnified=View::new(viewport,&e.doc,5.0,Vec2::ZERO);
        assert!((e.pick_tolerance_mm(v)*v.scale-6.0).abs()<0.01);
        assert!((e.pick_tolerance_mm(magnified)*magnified.scale-6.0).abs()<0.01);
    }
    #[test]
    fn marquee_predicate_ignores_malformed_or_empty_drag(){
        let rect=Bounds{x:10.,y:10.,w:10.,h:10.};
        let d=MarqueeDrag{start:p(0.,0.),current:p(15.,15.),additive:false};
        assert!(!d.contains(rect));
        let reversed=MarqueeDrag{start:p(15.,15.),current:p(0.,0.),additive:false};
        assert!(reversed.contains(rect));
    }
}
