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
        obj.group_id.is_none()
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
        let mut best=HANDLE_HIT_PX*HANDLE_HIT_PX;
        let mut hit=None;
        for (address,point) in node_edit::handles(&obj.shape,MAX_VISIBLE_NODES){
            let d=(v.screen(point)-pos).length_sq();
            if d<best{
                best=d;hit=Some((obj.id,address));
            }
        }
        hit
    }
    /// Draw editable anchors and cubic control handles only for the active object.
    pub(super) fn paint_node_handles(&self,p:&egui::Painter,v:View){
        let Some(obj)=self.active_node_object() else{return;};
        let active=self.active_node.map(|(_,address)|address);
        for (address,point) in node_edit::handles(&obj.shape,MAX_VISIBLE_NODES){
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
                self.status="Для редактирования узлов выберите кривую/линию. Группу сначала разгруппируйте, а текст преобразуйте в кривые.".into();
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
        self.node_drag_checkpointed=false;
        if let Some(target)=self.node_at(v,pos){
            // Do not snapshot the entire document just for grabbing a node.
            self.active_node=Some(target);
            self.node_drag=Some(target);
        }else{
            self.select_node_near(v,pos);
        }
    }
    pub(super) fn drag_node_to(&mut self,to:Point){
        let Some((id,address))=self.node_drag else{return;};
        if !self.node_edit_allowed(id){
            self.node_drag=None;
            self.node_drag_checkpointed=false;
            return;
        }
        let Some(obj)=self.doc.objects.iter().find(|obj|obj.id==id) else{return;};
        let Some(previous)=node_edit::point_at(&obj.shape,address) else{return;};
        if previous==to{return;}
        if !self.node_drag_checkpointed{
            // Validate before cloning the full document. This prevents
            // no-op/illegal drags from polluting history.
            let mut tested=obj.shape.clone();
            match node_edit::move_node_with_modes(
                &mut tested,address,to,&obj.node_modes){
                Ok(())=>{
                    self.checkpoint();
                    self.node_drag_checkpointed=true;
                    if let Some(obj)=self.doc.get_mut(id){obj.shape=tested;}
                },
                Err(reason)=>{
                    self.status=reason;
                    return;
                },
            }
        }else{
            let Some(obj)=self.doc.get_mut(id) else{return;};
            if let Err(reason)=node_edit::move_node_with_modes(
                &mut obj.shape,address,to,&obj.node_modes){
                self.status=reason;
                return;
            }
        }
        self.fill_cache.entries.clear();
        self.compound_cache.clear();
        self.bezier_preview.clear();
        self.status=format!("Узел: X {:.2} мм, Y {:.2} мм",to.x,to.y);
    }
    pub(super) fn nudge_active_node(&mut self,dx:f32,dy:f32){
        let Some((id,address))=self.active_node else{return;};
        if !self.node_edit_allowed(id){return;}
        let Some(p)=self.doc.objects.iter().find(|o|o.id==id)
            .and_then(|o|node_edit::point_at(&o.shape,address)) else{return;};
        let to=Point{x:p.x+dx,y:p.y+dy};
        if !to.x.is_finite()||!to.y.is_finite(){return;}
        let mut updated=self.doc.objects.iter().find(|o|o.id==id).unwrap().shape.clone();
        if node_edit::move_node_with_modes(&mut updated,address,to,
            &self.doc.objects.iter().find(|o|o.id==id).unwrap().node_modes).is_ok(){
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
                let obj=self.doc.get_mut(id).unwrap();
                obj.shape=updated;
                node_edit::modes_after_insert(&mut obj.node_modes,segment);
                self.active_node=Some((id,address));
                self.compound_cache.clear();self.fill_cache.entries.clear();
                self.status="Узел добавлен без изменения формы кривой (Ctrl+Z — отмена)".into();
            }
            Err(e)=>self.status=e,
        }
    }
    /// One-shot tangent commands leave native cubic geometry editable.
    /// Geometry is validated on a copy so rejected operations do not push
    /// history or partially mutate the document.
    pub(super) fn apply_tangent_operation(&mut self,operation:node_edit::TangentOperation){
        let Some((id,address))=self.active_node else{
            self.status="Сначала выберите внутренний узел кривой".into();
            return;
        };
        if !self.node_edit_allowed(id){
            self.status="Слой скрыт, заблокирован или объект сгруппирован".into();
            return;
        }
        let Some(obj)=self.doc.objects.iter().find(|obj|obj.id==id) else{return;};
        let mut updated=obj.shape.clone();
        match node_edit::set_tangent(&mut updated,address,operation){
            Ok(geometry_changed)=>{
                let previous_mode=node_edit::mode_at(&obj.node_modes,address);
                if geometry_changed || previous_mode!=Some(operation){
                    self.checkpoint();
                    if let Some(obj)=self.doc.get_mut(id){
                        obj.shape=updated;
                        node_edit::set_mode(&mut obj.node_modes,address,operation);
                    }
                    self.compound_cache.clear();
                    self.bezier_preview.clear();
                    self.fill_cache.entries.clear();
                    let command=match operation{
                        node_edit::TangentOperation::Corner=>"Угловой узел: независимые рычаги",
                        node_edit::TangentOperation::Smooth=>"Касательные сохраняют гладкость",
                        node_edit::TangentOperation::Symmetric=>"Касательные сохраняют симметрию",
                    };
                    self.status=format!("{command}. Режим сохранён в .kdr");
                }else{
                    self.status="Узел уже в этом режиме — изменений нет".into();
                }
            },
            Err(reason)=>{
                self.status=format!("Касательные не изменены: {reason}");
            },
        }
    }
    /// Reverse editable path direction without modifying its geometry.
    /// A complete node-mode reindex is part of this undoable transaction.
    pub(super) fn reverse_selected_path(&mut self){
        let Some(id)=self.selected else{return;};
        if !self.node_edit_allowed(id){
            self.status="Нельзя изменить путь на заблокированном слое".into();
            return;
        }
        let Some(obj)=self.doc.objects.iter().find(|o|o.id==id) else{return;};
        let mut updated=obj.shape.clone();
        if let Err(reason)=node_edit::reverse_path(&mut updated){
            self.status=reason;return;
        }
        let mut modes=obj.node_modes.clone();
        node_edit::modes_after_reverse(&updated,&mut modes);
        self.checkpoint();
        if let Some(obj)=self.doc.get_mut(id){
            obj.shape=updated;obj.node_modes=modes;
        }
        self.active_node=None;
        self.compound_cache.clear();self.bezier_preview.clear();
        self.fill_cache.entries.clear();
        self.status="Направление контура изменено (Ctrl+Z — отмена)".into();
    }
    /// Open or close a path, retaining anchors and editable handles.
    pub(super) fn set_selected_path_closed(&mut self,closed:bool){
        let Some(id)=self.selected else{return;};
        if !self.node_edit_allowed(id){
            self.status="Нельзя изменять контур на заблокированном слое".into();
            return;
        }
        let Some(obj)=self.doc.objects.iter().find(|o|o.id==id) else{return;};
        let mut updated=obj.shape.clone();
        match node_edit::set_closed(&mut updated,closed){
            Ok(false)=>self.status="Контур уже находится в этом состоянии".into(),
            Ok(true)=>{
                self.checkpoint();
                if let Some(obj)=self.doc.get_mut(id){obj.shape=updated;}
                self.compound_cache.clear();self.bezier_preview.clear();
                self.fill_cache.entries.clear();
                self.status=if closed{"Контур замкнут"}else{"Контур разомкнут"}.into();
            },
            Err(reason)=>self.status=reason,
        }
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
                let obj=self.doc.get_mut(id).unwrap();
                obj.shape=updated;
                node_edit::modes_after_remove(&mut obj.node_modes,address);
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
    #[test] fn node_drag_only_records_history_on_first_valid_movement(){
        let mut editor=Editor::default();
        let id=editor.doc.add(Shape::Bezier{
            start:Point{x:10.0,y:20.0},
            segments:vec![CubicSegment{
                control1:Point{x:15.0,y:15.0},
                control2:Point{x:35.0,y:15.0},
                end:Point{x:40.0,y:20.0},
            }],closed:false,
        },Style::default());
        editor.select_object(Some(id),false);
        editor.tool=Tool::Nodes;
        let view=View{
            rect:egui::Rect::from_min_size(Pos2::ZERO,
                egui::Vec2::new(200.0,200.0)),
            scale:1.0,
        };
        let original=editor.doc.clone();
        editor.begin_node_drag(view,view.screen(Point{x:10.0,y:20.0}));
        assert_eq!(editor.node_drag,Some((id,NodeAddress::BezierStart)));
        assert!(editor.undo.is_empty());
        editor.drag_node_to(Point{x:10.0,y:20.0});
        assert!(editor.undo.is_empty());
        editor.drag_node_to(Point{x:f32::NAN,y:20.0});
        assert!(editor.undo.is_empty());
        editor.drag_node_to(Point{x:12.0,y:23.0});
        editor.drag_node_to(Point{x:14.0,y:25.0});
        assert_eq!(editor.undo.len(),1);
        editor.undo();
        assert_eq!(editor.doc,original);
    }
    #[test] fn persistent_smooth_mode_survives_undo_redo_and_subsequent_drag(){
        let mut editor=Editor::default();
        let id=editor.doc.add(Shape::Bezier{
            start:Point{x:0.,y:0.},segments:vec![
                CubicSegment{control1:Point{x:3.,y:4.},
                    control2:Point{x:8.,y:7.},end:Point{x:10.,y:10.}},
                CubicSegment{control1:Point{x:13.,y:18.},
                    control2:Point{x:20.,y:18.},end:Point{x:30.,y:10.}},
            ],closed:false},Style::default());
        editor.select_object(Some(id),false);
        editor.tool=Tool::Nodes;
        editor.active_node=Some((id,NodeAddress::BezierEnd(0)));
        editor.apply_tangent_operation(node_edit::TangentOperation::Symmetric);
        let mode=node_edit::mode_at(&editor.doc.get_mut(id).unwrap().node_modes,
            NodeAddress::BezierEnd(0));
        assert_eq!(mode,Some(node_edit::TangentOperation::Symmetric));
        assert_eq!(editor.undo.len(),1);
        editor.undo();
        assert!(editor.doc.get_mut(id).unwrap().node_modes.is_empty());
        editor.redo();
        assert_eq!(node_edit::mode_at(&editor.doc.get_mut(id).unwrap().node_modes,
            NodeAddress::BezierEnd(0)),Some(node_edit::TangentOperation::Symmetric));
        let obj=editor.doc.get_mut(id).unwrap();
        node_edit::move_node_with_modes(&mut obj.shape,NodeAddress::BezierControl1(1),
            Point{x:16.,y:22.},&obj.node_modes).unwrap();
        let Shape::Bezier{segments,..}=&obj.shape else{panic!()};
        let anchor=segments[0].end;
        let other=segments[0].control2;
        let target=segments[1].control1;
        assert!((anchor.x-other.x-(target.x-anchor.x)).abs()<1e-4);
        assert!((anchor.y-other.y-(target.y-anchor.y)).abs()<1e-4);
        editor.doc.validate().unwrap();
    }
    #[test] fn path_reverse_and_closure_each_have_atomic_undo(){
        let mut editor=Editor::default();
        let id=editor.doc.add(Shape::Bezier{
            start:Point{x:0.,y:0.},segments:vec![
                CubicSegment{control1:Point{x:4.,y:0.},
                    control2:Point{x:6.,y:5.},end:Point{x:10.,y:10.}},
                CubicSegment{control1:Point{x:14.,y:14.},
                    control2:Point{x:18.,y:6.},end:Point{x:20.,y:0.}},
            ],closed:false},Style::default());
        editor.tool=Tool::Nodes;editor.selected=Some(id);
        let original=editor.doc.clone();
        editor.reverse_selected_path();
        assert_eq!(editor.undo.len(),1);
        editor.set_selected_path_closed(true);
        assert_eq!(editor.undo.len(),2);
        editor.set_selected_path_closed(true);
        assert_eq!(editor.undo.len(),2);
        editor.undo();
        let Shape::Bezier{closed,..}=&editor.doc.get_mut(id).unwrap().shape
            else{panic!()};
        assert!(!closed);
        editor.undo();
        assert_eq!(editor.doc,original);
        editor.redo();
        editor.redo();
        editor.doc.validate().unwrap();
    }
    #[test] fn tangent_command_is_undoable_and_not_repeated_as_noop(){
        let mut editor=Editor::default();
        let id=editor.doc.add(Shape::Bezier{
            start:Point{x:0.,y:0.},
            segments:vec![
                CubicSegment{control1:Point{x:1.,y:3.},
                    control2:Point{x:9.,y:4.},end:Point{x:10.,y:10.}},
                CubicSegment{control1:Point{x:18.,y:11.},
                    control2:Point{x:20.,y:15.},end:Point{x:25.,y:20.}},
            ],closed:false},Style::default());
        let before=editor.doc.clone();
        editor.select_object(Some(id),false);
        editor.tool=Tool::Nodes;
        editor.active_node=Some((id,NodeAddress::BezierEnd(0)));
        editor.apply_tangent_operation(node_edit::TangentOperation::Corner);
        assert_ne!(editor.doc,before);
        assert_eq!(editor.undo.len(),1);
        editor.apply_tangent_operation(node_edit::TangentOperation::Corner);
        assert_eq!(editor.undo.len(),1);
        editor.undo();
        assert_eq!(editor.doc,before);
        editor.redo();
        editor.doc.validate().unwrap();
    }
    #[test] fn tangent_command_rejects_locked_object_without_undo(){
        let mut editor=Editor::default();
        let id=editor.doc.add(Shape::Bezier{
            start:Point{x:0.,y:0.},segments:vec![
                CubicSegment{control1:Point{x:1.,y:1.},
                    control2:Point{x:8.,y:5.},end:Point{x:10.,y:10.}},
                CubicSegment{control1:Point{x:12.,y:15.},
                    control2:Point{x:20.,y:20.},end:Point{x:22.,y:25.}},
            ],closed:false},Style::default());
        editor.selected=Some(id);
        editor.tool=Tool::Nodes;
        editor.active_node=Some((id,NodeAddress::BezierEnd(0)));
        let before=editor.doc.clone();
        editor.doc.layers[0].locked=true;
        editor.apply_tangent_operation(node_edit::TangentOperation::Symmetric);
        assert_eq!(editor.doc.objects,before.objects);
        assert!(editor.undo.is_empty());
    }
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
