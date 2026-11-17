//! Non-destructive clipping GUI commands. No document mutation before preflight.
use super::Editor;

impl Editor {
    pub(super) fn apply_clipping_selection(&mut self) {
        let ids=self.selection_ids();
        if ids.len()!=2 || ids[0]==ids[1] {
            self.status="Для обтравки выберите два разных объекта".into();
            return;
        }
        let (source,target)=(ids[0],ids[1]);
        match self.doc.check_clip(source,target) {
            Ok(())=>{
                self.checkpoint();
                // check_clip guarantees an infallible commit for unchanged state.
                self.doc.set_clip(source,target).expect("clip preflight succeeded");
                self.selected=Some(target);
                self.multi_selected.clear();
                self.fill_cache.entries.clear();
                self.compound_cache.clear();
                self.status=format!("Обтравка создана: контур #{source}, объект #{target}. Ctrl+Z — отмена");
            }
            Err(error)=>self.status=format!("Обтравка не создана: {error}"),
        }
    }
    pub(super) fn remove_clipping_selection(&mut self){
        let ids=self.selection_ids();
        if ids.is_empty(){self.status="Выберите объект с обтравкой".into();return;}
        let candidates:Vec<_>=ids.iter().copied().filter(|id|
            self.doc.clip_links.iter().any(|c|c.target_id==*id)).collect();
        if candidates.is_empty(){self.status="У выделенных объектов нет обтравки".into();return;}
        let editable=candidates.iter().all(|id|self.doc.is_object_editable(*id));
        if !editable{
            self.status="Нельзя изменить обтравку на скрытом или заблокированном слое".into();
            return;
        }
        self.checkpoint();
        self.doc.clip_links.retain(|clip|!candidates.contains(&clip.target_id));
        self.status=format!("Снята обтравка с {} объектов",candidates.len());
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Shape,Style};
    #[test]
    fn clipping_commands_are_undoable(){
        let mut e=Editor::default();
        let a=e.doc.add(Shape::Ellipse{x:1.0,y:1.0,w:50.0,h:50.0},Style::default());
        let b=e.doc.add(Shape::Rectangle{x:0.0,y:0.0,w:60.0,h:60.0},Style::default());
        e.selected=Some(a);e.multi_selected=vec![b];
        e.apply_clipping_selection();
        assert_eq!(e.doc.clip_links.len(),1);
        e.undo();assert!(e.doc.clip_links.is_empty());
        e.redo();assert_eq!(e.doc.clip_links.len(),1);
        e.selected=Some(b);e.multi_selected.clear();
        e.remove_clipping_selection();
        assert!(e.doc.clip_links.is_empty());
        e.undo();assert_eq!(e.doc.clip_links.len(),1);
    }
    #[test]
    fn invalid_clip_does_not_append_undo(){
        let mut e=Editor::default();
        let a=e.doc.add(Shape::Polyline{points:vec![
            opencorel_core::Point{x:0.0,y:0.0},
            opencorel_core::Point{x:1.0,y:1.0}],closed:false},Style::default());
        let b=e.doc.objects[0].id;
        e.selected=Some(a);e.multi_selected=vec![b];
        let previous=e.undo.len();
        e.apply_clipping_selection();
        assert_eq!(e.undo.len(),previous);
        assert!(e.doc.clip_links.is_empty());
    }
}
