//! Professional path conversion and maintenance commands. All operations use
//! native KDR shape geometry, whole-selection validation and a single Undo.
use super::Editor;
use eframe::egui;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) enum PathCommand{Convert,ReduceNodes,JoinEndpoints}
impl Editor{
    pub(super) fn execute_path_command(&mut self,command:PathCommand){
        let ids=self.selection_ids();
        if ids.is_empty(){
            self.status="Выберите редактируемые векторные объекты".into();
            return;
        }
        let mut next=self.doc.clone();
        let result=match command{
            PathCommand::Convert=>
                next.convert_selection_to_bezier(&ids).map(|n|(n,None)),
            PathCommand::ReduceNodes=>next.simplify_selected_polylines(
                &ids,self.path_simplify_tolerance_mm).map(|n|(n,None)),
            PathCommand::JoinEndpoints=>next.join_open_paths(
                &ids,self.path_join_tolerance_mm).map(|id|(1,Some(id))),
        }.and_then(|(count,joined)|{next.validate()?;Ok((count,joined))});
        match result{
            Ok((0,_))=>self.status="Геометрия не требует изменений".into(),
            Ok((count,joined))=>{
                self.checkpoint();
                self.doc=next;
                if let Some(id)=joined{
                    self.selected=Some(id);self.multi_selected.clear();
                }
                self.fill_cache.entries.clear();
                self.vector_cache.clear();
                self.raster_cache.clear();
                self.compound_cache.clear();
                self.masked_canvas=Default::default();
                self.active_node=None;self.node_drag=None;
                self.status=match command{
                    PathCommand::Convert=>format!("В кривые преобразовано объектов: {count}"),
                    PathCommand::ReduceNodes=>format!("Упрощение: удалено узлов {count}"),
                    PathCommand::JoinEndpoints=>"Два контура соединены в один".into()
                };
                self.status.push_str(" · Ctrl+Z — отмена");
            }
            Err(e)=>self.status=format!("Контуры: {e}"),
        }
    }
    pub(super) fn path_workflow_menu(&mut self,ui:&mut egui::Ui){
        ui.label(egui::RichText::new("ВЕКТОРНАЯ ГЕОМЕТРИЯ").strong());
        ui.small("Правки остаются редактируемыми и сохраняются в KDR.");
        if ui.button("Преобразовать в кривые Безье").clicked(){
            self.execute_path_command(PathCommand::Convert);ui.close_menu();
        }
        ui.separator();
        ui.horizontal(|ui|{
            ui.label("Точность упрощения");
            ui.add(egui::DragValue::new(&mut self.path_simplify_tolerance_mm)
                .range(0.01..=10.0).speed(0.05).suffix(" мм"));
        });
        if ui.button("Упростить ломаные: удалить лишние узлы").clicked(){
            self.execute_path_command(PathCommand::ReduceNodes);ui.close_menu();
        }
        ui.small("Упрощение предназначено для открытых ломаных.");
        ui.separator();
        ui.horizontal(|ui|{
            ui.label("Допуск соединения");
            ui.add(egui::DragValue::new(&mut self.path_join_tolerance_mm)
                .range(0.01..=20.0).speed(0.05).suffix(" мм"));
        });
        if ui.add_enabled(self.selection_ids().len()==2,
            egui::Button::new("Соединить открытые концы 2 контуров")).clicked(){
            self.execute_path_command(PathCommand::JoinEndpoints);
            ui.close_menu();
        }
        ui.small("Стили и слои контуров должны совпадать.");
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Shape,Style,Point};
    #[test] fn rectangle_converts_to_editable_curve_with_undo(){
        let mut editor=Editor::default();
        let id=editor.doc.objects[0].id;
        let snapshot=editor.doc.clone();
        editor.selected=Some(id);
        editor.execute_path_command(PathCommand::Convert);
        assert!(matches!(editor.doc.get(id).unwrap().shape,Shape::Bezier{..}));
        editor.undo();
        assert_eq!(editor.doc,snapshot);
    }
    #[test] fn join_two_lines_keeps_first_id_and_is_undoable(){
        let mut editor=Editor::default();
        editor.doc=Default::default();
        let a=editor.doc.add(Shape::Polyline{points:vec![
            Point{x:0.0,y:0.0},Point{x:10.0,y:0.0}],closed:false},Style::default());
        let b=editor.doc.add(Shape::Polyline{points:vec![
            Point{x:10.05,y:0.0},Point{x:20.0,y:0.0}],closed:false},Style::default());
        let snapshot=editor.doc.clone();
        editor.selected=Some(a);editor.multi_selected=vec![b];
        editor.execute_path_command(PathCommand::JoinEndpoints);
        assert_eq!(editor.doc.objects.len(),1);
        assert_eq!(editor.selected,Some(a));
        editor.undo();
        assert_eq!(editor.doc,snapshot);
    }
}
