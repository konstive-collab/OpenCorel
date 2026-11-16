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
    /// Return a selected anchor to freely editable handles without
    /// collapsing or repositioning either control point.
    pub(super) fn unlink_active_tangents(&mut self){
        let Some((id,address))=self.active_node else{return;};
        if !self.node_edit_allowed(id){
            self.status="Нельзя изменять заблокированный узел".into();
            return;
        }
        let Some(obj)=self.doc.objects.iter().find(|o|o.id==id) else{return;};
        if !node_edit::has_tangent_pair(&obj.shape,address){
            self.status="Выберите внутренний узел кривой".into();
            return;
        }
        if node_edit::mode_at(&obj.node_modes,address).is_none(){
            self.status="Рычаги уже независимы".into();
            return;
        }
        self.checkpoint();
        if let Some(obj)=self.doc.get_mut(id){
            obj.node_modes.retain(|mode|mode.address!=address);
        }
        self.status="Свободные рычаги: геометрия не изменена. Ctrl+Z — отмена".into();
    }
    /// Continue an existing open Bézier path in-place. No checkpoint is made
    /// until the user actually appends a segment (or closes the path), so
    /// entering/exiting this mode without editing does not pollute Undo.
    pub(super) fn continue_selected_bezier(&mut self){
        let Some(id)=self.selected else{
            self.status="Сначала выделите открытую кривую".into();
            return;
        };
        if !self.multi_selected.is_empty() || !self.node_edit_allowed(id){
            self.status="Продолжить можно только один редактируемый контур".into();
            return;
        }
        let Some(obj)=self.doc.objects.iter().find(|o|o.id==id) else{return;};
        let Shape::Bezier{start,segments,closed:false}=&obj.shape else{
            self.status="Продолжить можно только открытую кривую Безье".into();
            return;
        };
        if segments.is_empty() || segments.len()>=10_000{
            self.status="Кривая пуста или достигнут предел 10 000 сегментов".into();
            return;
        }
        let first=*start;
        self.tool=Tool::Curve;
        self.curve_draft=Some(id);
        self.curve_start=Some(first);
        self.curve_resumed=true;
        self.curve_outgoing=None;
        self.curve_drag_anchor=None;
        self.active_node=None;self.node_drag=None;
        self.node_drag_checkpointed=false;
        self.status="Продолжение кривой: добавьте узлы, Enter — завершить, Esc — выйти".into();
    }

    /// Join exactly two selected, same-layer open Bézier objects in one
    /// native path. Refuse mismatched styles, clips, groups and locked layers
    /// rather than silently flattening or dropping object-level properties.
    pub(super) fn join_selected_beziers(
        &mut self,mode:opencorel_core::path_topology::JoinMode
    ){
        let ids=self.selection_ids();
        if ids.len()!=2 || ids[0]==ids[1]{
            self.status="Выберите ровно две кривые (V + Shift), затем F10".into();
            return;
        }
        let [id_a,id_b]=[ids[0],ids[1]];
        if !self.node_edit_allowed(id_a) || !self.node_edit_allowed(id_b){
            self.status="Соединение: объекты должны быть редактируемыми, без групп и блокировок".into();
            return;
        }
        let Some(a)=self.doc.objects.iter().find(|o|o.id==id_a).cloned() else{return;};
        let Some(b)=self.doc.objects.iter().find(|o|o.id==id_b).cloned() else{return;};
        if a.layer_id!=b.layer_id || a.style!=b.style ||
            self.doc.appearances.get(&id_a)!=self.doc.appearances.get(&id_b){
            self.status="Соединение требует одного слоя и одинаковых заливок, контуров и эффектов".into();
            return;
        }
        if self.doc.clip_links.iter().any(|link|
            [id_a,id_b].contains(&link.source_id) ||
            [id_a,id_b].contains(&link.target_id)){
            self.status="Сначала удалите связи обтравки для соединяемых контуров".into();
            return;
        }
        let (joined,modes,report)=match opencorel_core::path_topology::join_beziers(
            &a.shape,&a.node_modes,&b.shape,&b.node_modes,mode){
            Ok(result)=>result,
            Err(reason)=>{
                self.status=format!("Соединение отклонено: {reason}");
                return;
            }
        };
        self.checkpoint();
        if let Some(object)=self.doc.get_mut(id_a){
            object.shape=joined;
            object.node_modes=modes;
        }
        self.doc.remove(id_b);
        self.selected=Some(id_a);
        self.multi_selected.clear();
        self.active_node=None;self.node_drag=None;
        self.node_drag_checkpointed=false;
        self.fill_cache.entries.clear();self.vector_cache.clear();
        self.compound_cache.clear();self.bezier_preview.clear();
        self.status=format!(
            "Две кривые соединены. Расстояние между концами: {:.2} мм{}; Ctrl+Z — отмена",
            report.gap_mm,if report.added_connector{", добавлен соединительный сегмент"}else{""});
    }

    /// Split an open path at an interior selected anchor, keeping the original
    /// as the first half and creating a new editable object immediately above
    /// it in z-order. Copy appearance, style and the appropriate node modes.
    pub(super) fn split_selected_bezier(&mut self){
        let Some((id,address))=self.active_node else{
            self.status="Выберите внутренний узел открытой кривой (F10)".into();
            return;
        };
        if !self.node_edit_allowed(id){
            self.status="Разрезание недоступно: объект или слой заблокирован".into();
            return;
        }
        if self.doc.clip_links.iter().any(|link|
            link.source_id==id || link.target_id==id){
            self.status="Сначала удалите связи обтравки с кривой".into();
            return;
        }
        if self.doc.objects.len()>=100_000 || self.doc.next_id>=u64::MAX-1{
            self.status="Достигнут лимит объектов или идентификаторов".into();
            return;
        }
        let Some(original)=self.doc.objects.iter().find(|o|o.id==id).cloned() else{return;};
        let ((left,left_modes),(right,right_modes))=
            match opencorel_core::path_topology::split_bezier(
                &original.shape,&original.node_modes,address){
                Ok(result)=>result,
                Err(reason)=>{
                    self.status=format!("Разрезание отклонено: {reason}");
                    return;
                }
            };
        self.checkpoint();
        if let Some(obj)=self.doc.get_mut(id){
            obj.shape=left;
            obj.node_modes=left_modes;
        }
        let new_id=self.doc.add_to_layer(right,original.style.clone(),original.layer_id);
        if let Some(obj)=self.doc.get_mut(new_id){
            obj.text_format=original.text_format.clone();
            obj.node_modes=right_modes;
            // A fresh name avoids duplicate object labels and oversized names.
            obj.name=String::new();
        }
        if let Some(appearance)=self.doc.appearances.get(&id).cloned(){
            self.doc.appearances.insert(new_id,appearance);
        }
        // New segment should appear immediately after its original sibling.
        if let Some(created)=self.doc.objects.pop(){
            if let Some(index)=self.doc.objects.iter().position(|o|o.id==id){
                self.doc.objects.insert(index+1,created);
            }else{
                self.doc.objects.push(created);
            }
        }
        self.selected=Some(new_id);
        self.multi_selected.clear();
        self.active_node=None;self.node_drag=None;
        self.node_drag_checkpointed=false;
        self.fill_cache.entries.clear();self.vector_cache.clear();
        self.compound_cache.clear();self.bezier_preview.clear();
        self.status="Контур разделён на две независимые кривые. Ctrl+Z восстановит исходный".into();
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
    #[test]
    fn continuing_existing_bezier_is_single_undo_step(){
        let mut e=Editor::default();
        let id=e.doc.add(topology_curve(0.,10.,20.),Style::default());
        e.selected=Some(id);
        e.tool=Tool::Nodes;
        let before=e.doc.clone();
        e.continue_selected_bezier();
        assert!(e.tool==Tool::Curve);
        assert_eq!(e.curve_draft,Some(id));
        assert!(e.curve_resumed);
        assert!(e.undo.is_empty(),"Entering continue mode is not a document change");
        e.curve_click(Point{x:30.,y:10.});
        e.curve_click(Point{x:40.,y:10.});
        assert!(!e.curve_resumed);
        assert_eq!(e.undo.len(),1,"Two appended nodes are one gesture in undo");
        let Shape::Bezier{segments,..}=&e.doc.get_mut(id).unwrap().shape
            else{panic!()};
        assert_eq!(segments.len(),4);
        e.curve_finish(false);
        assert!(e.tool==Tool::Select);
        e.doc.validate().unwrap();
        e.undo();
        assert_eq!(e.doc,before);
        e.redo();
        let Shape::Bezier{segments,..}=&e.doc.get_mut(id).unwrap().shape
            else{panic!()};
        assert_eq!(segments.len(),4);
    }
    #[test]
    fn resume_tangent_from_existing_last_anchor_creates_no_zero_segment(){
        let mut e=Editor::default();
        let id=e.doc.add(topology_curve(0.,10.,20.),Style::default());
        e.selected=Some(id);
        e.tool=Tool::Nodes;
        let before=e.doc.clone();
        e.continue_selected_bezier();
        let last=Point{x:20.,y:10.};
        e.curve_click(last);
        assert!(e.undo.is_empty());
        let Shape::Bezier{segments,..}=&e.doc.get_mut(id).unwrap().shape
            else{panic!()};
        assert_eq!(segments.len(),2);
        e.curve_drag_anchor=Some(last);
        e.curve_drag_to(Point{x:23.,y:15.});
        assert_eq!(e.undo.len(),1);
        assert!(!e.curve_resumed);
        assert_eq!(e.curve_outgoing,Some(Point{x:23.,y:15.}));
        let Shape::Bezier{segments,..}=&e.doc.get_mut(id).unwrap().shape
            else{panic!()};
        assert_eq!(segments.len(),2);
        assert_eq!(segments[1].control2,Point{x:17.,y:5.});
        e.curve_drag_anchor=None;
        e.curve_click(Point{x:30.,y:10.});
        assert_eq!(e.undo.len(),1);
        e.curve_finish(false);
        e.doc.validate().unwrap();
        e.undo();
        assert_eq!(e.doc,before);
    }
    #[test]
    fn continuing_path_without_changes_does_not_create_undo(){
        let mut e=Editor::default();
        let id=e.doc.add(topology_curve(0.,10.,20.),Style::default());
        e.selected=Some(id);
        e.tool=Tool::Nodes;
        let before=e.doc.clone();
        e.continue_selected_bezier();
        e.curve_finish(false);
        assert_eq!(e.doc,before);
        assert!(e.undo.is_empty());
        assert!(!e.curve_resumed);
    }
    #[test]
    fn resuming_then_closing_uses_one_undo_step(){
        let mut e=Editor::default();
        let id=e.doc.add(topology_curve(0.,10.,20.),Style::default());
        e.selected=Some(id);
        e.tool=Tool::Nodes;
        let before=e.doc.clone();
        e.continue_selected_bezier();
        e.curve_finish(true);
        assert_eq!(e.undo.len(),1);
        let Shape::Bezier{closed,..}=&e.doc.get_mut(id).unwrap().shape
            else{panic!()};
        assert!(*closed);
        e.undo();
        assert_eq!(e.doc,before);
    }
    #[test]
    fn continuing_rejects_closed_path_and_locked_layer(){
        let mut e=Editor::default();
        let id=e.doc.add(topology_curve(0.,10.,20.),Style::default());
        e.selected=Some(id);
        e.tool=Tool::Nodes;
        if let Shape::Bezier{closed,..}=&mut e.doc.get_mut(id).unwrap().shape{
            *closed=true;
        }
        let old=e.doc.clone();
        e.continue_selected_bezier();
        assert!(e.tool==Tool::Nodes);
        assert_eq!(e.doc,old);
        assert!(!e.curve_resumed);
        if let Shape::Bezier{closed,..}=&mut e.doc.get_mut(id).unwrap().shape{
            *closed=false;
        }
        e.doc.layers[0].locked=true;
        let old=e.doc.clone();
        e.continue_selected_bezier();
        assert!(e.tool==Tool::Nodes);
        assert_eq!(e.doc,old);
        assert!(e.undo.is_empty());
    }

    fn topology_curve(x0:f32,x1:f32,x2:f32)->Shape{
        let segment=|a:f32,b:f32|opencorel_core::CubicSegment{
            control1:Point{x:a+(b-a)/3.,y:10.},
            control2:Point{x:a+2.*(b-a)/3.,y:10.},
            end:Point{x:b,y:10.},
        };
        Shape::Bezier{start:Point{x:x0,y:10.},
            segments:vec![segment(x0,x1),segment(x1,x2)],closed:false}
    }
    #[test]
    fn join_near_endpoints_is_one_undo_step_and_preserves_native_shape(){
        use opencorel_core::path_topology::JoinMode;
        let mut e=Editor::default();
        let style=Style::default();
        let a=e.doc.add(topology_curve(0.,10.,20.),style.clone());
        let b=e.doc.add(topology_curve(20.5,30.,40.),style);
        e.select_object(Some(a),false);
        e.select_object(Some(b),true);
        e.tool=Tool::Nodes;
        let previous=e.doc.clone();
        e.join_selected_beziers(JoinMode::Snap{tolerance_mm:2.});
        assert_eq!(e.doc.objects.len(),previous.objects.len()-1);
        assert_eq!(e.selected,Some(a));
        assert!(e.multi_selected.is_empty());
        assert_eq!(e.undo.len(),1);
        let Shape::Bezier{segments,..}=&e.doc.get_mut(a).unwrap().shape
            else{panic!("native curve")};
        assert_eq!(segments.len(),4);
        e.doc.validate().unwrap();
        e.undo();
        assert_eq!(e.doc,previous);
        e.redo();
        e.doc.validate().unwrap();
        assert!(e.doc.objects.iter().all(|obj|obj.id!=b));
    }
    #[test]
    fn bridge_join_preserves_both_paths_with_one_connector(){
        use opencorel_core::path_topology::JoinMode;
        let mut e=Editor::default();
        let a=e.doc.add(topology_curve(0.,10.,20.),Style::default());
        let b=e.doc.add(topology_curve(100.,110.,120.),Style::default());
        e.selected=Some(a);
        e.multi_selected=vec![b];
        e.tool=Tool::Nodes;
        e.join_selected_beziers(JoinMode::Bridge);
        assert_eq!(e.undo.len(),1);
        let Shape::Bezier{segments,..}=&e.doc.get_mut(a).unwrap().shape
            else{panic!("native curve")};
        assert_eq!(segments.len(),5);
        assert_eq!(segments[2].end,Point{x:100.,y:10.});
        e.doc.validate().unwrap();
    }
    #[test]
    fn incompatible_join_is_rejected_without_history_or_modification(){
        use opencorel_core::path_topology::JoinMode;
        let mut e=Editor::default();
        let a=e.doc.add(topology_curve(0.,10.,20.),Style::default());
        let b=e.doc.add(topology_curve(100.,110.,120.),Style::default());
        e.selected=Some(a);
        e.multi_selected=vec![b];
        let old=e.doc.clone();
        e.join_selected_beziers(JoinMode::Snap{tolerance_mm:2.});
        assert_eq!(e.doc,old);
        assert!(e.undo.is_empty());
        e.doc.get_mut(b).unwrap().style.fill="#FF0000".into();
        let old=e.doc.clone();
        e.join_selected_beziers(JoinMode::Bridge);
        assert_eq!(e.doc,old);
        assert!(e.undo.is_empty());
        e.doc.get_mut(b).unwrap().style.fill="#447BD4".into();
        e.doc.layers[0].locked=true;
        let old=e.doc.clone();
        e.join_selected_beziers(JoinMode::Bridge);
        assert_eq!(e.doc,old);
        assert!(e.undo.is_empty());
    }
    #[test]
    fn split_at_internal_anchor_creates_two_editable_siblings(){
        use opencorel_core::node_edit::{NodeModeBinding,TangentOperation};
        let mut e=Editor::default();
        let id=e.doc.add(Shape::Bezier{
            start:Point{x:0.,y:0.},segments:vec![
                CubicSegment{control1:Point{x:2.,y:0.},
                    control2:Point{x:8.,y:0.},end:Point{x:10.,y:0.}},
                CubicSegment{control1:Point{x:12.,y:0.},
                    control2:Point{x:18.,y:0.},end:Point{x:20.,y:0.}},
                CubicSegment{control1:Point{x:22.,y:0.},
                    control2:Point{x:28.,y:0.},end:Point{x:30.,y:0.}},
            ],closed:false,
        },Style::default());
        e.doc.get_mut(id).unwrap().node_modes=vec![
            NodeModeBinding{address:NodeAddress::BezierEnd(1),
                mode:TangentOperation::Smooth},
        ];
        e.doc.validate().unwrap();
        e.selected=Some(id);
        e.tool=Tool::Nodes;
        e.active_node=Some((id,NodeAddress::BezierEnd(0)));
        let old=e.doc.clone();
        e.split_selected_bezier();
        assert_eq!(e.undo.len(),1);
        assert_eq!(e.doc.objects.len(),old.objects.len()+1);
        let new_id=e.selected.unwrap();
        assert_ne!(new_id,id);
        let Shape::Bezier{segments:left,..}=&e.doc.get_mut(id).unwrap().shape
            else{panic!("left curve")};
        assert_eq!(left.len(),1);
        let right=e.doc.get_mut(new_id).unwrap();
        let Shape::Bezier{start,segments,..}=&right.shape else{panic!("right curve")};
        assert_eq!(*start,Point{x:10.,y:0.});
        assert_eq!(segments.len(),2);
        assert_eq!(right.node_modes.len(),1);
        assert_eq!(right.node_modes[0].address,NodeAddress::BezierEnd(0));
        e.doc.validate().unwrap();
        e.undo();
        assert_eq!(e.doc,old);
        e.redo();
        e.doc.validate().unwrap();
    }
    #[test]
    fn split_refuses_closed_curve_and_clipped_object(){
        let mut e=Editor::default();
        let id=e.doc.add(topology_curve(0.,10.,20.),Style::default());
        e.selected=Some(id);
        e.active_node=Some((id,NodeAddress::BezierEnd(0)));
        e.tool=Tool::Nodes;
        if let Shape::Bezier{closed,..}=&mut e.doc.get_mut(id).unwrap().shape{
            *closed=true;
        }
        let old=e.doc.clone();
        e.split_selected_bezier();
        assert_eq!(e.doc,old);
        assert!(e.undo.is_empty());
        if let Shape::Bezier{closed,..}=&mut e.doc.get_mut(id).unwrap().shape{
            *closed=false;
        }
        e.doc.clip_links.push(opencorel_core::ClipLink{source_id:id,target_id:1});
        let old=e.doc.clone();
        e.split_selected_bezier();
        assert_eq!(e.doc,old);
        assert!(e.undo.is_empty());
    }

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
    #[test] fn free_mode_preserves_path_and_undo_restores_coupling(){
        let mut editor=Editor::default();
        let id=editor.doc.add(Shape::Bezier{
            start:Point{x:0.,y:0.},segments:vec![
                CubicSegment{control1:Point{x:3.,y:4.},
                    control2:Point{x:8.,y:7.},end:Point{x:10.,y:10.}},
                CubicSegment{control1:Point{x:13.,y:18.},
                    control2:Point{x:20.,y:18.},end:Point{x:30.,y:10.}},
            ],closed:false},Style::default());
        editor.selected=Some(id);
        editor.tool=Tool::Nodes;
        editor.active_node=Some((id,NodeAddress::BezierEnd(0)));
        editor.apply_tangent_operation(node_edit::TangentOperation::Smooth);
        let shape_before=editor.doc.get_mut(id).unwrap().shape.clone();
        assert_eq!(editor.undo.len(),1);
        editor.unlink_active_tangents();
        assert!(editor.doc.get_mut(id).unwrap().node_modes.is_empty());
        assert_eq!(editor.doc.get_mut(id).unwrap().shape,shape_before);
        assert_eq!(editor.undo.len(),2);
        editor.unlink_active_tangents();
        assert_eq!(editor.undo.len(),2);
        editor.undo();
        assert_eq!(node_edit::mode_at(&editor.doc.get_mut(id).unwrap().node_modes,
            NodeAddress::BezierEnd(0)),Some(node_edit::TangentOperation::Smooth));
        assert_eq!(editor.doc.get_mut(id).unwrap().shape,shape_before);
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
