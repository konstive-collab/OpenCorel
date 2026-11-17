//! Atomic UI commands for the editable vector boolean engine.
use super::Editor;
use opencorel_core::{Document,Shape,Style};
use opencorel_core::boolean_ops::{self,BooleanOperation};

impl Editor{
    pub(super) fn apply_boolean(&mut self,operation:BooleanOperation){
        let ids=self.selection_ids();
        if ids.len()!=2 || ids[0]==ids[1]{
            self.status="Выберите ровно две отдельные замкнутые фигуры с Shift (первую, затем вторую)".into();
            return;
        }
        let (result,style,layer_id)={
            let Some(a)=self.doc.objects.iter().find(|o|o.id==ids[0]) else{
                self.status="Первая фигура не найдена".into();return;
            };
            let Some(b)=self.doc.objects.iter().find(|o|o.id==ids[1]) else{
                self.status="Вторая фигура не найдена".into();return;
            };
            if a.group_id.is_some() || b.group_id.is_some(){
                self.status="Разгруппируйте фигуры перед логической операцией".into();
                return;
            }
            if !a.visible||a.locked||!b.visible||b.locked||![a.layer_id,b.layer_id].iter().all(|id|
                self.doc.layer(*id).map(|l|l.visible&&!l.locked).unwrap_or(false)){
                self.status="Операция отменена: слой скрыт или заблокирован".into();
                return;
            }
            if a.style.fill=="none"||b.style.fill=="none"{
                self.status="Логические операции требуют заливки у обеих фигур".into();
                return;
            }
            let result=match boolean_ops::combine(&a.shape,&b.shape,operation){
                Ok(Some(shape))=>shape,
                Ok(None)=>{
                    self.status="Результат пуст — исходные фигуры оставлены без изменений".into();
                    return;
                }
                Err(e)=>{
                    self.status=format!("Логическая операция не выполнена: {e}");
                    return;
                }
            };
            (result,a.style.clone(),a.layer_id)
        };
        // Validate the candidate in an isolated document before removing either
        // source. All failures are atomic, and the existing project remains usable.
        let mut preview=Document::default();
        preview.add(result.clone(),style.clone());
        if let Err(error)=preview.validate(){
            self.status=format!("Нельзя применить результат: {error}");
            return;
        }
        if self.doc.next_id>=u64::MAX-1{
            self.status="Лимит идентификаторов объектов".into();
            return;
        }
        self.checkpoint();
        self.doc.objects.retain(|obj|obj.id!=ids[0]&&obj.id!=ids[1]);
        let new_id=self.doc.add_to_layer(result,style,layer_id);
        self.doc.get_mut(new_id).unwrap().name=format!("{} #{new_id}",operation.label());
        self.selected=Some(new_id);
        self.multi_selected.clear();
        self.active_node=None;
        self.node_drag=None;
        self.fill_cache.entries.clear();
        self.compound_cache.clear();
        self.status=format!(
            "{} выполнено. Результат — редактируемый составной контур. Цвет сохранён от первой фигуры; Ctrl+Z — вернуть обе исходные.",
            operation.label()
        );
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Point,Contour,CubicSegment};
    fn setup()->(Editor,u64,u64){
        let mut editor=Editor::default();
        editor.doc=Document::default();
        editor.undo.clear();editor.redo.clear();
        let a=editor.doc.add(Shape::Rectangle{x:0.,y:0.,w:100.,h:100.},
            Style::default());
        let b=editor.doc.add(Shape::Rectangle{x:50.,y:0.,w:100.,h:100.},
            Style{fill:"#FA1234".into(),..Style::default()});
        editor.select_object(Some(a),false);
        editor.select_object(Some(b),true);
        (editor,a,b)
    }
    #[test] fn union_is_one_editable_object_and_undo_restores_source(){
        let (mut editor,a,b)=setup();
        let before=editor.doc.clone();
        editor.apply_boolean(BooleanOperation::Union);
        assert_eq!(editor.doc.objects.len(),1);
        assert!(matches!(editor.doc.objects[0].shape,Shape::CompoundPath{..}));
        assert_eq!(editor.doc.objects[0].style.fill,"#447BD4");
        editor.doc.validate().unwrap();
        assert!(editor.doc.get_mut(a).is_none());
        assert!(editor.doc.get_mut(b).is_none());
        editor.undo();
        assert_eq!(editor.doc,before);
    }
    #[test] fn subtract_keeps_hole_and_original_style(){
        let (mut editor,a,b)=setup();
        // Put B strictly inside A so A-B has a counter/hole.
        if let Shape::Rectangle{x,y,w,h}=&mut editor.doc.get_mut(b).unwrap().shape{
            *x=30.;*y=30.;*w=40.;*h=40.;
        }
        editor.apply_boolean(BooleanOperation::Subtract);
        assert_eq!(editor.doc.objects.len(),1);
        let Shape::CompoundPath{contours}=&editor.doc.objects[0].shape else{panic!()};
        assert_eq!(contours.len(),2);
        assert_eq!(editor.doc.objects[0].style.fill,"#447BD4");
        editor.doc.validate().unwrap();
        assert_eq!(editor.doc.objects[0].layer_id,1);
        assert_ne!(editor.doc.objects[0].id,a);
    }
    #[test] fn empty_intersection_preserves_both_objects_and_history(){
        let (mut editor,_,b)=setup();
        if let Shape::Rectangle{x,..}=&mut editor.doc.get_mut(b).unwrap().shape{
            *x=300.;
        }
        let before=editor.doc.clone();
        editor.apply_boolean(BooleanOperation::Intersect);
        assert_eq!(editor.doc,before);
        assert!(editor.undo.is_empty());
        assert!(editor.status.contains("пуст"));
    }
    #[test] fn locked_layer_blocks_mutation(){
        let (mut editor,_,b)=setup();
        let locked=editor.doc.add_layer("locked".into());
        editor.doc.get_mut(b).unwrap().layer_id=locked;
        editor.doc.layers[1].locked=true;
        let before=editor.doc.clone();
        editor.apply_boolean(BooleanOperation::Xor);
        assert_eq!(editor.doc,before);
        assert!(editor.undo.is_empty());
    }
    #[test] fn open_path_fails_atomically(){
        let (mut editor,_,b)=setup();
        editor.doc.get_mut(b).unwrap().shape=Shape::Polyline{
            points:vec![Point{x:0.,y:0.},Point{x:50.,y:0.}],closed:false};
        let before=editor.doc.clone();
        editor.apply_boolean(BooleanOperation::Union);
        assert_eq!(editor.doc,before);
        assert!(editor.undo.is_empty());
    }
    #[test] fn grouped_objects_must_be_ungrouped_first(){
        let (mut editor,a,b)=setup();
        editor.doc.group_objects(&[a,b]).unwrap();
        let before=editor.doc.clone();
        editor.apply_boolean(BooleanOperation::Union);
        assert_eq!(editor.doc,before);
        assert!(editor.undo.is_empty());
    }
}
