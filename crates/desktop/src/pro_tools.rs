//! Professional procedural shapes and repeat patterns: native editable
//! geometry, full document validation, one Undo checkpoint per operation.
use super::Editor;
use eframe::egui::{self,DragValue};
use opencorel_core::{Bounds,Point,Shape,Style};
use opencorel_core::procedural::{self,Parameters,Preset};
use opencorel_core::arrays::RepeatPattern;

pub(super) struct ProToolsState{
    pub preset:Preset,
    pub width_mm:f32,pub height_mm:f32,
    pub detail:usize,pub ratio:f32,pub turns:f32,
    pub count:usize,pub dx_mm:f32,pub dy_mm:f32,
    pub columns:usize,pub rows:usize,pub sweep_degrees:f32,
}
impl Default for ProToolsState{
    fn default()->Self{
        Self{preset:Preset::Star,width_mm:100.,height_mm:100.,
            detail:5,ratio:0.45,turns:3.,
            count:8,dx_mm:110.,dy_mm:110.,
            columns:4,rows:3,sweep_degrees:360.}
    }
}
impl Editor{
    pub(super) fn procedural_menu(&mut self,ui:&mut egui::Ui){
        ui.heading("Параметрические фигуры");
        ui.small("Создаются как обычные редактируемые векторы (.kdr/.svg).");
        egui::ComboBox::from_id_salt("procedural_shape_preset")
            .selected_text(self.pro_tools.preset.label()).show_ui(ui,|ui|{
                for kind in Preset::ALL{
                    ui.selectable_value(&mut self.pro_tools.preset,kind,kind.label());
                }
            });
        ui.horizontal(|ui|{
            ui.label("Ширина, мм");
            ui.add(DragValue::new(&mut self.pro_tools.width_mm)
                .speed(1.0).range(1.0..=20_000.));
            ui.label("Высота, мм");
            ui.add(DragValue::new(&mut self.pro_tools.height_mm)
                .speed(1.0).range(1.0..=20_000.));
        });
        ui.horizontal(|ui|{
            ui.label("Лучей / зубцов / шагов");
            ui.add(DragValue::new(&mut self.pro_tools.detail).range(3..=36));
        });
        ui.horizontal(|ui|{
            ui.label("Внутренний радиус");
            ui.add(DragValue::new(&mut self.pro_tools.ratio)
                .speed(0.01).range(0.10..=0.90));
        });
        ui.horizontal(|ui|{
            ui.label("Оборотов / волн");
            ui.add(DragValue::new(&mut self.pro_tools.turns)
                .speed(0.1).range(1.0..=20.0));
        });
        if ui.button("Создать фигуру на странице").clicked(){
            let width=self.pro_tools.width_mm;
            let height=self.pro_tools.height_mm;
            let params=Parameters{
                bounds:Bounds{x:(self.doc.page.width_mm-width)*0.5,
                    y:(self.doc.page.height_mm-height)*0.5,w:width,h:height},
                detail:self.pro_tools.detail,ratio:self.pro_tools.ratio,
                turns:self.pro_tools.turns
            };
            match procedural::generate(self.pro_tools.preset,params){
                Ok(shape)=>{
                    let open=matches!(&shape,Shape::Polyline{closed:false,..}|
                        Shape::Bezier{closed:false,..});
                    let style=if open{Style{fill:"none".into(),stroke:"#117CC1".into(),
                        stroke_width_mm:1.0}}else{Style::default()};
                    if self.doc.objects.len()>=100_000{
                        self.status="Достигнут лимит объектов".into();
                    }else if let Some(id)=self.add_shape(shape,style){
                        self.doc.get_mut(id).unwrap().name=
                            self.pro_tools.preset.label().into();
                        self.status=format!("Создано: {}. F10 — редактирование узлов.",
                            self.pro_tools.preset.label());
                    }
                }
                Err(error)=>self.status=error
            }
            ui.close_menu();
        }
    }
    fn commit_repeat(&mut self,pattern:RepeatPattern){
        let selection=self.selection_ids();
        let mut candidate=self.doc.clone();
        match candidate.duplicate_pattern(&selection,pattern){
            Ok(ids)=>{
                self.checkpoint();
                self.doc=candidate;
                self.selected=ids.first().copied();
                self.multi_selected=ids.into_iter().skip(1).collect();
                self.fill_cache.entries.clear();
                self.compound_cache.clear();
                self.vector_cache.clear();
                self.status=format!("Массив: создано {} объектов. Ctrl+Z — отмена.",
                    self.selection_ids().len());
            }
            Err(error)=>self.status=format!("Массив не создан: {error}")
        }
    }
    pub(super) fn array_menu(&mut self,ui:&mut egui::Ui){
        ui.heading("Массивы и повторения");
        ui.small("Выберите объекты (Shift). Оригиналы остаются на месте.");
        ui.horizontal(|ui|{
            ui.label("Копий с оригиналом");
            ui.add(DragValue::new(&mut self.pro_tools.count).range(2..=360));
        });
        ui.horizontal(|ui|{
            ui.label("Шаг X, мм");
            ui.add(DragValue::new(&mut self.pro_tools.dx_mm)
                .speed(1.).range(-100_000.0..=100_000.0));
            ui.label("Шаг Y, мм");
            ui.add(DragValue::new(&mut self.pro_tools.dy_mm)
                .speed(1.).range(-100_000.0..=100_000.0));
        });
        if ui.button("Линейный массив").clicked(){
            self.commit_repeat(RepeatPattern::Linear{count:self.pro_tools.count,
                dx_mm:self.pro_tools.dx_mm,dy_mm:self.pro_tools.dy_mm});
            ui.close_menu();
        }
        ui.separator();
        ui.horizontal(|ui|{
            ui.label("Столбцов");
            ui.add(DragValue::new(&mut self.pro_tools.columns).range(1..=40));
            ui.label("Строк");
            ui.add(DragValue::new(&mut self.pro_tools.rows).range(1..=40));
        });
        if ui.button("Прямоугольная сетка").clicked(){
            self.commit_repeat(RepeatPattern::Grid{columns:self.pro_tools.columns,
                rows:self.pro_tools.rows,dx_mm:self.pro_tools.dx_mm,
                dy_mm:self.pro_tools.dy_mm});
            ui.close_menu();
        }
        ui.separator();
        ui.horizontal(|ui|{
            ui.label("Угловой размах");
            ui.add(DragValue::new(&mut self.pro_tools.sweep_degrees)
                .speed(1.).range(-3600.0..=3600.0).suffix("°"));
        });
        if ui.button("Круговой массив").on_hover_text(
            "Вращение вокруг центра выделения; текст и растры при произвольном угле не поддерживаются"
        ).clicked(){
            if let Some(bounds)=self.selection_bounds(){
                let center=Point{x:bounds.x+bounds.w*0.5,y:bounds.y+bounds.h*0.5};
                self.commit_repeat(RepeatPattern::Radial{
                    count:self.pro_tools.count,center,
                    sweep_degrees:self.pro_tools.sweep_degrees});
            }else{
                self.status="Сначала выделите объекты для кругового массива".into();
            }
            ui.close_menu();
        }
        ui.small("Массивы сохраняют стили, шрифты, группы и внутренние обтравки.");
        ui.small("Предел: 1500 новых объектов за одну операцию.");
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn repeat_is_undoable_in_editor(){
        let mut editor=Editor::default();
        editor.selected=editor.doc.objects.first().map(|o|o.id);
        let before=editor.doc.clone();
        editor.commit_repeat(RepeatPattern::Linear{count:4,dx_mm:100.,dy_mm:0.});
        assert_eq!(editor.doc.objects.len(),before.objects.len()+3);
        editor.undo();
        assert_eq!(editor.doc,before);
    }
    #[test] fn generator_creates_native_closed_vectors(){
        let shape=procedural::generate(Preset::Gear,Parameters{
            bounds:Bounds{x:0.,y:0.,w:80.,h:80.},detail:16,ratio:0.7,turns:3.
        }).unwrap();
        let Shape::Polyline{closed,points}=shape else{panic!()};
        assert!(closed);
        assert_eq!(points.len(),64);
    }
}
