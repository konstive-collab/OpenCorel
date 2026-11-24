//! Node editing presentation/controller; geometric operations live in opencorel-core.
use super::{Editor,Tool,View};
use eframe::egui::{self,Color32,Pos2,Stroke};
use opencorel_core::{Point,Shape};
use opencorel_core::node_edit::{self,NodeAddress};

const MAX_VISIBLE_NODES:usize=750;
const HANDLE_HIT_PX:f32=11.0;
impl Editor{
    pub(super) fn node_edit_allowed(&self,id:u64)->bool{
        let Some(obj)=self.doc.objects.iter().find(|obj|obj.id==id) else{return false;};
        (obj.group_id.is_none()||obj.group_id
            .is_some_and(|group|self.doc.symbol_instance_for_group(group).is_some()))
            && self.doc.layer(obj.layer_id).map(|l|l.visible&&!l.locked).unwrap_or(false)
            && node_edit::editable(&obj.shape)
    }
    fn active_node_object(&self)->Option<&opencorel_core::Object>{
        if self.tool!=Tool::Nodes || !self.multi_selected.is_empty(){return None;}
        let id=self.selected?;
        if !self.node_edit_allowed(id){return None;}
        self.doc.objects.iter().find(|obj|obj.id==id)
    }
    /// Pick a visible node in screen pixels, independent of current zoom.
    pub(super) fn node_at(&self,v:View,pos:Pos2)->Option<(u64,NodeAddress)>{
        let obj=self.active_node_object()?;
        node_edit::nearest_handle(&obj.shape,v.mm(pos),
            HANDLE_HIT_PX/v.scale.max(0.001))
            .map(|(address,_)|(obj.id,address))
    }
    /// Draw editable anchors and cubic control handles only for the active object.
    pub(super) fn paint_node_handles(&self,p:&egui::Painter,v:View){
        let Some(obj)=self.active_node_object() else{return;};
        let active=self.active_node.and_then(|(id,address)|
            (id==obj.id).then_some(address));
        let visible=node_edit::handles(&obj.shape,MAX_VISIBLE_NODES);
        let hidden_active=active.filter(|address|
            !visible.iter().any(|(other,_)|other==address));
        for (address,point) in visible{
            let center=v.screen(point);
            if !v.rect.expand(14.0).contains(center){continue;}
            if address.is_handle(){
                let anchor=match address{
                    NodeAddress::BezierControl1(i)=>{
                        if i==0{Some(NodeAddress::BezierStart)}
                        else{Some(NodeAddress::BezierEnd(i-1))}
                    },
                    NodeAddress::BezierControl2(i)=>Some(NodeAddress::BezierEnd(i)),
                    NodeAddress::CompoundControl1(j,i)=>{
                        if i==0{Some(NodeAddress::CompoundStart(j))}
                        else{Some(NodeAddress::CompoundEnd(j,i-1))}
                    },
                    NodeAddress::CompoundControl2(j,i)=>Some(NodeAddress::CompoundEnd(j,i)),
                    _=>None
                };
                if let Some(anchor)=anchor.and_then(|a|node_edit::point_at(&obj.shape,a)){
                    p.line_segment([v.screen(anchor),center],
                        Stroke::new(1.0,Color32::from_rgb(78,183,170)));
                }
            }
            let selected=active==Some(address);
            let fill=if selected{Color32::from_rgb(255,189,63)}
                else if address.is_handle(){Color32::from_rgb(151,208,207)}
                else{Color32::WHITE};
            p.circle_filled(center,if selected{6.0}else{4.5},fill);
            p.circle_stroke(center,if selected{6.0}else{4.5},
                Stroke::new(1.5,Color32::from_rgb(13,130,118)));
        }
        // Selected nodes beyond the rendering cap remain obvious and usable.
        if let Some(address)=hidden_active{
            if let Some(point)=node_edit::point_at(&obj.shape,address){
                let center=v.screen(point);
                if v.rect.expand(14.0).contains(center){
                    p.circle_filled(center,6.0,Color32::from_rgb(255,189,63));
                    p.circle_stroke(center,6.0,
                        Stroke::new(1.5,Color32::from_rgb(13,130,118)));
                }
            }
        }
    }
    pub(super) fn select_node_near(&mut self,v:View,pos:Pos2){
        if let Some(target)=self.node_at(v,pos){
            self.active_node=Some(target);
            self.status="Узел выбран: перетащите его; Delete — удалить, двойной щелчок по сегменту — добавить".into();
            return;
        }
        let clicked=self.doc.hit_test(v.mm(pos),6.0/v.scale.max(0.001));
        if let Some(id)=clicked{
            if !self.node_edit_allowed(id){
                self.select_object(Some(id),false);
                self.active_node=None;
                self.status="Для редактирования узлов выберите кривую/линию. Обычную группу сначала разгруппируйте; текст преобразуйте в кривые.".into();
            }else{
                self.select_object(Some(id),false);
                self.active_node=None;
                self.status="Кривая выбрана. Теперь щёлкните по узлу или дважды по сегменту.".into();
            }
        }else{
            self.active_node=None;
        }
    }
    pub(super) fn begin_node_drag(&mut self,v:View,pos:Pos2){
        if let Some(target)=self.node_at(v,pos){
            self.checkpoint();
            self.active_node=Some(target);
            self.node_drag=Some(target);
        }else{
            self.select_node_near(v,pos);
        }
    }
    pub(super) fn drag_node_to(&mut self,to:Point){
        let Some((id,address))=self.node_drag else{return;};
        if !self.node_edit_allowed(id){self.node_drag=None;return;}
        let Some(obj)=self.doc.get_mut(id) else{return;};
        match node_edit::move_handle(&mut obj.shape,address,to,self.node_handle_mode){
            Ok(())=>{
                self.fill_cache.entries.clear();
                self.compound_cache.clear();
                self.status=format!("Узел: X {:.2} мм, Y {:.2} мм",to.x,to.y);
            }
            Err(e)=>self.status=e,
        }
    }
    pub(super) fn nudge_active_node(&mut self,dx:f32,dy:f32){
        let Some((id,address))=self.active_node else{return;};
        if !self.node_edit_allowed(id){return;}
        let Some(p)=self.doc.objects.iter().find(|o|o.id==id)
            .and_then(|o|node_edit::point_at(&o.shape,address)) else{return;};
        let to=Point{x:p.x+dx,y:p.y+dy};
        if !to.x.is_finite()||!to.y.is_finite(){return;}
        let mut updated=self.doc.objects.iter().find(|o|o.id==id).unwrap().shape.clone();
        if node_edit::move_handle(&mut updated,address,to,self.node_handle_mode).is_ok(){
            self.checkpoint();
            self.doc.get_mut(id).unwrap().shape=updated;
            self.compound_cache.clear();
            self.fill_cache.entries.clear();
        }
    }
    pub(super) fn insert_node_at_pointer(&mut self,v:View,pos:Pos2){
        if self.node_at(v,pos).is_some(){return;}
        let Some(obj)=self.active_node_object() else{
            self.status="Сначала выберите одну незаблокированную кривую".into();return;
        };
        let id=obj.id;
        let tolerance=10.0/v.scale.max(0.001);
        let Some((segment,t))=node_edit::nearest_segment(&obj.shape,v.mm(pos),tolerance) else{
            self.status="Дважды щёлкните ближе к линии кривой, чтобы добавить узел".into();
            return;
        };
        let mut updated=obj.shape.clone();
        match node_edit::insert_node(&mut updated,segment,t){
            Ok(address)=>{
                self.checkpoint();
                self.doc.get_mut(id).unwrap().shape=updated;
                self.active_node=Some((id,address));
                self.compound_cache.clear();self.fill_cache.entries.clear();
                self.status="Узел добавлен без изменения формы кривой (Ctrl+Z — отмена)".into();
            }
            Err(e)=>self.status=e,
        }
    }
    /// Apply a geometry command transactionally; invalid operations must not
    /// create an undo step or partially modify the selected object.
    fn edit_active_shape(&mut self,
        operation:impl FnOnce(&mut Shape)->Result<(),String>,success:&str){
        let Some(id)=self.selected else{
            self.status="Выберите векторный контур".into();return;
        };
        if !self.node_edit_allowed(id){
            self.status="Выберите несгруппированный контур на доступном слое".into();return;
        }
        let Some(obj)=self.doc.objects.iter().find(|o|o.id==id) else{return;};
        let mut updated=obj.shape.clone();
        match operation(&mut updated){
            Ok(())=>{
                self.checkpoint();
                self.doc.get_mut(id).unwrap().shape=updated;
                self.active_node=None;
                self.node_drag=None;
                self.fill_cache.entries.clear();
                self.compound_cache.clear();
                self.status=success.into();
            }
            Err(error)=>self.status=error
        }
    }
    pub(super) fn set_selected_anchor_mode(&mut self,mode:node_edit::AnchorMode){
        let Some((id,address))=self.active_node else{
            self.status="Сначала выберите опорный узел".into();return;
        };
        if self.selected!=Some(id){return;}
        self.edit_active_shape(|shape|node_edit::set_anchor_mode(shape,address,mode),
            "Тип узла изменён (Ctrl+Z — отмена)");
    }
    pub(super) fn straighten_selected_segment(&mut self){
        let Some((id,address))=self.active_node else{
            self.status="Сначала выберите конечный узел сегмента".into();return;
        };
        if self.selected!=Some(id){return;}
        let segment=match address{
            NodeAddress::BezierEnd(i)=>node_edit::SegmentAddress::Bezier(i),
            NodeAddress::CompoundEnd(j,i)=>node_edit::SegmentAddress::Compound(j,i),
            _=>{
                self.status="Для выпрямления выберите конечный узел сегмента".into();return;
            }
        };
        self.edit_active_shape(|shape|node_edit::straighten_segment(shape,segment),
            "Сегмент выпрямлен (Ctrl+Z — отмена)");
    }
    pub(super) fn set_selected_path_closed(&mut self,closed:bool){
        self.edit_active_shape(|shape|node_edit::set_closed(shape,closed),
            if closed{"Контур замкнут (Ctrl+Z — отмена)"}
            else{"Контур разомкнут (Ctrl+Z — отмена)"});
    }
    pub(super) fn convert_selected_polyline_to_bezier(&mut self){
        self.edit_active_shape(node_edit::polyline_to_bezier,
            "Ломаная преобразована в редактируемую кривую Безье (Ctrl+Z — отмена)");
    }
    pub(super) fn reverse_selected_path(&mut self){
        self.edit_active_shape(node_edit::reverse_path,
            "Направление контура изменено (Ctrl+Z — отмена)");
    }
    /// Split an open path into two real independent objects. Clone the
    /// document to validate the complete operation before creating an undo
    /// checkpoint; preserve original layer, styling and vector appearance.
    pub(super) fn split_selected_open_path(&mut self){
        let Some((id,address))=self.active_node else{
            self.status="Выберите внутренний узел открытого контура".into();return;
        };
        if self.selected!=Some(id)||!self.node_edit_allowed(id){
            self.status="Нельзя разделить заблокированный или сгруппированный контур".into();return;
        }
        if self.doc.clip_links.iter().any(|link|
            link.source_id==id||link.target_id==id){
            self.status="Сначала снимите обтравочную маску с контура".into();return;
        }
        let Some(source)=self.doc.get(id).cloned() else{return;};
        let (left,right)=match node_edit::split_open_at_node(&source.shape,address){
            Ok(parts)=>parts,
            Err(error)=>{self.status=error;return;}
        };
        let mut proposed=self.doc.clone();
        proposed.get_mut(id).expect("selected").shape=left;
        let other_id=proposed.copy_object(&source,source.layer_id,0.,0.);
        proposed.get_mut(other_id).expect("copied").shape=right;
        if let Err(error)=proposed.validate(){
            self.status=format!("Разделение отменено: {error}");
            return;
        }
        self.checkpoint();
        self.doc=proposed;
        self.selected=Some(other_id);
        self.multi_selected.clear();
        self.active_node=None;self.node_drag=None;
        self.fill_cache.entries.clear();
        self.vector_cache.clear();
        self.raster_cache.clear();
        self.compound_cache.clear();
        self.masked_canvas=Default::default();
        self.status="Контур разделён на два независимых объекта (Ctrl+Z — отмена)".into();
    }
    pub(super) fn break_selected_closed_path(&mut self){
        let Some((id,address))=self.active_node else{
            self.status="Сначала выберите узел замкнутого контура".into();return;
        };
        if self.selected!=Some(id){return;}
        self.edit_active_shape(|shape|node_edit::break_closed_at_node(shape,address),
            "Контур разорван в выбранном узле без потери сегментов (Ctrl+Z — отмена)");
    }
    pub(super) fn remove_active_node(&mut self){
        let Some((id,address))=self.active_node else{
            self.status="Выберите узел кривой перед удалением".into();return;
        };
        if !self.node_edit_allowed(id){
            self.status="Узлы этого объекта сейчас недоступны".into();return;
        }
        let Some(obj)=self.doc.objects.iter().find(|o|o.id==id) else{return;};
        let mut updated=obj.shape.clone();
        match node_edit::remove_node(&mut updated,address){
            Ok(())=>{
                self.checkpoint();
                self.doc.get_mut(id).unwrap().shape=updated;
                self.active_node=None;
                self.compound_cache.clear();self.fill_cache.entries.clear();
                self.status="Узел удалён. Ctrl+Z восстановит контур".into();
            }
            Err(e)=>self.status=e,
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{CubicSegment,Style};
    #[test] fn node_editor_nudge_and_undo(){
        let mut editor=Editor::default();
        let id=editor.doc.add(Shape::Bezier{
            start:Point{x:10.,y:20.},
            segments:vec![CubicSegment{control1:Point{x:15.,y:15.},
                control2:Point{x:35.,y:15.},end:Point{x:40.,y:20.}}],
            closed:false},Style::default());
        editor.select_object(Some(id),false);
        editor.tool=Tool::Nodes;
        editor.active_node=Some((id,NodeAddress::BezierStart));
        editor.nudge_active_node(2.,3.);
        let moved=node_edit::point_at(&editor.doc.objects.last().unwrap().shape,
            NodeAddress::BezierStart).unwrap();
        assert_eq!(moved,Point{x:12.,y:23.});
        editor.undo();
        let restored=node_edit::point_at(&editor.doc.objects.last().unwrap().shape,
            NodeAddress::BezierStart).unwrap();
        assert_eq!(restored,Point{x:10.,y:20.});
    }
    #[test] fn node_editor_can_hit_nodes_beyond_render_cap(){
        let mut editor=Editor::default();
        editor.doc=Default::default();
        let segments=(0..1000).map(|i|{
            let x=(i+1) as f32*2.0;
            CubicSegment{control1:Point{x:x-1.5,y:4.},
                control2:Point{x:x-0.5,y:4.},end:Point{x,y:0.}}
        }).collect();
        let id=editor.doc.add(Shape::Bezier{
            start:Point{x:0.,y:0.},segments,closed:false
        },Style::default());
        editor.select_object(Some(id),false);
        editor.tool=Tool::Nodes;
        let v=View{
            rect:egui::Rect::from_min_size(egui::Pos2::ZERO,
                egui::Vec2::new(4000.,100.)),
            scale:1.,
        };
        assert_eq!(editor.node_at(v,v.screen(Point{x:1902.,y:0.})),
            Some((id,NodeAddress::BezierEnd(950))));
    }
    #[test] fn split_open_bezier_creates_two_objects_with_single_undo(){
        let mut editor=Editor::default();
        editor.doc=Default::default();
        let id=editor.doc.add(Shape::Bezier{
            start:Point{x:0.,y:0.},segments:vec![
                CubicSegment{control1:Point{x:2.,y:2.},
                    control2:Point{x:8.,y:2.},end:Point{x:10.,y:0.}},
                CubicSegment{control1:Point{x:12.,y:2.},
                    control2:Point{x:18.,y:2.},end:Point{x:20.,y:0.}},
            ],closed:false
        },Style::default());
        let original=editor.doc.clone();
        editor.select_object(Some(id),false);
        editor.tool=Tool::Nodes;
        editor.active_node=Some((id,NodeAddress::BezierEnd(0)));
        editor.split_selected_open_path();
        assert_eq!(editor.doc.objects.len(),2);
        assert!(editor.doc.get(id).is_some());
        assert_ne!(editor.selected,Some(id));
        editor.doc.validate().unwrap();
        editor.undo();
        assert_eq!(editor.doc,original);
    }
    #[test] fn node_editor_break_closed_path_is_undoable(){
        let mut editor=Editor::default();
        editor.doc=Default::default();
        let id=editor.doc.add(Shape::Polyline{points:vec![
            Point{x:0.,y:0.},Point{x:5.,y:0.},Point{x:5.,y:5.}
        ],closed:true},Style::default());
        let before=editor.doc.clone();
        editor.select_object(Some(id),false);
        editor.tool=Tool::Nodes;
        editor.active_node=Some((id,NodeAddress::Polyline(1)));
        editor.break_selected_closed_path();
        let Shape::Polyline{points,closed}=&editor.doc.get(id).unwrap().shape
            else{panic!()};
        assert!(!*closed);
        assert_eq!(points.len(),4);
        editor.undo();
        assert_eq!(editor.doc,before);
    }
    #[test] fn node_editor_nudge_respects_symmetric_handle_mode(){
        let mut editor=Editor::default();
        editor.doc=Default::default();
        let id=editor.doc.add(Shape::Bezier{start:Point{x:0.,y:0.},segments:vec![
            CubicSegment{control1:Point{x:1.,y:0.},control2:Point{x:8.,y:0.},
                end:Point{x:10.,y:0.}},
            CubicSegment{control1:Point{x:12.,y:0.},control2:Point{x:18.,y:0.},
                end:Point{x:20.,y:0.}}
        ],closed:false},Style::default());
        editor.select_object(Some(id),false);
        editor.tool=Tool::Nodes;
        editor.node_handle_mode=node_edit::HandleMode::Symmetric;
        editor.active_node=Some((id,NodeAddress::BezierControl1(1)));
        editor.nudge_active_node(0.,3.);
        let Shape::Bezier{segments,..}=&editor.doc.get(id).unwrap().shape
            else{panic!()};
        assert_eq!(segments[0].control2,Point{x:8.,y:-3.});
        editor.undo();
        let Shape::Bezier{segments,..}=&editor.doc.get(id).unwrap().shape
            else{panic!()};
        assert_eq!(segments[0].control2,Point{x:8.,y:0.});
    }
    #[test] fn node_tool_rejects_locked_or_grouped_object(){
        let mut editor=Editor::default();
        let id=editor.doc.add(Shape::Polyline{
            points:vec![Point{x:0.,y:0.},Point{x:5.,y:5.}],closed:false
        },Style::default());
        assert!(editor.node_edit_allowed(id));
        editor.doc.layers[0].locked=true;
        assert!(!editor.node_edit_allowed(id));
        editor.doc.layers[0].locked=false;
        editor.doc.objects.last_mut().unwrap().group_id=Some(9);
        assert!(!editor.node_edit_allowed(id));
    }
}
