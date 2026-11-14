//! Corel-style, independently implemented workspace layout.
//! Uses familiar panel positions and workflows but no proprietary assets.
use super::{color, Editor, Tool};
use eframe::egui::{self, Color32, RichText, Vec2};
use opencorel_core::{Bounds, Shape, valid_paint};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum InspectorTab { Objects, Properties, Page }

const SWATCHES: [&str; 27] = [
    "#FFFFFF", "#E8E8E8", "#C0C0C0", "#808080", "#333333", "#000000",
    "#7A0018", "#E02020", "#FF8B00", "#FFD523", "#8AC440", "#16A05A",
    "#067E65", "#00B5CA", "#117CC1", "#1244A5", "#553DB6", "#9D42AA",
    "#ED52A9", "#F5B5B5", "#F8D5AB", "#FFF2B3", "#D1ECC3", "#B7E7EC",
    "#B9CDEC", "#E4C9F1", "#725440",
];

impl Editor {
    pub(super) fn change_tool(&mut self, next: Tool) {
        self.tool=next;
        if next!=Tool::Curve { self.curve_start=None; self.curve_draft=None; }
        self.drag_start=None;
        self.creation_start=None;
    }

    pub(super) fn new_document(&mut self) {
        self.checkpoint();
        self.doc=Default::default();
        self.selected=None;
        self.active_layer=1;
        self.pan=Vec2::ZERO;
        self.zoom=1.0;
        self.curve_start=None;
        self.curve_draft=None;
        self.file="untitled.ocd".into();
        self.status="Создан новый документ".into();
    }

    pub(super) fn top_workspace(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("main_menubar").exact_height(28.0).show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.label(RichText::new("OpenCorel").strong().size(15.0));
                ui.separator();
                ui.menu_button("Файл",|ui|{
                    if ui.button("Новый                 Ctrl+N").clicked(){self.new_document();ui.close_menu();}
                    if ui.button("Открыть OCD      Ctrl+O").clicked(){self.open();ui.close_menu();}
                    if ui.button("Импорт SVG").clicked(){self.import_svg();ui.close_menu();}
                    if ui.button("Сохранить           Ctrl+S").clicked(){self.save();ui.close_menu();}
                    if ui.button("Экспорт SVG").clicked(){self.svg();ui.close_menu();}
                });
                ui.menu_button("Правка",|ui|{
                    if ui.add_enabled(!self.undo.is_empty(),egui::Button::new("Отменить      Ctrl+Z")).clicked(){self.undo();ui.close_menu();}
                    if ui.add_enabled(!self.redo.is_empty(),egui::Button::new("Повторить     Ctrl+Y")).clicked(){self.redo();ui.close_menu();}
                    if ui.button("Дублировать     Ctrl+D").clicked(){self.duplicate_selection();ui.close_menu();}
                    if ui.button("Удалить       Del").clicked(){self.delete_selection();ui.close_menu();}
                });
                ui.menu_button("Вид",|ui|{
                    if ui.button("Показать всю страницу  F4").clicked(){self.fit_page();ui.close_menu();}
                    ui.checkbox(&mut self.show_grid,"Сетка");
                    ui.checkbox(&mut self.snap_grid,"Привязка к сетке");
                    ui.checkbox(&mut self.show_rulers,"Линейки");
                    ui.checkbox(&mut self.light_theme,"Светлая тема");
                });
                ui.menu_button("Макет",|ui|{
                    if ui.button("A4 — 210×297 мм").clicked(){self.page_size(210.0,297.0);ui.close_menu();}
                    if ui.button("A3 — 297×420 мм").clicked(){self.page_size(297.0,420.0);ui.close_menu();}
                    if ui.button("A1 — 594×841 мм").clicked(){self.page_size(594.0,841.0);ui.close_menu();}
                    if ui.button("Баннер 3×2 м").clicked(){self.page_size(3000.0,2000.0);ui.close_menu();}
                });
                ui.menu_button("Объект",|ui|{
                    if ui.button("На передний план").clicked(){self.reorder_selection(true);ui.close_menu();}
                    if ui.button("На задний план").clicked(){self.reorder_selection(false);ui.close_menu();}
                    if ui.button("Дублировать").clicked(){self.duplicate_selection();ui.close_menu();}
                });
                ui.menu_button("Окно",|ui|{
                    ui.selectable_value(&mut self.inspector_tab,InspectorTab::Objects,"Объекты и слои");
                    ui.selectable_value(&mut self.inspector_tab,InspectorTab::Properties,"Свойства");
                    ui.selectable_value(&mut self.inspector_tab,InspectorTab::Page,"Страница");
                });
                ui.menu_button("Справка",|ui|{
                    ui.label("OpenCorel — прототип векторного редактора");
                    ui.label("CDR, CMYK и PDF/X ещё не реализованы.");
                    ui.hyperlink_to("Исходники и задачи","https://github.com/konstive-collab/OpenCorel");
                });
            });
        });
        egui::TopBottomPanel::top("main_toolbar").exact_height(40.0).show(ctx,|ui|{
            ui.horizontal(|ui|{
                if ui.button("▤ Новый").on_hover_text("Ctrl+N").clicked(){self.new_document();}
                if ui.button("▣ Открыть").on_hover_text("Ctrl+O").clicked(){self.open();}
                if ui.button("▣ Сохранить").on_hover_text("Ctrl+S").clicked(){self.save();}
                ui.separator();
                if ui.add_enabled(!self.undo.is_empty(),egui::Button::new("↶")).on_hover_text("Отменить").clicked(){self.undo();}
                if ui.add_enabled(!self.redo.is_empty(),egui::Button::new("↷")).on_hover_text("Повторить").clicked(){self.redo();}
                ui.separator();
                if ui.button("Импорт SVG").clicked(){self.import_svg();}
                if ui.button("SVG ↗").on_hover_text("Экспорт SVG").clicked(){self.svg();}
                ui.separator();
                ui.label("Масштаб");
                if ui.small_button("−").clicked(){self.zoom=(self.zoom/1.25).max(0.2);}
                ui.label(format!("{:.0}%",self.zoom*100.0));
                if ui.small_button("+").clicked(){self.zoom=(self.zoom*1.25).min(8.0);}
                if ui.small_button("Страница").clicked(){self.fit_page();}
                ui.separator();
                ui.checkbox(&mut self.snap_grid,"Привязка");
                ui.checkbox(&mut self.show_grid,"Сетка");
            });
        });
        egui::TopBottomPanel::top("property_bar").exact_height(39.0).show(ctx,|ui|{
            ui.horizontal(|ui|{
                ui.label(RichText::new("Документ").strong());
                ui.add(egui::TextEdit::singleline(&mut self.file).desired_width(230.0));
                ui.separator();
                if let Some(id)=self.selected {
                    if let Some(object)=self.doc.objects.iter().find(|o|o.id==id){
                        let b=object.shape.bounds();
                        ui.label(format!("X {:.1}   Y {:.1}   W {:.1}   H {:.1} мм",b.x,b.y,b.w,b.h));
                    }
                } else {
                    ui.label(format!("Лист: {:.1} × {:.1} мм",self.doc.page.width_mm,self.doc.page.height_mm));
                }
                ui.separator();
                ui.label("Ед.: мм");
                ui.separator();
                ui.label(if self.snap_grid{"Привязка: 10 мм"}else{"Без привязки"});
            });
        });
    }

    pub(super) fn tool_sidebar(&mut self,ctx:&egui::Context){
        egui::SidePanel::left("vertical_toolbox").exact_width(49.0).resizable(false).show(ctx,|ui|{
            ui.add_space(6.0);
            for (tool,symbol,hint) in [
                (Tool::Select,"↖","Выделение — V"),
                (Tool::Curve,"⌁","Кривая Безье — B"),
                (Tool::Rect,"▭","Прямоугольник — F6"),
                (Tool::Ellipse,"◯","Эллипс — F7"),
                (Tool::Text,"T","Текст — F8"),
            ] {
                let response=ui.add_sized([34.0,35.0],
                    egui::Button::new(RichText::new(symbol).size(19.0))
                    .selected(self.tool==tool));
                if response.on_hover_text(hint).clicked(){self.change_tool(tool);}
            }
            ui.separator();
            if ui.small_button("⌕").on_hover_text("Страница целиком — F4").clicked(){self.fit_page();}
            ui.separator();
            ui.label(RichText::new("mm").small());
        });
    }

    pub(super) fn color_palette(&mut self,ctx:&egui::Context){
        egui::SidePanel::right("color_palette").exact_width(42.0).resizable(false).show(ctx,|ui|{
            ui.vertical_centered(|ui|{
                ui.label(RichText::new("Цвет").small());
                for swatch in SWATCHES {
                    let response=ui.add_sized([28.0,20.0],
                        egui::Button::new("").fill(color(swatch)))
                        .on_hover_text(format!("{swatch}\nЛКМ: заливка\nПКМ: контур"));
                    if response.clicked(){self.apply_color(swatch,false);}
                    if response.clicked_by(egui::PointerButton::Secondary){self.apply_color(swatch,true);}
                }
                ui.label("×");
                if ui.small_button("Нет").on_hover_text("Без заливки").clicked(){self.apply_color("none",false);}
            });
        });
    }

    pub(super) fn inspector(&mut self,ctx:&egui::Context){
        egui::SidePanel::right("inspector_docker").default_width(272.0)
            .width_range(220.0..=450.0).resizable(true).show(ctx,|ui|{
            ui.horizontal(|ui|{
                ui.selectable_value(&mut self.inspector_tab,InspectorTab::Objects,"Объекты");
                ui.selectable_value(&mut self.inspector_tab,InspectorTab::Properties,"Свойства");
                ui.selectable_value(&mut self.inspector_tab,InspectorTab::Page,"Лист");
            });
            ui.separator();
            match self.inspector_tab{
                InspectorTab::Objects=>self.inspector_objects(ui),
                InspectorTab::Properties=>self.inspector_properties(ui),
                InspectorTab::Page=>self.inspector_page(ui),
            }
        });
    }

    fn inspector_objects(&mut self,ui:&mut egui::Ui){
        ui.horizontal(|ui|{
            ui.heading("Слои");
            if ui.small_button("+").on_hover_text("Создать слой").clicked(){
                self.checkpoint();
                let name=format!("Слой {}",self.doc.layers.len()+1);
                self.active_layer=self.doc.add_layer(name);
            }
        });
        let layers:Vec<_>=self.doc.layers.iter().map(|l|(l.id,l.name.clone(),l.visible,l.locked)).collect();
        for (id,name,mut visible,mut locked) in layers{
            ui.horizontal(|ui|{
                if ui.selectable_label(self.active_layer==id,format!("{} {}",if visible{"◉"}else{"○"},name)).clicked(){
                    self.active_layer=id;
                }
                if ui.checkbox(&mut visible,"").on_hover_text("Видимость слоя").changed(){
                    self.checkpoint();
                    if let Some(layer)=self.doc.layers.iter_mut().find(|l|l.id==id){layer.visible=visible;}
                }
                if ui.checkbox(&mut locked,"🔒").on_hover_text("Блокировать слой").changed(){
                    self.checkpoint();
                    if let Some(layer)=self.doc.layers.iter_mut().find(|l|l.id==id){layer.locked=locked;}
                }
            });
        }
        ui.separator();
        ui.label(RichText::new("Объекты").strong());
        let count=self.doc.objects.len();
        // Virtualized rows: remain responsive when designs contain thousands of objects.
        egui::ScrollArea::vertical().id_salt("objects_list").show_rows(ui,23.0,count,|ui,rows|{
            for row in rows{
                let index=count-1-row;
                let obj=&self.doc.objects[index];
                let id=obj.id;
                let layer_id=obj.layer_id;
                let name=match &obj.shape{
                    Shape::Rectangle{..}=>"▭  Прямоугольник".to_string(),
                    Shape::Ellipse{..}=>"◯  Эллипс".into(),
                    Shape::Text{value,..}=>format!("T  {}",value.chars().take(24).collect::<String>()),
                    Shape::Bezier{..}=>"⌁  Кривая Безье".into(),
                    Shape::Polyline{..}=>"⌁  Линия".into(),
                };
                let visible=self.doc.layer(layer_id).map(|l|l.visible).unwrap_or(false);
                if ui.add_enabled(visible,egui::Button::new(name)
                    .selected(self.selected==Some(id)).min_size(Vec2::new(ui.available_width(),20.0)))
                    .clicked(){
                    self.selected=Some(id);
                    self.active_layer=layer_id;
                    self.inspector_tab=InspectorTab::Properties;
                }
            }
        });
    }

    fn inspector_properties(&mut self,ui:&mut egui::Ui){
        let Some(id)=self.selected else{
            ui.heading("Свойства объекта");
            ui.label("Выделите элемент на листе или во вкладке «Объекты».");
            if self.tool==Tool::Text{
                ui.label("Содержимое нового текстового объекта:");
                ui.text_edit_multiline(&mut self.text);
            }
            return;
        };
        let Some(index)=self.doc.objects.iter().position(|o|o.id==id) else{
            self.selected=None;return;
        };
        let mut object=self.doc.objects[index].clone();
        let editing_allowed=self.doc.layer(object.layer_id).map(|l|l.visible&&!l.locked).unwrap_or(false);
        ui.heading(format!("Объект #{id}"));
        if !editing_allowed {ui.colored_label(Color32::RED,"Слой скрыт или заблокирован");}
        ui.add_enabled_ui(editing_allowed,|ui|{
            let mut changed=false;
            match &mut object.shape{
                Shape::Rectangle{x,y,w,h}|Shape::Ellipse{x,y,w,h}=>{
                    for (label,value) in [("X мм",x),("Y мм",y),("Ширина мм",w),("Высота мм",h)]{
                        ui.horizontal(|ui|{
                            ui.label(label);
                            changed|=ui.add(egui::DragValue::new(value).speed(1.0)).changed();
                        });
                    }
                    *w=w.max(0.1);
                    *h=h.max(0.1);
                }
                Shape::Text{x,y,value,size_mm}=>{
                    ui.label("Текст:");
                    changed|=ui.text_edit_multiline(value).changed();
                    for (label,num) in [("X мм",x),("Y мм",y),("Размер мм",size_mm)]{
                        ui.horizontal(|ui|{
                            ui.label(label);
                            changed|=ui.add(egui::DragValue::new(num).speed(0.5)).changed();
                        });
                    }
                    *size_mm=size_mm.max(0.1);
                }
                Shape::Polyline{points,closed}=>{
                    ui.label(format!("Узлов: {}",points.len()));
                    changed|=ui.checkbox(closed,"Замкнуто").changed();
                }
                Shape::Bezier{start,segments,closed}=>{
                    ui.label(format!("Сегментов: {}",segments.len()));
                    changed|=ui.checkbox(closed,"Замкнуто").changed();
                    changed|=ui.add(egui::DragValue::new(&mut start.x).prefix("X: ")).changed();
                    changed|=ui.add(egui::DragValue::new(&mut start.y).prefix("Y: ")).changed();
                    for (i,seg) in segments.iter_mut().enumerate(){
                        ui.collapsing(format!("Узел {}",i+1),|ui|{
                            for (label,p) in [("Контроль 1",&mut seg.control1),
                                ("Контроль 2",&mut seg.control2),("Конец",&mut seg.end)]{
                                ui.label(label);
                                changed|=ui.add(egui::DragValue::new(&mut p.x).prefix("X: ")).changed();
                                changed|=ui.add(egui::DragValue::new(&mut p.y).prefix("Y: ")).changed();
                            }
                        });
                    }
                }
            }
            ui.separator();
            ui.label("Заливка: #RRGGBB или none");
            changed|=ui.text_edit_singleline(&mut object.style.fill).changed();
            ui.label("Контур: #RRGGBB или none");
            changed|=ui.text_edit_singleline(&mut object.style.stroke).changed();
            ui.label("Толщина обводки (мм)");
            changed|=ui.add(egui::DragValue::new(&mut object.style.stroke_width_mm)
                .speed(0.1).range(0.0..=1000.0)).changed();
            if changed{
                if valid_paint(&object.style.fill)&&valid_paint(&object.style.stroke){
                    self.checkpoint();
                    self.doc.objects[index]=object.clone();
                }else{
                    self.status="Цвет задаётся в виде #RRGGBB или none".into();
                }
            }
            ui.separator();
            ui.horizontal(|ui|{
                if ui.button("Дублировать").clicked(){self.duplicate_selection();}
                if ui.button("Удалить").clicked(){self.delete_selection();}
            });
            ui.horizontal(|ui|{
                if ui.small_button("На передний план").clicked(){self.reorder_selection(true);}
                if ui.small_button("На задний план").clicked(){self.reorder_selection(false);}
            });
        });
    }

    fn inspector_page(&mut self,ui:&mut egui::Ui){
        ui.heading("Параметры листа");
        ui.label("Наименование документа");
        let mut name=self.doc.name.clone();
        if ui.text_edit_singleline(&mut name).changed(){
            self.checkpoint();self.doc.name=name;
        }
        let (mut w,mut h)=(self.doc.page.width_mm,self.doc.page.height_mm);
        ui.label("Размер в миллиметрах:");
        let width=ui.add(egui::DragValue::new(&mut w).speed(1.0).range(1.0..=100_000.0)).changed();
        let height=ui.add(egui::DragValue::new(&mut h).speed(1.0).range(1.0..=100_000.0)).changed();
        if width || height{self.page_size(w,h);}
        ui.horizontal_wrapped(|ui|{
            if ui.small_button("A4").clicked(){self.page_size(210.0,297.0);}
            if ui.small_button("A3").clicked(){self.page_size(297.0,420.0);}
            if ui.small_button("A1").clicked(){self.page_size(594.0,841.0);}
            if ui.small_button("3×2 м").clicked(){self.page_size(3000.0,2000.0);}
        });
        ui.separator();
        ui.checkbox(&mut self.show_grid,"Показать сетку");
        ui.checkbox(&mut self.snap_grid,"Привязка к сетке 10 мм");
        ui.checkbox(&mut self.show_rulers,"Показать линейки");
        ui.label("Один редактируемый лист. Многостраничность — следующий этап.");
    }

    pub(super) fn workspace_footer(&mut self,ctx:&egui::Context){
        egui::TopBottomPanel::bottom("statusbar").exact_height(26.0).show(ctx,|ui|{
            ui.horizontal(|ui|{
                ui.label(&self.status);
                ui.separator();
                let pos=if let Some(p)=self.cursor_mm{
                    format!("X: {:.1} мм   Y: {:.1} мм",p.x,p.y)
                }else{"X: —   Y: —".into()};
                ui.label(pos);
                ui.separator();
                ui.label(format!("Объектов: {}",self.doc.objects.len()));
            });
        });
        egui::TopBottomPanel::bottom("page_navigator").exact_height(29.0).show(ctx,|ui|{
            ui.horizontal(|ui|{
                ui.label("◀  ▶");
                ui.add(egui::Button::new(RichText::new("Страница 1").strong()).selected(true));
                ui.separator();
                ui.label("Цветовая палитра документа");
                for swatch in SWATCHES.iter().take(14){
                    let res=ui.add_sized([16.0,16.0],egui::Button::new("").fill(color(swatch)))
                        .on_hover_text(format!("{swatch} — заливка"));
                    if res.clicked(){self.apply_color(swatch,false);}
                }
            });
        });
    }

    pub(super) fn page_size(&mut self,w:f32,h:f32){
        if !w.is_finite()||!h.is_finite()||w<1.0||h<1.0||w>100_000.0||h>100_000.0{return;}
        if self.doc.page.width_mm==w && self.doc.page.height_mm==h{return;}
        self.checkpoint();
        self.doc.page.width_mm=w;
        self.doc.page.height_mm=h;
    }

    pub(super) fn fit_page(&mut self){
        self.zoom=1.0;
        self.pan=Vec2::ZERO;
    }

    fn apply_color(&mut self,value:&str,is_stroke:bool){
        if !valid_paint(value){return;}
        let Some(id)=self.selected else {
            self.status="Сначала выберите объект".into();
            return;
        };
        let layer_id=self.doc.objects.iter().find(|o|o.id==id).map(|o|o.layer_id);
        if !layer_id.and_then(|layer|self.doc.layer(layer)).map(|l|l.visible&&!l.locked).unwrap_or(false){
            self.status="Слой недоступен для редактирования".into();
            return;
        }
        self.checkpoint();
        if let Some(object)=self.doc.get_mut(id){
            if is_stroke{
                object.style.stroke=value.into();
                if value!="none" && object.style.stroke_width_mm==0.0 {
                    object.style.stroke_width_mm=0.5;
                }
            }else{object.style.fill=value.into();}
        }
    }

    pub(super) fn delete_selection(&mut self){
        let Some(id)=self.selected else {return;};
        let layer_id=self.doc.objects.iter().find(|o|o.id==id).map(|o|o.layer_id);
        if !layer_id.and_then(|layer|self.doc.layer(layer)).map(|l|l.visible&&!l.locked).unwrap_or(false){
            self.status="Заблокированный слой".into();return;
        }
        self.checkpoint();
        self.doc.remove(id);
        self.selected=None;
    }

    pub(super) fn duplicate_selection(&mut self){
        let Some(id)=self.selected else {return;};
        let Some(obj)=self.doc.objects.iter().find(|o|o.id==id).cloned() else {return;};
        let allowed=self.doc.layer(obj.layer_id).map(|l|l.visible&&!l.locked).unwrap_or(false);
        if !allowed{self.status="Заблокированный слой".into();return;}
        let mut shape=obj.shape;
        shape.translate(10.0,10.0);
        self.checkpoint();
        let new_id=self.doc.add_to_layer(shape,obj.style,obj.layer_id);
        self.selected=Some(new_id);
    }

    pub(super) fn reorder_selection(&mut self,front:bool){
        let Some(id)=self.selected else{return;};
        let Some(index)=self.doc.objects.iter().position(|o|o.id==id) else{return;};
        let layer=self.doc.objects[index].layer_id;
        if !self.doc.layer(layer).map(|l|l.visible&&!l.locked).unwrap_or(false){return;}
        self.checkpoint();
        let obj=self.doc.objects.remove(index);
        if front{self.doc.objects.push(obj);}else{self.doc.objects.insert(0,obj);}
    }
}
