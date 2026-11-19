//! Publication-wide Find/Replace and text casing, tightly integrated with
//! KDR pages, Undo and document-level validation (not raster image OCR).
use super::{Editor,Tool};
use eframe::egui::{self,RichText,Color32};
use opencorel_core::text_workflow::{TextCase,TextHit};
impl Editor{
    pub(super) fn run_text_search(&mut self){
        match self.doc.find_text(&self.text_find_query,
            self.text_case_sensitive,self.text_all_pages){
            Ok(results)=>{
                let count:usize=results.iter().map(|r|r.occurrences).sum();
                self.status=format!("Поиск: {count} совпадений в {} объектах",results.len());
                self.text_hits=results;
            }
            Err(e)=>{self.text_hits.clear();self.status=format!("Поиск: {e}");}
        }
    }
    pub(super) fn run_text_replace(&mut self){
        let mut candidate=self.doc.clone();
        let changed=candidate.replace_text(&self.text_find_query,
            &self.text_replacement,self.text_case_sensitive,self.text_all_pages)
            .and_then(|count|{candidate.validate()?;Ok(count)});
        match changed{
            Ok(0)=>self.status="Совпадений для замены нет".into(),
            Ok(count)=>{
                self.checkpoint();
                self.doc=candidate;
                self.fill_cache.entries.clear();
                self.vector_cache.clear();
                self.compound_cache.clear();
                self.masked_canvas=Default::default();
                self.status=format!("Заменено {count} вхождений · Ctrl+Z — одна отмена");
                self.run_text_search();
            },
            Err(e)=>self.status=format!("Замена отменена: {e}"),
        }
    }
    pub(super) fn run_text_case(&mut self,mode:TextCase){
        let selection=self.selection_ids();
        if selection.is_empty(){
            self.status="Выделите текстовые объекты для изменения регистра".into();
            return;
        }
        let mut candidate=self.doc.clone();
        let outcome=candidate.transform_text_case(&selection,mode)
            .and_then(|count|{candidate.validate()?;Ok(count)});
        match outcome{
            Ok(0)=>self.status="Текстовые объекты не изменены".into(),
            Ok(count)=>{
                self.checkpoint();
                self.doc=candidate;
                self.vector_cache.clear();
                self.masked_canvas=Default::default();
                self.status=format!("Регистр изменён: {count} надписей · Ctrl+Z — отмена");
            },
            Err(e)=>self.status=format!("Ошибка регистра: {e}")
        }
    }
    pub(super) fn open_text_hit(&mut self,page_id:u64,object_id:u64){
        if page_id!=self.doc.active_page_id{self.activate_page(page_id);}
        let Some(layer)=self.doc.get(object_id).map(|obj|obj.layer_id) else{
            self.status="Текстовый объект уже удалён".into();return;
        };
        self.change_tool(Tool::Select);
        self.selected=Some(object_id);
        self.multi_selected.clear();
        self.active_layer=layer;
        self.inspector_tab=super::workspace::InspectorTab::Properties;
        self.show_inspector=true;
        self.status=format!("Выбрана надпись #{object_id} на странице {page_id}");
    }
    pub(super) fn text_search_dialog(&mut self,ctx:&egui::Context){
        if !self.show_text_search{return;}
        let mut open=true;
        let mut run_search=false;
        let mut run_replace=false;
        let mut chosen=None;
        let mut chosen_case=None;
        egui::Window::new("Поиск и замена текста · Ctrl+F / Ctrl+H")
            .open(&mut open)
            .default_width(530.0)
            .resizable(true)
            .show(ctx,|ui|{
                ui.label(RichText::new("Поиск по редактируемым текстовым объектам").strong());
                ui.horizontal(|ui|{
                    ui.label("Найти");
                    let response=ui.add(egui::TextEdit::singleline(&mut self.text_find_query)
                        .hint_text("Слово или фраза…").desired_width(350.));
                    if response.lost_focus()&&ui.input(|i|
                        i.key_pressed(egui::Key::Enter)){run_search=true;}
                });
                ui.horizontal(|ui|{
                    ui.label("Заменить");
                    ui.add(egui::TextEdit::singleline(&mut self.text_replacement)
                        .hint_text("Новое значение…").desired_width(350.));
                });
                ui.horizontal_wrapped(|ui|{
                    ui.checkbox(&mut self.text_case_sensitive,"Учитывать регистр");
                    ui.checkbox(&mut self.text_all_pages,"Все страницы");
                });
                ui.horizontal(|ui|{
                    if ui.button("Найти все").clicked(){run_search=true;}
                    if ui.button("Заменить все совпадения").clicked(){run_replace=true;}
                });
                ui.small(RichText::new(
                    "Операция затрагивает только редактируемый текст. Нельзя изменять заблокированные слои."
                ).color(Color32::GRAY));
                ui.separator();
                ui.label(format!("Совпадений: {} · объектов: {}",
                    self.text_hits.iter().map(|m|m.occurrences).sum::<usize>(),
                    self.text_hits.len()));
                let names:std::collections::HashMap<_,_>=self.doc.pages().into_iter()
                    .map(|p|(p.id,p.name)).collect();
                egui::ScrollArea::vertical().max_height(310.).show(ui,|ui|{
                    for hit in self.text_hits.iter().take(150){
                        let page=names.get(&hit.page_id).map(String::as_str)
                            .unwrap_or("Страница");
                        let preview=hit.preview.replace('\n'," ↵ ");
                        if ui.button(format!("{} · #{} · {}×  {}",
                            page,hit.object_id,hit.occurrences,preview))
                            .on_hover_text("Перейти к объекту на странице")
                            .clicked(){
                            chosen=Some((hit.page_id,hit.object_id));
                        }
                    }
                });
                if self.text_hits.len()>150{
                    ui.small("Показаны первые 150 результатов, остальные учтены в статистике.");
                }
                ui.separator();
                ui.label("Изменить регистр выделенного текста:");
                ui.horizontal_wrapped(|ui|{
                    for mode in [TextCase::Upper,TextCase::Lower,TextCase::Title,TextCase::Sentence]{
                        if ui.small_button(mode.label()).clicked(){
                            // Store a pending action until the window closure completes.
                            chosen_case=Some(mode);
                        }
                    }
                });
            });
        self.show_text_search=open;
        if run_replace{self.run_text_replace();}
        else if run_search{self.run_text_search();}
        if let Some((page,id))=chosen{self.open_text_hit(page,id);}
        if let Some(mode)=chosen_case{self.run_text_case(mode);}
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn document_text_replacement_uses_single_undo_checkpoint(){
        let mut editor=Editor::default();
        editor.text_find_query="ВАША".into();
        editor.text_replacement="НОВАЯ".into();
        let before=editor.doc.clone();
        editor.run_text_replace();
        assert!(editor.doc.to_svg().contains("НОВАЯ"));
        editor.undo();
        assert_eq!(editor.doc,before);
    }
    #[test] fn selected_text_case_changes_only_text_and_is_undoable(){
        let mut editor=Editor::default();
        let text_id=editor.doc.objects.iter()
            .find(|o|matches!(o.shape,opencorel_core::Shape::Text{..}))
            .unwrap().id;
        editor.selected=Some(text_id);
        let before=editor.doc.clone();
        editor.run_text_case(TextCase::Lower);
        editor.undo();
        assert_eq!(editor.doc,before);
    }
}
