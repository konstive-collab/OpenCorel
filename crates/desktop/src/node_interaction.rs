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
        match node_edit::move_node(&mut obj.shape,address,to){
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
        if node_edit::move_node(&mut updated,address,to).is_ok(){
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
