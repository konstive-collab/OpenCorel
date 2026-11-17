//! Core editor actions for real multi-page documents.
//! All mutations are atomic and undoable; page navigation never clones
//! a document into undo history or leaks selection across page boundaries.
use super::{Editor,Tool};
use opencorel_core::Document;

impl Editor{
    pub(super) fn reset_page_interaction(&mut self){
        self.selected=None;self.multi_selected.clear();
        self.active_layer=1;
        self.tool=Tool::Select;
        self.active_node=None;self.node_drag=None;self.node_drag_checkpointed=false;
        self.curve_start=None;self.curve_draft=None;
        self.curve_resumed=false;self.curve_outgoing=None;
        self.curve_drag_anchor=None;
        self.drag_start=None;self.selection_drag_checkpointed=false;
        self.marquee_drag=None;self.creation_start=None;
        self.resize_drag=None;self.resize_drag_base=None;
        self.resize_drag_checkpointed=false;
        self.guide_drag=None;self.guide_numeric_session=None;
        self.freehand_samples.clear();self.freehand_overflow=false;
        self.inline_text=None;
        self.fill_cache.entries.clear();
        self.vector_cache.clear();
        self.compound_cache.clear();
        self.bezier_preview.clear();
        self.raster_cache.clear();
        self.pan=eframe::egui::Vec2::ZERO;self.zoom=1.0;
        self.page_rename_mode=false;
    }
    pub(super) fn accept_page_change(&mut self,new_doc:Document,description:String){
        self.checkpoint();
        self.doc=new_doc;
        self.reset_page_interaction();
        self.status=description;
    }
    pub(super) fn navigate_page(&mut self,index:usize){
        if index==self.doc.active_page_index{return;}
        match self.doc.switch_page(index){
            Ok(())=>{
                self.reset_page_interaction();
                self.status=format!("Страница {} из {}: {}",
                    self.doc.active_page_index+1,self.doc.page_count(),
                    self.doc.active_page_title);
            },
            Err(e)=>self.status=format!("Ошибка переключения страницы: {e}"),
        }
    }
    pub(super) fn create_page(&mut self,duplicate:bool){
        let mut preview=self.doc.clone();
        match preview.append_page(duplicate){
            Ok(index)=>self.accept_page_change(preview,format!(
                "{}: страница {} из {}",
                if duplicate{"Создана копия"}else{"Добавлена пустая страница"},
                index+1,self.doc.page_count()+1)),
            Err(e)=>self.status=format!("Ошибка добавления страницы: {e}"),
        }
    }
    pub(super) fn remove_current_page(&mut self){
        let mut preview=self.doc.clone();
        let current=preview.active_page_index;
        match preview.delete_page(current){
            Ok(())=>self.accept_page_change(preview,
                format!("Страница {} удалена; Ctrl+Z — отмена",current+1)),
            Err(e)=>self.status=format!("Ошибка удаления страницы: {e}"),
        }
    }
    pub(super) fn shift_current_page(&mut self,forward:bool){
        let mut preview=self.doc.clone();
        let current=preview.active_page_index;
        let target=if forward{current.checked_add(1)}else{current.checked_sub(1)};
        let Some(target)=target.filter(|i|*i<preview.page_count())else{return;};
        match preview.move_page(current,target){
            Ok(())=>self.accept_page_change(preview,
                format!("Страница перемещена на позицию {}; Ctrl+Z — отмена",
                    target+1)),
            Err(e)=>self.status=format!("Ошибка перемещения страницы: {e}"),
        }
    }
    pub(super) fn rename_current_page(&mut self,proposed:&str){
        let mut preview=self.doc.clone();
        let index=preview.active_page_index;
        match preview.rename_page(index,proposed){
            Ok(())=>{
                if preview==self.doc{self.page_rename_mode=false;return;}
                self.accept_page_change(preview,
                    format!("Страница переименована: {proposed}"));
            },
            Err(e)=>self.status=format!("Ошибка имени страницы: {e}"),
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Shape,Style};
    #[test]
    fn navigation_preserves_page_objects_but_resets_selection(){
        let mut e=Editor::default();
        e.doc=Document::default();
        let original=e.doc.add(Shape::Rectangle{x:2.,y:3.,w:5.,h:8.},
            Style::default());
        e.selected=Some(original);
        e.create_page(false);
        assert_eq!(e.doc.page_count(),2);
        assert_eq!(e.doc.objects.len(),0);
        assert!(e.selected.is_none());
        e.doc.add(Shape::Ellipse{x:8.,y:9.,w:12.,h:12.},
            Style::default());
        e.navigate_page(0);
        assert_eq!(e.doc.objects.len(),1);
        assert!(matches!(e.doc.objects[0].shape,Shape::Rectangle{..}));
        assert!(e.selected.is_none());
        e.navigate_page(1);
        assert!(matches!(e.doc.objects[0].shape,Shape::Ellipse{..}));
        e.doc.validate().unwrap();
    }
    #[test]
    fn page_mutations_are_one_undo_step_and_delete_last_is_rejected(){
        let mut e=Editor::default();e.doc=Document::default();
        let original=e.doc.clone();
        e.create_page(true);
        assert_eq!(e.undo.len(),1);
        e.undo();
        assert_eq!(e.doc,original);
        e.create_page(false);
        e.remove_current_page();
        assert_eq!(e.doc.page_count(),1);
        let before=e.doc.clone();
        e.remove_current_page();
        assert_eq!(e.doc,before);
    }
}
