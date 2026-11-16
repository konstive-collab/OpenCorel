//! Freehand mouse-gesture/controller. One stroke creates one native Bézier
//! object and exactly one history snapshot, only after geometry is validated.
//! The live preview is a lightweight polyline; no fitting runs each frame.
use super::{Editor,Tool};
use opencorel_core::{Point,Shape,Style};
use opencorel_core::freehand;

const MIN_POINTER_STEP_PX:f32=1.0;

impl Editor{
    pub(super) fn freehand_begin(&mut self,p:Point){
        self.freehand_samples.clear();
        self.freehand_overflow=false;
        if self.tool!=Tool::Freehand || !self.active_layer_editable(){
            self.status="Нельзя рисовать на скрытом или заблокированном слое".into();
            return;
        }
        if !p.x.is_finite()||!p.y.is_finite()||
           p.x.abs()>100_000.0||p.y.abs()>100_000.0{
            self.status="Координаты штриха выходят за допустимые пределы".into();
            return;
        }
        self.freehand_samples.push(p);
    }
    pub(super) fn freehand_extend(&mut self,p:Point,scale:f32){
        if self.freehand_samples.is_empty()||self.freehand_overflow{return;}
        if !p.x.is_finite()||!p.y.is_finite()||
           p.x.abs()>100_000.0||p.y.abs()>100_000.0{
            self.freehand_overflow=true;
            return;
        }
        let last=*self.freehand_samples.last().unwrap();
        let delta=((p.x as f64-last.x as f64).powi(2)+
            (p.y as f64-last.y as f64).powi(2)).sqrt();
        let min_step=(MIN_POINTER_STEP_PX/scale.max(0.001)).clamp(0.001,100.0);
        if delta<min_step as f64{return;}
        if self.freehand_samples.len()>=freehand::MAX_SAMPLES{
            self.freehand_overflow=true;
            return;
        }
        self.freehand_samples.push(p);
    }
    pub(super) fn freehand_finish(&mut self,p:Option<Point>,scale:f32){
        if self.freehand_samples.is_empty(){return;}
        if let Some(p)=p{
            self.freehand_extend(p,scale);
        }
        let samples=std::mem::take(&mut self.freehand_samples);
        let overflow=std::mem::replace(&mut self.freehand_overflow,false);
        if overflow{
            self.status="Штрих слишком длинный (4096 точек): разделите его на несколько жестов".into();
            return;
        }
        if !self.active_layer_editable(){
            self.status="Слой заблокирован: штрих не был добавлен".into();
            return;
        }
        let tolerance=(self.freehand_precision_px/scale.max(0.001))
            .clamp(0.0,100.0);
        match freehand::fit_stroke(&samples,tolerance,self.freehand_smoothness){
            Ok(shape)=>{
                let Shape::Bezier{segments,..}=&shape else{unreachable!()};
                let count=segments.len();
                let style=Style{
                    fill:"none".into(),stroke:"#263A4E".into(),
                    stroke_width_mm:0.8,
                };
                if self.add_shape(shape,style).is_some(){
                    self.bezier_preview.clear();
                    self.status=format!("Штрих готов: {} отсчётов → {} редактируемых сегментов Безье. F10 — узлы; Ctrl+Z — отмена",
                        samples.len(),count);
                }
            },
            Err(reason)=>{
                self.status=format!("Свободное рисование: {reason}");
            },
        }
    }
    /// Refits a selected open polyline/Bézier at user-specified precision.
    /// This changes topology, so persisted individual node modes are removed.
    /// Refuse silent growth of already editable cubic geometry when the
    /// explicit command was to simplify, rather than convert.
    pub(super) fn refit_selected_path(&mut self,convert_polyline:bool){
        let Some(id)=self.selected else{
            self.status="Сначала выделите открытый векторный контур".into();
            return;
        };
        if !self.multi_selected.is_empty() || !self.node_edit_allowed(id){
            self.status="Нужен один незаблокированный несгруппированный контур".into();
            return;
        }
        let Some(obj)=self.doc.objects.iter().find(|o|o.id==id) else{return;};
        match (&obj.shape,convert_polyline){
            (Shape::Polyline{closed:false,..},true)|
            (Shape::Bezier{closed:false,..},false)=>{},
            _=>{
                self.status=if convert_polyline{
                    "Выберите открытую ломаную для преобразования в гладкую кривую"
                }else{
                    "Выберите открытую кривую Безье для уменьшения числа узлов"
                }.into();
                return;
            }
        }
        let old_count=match &obj.shape{
            Shape::Bezier{segments,..}=>segments.len(),
            Shape::Polyline{points,..}=>points.len().saturating_sub(1),
            _=>0,
        };
        let new_shape=match freehand::refit_open_path(
            &obj.shape,self.refit_tolerance_mm,self.freehand_smoothness){
            Ok(shape)=>shape,
            Err(reason)=>{
                self.status=format!("Переработка контура отклонена: {reason}");
                return;
            }
        };
        let Shape::Bezier{segments,..}=&new_shape else{unreachable!()};
        let new_count=segments.len();
        if !convert_polyline && new_count>=old_count{
            self.status=format!("Упрощение не уменьшает число сегментов ({} → {}). Увеличьте допуск",
                old_count,new_count);
            return;
        }
        self.checkpoint();
        if let Some(obj)=self.doc.get_mut(id){
            obj.shape=new_shape;
            obj.node_modes.clear();
        }
        self.active_node=None;
        self.compound_cache.clear();
        self.bezier_preview.clear();
        self.vector_cache.clear();
        self.fill_cache.entries.clear();
        self.status=format!("Контур переработан: {} → {} сегментов. Режимы отдельных узлов сброшены. Ctrl+Z — отмена",
            old_count,new_count);
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{CubicSegment,NodeModeBinding};
    #[test]
    fn one_freehand_gesture_has_one_checkpoint_and_editable_shape(){
        let mut e=Editor::default();
        e.doc=Default::default();
        e.tool=Tool::Freehand;
        e.freehand_precision_px=0.2;
        e.freehand_begin(Point{x:0.,y:0.});
        for i in 1..300{
            e.freehand_extend(Point{x:i as f32*0.2,
                y:(i as f32*0.04).sin()*5.},10.0);
        }
        assert!(e.undo.is_empty());
        e.freehand_finish(None,10.0);
        assert_eq!(e.undo.len(),1);
        assert_eq!(e.doc.objects.len(),1);
        assert!(matches!(e.doc.objects[0].shape,Shape::Bezier{closed:false,..}));
        assert_eq!(e.doc.objects[0].style.fill,"none");
        e.doc.validate().unwrap();
        e.undo();
        assert_eq!(e.doc.objects.len(),0);
        e.redo();
        assert_eq!(e.doc.objects.len(),1);
    }
    #[test]
    fn tap_and_locked_layer_do_not_create_documents(){
        let mut e=Editor::default();
        e.doc=Default::default();
        e.tool=Tool::Freehand;
        e.freehand_begin(Point{x:1.,y:1.});
        e.freehand_finish(None,10.0);
        assert!(e.undo.is_empty());
        assert!(e.doc.objects.is_empty());
        e.doc.layers[0].locked=true;
        e.freehand_begin(Point{x:1.,y:1.});
        e.freehand_extend(Point{x:20.,y:20.},10.0);
        e.freehand_finish(None,10.0);
        assert!(e.doc.objects.is_empty());
    }
    #[test]
    fn long_stroke_limit_does_not_truncate_and_publish_partial_shape(){
        let mut e=Editor::default();
        e.doc=Default::default();
        e.tool=Tool::Freehand;
        e.freehand_begin(Point{x:0.,y:0.});
        for i in 1..=freehand::MAX_SAMPLES+4{
            e.freehand_extend(Point{x:i as f32*0.1,y:i as f32*0.05},20.);
        }
        assert!(e.freehand_overflow);
        e.freehand_finish(None,20.);
        assert!(e.undo.is_empty());
        assert!(e.doc.objects.is_empty());
    }
    #[test]
    fn refit_polyline_then_simplify_cubic_preserves_undo(){
        let mut e=Editor::default();
        e.doc=Default::default();
        let mut points=Vec::new();
        for i in 0..80{
            let x=i as f32;
            points.push(Point{x,y:(x*0.12).sin()*10.});
        }
        let id=e.doc.add(Shape::Polyline{points,closed:false},
            Style::default());
        e.selected=Some(id);e.tool=Tool::Nodes;
        let old=e.doc.clone();
        e.refit_tolerance_mm=0.4;
        e.refit_selected_path(true);
        assert_eq!(e.undo.len(),1);
        assert!(matches!(e.doc.get_mut(id).unwrap().shape,Shape::Bezier{..}));
        e.refit_tolerance_mm=5.0;
        e.refit_selected_path(false);
        assert_eq!(e.undo.len(),2);
        e.doc.validate().unwrap();
        e.undo();
        e.undo();
        assert_eq!(e.doc,old);
    }
    #[test]
    fn refit_declines_closed_and_locked_without_undo(){
        let mut e=Editor::default();
        e.doc=Default::default();
        let id=e.doc.add(Shape::Polyline{
            points:vec![Point{x:0.,y:0.},Point{x:1.,y:5.},
                Point{x:2.,y:0.}],closed:true},Style::default());
        e.selected=Some(id);e.tool=Tool::Nodes;
        let old=e.doc.clone();
        e.refit_selected_path(true);
        assert_eq!(e.doc,old);assert!(e.undo.is_empty());
        if let Shape::Polyline{closed,..}=&mut e.doc.get_mut(id).unwrap().shape{
            *closed=false;
        }
        e.doc.layers[0].locked=true;
        let old=e.doc.clone();
        e.refit_selected_path(true);
        assert_eq!(e.doc,old);assert!(e.undo.is_empty());
    }
}
