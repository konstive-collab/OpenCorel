//! Layer/object-tree professional operations. All edits validated before
//! committing, each operation uses a single Undo checkpoint.
use super::Editor;
use eframe::egui::{self,RichText,Color32};
use opencorel_core::Document;
impl Editor{
    fn mutate_layer(&mut self,
        edit:impl FnOnce(&mut Document)->Result<bool,String>,
        status:&str){
        let mut candidate=self.doc.clone();
        match edit(&mut candidate).and_then(|changed|{
            if changed{candidate.validate()?;}Ok(changed)
        }){
            Ok(true)=>{
                self.checkpoint();self.doc=candidate;
                self.fill_cache.entries.clear();self.vector_cache.clear();
                self.raster_cache.clear();self.compound_cache.clear();
                self.status=format!("{status}. Ctrl+Z — отмена");
            }
            Ok(false)=>{},
            Err(error)=>self.status=format!("Слои: {error}"),
        }
    }
    pub(super) fn rename_active_layer(&mut self,name:&str){
        let id=self.active_layer;
        if self.doc.layer(id).is_some_and(|l|l.name==name){return;}
        self.mutate_layer(|d|{d.rename_layer(id,name)?;Ok(true)},"Слой переименован");
    }
    pub(super) fn delete_active_layer(&mut self){
        let id=self.active_layer;
        let mut candidate=self.doc.clone();
        match candidate.delete_empty_layer(id).and_then(|()|candidate.validate()){
            Ok(())=>{
                self.checkpoint();self.doc=candidate;
                self.active_layer=1;
                self.status="Пустой слой удалён · Ctrl+Z — отмена".into();
            },
            Err(e)=>self.status=format!("Слой не удалён: {e}")
        }
    }
    pub(super) fn reorder_active_layer(&mut self,up:bool){
        let id=self.active_layer;
        self.mutate_layer(|d|d.reorder_layer(id,up),
            "Изменён порядок слоёв");
    }
    pub(super) fn send_selection_to_layer(&mut self,layer_id:u64){
        let selection=self.selection_ids();
        self.mutate_layer(|d|Ok(d.move_to_layer(&selection,layer_id)?>0),
            "Выделение перенесено в другой слой");
        if self.selected.is_some(){self.active_layer=layer_id;}
    }
    pub(super) fn z_step(&mut self,front:bool){
        let selection=self.selection_ids();
        self.mutate_layer(|d|Ok(d.step_z_order(&selection,front)?>0),
            if front{"Объекты подняты на один уровень"}
            else{"Объекты опущены на один уровень"});
    }
    pub(super) fn select_layer_members(&mut self,id:u64){
        if self.doc.layer(id).is_none(){return;}
        let ids:Vec<_>=self.doc.objects.iter().filter(|o|o.layer_id==id)
            .map(|o|o.id).collect();
        self.selected=ids.first().copied();
        self.multi_selected=ids.into_iter().skip(1).collect();
        self.active_layer=id;
        self.status=format!("Выбрано {} объектов слоя",self.selection_ids().len());
    }
    pub(super) fn layer_actions(&mut self,ui:&mut egui::Ui){
        ui.collapsing("Управление активным слоем",|ui|{
            ui.small(RichText::new("Изменения сохраняются в KDR и поддерживают Ctrl+Z")
                .color(Color32::GRAY));
            if let Some(layer)=self.doc.layer(self.active_layer){
                let mut new_name=layer.name.clone();
                ui.horizontal(|ui|{
                    ui.label("Имя");
                    if ui.text_edit_singleline(&mut new_name).changed(){
                        self.rename_active_layer(&new_name);
                    }
                });
                ui.horizontal_wrapped(|ui|{
                    if ui.small_button("↑ Слой выше").clicked(){self.reorder_active_layer(true);}
                    if ui.small_button("↓ Слой ниже").clicked(){self.reorder_active_layer(false);}
                    if ui.small_button("Выделить объекты").clicked(){
                        self.select_layer_members(self.active_layer);
                    }
                });
                if ui.add_enabled(self.active_layer!=1,
                    egui::Button::new("Удалить пустой слой")).clicked(){
                    self.delete_active_layer();
                }
            }
        });
        if !self.selection_ids().is_empty(){
            ui.horizontal_wrapped(|ui|{
                ui.label("Перенести выделение:");
                let layers:Vec<_>=self.doc.layers.iter()
                    .map(|l|(l.id,l.name.clone(),l.visible&&!l.locked)).collect();
                for (id,name,allowed) in layers{
                    if ui.add_enabled(allowed,egui::Button::new(name)).clicked(){
                        self.send_selection_to_layer(id);
                    }
                }
            });
        }
        ui.horizontal(|ui|{
            let has_selection=!self.selection_ids().is_empty();
            if ui.add_enabled(has_selection,egui::Button::new("На уровень ↑"))
                .on_hover_text("На одну позицию выше").clicked(){self.z_step(true);}
            if ui.add_enabled(has_selection,egui::Button::new("На уровень ↓"))
                .on_hover_text("На одну позицию ниже").clicked(){self.z_step(false);}
        });
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn edit_and_undo_layer_name(){
        let mut editor=Editor::default();
        let id=editor.doc.add_layer("NEW".into());
        editor.active_layer=id;
        editor.rename_active_layer("Art");
        assert_eq!(editor.doc.layer(id).unwrap().name,"Art");
        editor.undo();
        assert_eq!(editor.doc.layer(id).unwrap().name,"NEW");
    }
    #[test] fn moving_selection_to_layer_can_be_undone(){
        let mut editor=Editor::default();
        let id=editor.doc.add_layer("Top".into());
        let obj=editor.doc.objects[0].id;
        editor.select_object(Some(obj),false);
        editor.send_selection_to_layer(id);
        assert_eq!(editor.doc.get(obj).unwrap().layer_id,id);
        editor.undo();
        assert_eq!(editor.doc.get(obj).unwrap().layer_id,1);
    }
}
