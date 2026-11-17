//! Smart Shape Studio: 93 professional page-space vector symbols.
//! The same Rust geometry engine is used by pointer dragging, parameterized
//! conversion, keyboard-driven creation and direct canvas placement.
use super::{Editor,Tool};
use eframe::egui::{self,RichText};
use opencorel_core::{Bounds,Point,Shape,Style};
use opencorel_core::smart_shapes::{self,Category,Options,SmartShape};
use std::collections::HashSet;

impl Editor{
    fn smart_options(&self)->Options{
        Options{
            inset:self.smart_inset,
            detail:self.smart_detail,
            rotation_degrees:self.smart_rotation_degrees,
        }
    }
    fn smart_shape_style(kind:SmartShape)->Style{
        if kind.is_open(){
            Style{fill:"none".into(),stroke:"#262D3B".into(),
                stroke_width_mm:1.0}
        }else{Style::default()}
    }
    /// Mouse-down places an initial editable native shape. The single undo
    /// entry is created by add_shape; mouse-move regenerates its geometry
    /// in-place without recording history or allocating new object IDs.
    pub(super) fn start_smart_creation(&mut self,start:Point){
        let b=Bounds{x:start.x,y:start.y,w:1.0,h:1.0};
        match smart_shapes::create(self.smart_kind,b,self.smart_options()){
            Ok(shape)=>{
                if let Some(id)=self.add_shape(shape,Self::smart_shape_style(self.smart_kind)){
                    if let Some(obj)=self.doc.get_mut(id){
                        obj.name=self.smart_kind.label().into();
                    }
                    self.creation_start=Some((start,id));
                    self.status=format!("{} — потяните мышь для размера. Ctrl+Z — отмена.",
                        self.smart_kind.label());
                }
            }
            Err(e)=>self.status=format!("Фигура не создана: {e}"),
        }
    }
    /// Regenerate the in-progress path, preserving fill and editable layer.
    pub(super) fn resize_smart_creation(&mut self,id:u64,bounds:Bounds){
        let kind=self.smart_kind;
        match smart_shapes::create(kind,bounds,self.smart_options()){
            Ok(shape)=>{
                if let Some(obj)=self.doc.get_mut(id){
                    obj.shape=shape;
                }
            },
            Err(e)=>self.status=format!("Размер фигуры отклонён: {e}"),
        }
    }
    /// Place one standard-size smart shape at an exact point.
    /// Unlike drag-start, a click does not leave an unfinished drag session.
    pub(super) fn place_smart_at(&mut self,top_left:Point){
        let b=Bounds{x:top_left.x,y:top_left.y,
            w:self.smart_width_mm,h:self.smart_height_mm};
        match smart_shapes::create(self.smart_kind,b,self.smart_options()){
            Ok(shape)=>{
                if let Some(id)=self.add_shape(shape,Self::smart_shape_style(self.smart_kind)){
                    if let Some(obj)=self.doc.get_mut(id){
                        obj.name=self.smart_kind.label().into();
                    }
                    self.status=format!("{}: создана по центру страницы. Ctrl+Z — отмена.",
                        self.smart_kind.label());
                }
            }
            Err(e)=>self.status=format!("Не удалось создать фигуру: {e}"),
        }
    }
    pub(super) fn insert_smart_in_center(&mut self){
        let at=Point{
            x:(self.doc.page.width_mm-self.smart_width_mm)/2.,
            y:(self.doc.page.height_mm-self.smart_height_mm)/2.,
        };
        self.place_smart_at(at);
    }

    /// Batch transform only editable native vector paths, retaining all
    /// styling, layer, z-order and appearance metadata. No images or text are
    /// silently destroyed. Preview clone ensures full atomicity.
    pub(super) fn retype_selected_shapes(&mut self){
        let ids:HashSet<u64>=self.selection_ids().into_iter().collect();
        if ids.is_empty(){
            self.status="Сначала выделите одну или несколько фигур".into();return;
        }
        if ids.len()>800{
            self.status="Преобразование ограничено 800 объектами".into();return;
        }
        if self.doc.objects.iter().filter(|o|ids.contains(&o.id)).count()!=ids.len()||
            self.doc.objects.iter().filter(|o|ids.contains(&o.id)).any(|o|
                o.group_id.is_some() || !o.visible || o.locked ||
                !self.doc.layer(o.layer_id).is_some_and(|l|l.visible&&!l.locked)||
                matches!(o.shape,Shape::Raster{..}|Shape::EmbeddedSvg{..}|Shape::Text{..}))||
            self.doc.clip_links.iter().any(|l|
                ids.contains(&l.source_id)||ids.contains(&l.target_id)){
            self.status="Замена допускает только доступные несгруппированные векторы без обтравки. Текст, растры и SVG не заменяются.".into();
            return;
        }
        let mut preview=self.doc.clone();
        for o in &mut preview.objects{
            if !ids.contains(&o.id){continue;}
            let b=o.bounds();
            if b.w<=0.1||b.h<=0.1{
                self.status="Вырожденная фигура не имеет достаточной площади".into();return;
            }
            let next=match smart_shapes::create(self.smart_kind,b,self.smart_options()){
                Ok(shape)=>shape,
                Err(e)=>{
                    self.status=format!("Фигура #{}: {e}",o.id);return;
                }
            };
            o.shape=next;
            o.node_modes.clear();
            if self.smart_kind.is_open(){
                o.style.fill="none".into();
                if o.style.stroke=="none"{
                    o.style.stroke="#262D3B".into();
                    o.style.stroke_width_mm=1.0;
                }
            }
            o.name=self.smart_kind.label().into();
        }
        if let Err(e)=preview.validate(){
            self.status=format!("Фигуры не заменены: {e}");return;
        }
        if preview==self.doc{return;}
        self.checkpoint();
        self.doc=preview;
        self.active_node=None;self.node_drag=None;
        self.node_drag_checkpointed=false;
        self.fill_cache.entries.clear();
        self.compound_cache.clear();
        self.vector_cache.clear();
        self.bezier_preview.clear();
        self.raster_cache.clear();
        self.status=format!("{}: преобразовано {} фигур без потери оформления. Ctrl+Z — отмена.",
            self.smart_kind.label(),ids.len());
    }
    pub(super) fn smart_shape_parameters(&mut self,ui:&mut egui::Ui){
        ui.label(RichText::new(self.smart_kind.label()).strong());
        ui.horizontal(|ui|{
            ui.label("Внутренний радиус / вырез");
            ui.add(egui::DragValue::new(&mut self.smart_inset)
                .range(0.1..=0.9).speed(0.01).fixed_decimals(2));
        });
        ui.horizontal(|ui|{
            ui.label("Детализация / частота");
            ui.add(egui::DragValue::new(&mut self.smart_detail)
                .range(3..=32).speed(1.0));
        });
        ui.horizontal(|ui|{
            ui.label("Поворот");
            ui.add(egui::DragValue::new(&mut self.smart_rotation_degrees)
                .range(-3600.0..=3600.0).speed(1.0).suffix("°"));
        });
        ui.horizontal(|ui|{
            ui.label("Ширина");
            ui.add(egui::DragValue::new(&mut self.smart_width_mm)
                .range(0.1..=50_000.0).speed(1.0).suffix(" мм"));
            ui.label("Высота");
            ui.add(egui::DragValue::new(&mut self.smart_height_mm)
                .range(0.1..=50_000.0).speed(1.0).suffix(" мм"));
        });
        ui.horizontal_wrapped(|ui|{
            if ui.button("Добавить в центр страницы").clicked(){
                self.insert_smart_in_center();
            }
            if ui.add_enabled(!self.selection_ids().is_empty(),
                egui::Button::new("Заменить выбранные векторы")).clicked(){
                self.retype_selected_shapes();
            }
        });
        ui.small("Все фигуры сохраняются как редактируемые нативные контуры. Замена пересоздаёт геометрию, но сохраняет заливку, слой и оформление.");
    }
    pub(super) fn smart_shape_picker(&mut self,ui:&mut egui::Ui){
        ui.label(RichText::new("БИБЛИОТЕКА ФИГУР · 93").strong());
        ui.add(egui::TextEdit::singleline(&mut self.smart_search)
            .hint_text("Найти фигуру..."));
        let query=self.smart_search.trim().to_lowercase();
        egui::ScrollArea::vertical().id_salt("smart_shape_flyout")
            .max_height(440.0).show(ui,|ui|{
                for cat in Category::ALL{
                    if !SmartShape::ALL.iter().any(|kind|
                        kind.category()==cat&&(query.is_empty()||
                            kind.label().to_lowercase().contains(&query))){
                        continue;
                    }
                    ui.collapsing(cat.label(),|ui|{
                        for kind in SmartShape::ALL{
                            if kind.category()!=cat||(!query.is_empty()&&
                                !kind.label().to_lowercase().contains(&query)){continue;}
                            if ui.selectable_label(self.smart_kind==kind,kind.label()).clicked(){
                                self.smart_kind=kind;
                                self.change_tool(Tool::Smart);
                                ui.close_menu();
                            }
                        }
                    });
                }
            });
        ui.separator();
        if ui.button("Создать все 93 фигуры на четырёх листах A4")
            .on_hover_text("Векторный каталог для просмотра, копирования и тестирования всех фигур")
            .clicked(){
            self.create_smart_catalog();
        }
        self.smart_shape_parameters(ui);
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::Document;
    #[test]
    fn smart_shape_draw_is_one_history_entry(){
        let mut e=Editor::default();
        e.doc=Document::default();
        e.smart_kind=SmartShape::Heart;
        e.start_smart_creation(Point{x:5.,y:6.});
        assert_eq!(e.doc.objects.len(),1);
        assert_eq!(e.undo.len(),1);
        let id=e.selected.unwrap();
        e.resize_smart_creation(id,Bounds{x:5.,y:6.,w:55.,h:66.});
        assert_eq!(e.undo.len(),1);
        assert!(e.doc.objects[0].bounds().w>40.);
        e.doc.validate().unwrap();
        e.undo();
        assert!(e.doc.objects.is_empty());
    }
    #[test]
    fn conversion_preserves_custom_style_layer_and_undo(){
        let mut e=Editor::default();
        e.doc=Document::default();
        let id=e.doc.add(Shape::Rectangle{x:20.,y:30.,w:40.,h:50.},
            Style{fill:"#AA3311".into(),stroke:"#113355".into(),
                stroke_width_mm:0.5});
        let original=e.doc.clone();
        e.selected=Some(id);
        e.smart_kind=SmartShape::Star12;
        e.retype_selected_shapes();
        assert!(matches!(e.doc.objects[0].shape,Shape::Polyline{..}));
        assert_eq!(e.doc.objects[0].style,original.objects[0].style);
        assert_eq!(e.doc.objects[0].layer_id,original.objects[0].layer_id);
        assert_eq!(e.undo.len(),1);
        e.undo();assert_eq!(e.doc,original);
    }
    #[test]
    fn conversion_rejects_locked_and_text_objects_without_history(){
        let mut e=Editor::default();
        e.doc=Document::default();
        let id=e.doc.add(Shape::Text{x:20.,y:20.,
            value:"Never destroy".into(),size_mm:6.},Style::default());
        e.selected=Some(id);
        let original=e.doc.clone();
        e.retype_selected_shapes();
        assert_eq!(e.doc,original);
        assert!(e.undo.is_empty());
        e.doc.objects.clear();
        let id=e.doc.add(Shape::Rectangle{x:1.,y:1.,w:10.,h:10.},
            Style::default());
        e.selected=Some(id);
        e.doc.layers[0].locked=true;
        let old=e.doc.clone();
        e.retype_selected_shapes();
        assert_eq!(e.doc,old);
        assert!(e.undo.is_empty());
    }
    #[test]
    fn open_smart_shapes_start_without_invisible_fills(){
        let mut e=Editor::default();
        e.doc=Document::default();
        e.smart_kind=SmartShape::Wave;
        e.start_smart_creation(Point{x:3.,y:3.});
        let obj=&e.doc.objects[0];
        assert_eq!(obj.style.fill,"none");
        assert_ne!(obj.style.stroke,"none");
        assert!(matches!(obj.shape,Shape::Polyline{closed:false,..}));
    }
}
