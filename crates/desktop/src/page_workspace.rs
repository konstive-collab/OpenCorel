//! Professional page navigator, page-level commands and batch export docker.
use super::{Editor,Tool};
use eframe::egui::{self,RichText,Color32};
use opencorel_core::{Document,Page};
use opencorel_export::batch::{self,PageExportFormat};
use std::collections::HashSet;
impl Editor{
    fn reset_page_interactions(&mut self){
        self.selected=None;
        self.multi_selected.clear();
        self.active_node=None;
        self.node_drag=None;
        self.curve_draft=None;
        self.curve_start=None;
        self.drag_start=None;
        self.creation_start=None;
        self.resize_drag=None;
        self.active_layer=1;
        self.tool=Tool::Select;
        self.fill_cache.entries.clear();
        self.vector_cache.clear();
        self.raster_cache.clear();
        self.compound_cache.clear();
        self.masked_canvas=Default::default();
        self.fit_page();
    }
    /// Page navigation only changes the in-memory active context; it does not
    /// create an undo step or modify artwork.
    pub(super) fn activate_page(&mut self,page_id:u64){
        if page_id==self.doc.active_page_id{return;}
        match self.doc.switch_page(page_id){
            Ok(())=>{
                self.reset_page_interactions();
                self.status=format!("Страница {} из {}",
                    self.doc.page_index()+1,self.doc.pages().len());
            }
            Err(error)=>self.status=format!("Ошибка переключения страницы: {error}")
        }
    }
    fn mutate_pages(&mut self,operation:impl FnOnce(&mut Document)->Result<(),String>,
        description:&str){
        let mut candidate=self.doc.clone();
        match operation(&mut candidate).and_then(|()|candidate.validate()){
            Ok(())=>{
                self.checkpoint();
                self.doc=candidate;
                self.reset_page_interactions();
                self.batch_page_selection=None;
                self.status=format!("{description}. Ctrl+Z — отмена");
            }
            Err(error)=>self.status=format!("Ошибка страницы: {error}"),
        }
    }
    pub(super) fn append_page(&mut self,width:f32,height:f32){
        self.mutate_pages(|doc|{doc.add_page(Page{width_mm:width,height_mm:height})?;Ok(())},
            "Добавлена новая страница");
    }
    pub(super) fn copy_page(&mut self){
        self.mutate_pages(|doc|{doc.duplicate_page()?;Ok(())},
            "Страница с объектами и слоями дублирована");
    }
    pub(super) fn remove_page(&mut self){
        self.mutate_pages(|doc|doc.delete_page(doc.active_page_id),
            "Страница удалена");
    }
    pub(super) fn move_active_page(&mut self,shift:isize){
        let index=self.doc.page_index();
        let destination=index as isize+shift;
        if destination<0||destination>=self.doc.pages().len() as isize{return;}
        let id=self.doc.active_page_id;
        self.mutate_pages(|doc|doc.move_page(id,destination as usize),
            "Порядок страниц изменён");
    }
    pub(super) fn rename_active_page(&mut self,new_name:&str){
        let active=self.doc.active_page_id;
        if self.doc.pages().iter().find(|p|p.id==active)
            .is_some_and(|p|p.name==new_name){return;}
        self.mutate_pages(|doc|doc.rename_page(active,new_name),
            "Название страницы обновлено");
    }
    pub(super) fn transfer_selected_page(&mut self,dest:u64,remove_original:bool){
        let ids=self.selection_ids();
        let mut candidate=self.doc.clone();
        let outcome=candidate.transfer_objects_to_page(&ids,dest,remove_original)
            .and_then(|new_ids|{candidate.validate()?;Ok(new_ids)});
        match outcome{
            Ok(ids)=>{
                self.checkpoint();self.doc=candidate;
                if remove_original{
                    self.selected=None;self.multi_selected.clear();
                }
                self.fill_cache.entries.clear();self.vector_cache.clear();
                self.compound_cache.clear();
                self.status=format!("{} {} объектов на другую страницу · Ctrl+Z — отмена",
                    if remove_original{"Перемещено"}else{"Скопировано"},ids.len());
            }
            Err(e)=>self.status=format!("Перенос между страницами: {e}")
        }
    }
    pub(super) fn page_menu(&mut self,ui:&mut egui::Ui){
        ui.label(format!("Документ: {} страниц",self.doc.pages().len()));
        if ui.button("＋ Страница текущего размера").clicked(){
            self.append_page(self.doc.page.width_mm,self.doc.page.height_mm);
            ui.close_menu();
        }
        if ui.button("＋ Страница A4").clicked(){
            self.append_page(210.,297.);
            ui.close_menu();
        }
        if ui.button("Дублировать страницу с объектами").clicked(){
            self.copy_page();ui.close_menu();
        }
        if ui.add_enabled(self.doc.pages().len()>1,
            egui::Button::new("Удалить текущую страницу")).clicked(){
            self.remove_page();ui.close_menu();
        }
        ui.horizontal(|ui|{
            if ui.small_button("← Раньше").clicked(){self.move_active_page(-1);}
            if ui.small_button("Позже →").clicked(){self.move_active_page(1);}
        });
        if !self.selection_ids().is_empty()&&self.doc.pages().len()>1{
            ui.separator();
            ui.menu_button("Копировать выделение на страницу",|ui|{
                for (index,page) in self.doc.pages().iter().enumerate(){
                    if page.id==self.doc.active_page_id{continue;}
                    if ui.button(format!("{} · {}",index+1,page.name)).clicked(){
                        self.transfer_selected_page(page.id,false);ui.close_menu();
                    }
                }
            });
            ui.menu_button("Переместить выделение на страницу",|ui|{
                for (index,page) in self.doc.pages().iter().enumerate(){
                    if page.id==self.doc.active_page_id{continue;}
                    if ui.button(format!("{} · {}",index+1,page.name)).clicked(){
                        self.transfer_selected_page(page.id,true);ui.close_menu();
                    }
                }
            });
        }
        ui.separator();
        ui.label("Перейти к странице:");
        for (index,page) in self.doc.pages().iter().enumerate(){
            if ui.selectable_label(page.id==self.doc.active_page_id,
                format!("{:02} · {}",index+1,page.name)).clicked(){
                self.activate_page(page.id);ui.close_menu();
            }
        }
    }
    /// The page strip is a true navigator, not a static status label.
    pub(super) fn page_tabs(&mut self,ui:&mut egui::Ui){
        let pages=self.doc.pages();
        let active=self.doc.active_page_id;
        ui.horizontal(|ui|{
            ui.label(RichText::new("ЛИСТЫ").strong().size(10.));
            if ui.small_button("◀").on_hover_text("Предыдущая страница").clicked(){
                let idx=self.doc.page_index();
                if idx>0{self.activate_page(pages[idx-1].id);}
            }
            if ui.small_button("▶").on_hover_text("Следующая страница").clicked(){
                let idx=self.doc.page_index();
                if idx+1<pages.len(){self.activate_page(pages[idx+1].id);}
            }
            egui::ScrollArea::horizontal().id_salt("opencorel_page_tabs")
                .auto_shrink([false,true]).show(ui,|ui|{
                    ui.horizontal(|ui|{
                        for (i,page) in pages.iter().enumerate(){
                            let label=format!("{} {}",i+1,page.name);
                            if ui.selectable_label(page.id==active,label)
                                .on_hover_text(format!("Страница {} · {} объектов",
                                    i+1,self.doc.page_object_count(page.id).unwrap_or(0))).clicked(){
                                self.activate_page(page.id);
                            }
                        }
                    });
                });
            if ui.small_button("＋").on_hover_text("Добавить страницу").clicked(){
                self.append_page(self.doc.page.width_mm,self.doc.page.height_mm);
            }
            if ui.small_button("⧉").on_hover_text("Дублировать страницу").clicked(){
                self.copy_page();
            }
            ui.label(format!("{}/{}",self.doc.page_index()+1,self.doc.pages().len()));
        });
    }
    fn selected_page_ids(&self)->Vec<u64>{
        self.doc.pages().into_iter().filter(|p|self.batch_page_selection.as_ref()
            .is_none_or(|ids|ids.contains(&p.id))).map(|p|p.id).collect()
    }
    pub(super) fn batch_export_picker(&mut self){
        let selection=self.selected_page_ids();
        if selection.is_empty(){self.status="Выберите хотя бы одну страницу".into();return;}
        if let Some(folder)=rfd::FileDialog::new().pick_folder(){
            self.status=match batch::export_pages(&self.doc,&folder,
                self.batch_format,&selection,self.proof_dpi){
                Ok(paths)=>format!("Пакетный экспорт: {} страниц → {}",
                    paths.len(),folder.display()),
                Err(error)=>format!("Ошибка пакетного экспорта: {error}")
            };
        }
    }
    pub(super) fn pdf_book_picker(&mut self){
        let selected=self.selected_page_ids();
        if selected.is_empty(){self.status="Не выбраны страницы для PDF".into();return;}
        if let Some(path)=rfd::FileDialog::new()
            .add_filter("Многостраничный PDF (RGB)",&["pdf"])
            .set_file_name("OpenCorel-document.pdf")
            .save_file(){
            let path=path.with_extension("pdf");
            self.status=match opencorel_export::pdf_book::export_pdf_book(
                &self.doc,&path,&selected){
                Ok(())=>format!("Единый PDF: {} страниц → {}",
                    selected.len(),path.display()),
                Err(e)=>format!("Ошибка общего PDF: {e}"),
            };
        }
    }
    pub(super) fn export_docker(&mut self,ui:&mut egui::Ui){
        ui.heading("Центр экспорта");
        ui.small("Несколько страниц за один запуск, без изменения исходного KDR.");
        ui.separator();
        ui.label(RichText::new("Формат вывода").strong());
        egui::ComboBox::from_id_salt("professional_export_type")
            .selected_text(self.batch_format.label())
            .show_ui(ui,|ui|{
                for kind in [PageExportFormat::Svg,PageExportFormat::Pdf,
                    PageExportFormat::Png,PageExportFormat::Jpeg]{
                    ui.selectable_value(&mut self.batch_format,kind,kind.label());
                }
            });
        if matches!(self.batch_format,PageExportFormat::Png|PageExportFormat::Jpeg){
            ui.horizontal(|ui|{
                ui.label("DPI");
                ui.add(egui::DragValue::new(&mut self.proof_dpi).range(36..=600));
            });
        }
        ui.separator();
        ui.horizontal(|ui|{
            ui.label(RichText::new("Страницы").strong());
            if ui.small_button("Все").clicked(){self.batch_page_selection=None;}
            if ui.small_button("Ни одной").clicked(){
                self.batch_page_selection=Some(HashSet::new());
            }
        });
        let pages=self.doc.pages();
        egui::ScrollArea::vertical()
            .id_salt("export_page_queue")
            .max_height(340.)
            .show(ui,|ui|{
                for (index,page) in &pages.iter().enumerate().collect::<Vec<_>>(){
                    let mut checked=self.batch_page_selection.as_ref()
                        .is_none_or(|s|s.contains(&page.id));
                    if ui.checkbox(&mut checked,format!(
                        "Стр. {} · {} ({} объектов)",index+1,
                        page.name,self.doc.page_object_count(page.id).unwrap_or(0))).changed(){
                        let ids=self.batch_page_selection.get_or_insert_with(||pages
                            .iter().map(|p|p.id).collect());
                        if checked{ids.insert(page.id);}
                        else{ids.remove(&page.id);}
                    }
                }
            });
        ui.separator();
        let count=self.selected_page_ids().len();
        ui.label(format!("В очереди: {} из {} страниц",count,pages.len()));
        if ui.add_enabled(count>0,egui::Button::new("Экспортировать выбранные страницы..."))
            .clicked(){self.batch_export_picker();}
        if ui.add_enabled(count>0,egui::Button::new("Единый многостраничный PDF..."))
            .on_hover_text("Все выбранные страницы в одном PDF, RGB, не PDF/X")
            .clicked(){self.pdf_book_picker();}
        ui.small("Файлы: название_документа_p01_имя_страницы.ext");
        ui.small("Существующие файлы не перезаписываются. PDF — RGB, не PDF/X.");
        ui.separator();
        ui.collapsing("Экспорт текущей страницы",|ui|{
            ui.horizontal_wrapped(|ui|{
                if ui.button("SVG").clicked(){self.export_dialog("svg");}
                if ui.button("PDF").clicked(){self.export_dialog("pdf");}
                if ui.button("PNG").clicked(){self.export_dialog("png");}
                if ui.button("JPEG").clicked(){self.export_dialog("jpg");}
            });
        });
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Shape,Style};
    #[test] fn tabs_preserve_objects_and_use_undo_for_structural_changes(){
        let mut e=Editor::default();
        let original=e.doc.clone();
        e.append_page(210.,297.);
        assert_eq!(e.doc.pages().len(),2);
        assert_eq!(e.doc.objects.len(),0);
        assert_eq!(e.undo.len(),1);
        e.undo();
        assert_eq!(e.doc,original);
    }
    #[test] fn cloning_a_page_preserves_shapes_without_sharing_object_ids(){
        let mut e=Editor::default();
        let first=e.doc.objects[0].id;
        e.copy_page();
        assert_eq!(e.doc.objects.len(),2);
        assert_ne!(first,e.doc.objects[0].id);
        e.activate_page(1);
        assert_eq!(e.doc.objects[0].id,first);
        e.doc.validate().unwrap();
    }
    #[test] fn reordering_is_undoable(){
        let mut e=Editor::default();
        e.append_page(100.,100.);
        let before=e.doc.clone();
        e.move_active_page(-1);
        assert_eq!(e.doc.pages()[0].id,e.doc.active_page_id);
        e.undo();
        assert_eq!(e.doc,before);
    }
    #[test] fn selected_objects_can_move_to_other_page_and_undo(){
        let mut e=Editor::default();
        let id=e.doc.objects[0].id;
        let target=e.doc.add_page(Page{width_mm:210.,height_mm:297.}).unwrap();
        e.activate_page(1);
        e.select_object(Some(id),false);
        let before=e.doc.clone();
        e.transfer_selected_page(target,true);
        assert!(e.doc.objects.iter().all(|o|o.id!=id));
        e.undo();
        assert_eq!(e.doc,before);
    }
    #[test] fn batch_selection_defaults_to_all(){
        let mut e=Editor::default();
        e.append_page(100.,100.);
        assert_eq!(e.selected_page_ids().len(),2);
        e.batch_page_selection=Some(HashSet::new());
        assert_eq!(e.selected_page_ids().len(),0);
    }
}
