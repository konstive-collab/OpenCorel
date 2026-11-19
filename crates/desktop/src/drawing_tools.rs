//! Pointer-driven freehand, window/crossing selection, style brush.
//! Native strokes and selection IDs integrate with KDR / SVG / PDF / Undo.
use super::{Editor,Tool,View};
use eframe::egui::{self,Color32,Pos2,Stroke,Rect};
use opencorel_core::{Bounds,Point,Shape,Style};
use opencorel_core::drawing::simplify_stroke;
use opencorel_core::style_tools::StyleSample;
use std::collections::HashSet;

impl Editor{
    pub(super) fn begin_marquee(&mut self,p:Point){
        self.marquee_start=Some(p);
        self.marquee_end=Some(p);
    }
    pub(super) fn update_marquee(&mut self,p:Point){
        if self.marquee_start.is_some(){self.marquee_end=Some(p);}
    }
    pub(super) fn finish_marquee(&mut self,additive:bool){
        let Some(start)=self.marquee_start.take() else{return;};
        let end=self.marquee_end.take().unwrap_or(start);
        let bounds=Bounds{x:start.x.min(end.x),y:start.y.min(end.y),
            w:(start.x-end.x).abs(),h:(start.y-end.y).abs()};
        if bounds.w<0.4&&bounds.h<0.4{return;}
        let ids=self.doc.marquee_select(bounds,end.x<start.x);
        let mut selected=if additive{self.selection_ids()}else{Vec::new()};
        for id in ids{if !selected.contains(&id){selected.push(id);}}
        self.selected=selected.first().copied();
        self.multi_selected=selected.into_iter().skip(1).collect();
        self.status=format!("Рамка: выбрано {} объектов; {}",
            self.selection_ids().len(),
            if end.x<start.x{"пересечение"}else{"внутри рамки"});
    }
    pub(super) fn begin_freehand(&mut self,p:Point){
        self.freehand_points.clear();
        if self.active_layer_editable(){self.freehand_points.push(p);}
        else{self.status="Нельзя рисовать на заблокированном слое".into();}
    }
    pub(super) fn extend_freehand(&mut self,p:Point){
        let should_add=self.freehand_points.last().is_some_and(|last|
            (last.x-p.x).hypot(last.y-p.y)>0.12);
        if should_add&&self.freehand_points.len()<30_000{self.freehand_points.push(p);}
    }
    pub(super) fn finish_freehand(&mut self){
        let raw=std::mem::take(&mut self.freehand_points);
        if raw.len()<2{return;}
        let points=match simplify_stroke(&raw,self.freehand_smoothing_mm){
            Ok(points)=>points,
            Err(error)=>{self.status=format!("Карандаш: {error}");return;}
        };
        if points.len()<2{return;}
        if !opencorel_core::valid_color(&self.freehand_color){
            self.status="Недопустимый цвет линии (используйте #RRGGBB)".into();
            return;
        }
        let width=self.freehand_width_mm;
        if !width.is_finite()||!(0.05..=100.0).contains(&width){return;}
        let count=points.len();
        let shape=Shape::Polyline{points,closed:false};
        let style=Style{fill:"none".into(),stroke:self.freehand_color.clone(),
            stroke_width_mm:width};
        if let Some(id)=self.add_shape(shape,style){
            if let Some(object)=self.doc.get_mut(id){object.name="Свободная линия".into();}
            self.status=format!("Карандаш: {count} узлов из {} точек; F10 — редактировать",raw.len());
        }
    }
    pub(super) fn capture_style(&mut self,id:u64){
        match self.doc.sample_style(id){
            Ok(style)=>{
                self.style_clipboard=Some(style);
                self.status="Пипетка: оформление захвачено. Выделите другой объект и вставьте стиль Ctrl+Shift+V".into();
            }
            Err(error)=>self.status=error,
        }
    }
    pub(super) fn copy_style_selected(&mut self){
        let Some(id)=self.selected else{
            self.status="Сначала выделите объект-образец".into();return;
        };
        self.capture_style(id);
    }
    pub(super) fn paste_style_selected(&mut self){
        let Some(style)=self.style_clipboard.clone() else{
            self.status="Буфер оформления пуст: выберите образец через пипетку или Ctrl+Shift+C".into();
            return;
        };
        let ids=self.selection_ids();
        if ids.is_empty(){
            self.status="Выделите целевые объекты для вставки оформления".into();
            return;
        }
        let mut copy=self.doc.clone();
        match copy.paste_style(&style,&ids).and_then(|changed|{
            copy.validate()?;
            Ok(changed)
        }){
            Ok(0)=>self.status="Оформление уже совпадает".into(),
            Ok(changed)=>{
                self.checkpoint();
                self.doc=copy;
                self.fill_cache.entries.clear();
                self.vector_cache.clear();
                self.compound_cache.clear();
                self.masked_canvas=Default::default();
                self.status=format!("Пипетка: изменено объектов {changed}; Ctrl+Z — отмена");
            },
            Err(e)=>self.status=format!("Пипетка: {e}")
        }
    }
    /// Select target objects first, then click a source with the eyedropper.
    /// Source is not modified. With no selection, just sample.
    pub(super) fn eyedropper_click(&mut self,p:Point){
        let Some(source)=self.doc.hit_test(p,2.0) else{
            self.status="Пипетка: щёлкните по объекту-образцу".into();return;
        };
        self.capture_style(source);
        let targets:Vec<_>=self.selection_ids().into_iter().filter(|id|*id!=source).collect();
        if !targets.is_empty(){
            let saved=self.selection_ids();
            self.selected=targets.first().copied();
            self.multi_selected=targets.into_iter().skip(1).collect();
            self.paste_style_selected();
            self.selected=saved.first().copied();
            self.multi_selected=saved.into_iter().skip(1).collect();
        }
        self.change_tool(Tool::Select);
    }
    pub(super) fn paint_drawing_overlays(&self,p:&egui::Painter,v:View){
        if let (Some(a),Some(b))=(self.marquee_start,self.marquee_end){
            let rect=Rect::from_two_pos(v.screen(a),v.screen(b));
            let crossing=b.x<a.x;
            let tone=if crossing{Color32::from_rgb(31,145,208)}
                else{Color32::from_rgb(15,170,125)};
            p.rect_filled(rect,0.0,Color32::from_rgba_premultiplied(
                tone.r(),tone.g(),tone.b(),35));
            p.rect_stroke(rect,0.0,Stroke::new(1.3,tone),egui::StrokeKind::Inside);
        }
        if self.freehand_points.len()>=2{
            p.add(egui::Shape::line(self.freehand_points.iter()
                .map(|pt|v.screen(*pt)).collect(),
                Stroke::new((self.freehand_width_mm*v.scale).max(1.0),
                    Color32::from_rgb(13,155,182))));
        }
    }
    pub(super) fn drawing_inspector(&mut self,ui:&mut egui::Ui){
        if self.tool==Tool::Freehand{
            ui.heading("Карандаш — свободная линия");
            ui.label("Проведите мышью по листу. Линия станет обычным редактируемым контуром.");
            ui.horizontal(|ui|{
                ui.label("Толщина, мм");
                ui.add(egui::DragValue::new(&mut self.freehand_width_mm)
                    .speed(0.1).range(0.05..=100.0));
            });
            ui.horizontal(|ui|{
                ui.label("Сглаживание, мм");
                ui.add(egui::DragValue::new(&mut self.freehand_smoothing_mm)
                    .speed(0.05).range(0.01..=10.0));
            });
            ui.horizontal(|ui|{
                ui.label("Цвет");
                ui.text_edit_singleline(&mut self.freehand_color);
            });
            ui.small("F10 редактирует точки после рисования, Ctrl+Z отменяет всю линию.");
        }else if self.tool==Tool::Eyedropper{
            ui.heading("Пипетка оформления");
            ui.label("Выделите целевые объекты, затем пипеткой щёлкните по образцу.");
            ui.label("Копируются заливка, обводка, прозрачность, градиент и формат текста.");
            ui.label("Также: Ctrl+Shift+C — взять стиль, Ctrl+Shift+V — применить.");
        }
        ui.separator();
        if ui.button("Копировать стиль выделения · Ctrl+Shift+C").clicked(){
            self.copy_style_selected();
        }
        if ui.add_enabled(self.style_clipboard.is_some()&&!self.selection_ids().is_empty(),
            egui::Button::new("Применить сохранённый стиль · Ctrl+Shift+V"))
            .clicked(){self.paste_style_selected();}
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn freehand_creates_editable_simplified_vector_and_undo(){
        let mut e=Editor::default();
        let count=e.doc.objects.len();
        e.begin_freehand(Point{x:10.,y:10.});
        for i in 1..80{
            e.extend_freehand(Point{x:10.+i as f32*0.5,y:10.});
        }
        e.finish_freehand();
        assert_eq!(e.doc.objects.len(),count+1);
        assert!(matches!(e.doc.objects.last().unwrap().shape,
            Shape::Polyline{closed:false,..}));
        e.doc.validate().unwrap();
        e.undo();
        assert_eq!(e.doc.objects.len(),count);
    }
    #[test] fn marquee_respects_crossing_and_additive_selection(){
        let mut e=Editor::default();
        let id=e.doc.objects[0].id;
        e.begin_marquee(Point{x:50.,y:100.});
        e.update_marquee(Point{x:500.,y:500.});
        e.finish_marquee(false);
        assert!(!e.is_selected(id)); // background extends past window
        e.begin_marquee(Point{x:500.,y:500.});
        e.update_marquee(Point{x:50.,y:100.});
        e.finish_marquee(false);
        assert!(e.is_selected(id));
    }
    #[test] fn sampled_style_paste_is_undoable(){
        let mut e=Editor::default();
        let source=e.doc.objects[0].id;
        let target=e.doc.objects[1].id;
        e.capture_style(source);
        e.selected=Some(target);
        let before=e.doc.clone();
        e.paste_style_selected();
        assert_eq!(e.doc.get(target).unwrap().style,e.doc.get(source).unwrap().style);
        e.undo();
        assert_eq!(e.doc,before);
    }
}
