//! Geometry inspection and path-array tools.
use super::Editor;
use eframe::egui;
use opencorel_core::path_array::{PathArraySettings,Spacing};
use opencorel_core::path_measure::measure_shape;

#[derive(Default)]
pub(super) struct GeometryPanel{
    settings:PathArraySettings,
    fixed_step:bool,
    count:u16,
    step:f32,
    reversed:bool,
    report:String,
}
impl Editor{
    fn apply_path_array(&mut self,guide:u64,motif:u64,settings:PathArraySettings){
        let mut draft=self.doc.clone();
        match draft.repeat_along_path(guide,motif,settings){
            Ok(ids)=>{
                self.checkpoint();self.doc=draft;
                self.selected=ids.first().copied();
                self.multi_selected=ids.into_iter().skip(1).collect();
                self.active_node=None;self.node_drag=None;
                self.fill_cache.entries.clear();self.compound_cache.clear();
                self.status="Массив вдоль контура создан. Ctrl+Z — отмена.".into();
            }
            Err(error)=>self.status=format!("Массив: {error}"),
        }
    }
    pub(super) fn geometry_docker(&mut self,ui:&mut egui::Ui){
        ui.heading("Геометрия");
        let ids=self.selection_ids();
        if ids.is_empty(){ui.label("Выберите векторный объект.");return;}
        if ids.len()==1{
            let Some(o)=self.doc.get(ids[0]) else{return;};
            let b=o.bounds();
            ui.label(format!("Рамка: {:.3} × {:.3} мм",b.w,b.h));
            ui.label(format!("Положение: {:.3}, {:.3} мм",b.x,b.y));
            ui.add(egui::DragValue::new(&mut self.geometry_panel.settings.tolerance_mm)
                .range(0.001..=10.).speed(0.01).prefix("Допуск: ").suffix(" мм"));
            if ui.button("Измерить контур").clicked(){
                self.geometry_panel.report=match measure_shape(&o.shape,
                    f64::from(self.geometry_panel.settings.tolerance_mm)){
                    Ok(contours)=>{
                        let total: f64=contours.iter().map(|c|c.length_mm()).sum();
                        let closed=contours.iter().filter(|c|c.closed).count();
                        let mut text=format!("Длина: {total:.3} мм\nКонтуров: {} (замкнутых: {closed})",contours.len());
                        if contours.len()==1{
                            if let Some(area)=contours[0].signed_area_mm2(){
                                text.push_str(&format!("\nМодуль ориентированной площади: {:.3} мм²",area.abs()));
                            }
                        }
                        text
                    }
                    Err(error)=>error,
                };
            }
            ui.small("Для самопересекающихся кривых ориентированная площадь не равна площади заливки.");
            if !self.geometry_panel.report.is_empty(){ui.label(&self.geometry_panel.report);}
            return;
        }
        if ids.len()!=2{ui.label("Для массива выберите ровно два объекта.");return;}
        ui.heading("Массив вдоль пути");
        ui.small("Первый выделенный объект — направляющая, второй — образец.");
        ui.checkbox(&mut self.geometry_panel.reversed,"Поменять роли объектов");
        let (guide,motif)=if self.geometry_panel.reversed{(ids[1],ids[0])}else{(ids[0],ids[1])};
        ui.label(format!("Направляющая #{guide} • образец #{motif}"));
        let panel=&mut self.geometry_panel;
        if panel.count==0{panel.count=10;}
        if panel.step<=0.{panel.step=10.;}
        ui.checkbox(&mut panel.fixed_step,"Фиксированный шаг");
        if panel.fixed_step{
            ui.add(egui::DragValue::new(&mut panel.step).range(0.01..=100_000.)
                .speed(0.5).prefix("Шаг: ").suffix(" мм"));
            panel.settings.spacing=Spacing::StepMm(panel.step);
        }else{
            ui.add(egui::Slider::new(&mut panel.count,1..=1000).text("Количество"));
            panel.settings.spacing=Spacing::Count(panel.count);
        }
        let s=&mut panel.settings;
        for (label,value) in [("Начальный отступ",&mut s.start_offset_mm),
            ("Конечный отступ",&mut s.end_offset_mm)]{
            ui.add(egui::DragValue::new(value).range(0.0..=100_000.).speed(0.5)
                .prefix(format!("{label}: ")).suffix(" мм"));
        }
        ui.add(egui::DragValue::new(&mut s.normal_offset_mm).range(-100_000.0..=100_000.)
            .speed(0.5).prefix("Поперечный отступ: ").suffix(" мм"));
        ui.checkbox(&mut s.rotate_to_tangent,"Поворот по касательной");
        ui.add(egui::Slider::new(&mut s.angle_offset_deg,-360.0..=360.).text("Доп. поворот °"));
        ui.add(egui::DragValue::new(&mut s.start_scale).range(0.01..=100.)
            .speed(0.05).prefix("Начальный масштаб ×"));
        ui.add(egui::DragValue::new(&mut s.end_scale).range(0.01..=100.)
            .speed(0.05).prefix("Конечный масштаб ×"));
        ui.add(egui::DragValue::new(&mut s.tolerance_mm).range(0.001..=10.)
            .speed(0.01).prefix("Допуск: ").suffix(" мм"));
        let settings=*s;
        if ui.button("Создать массив").clicked(){self.apply_path_array(guide,motif,settings);}
        ui.small("Исходные объекты сохраняются. Копии остаются обычными редактируемыми векторами.");
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Shape,Style,Point};
    #[test] fn array_is_one_undoable_operation(){
        let mut e=Editor::default();e.doc=Default::default();
        let a=e.doc.add(Shape::Polyline{points:vec![Point{x:0.,y:0.},Point{x:100.,y:0.}],closed:false},Style::default());
        let b=e.doc.add(Shape::Rectangle{x:10.,y:10.,w:5.,h:5.},Style::default());
        let before=e.doc.clone();
        e.apply_path_array(a,b,PathArraySettings{spacing:Spacing::Count(4),..Default::default()});
        assert_eq!(e.doc.objects.len(),6);
        e.undo();assert_eq!(e.doc,before);
        e.redo();assert_eq!(e.doc.objects.len(),6);
    }
}
