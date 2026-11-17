//! Production text-on-path command: preserve editable text and guide,
//! generate editable native cubic outlines as one undoable object.
use super::Editor;
use eframe::egui;
use opencorel_core::Shape;
use opencorel_typography::path_text::TextPathOptions;

impl Editor{
    pub(super) fn selection_is_text_plus_guide(&self)->bool{
        let ids=self.selection_ids();
        if ids.len()!=2||ids[0]==ids[1]{return false;}
        let Some(a)=self.doc.objects.iter().find(|o|o.id==ids[0])else{return false;};
        let Some(b)=self.doc.objects.iter().find(|o|o.id==ids[1])else{return false;};
        let is_guide=|shape:&Shape|matches!(shape,
            Shape::Polyline{..}|Shape::Bezier{..}|Shape::CompoundPath{..}|
            Shape::Rectangle{..}|Shape::Ellipse{..});
        matches!(a.shape,Shape::Text{..})&&is_guide(&b.shape)||
            matches!(b.shape,Shape::Text{..})&&is_guide(&a.shape)
    }
    pub(super) fn create_text_on_path(&mut self){
        if !self.selection_is_text_plus_guide(){
            self.status="Выберите ровно два объекта: редактируемый текст и направляющую (контур, прямоугольник или эллипс)".into();
            return;
        }
        let ids=self.selection_ids();
        let selected:Vec<_>=ids.iter().filter_map(|id|
            self.doc.objects.iter().find(|o|o.id==*id).cloned()).collect();
        if selected.len()!=2||selected.iter().any(|o|
            o.group_id.is_some()||
            !self.doc.layer(o.layer_id).is_some_and(|l|l.visible&&!l.locked))||
            self.doc.clip_links.iter().any(|link|
                ids.contains(&link.source_id)||ids.contains(&link.target_id)){
            self.status="Текст и направляющая должны быть доступными объектами вне групп и обтравки".into();
            return;
        }
        let Some(text)=selected.iter().find(|o|matches!(o.shape,Shape::Text{..}))
            else{return;};
        let Some(guide)=selected.iter().find(|o|o.id!=text.id)else{return;};
        let options=TextPathOptions{
            start_offset_mm:self.text_path_start_mm,
            baseline_shift_mm:self.text_path_shift_mm,
            reverse_guide:self.text_path_reverse,
        };
        let geometry=match self.text_outliner.outline_on_path(text,&guide.shape,options){
            Ok(s)=>s,
            Err(e)=>{self.status=format!("Текст по кривой: {e}");return;}
        };
        let mut preview=self.doc.clone();
        if preview.next_id>=u64::MAX-1{
            self.status="Достигнут предел идентификаторов документа".into();
            return;
        }
        let id=preview.add_to_layer(geometry,text.style.clone(),text.layer_id);
        if let Some(appearance)=self.doc.appearances.get(&text.id).cloned(){
            preview.appearances.insert(id,appearance);
        }
        let Some(obj)=preview.get_mut(id)else{return;};
        obj.name=format!("Текст по кривой ({})",text.id);
        if let Err(e)=preview.validate(){
            self.status=format!("Создание текста по кривой отклонено: {e}");
            return;
        }
        self.checkpoint();
        self.doc=preview;
        self.selected=Some(id);self.multi_selected.clear();
        self.active_node=None;self.node_drag=None;
        self.node_drag_checkpointed=false;
        self.text_preview.clear();self.fill_cache.entries.clear();
        self.compound_cache.clear();self.vector_cache.clear();
        self.bezier_preview.clear();self.raster_cache.clear();
        self.status="Текст вдоль кривой создан как редактируемые векторные контуры. Исходные текст и направляющая сохранены. Ctrl+Z — отмена.".into();
    }
    pub(super) fn text_path_controls(&mut self,ui:&mut egui::Ui){
        ui.label("Выделите текст + кривую/эллипс/прямоугольник (Shift).");
        ui.horizontal(|ui|{
            ui.label("Начальный отступ, мм");
            ui.add(egui::DragValue::new(&mut self.text_path_start_mm)
                .speed(0.5).range(0.0..=2_000_000.0).suffix(" мм"));
        });
        ui.horizontal(|ui|{
            ui.label("Сдвиг от контура, мм");
            ui.add(egui::DragValue::new(&mut self.text_path_shift_mm)
                .speed(0.2).range(-5000.0..=5000.0).suffix(" мм"));
        });
        ui.checkbox(&mut self.text_path_reverse,"Обратить направление кривой");
        if ui.add_enabled(self.selection_is_text_plus_guide(),
            egui::Button::new("Создать текст вдоль кривой")).clicked(){
            self.create_text_on_path();
        }
        ui.small("Настоящие контуры глифов, повёрнутые по касательной к кривой. Исходная надпись остаётся редактируемой; векторная копия не обновляется автоматически.");
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Document,Point,Style};
    #[test]
    fn selection_requires_exactly_one_text_and_one_guide(){
        let mut e=Editor::default();e.doc=Document::default();
        let text=e.doc.add(Shape::Text{x:0.,y:0.,
            value:"A".into(),size_mm:8.},Style::default());
        let guide=e.doc.add(Shape::Polyline{points:vec![
            Point{x:0.,y:20.},Point{x:100.,y:20.}],closed:false},
            Style::default());
        e.selected=Some(text);e.multi_selected=vec![guide];
        assert!(e.selection_is_text_plus_guide());
        e.multi_selected.clear();
        assert!(!e.selection_is_text_plus_guide());
        e.selected=Some(guide);e.multi_selected=vec![text];
        assert!(e.selection_is_text_plus_guide());
    }
    #[test]
    fn invalid_inputs_are_rejected_without_any_checkpoint(){
        let mut e=Editor::default();e.doc=Document::default();
        let text=e.doc.add(Shape::Text{x:0.,y:0.,
            value:"text".into(),size_mm:10.},Style::default());
        let guide=e.doc.add(Shape::Polyline{points:vec![
            Point{x:0.,y:20.},Point{x:40.,y:20.}],closed:false},
            Style::default());
        e.selected=Some(text);e.multi_selected=vec![guide];
        e.doc.layers[0].locked=true;
        let before=e.doc.clone();
        e.create_text_on_path();
        assert_eq!(e.doc,before);
        assert!(e.undo.is_empty());
        e.doc.layers[0].locked=false;
        e.text_path_start_mm=f32::NAN;
        let before=e.doc.clone();
        e.create_text_on_path();
        assert_eq!(e.doc,before);
        assert!(e.undo.is_empty());
    }
    #[test]
    fn no_font_or_missing_glyph_never_changes_source(){
        let mut e=Editor::default();e.doc=Document::default();
        let text=e.doc.add(Shape::Text{x:0.,y:0.,
            value:"Hello".into(),size_mm:7.},Style::default());
        e.doc.get_mut(text).unwrap().text_format.family=
            "AnImaginaryFontThatCannotExist-1234".into();
        let guide=e.doc.add(Shape::Ellipse{x:0.,y:0.,w:90.,h:60.},
            Style::default());
        e.selected=Some(text);e.multi_selected=vec![guide];
        let original=e.doc.clone();
        e.create_text_on_path();
        assert_eq!(e.doc,original);
        assert!(e.undo.is_empty());
    }
}
