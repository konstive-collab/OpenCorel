//! Device-oriented output UI. No serial, USB or network device commands are
//! emitted: the user must validate the generated PLT in a device dry-run.
use super::Editor;
use eframe::egui::{self,DragValue};
use opencorel_export::hpgl::{Origin,Scope};
impl Editor{
    pub(super) fn device_menu(&mut self,ui:&mut egui::Ui){
        ui.heading("Плоттер / резка — HP-GL/1");
        ui.label("Экспорт PLT для совместимых плоттеров.");
        ui.small("OpenCorel не подключается к оборудованию и не запускает резку.");
        ui.separator();
        ui.horizontal(|ui|{
            ui.label("Перо (1–8)");
            ui.add(DragValue::new(&mut self.plotter_options.pen).range(1..=8));
            ui.label("Единиц/мм");
            ui.add(DragValue::new(&mut self.plotter_options.units_per_mm)
                .range(1..=1000));
        });
        ui.horizontal(|ui|{
            ui.label("Допуск кривой, мм");
            ui.add(DragValue::new(&mut self.plotter_options.tolerance_mm)
                .speed(0.01).range(0.01..=2.0));
        });
        ui.horizontal(|ui|{
            ui.label("Ноль координат:");
            ui.selectable_value(&mut self.plotter_options.origin,
                Origin::BottomLeft,"Снизу слева");
            ui.selectable_value(&mut self.plotter_options.origin,
                Origin::TopLeft,"Сверху слева");
        });
        ui.separator();
        ui.label("Какие контуры отправлять:");
        ui.selectable_value(&mut self.plotter_options.scope,
            Scope::StrokedOnly,"Только объекты с обводкой");
        ui.selectable_value(&mut self.plotter_options.scope,
            Scope::AllVectorOutlines,"Все векторные контуры");
        ui.horizontal(|ui|{
            ui.label("Смещение X, мм:");
            ui.add(DragValue::new(&mut self.plotter_options.offset_x_mm)
                .speed(1.).range(0.0..=5000.0));
            ui.label("Y:");
            ui.add(DragValue::new(&mut self.plotter_options.offset_y_mm)
                .speed(1.).range(0.0..=5000.0));
        });
        ui.separator();
        if ui.button("Проверить режущие траектории").clicked(){
            self.status=match opencorel_export::hpgl::analyze(
                &self.doc,self.plotter_options){
                Ok(report)=>format!(
                    "PLT: {} контуров, {} вершин, {:.1} мм реза, {:.1} мм холостого хода; {} без обводки пропущено",
                    report.path_count,report.vertex_count,
                    report.cut_length_mm,report.travel_length_mm,
                    report.skipped_unstroked),
                Err(error)=>format!("Ошибка подготовки плоттера: {error}")
            };
            ui.close_menu();
        }
        if ui.button("Экспортировать .PLT...").clicked(){
            self.export_dialog("plt");
            ui.close_menu();
        }
        ui.small("Подготовьте текст в кривые. Обтравки и растр не режутся.");
        ui.small("Перед реальной резкой проверьте начало координат и масштаб на оборудовании.");
    }
}
