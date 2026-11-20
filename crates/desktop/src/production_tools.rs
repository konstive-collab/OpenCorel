//! Production tasks: offline QR artwork, RGB palette reconciliation and
//! multi-page object explorer. Every mutation uses the existing Undo history.
use super::{Editor,Tool};
use super::workspace::InspectorTab;
use eframe::egui::{self,Color32,Pos2,Rect,RichText,Vec2};
use opencorel_core::{Shape,Point};
use opencorel_core::qr_vector::{self,QrSpec,QrCorrection};
use opencorel_core::recolor::{ColorChannels,RecolorReport};
use opencorel_core::object_search::{ObjectQuery,ObjectKind,ObjectHit};

fn parse_hex(value:&str)->Option<Color32>{
    if !opencorel_core::valid_color(value){return None;}
    let get=|offset|u8::from_str_radix(&value[offset..offset+2],16).ok();
    Some(Color32::from_rgb(get(1)?,get(3)?,get(5)?))
}
impl Editor{
    fn qr_spec(&self)->QrSpec{
        QrSpec{
            payload:self.qr_text.clone(),
            origin:Point{x:0.0,y:0.0},
            size_mm:self.qr_size_mm,
            foreground:self.qr_foreground.clone(),
            background:self.qr_background.clone(),
            correction:match self.qr_ecc{
                0=>QrCorrection::Low,1=>QrCorrection::Medium,
                2=>QrCorrection::Quartile,_=>QrCorrection::High
            },
        }
    }
    pub(super) fn create_vector_qr(&mut self){
        let mut spec=self.qr_spec();
        spec.origin=Point{
            x:(self.doc.page.width_mm-spec.size_mm)*0.5,
            y:(self.doc.page.height_mm-spec.size_mm)*0.5
        };
        let mut candidate=self.doc.clone();
        let inserted=candidate.create_qr(&spec,self.active_layer).and_then(|ids|{
            candidate.validate()?;Ok(ids)
        });
        match inserted{
            Ok(ids)=>{
                self.checkpoint();
                self.doc=candidate;
                self.selected=Some(ids[0]);self.multi_selected=vec![ids[1]];
                self.fill_cache.entries.clear();
                self.vector_cache.clear();self.compound_cache.clear();
                self.masked_canvas=Default::default();
                self.show_qr_dialog=false;
                self.status="QR создан как два сгруппированных редактируемых вектора; Ctrl+Z — отмена".into();
            }
            Err(e)=>{self.qr_status=e.clone();self.status=format!("QR: {e}");}
        }
    }
    pub(super) fn qr_dialog(&mut self,ctx:&egui::Context){
        if !self.show_qr_dialog{return;}
        let mut open=true;
        let mut create=false;
        egui::Window::new("Векторный QR-код · Производство")
            .open(&mut open).resizable(true).default_width(465.0)
            .show(ctx,|ui|{
                ui.label(RichText::new("Содержимое QR (URL, текст, контакт)").strong());
                ui.add(egui::TextEdit::multiline(&mut self.qr_text)
                    .desired_rows(3).hint_text("https://..."));
                ui.small(format!("{} / 1024 UTF-8 байт",self.qr_text.len()));
                ui.horizontal(|ui|{
                    ui.label("Размер (мм)");
                    ui.add(egui::DragValue::new(&mut self.qr_size_mm)
                        .range(10.0..=2000.0).speed(2.0));
                    ui.label("Коррекция");
                    egui::ComboBox::from_id_salt("qr_ecc_mode")
                        .selected_text(match self.qr_ecc{
                            0=>"L · 7%",1=>"M · 15%",2=>"Q · 25%",_=>"H · 30%"
                        }).show_ui(ui,|ui|{
                            for (n,label) in ["L · 7%","M · 15%","Q · 25%","H · 30%"]
                                .iter().enumerate(){
                                ui.selectable_value(&mut self.qr_ecc,n as u8,*label);
                            }
                        });
                });
                ui.horizontal(|ui|{
                    ui.label("Тёмный #RGB");
                    ui.add(egui::TextEdit::singleline(&mut self.qr_foreground)
                        .desired_width(100.0));
                    ui.label("Фон #RGB");
                    ui.add(egui::TextEdit::singleline(&mut self.qr_background)
                        .desired_width(100.0));
                });
                ui.separator();
                let spec=self.qr_spec();
                match qr_vector::generate(&spec){
                    Ok(art)=>{
                        ui.label(format!("Модулей: {} × {} · размер модуля: {:.2} мм · контуров: {}",
                            art.matrix_side,art.matrix_side,art.effective_module_mm,art.runs));
                        let side=220.0;
                        let (rect,_)=ui.allocate_exact_size(Vec2::splat(side),
                            egui::Sense::hover());
                        let back=parse_hex(&spec.background).unwrap_or(Color32::WHITE);
                        let fore=parse_hex(&spec.foreground).unwrap_or(Color32::BLACK);
                        ui.painter().rect_filled(rect,0.0,back);
                        if let Shape::CompoundPath{contours}=art.modules{
                            let scale=side/spec.size_mm;
                            for path in contours{
                                let p=path.start;
                                let q=path.segments[1].end;
                                let rectangle=Rect::from_min_max(
                                    Pos2::new(rect.left()+p.x*scale,
                                        rect.top()+p.y*scale),
                                    Pos2::new(rect.left()+q.x*scale,
                                        rect.top()+q.y*scale));
                                ui.painter().rect_filled(rectangle,0.0,fore);
                            }
                        }
                        ui.small("Превью соответствует геометрии создаваемых контуров.");
                    }
                    Err(e)=>{ui.colored_label(Color32::from_rgb(221,113,85),e);}
                }
                if !self.qr_status.is_empty(){
                    ui.small(RichText::new(&self.qr_status).color(Color32::LIGHT_RED));
                }
                ui.separator();
                if ui.button("Создать редактируемый QR в центре листа").clicked(){
                    create=true;
                }
                ui.small("Обязательная светлая зона — 4 модуля. Не является QR-сервисом.");
            });
        self.show_qr_dialog=open;
        if create{self.create_vector_qr();}
    }
    fn recolor_channels(&self)->ColorChannels{
        ColorChannels{fills:self.recolor_fills,
            strokes:self.recolor_strokes,
            gradient_stops:self.recolor_stops}
    }
    pub(super) fn preview_recolor_action(&mut self){
        match self.doc.preview_recolor(
            &self.recolor_from,&self.recolor_to,
            self.recolor_channels(),self.recolor_all_pages){
            Ok(r)=>{
                self.recolor_summary=format!(
                    "Страниц {}, объектов {}, заливок {}, контуров {}, точек градиента {}",
                    r.pages,r.objects,r.fill_slots,r.stroke_slots,r.gradient_slots);
                self.status=format!("Предпросмотр изменения цветов: {} позиций",r.total());
            }
            Err(e)=>{self.recolor_summary=e.clone();self.status=format!("Цвет: {e}");}
        }
    }
    pub(super) fn apply_recolor_action(&mut self){
        let mut candidate=self.doc.clone();
        match candidate.recolor_rgb(&self.recolor_from,&self.recolor_to,
            self.recolor_channels(),self.recolor_all_pages)
            .and_then(|r|{candidate.validate()?;Ok(r)}){
            Ok(r) if r.total()==0=>self.status="Совпадений RGB нет".into(),
            Ok(r)=>{
                self.checkpoint();
                self.doc=candidate;
                self.fill_cache.entries.clear();
                self.vector_cache.clear();self.compound_cache.clear();
                self.masked_canvas=Default::default();
                self.recolor_summary=format!(
                    "Заменено {} цветовых позиций на {} страницах",
                    r.total(),r.pages);
                self.status=format!("RGB-палитра обновлена · Ctrl+Z — отмена ({})",r.total());
                self.inventory_lines.clear();
            },
            Err(e)=>{self.recolor_summary=e.clone();self.status=format!("Цвет: {e}");}
        }
    }
    pub(super) fn select_same_rgb(&mut self){
        match self.doc.objects_with_rgb(&self.recolor_from,self.recolor_channels()){
            Ok(ids)=>{
                self.selected=ids.first().copied();
                self.multi_selected=ids.into_iter().skip(1).collect();
                self.change_tool(Tool::Select);
                self.status=format!("Выделено совпадающих объектов: {}",self.selection_ids().len());
            }
            Err(e)=>self.status=format!("Выделение по RGB: {e}")
        }
    }
    pub(super) fn recolor_dialog(&mut self,ctx:&egui::Context){
        if !self.show_recolor_dialog{return;}
        let mut open=true;
        let mut preview=false;
        let mut apply=false;
        let mut select=false;
        egui::Window::new("Массовая замена RGB · все страницы")
            .open(&mut open).resizable(true).default_width(425.0)
            .show(ctx,|ui|{
                ui.label(RichText::new("Цветовая палитра публикации").strong());
                ui.horizontal(|ui|{
                    ui.label("Найти");
                    ui.add(egui::TextEdit::singleline(&mut self.recolor_from)
                        .desired_width(110.0));
                    ui.label("Заменить");
                    ui.add(egui::TextEdit::singleline(&mut self.recolor_to)
                        .desired_width(110.0));
                });
                ui.horizontal(|ui|{
                    for color in [&self.recolor_from,&self.recolor_to]{
                        if let Some(rgb)=parse_hex(color){
                            let (rect,_)=ui.allocate_exact_size(Vec2::new(130.,19.),
                                egui::Sense::hover());
                            ui.painter().rect_filled(rect,3.0,rgb);
                        }
                    }
                });
                ui.checkbox(&mut self.recolor_all_pages,"Все страницы (включая неактивные)");
                ui.horizontal_wrapped(|ui|{
                    ui.checkbox(&mut self.recolor_fills,"Заливки");
                    ui.checkbox(&mut self.recolor_strokes,"Обводки");
                    ui.checkbox(&mut self.recolor_stops,"Точки градиента");
                });
                ui.small("RGB #RRGGBB. Заблокированные и скрытые совпадения запрещают замену.");
                ui.horizontal_wrapped(|ui|{
                    if ui.button("Посчитать совпадения").clicked(){preview=true;}
                    if ui.button("Выделить совпадающие на листе").clicked(){select=true;}
                });
                if !self.recolor_summary.is_empty(){
                    ui.label(&self.recolor_summary);
                }
                ui.separator();
                if ui.button("Заменить во всём выбранном диапазоне · Ctrl+Z")
                    .clicked(){apply=true;}
                ui.small("Изображения и внедрённый SVG не перекрашиваются; данные KDR сохраняются.");
            });
        self.show_recolor_dialog=open;
        if preview{self.preview_recolor_action();}
        if select{self.select_same_rgb();}
        if apply{self.apply_recolor_action();}
    }
    fn object_query(&self)->ObjectQuery{
        ObjectQuery{
            phrase:self.find_object_text.clone(),
            layer_phrase:self.find_object_layer.clone(),
            rgb_color:if self.find_object_color.trim().is_empty(){None}
                else{Some(self.find_object_color.trim().into())},
            kind:ObjectKind::ALL.get(self.find_object_kind).copied()
                .unwrap_or(ObjectKind::Any),
            all_pages:self.find_object_all_pages,
            include_hidden:self.find_object_include_hidden,
        }
    }
    pub(super) fn run_object_search(&mut self){
        match self.doc.find_objects(&self.object_query()){
            Ok(hits)=>{
                self.status=format!("Найдено объектов: {}",hits.len());
                self.object_search_results=hits;
            }
            Err(e)=>{
                self.object_search_results.clear();
                self.status=format!("Поиск объектов: {e}");
            }
        }
    }
    pub(super) fn open_object_result(&mut self,page:u64,id:u64){
        if self.doc.active_page_id!=page{self.activate_page(page);}
        let Some(obj)=self.doc.get(id) else{
            self.status="Объект уже удалён или переименован".into();return;
        };
        let layer=obj.layer_id;
        self.change_tool(Tool::Select);
        self.selected=Some(id);
        self.multi_selected.clear();
        self.active_layer=layer;
        self.inspector_tab=InspectorTab::Properties;
        self.show_inspector=true;
        self.status=format!("Объект #{id}, страница {page}, слой #{layer}");
    }
    pub(super) fn object_finder_dialog(&mut self,ctx:&egui::Context){
        if !self.show_object_finder{return;}
        let mut open=true;
        let mut search=false;
        let mut chosen=None;
        egui::Window::new("Навигатор объектов · весь макет")
            .open(&mut open).resizable(true).default_width(560.0)
            .show(ctx,|ui|{
                ui.label(RichText::new("Имя, содержимое надписи, тип, слой и RGB").strong());
                ui.horizontal(|ui|{
                    ui.label("Название / текст");
                    ui.add(egui::TextEdit::singleline(&mut self.find_object_text)
                        .desired_width(310.0));
                });
                ui.horizontal(|ui|{
                    ui.label("Слой");
                    ui.add(egui::TextEdit::singleline(&mut self.find_object_layer)
                        .desired_width(160.0));
                    ui.label("RGB");
                    ui.add(egui::TextEdit::singleline(&mut self.find_object_color)
                        .desired_width(105.0));
                });
                egui::ComboBox::from_id_salt("object_kind_filter")
                    .selected_text(ObjectKind::ALL.get(self.find_object_kind)
                        .copied().unwrap_or(ObjectKind::Any).label())
                    .show_ui(ui,|ui|{
                        for (i,kind) in ObjectKind::ALL.iter().enumerate(){
                            ui.selectable_value(&mut self.find_object_kind,i,kind.label());
                        }
                    });
                ui.horizontal_wrapped(|ui|{
                    ui.checkbox(&mut self.find_object_all_pages,"Все страницы");
                    ui.checkbox(&mut self.find_object_include_hidden,"Включая скрытые");
                });
                if ui.button("Найти объекты").clicked(){search=true;}
                ui.separator();
                ui.label(format!("Найдено {} · показано до 160",
                    self.object_search_results.len()));
                let names:std::collections::HashMap<_,_>=self.doc.pages().into_iter()
                    .map(|p|(p.id,p.name)).collect();
                egui::ScrollArea::vertical().max_height(360.0).show(ui,|ui|{
                    for hit in self.object_search_results.iter().take(160){
                        let page=names.get(&hit.page_id).map(String::as_str)
                            .unwrap_or("Страница");
                        if ui.button(format!("{} · #{} · {} · {} [{}]",
                            page,hit.object_id,hit.kind,hit.name,hit.layer_name))
                            .on_hover_text("Перейти к объекту и открыть его свойства")
                            .clicked(){
                            chosen=Some((hit.page_id,hit.object_id));
                        }
                    }
                });
                ui.small("Результаты не меняют проект; переключение страницы — без шага Undo.");
            });
        self.show_object_finder=open;
        if search{self.run_object_search();}
        if let Some((page,id))=chosen{self.open_object_result(page,id);}
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn qr_creation_is_undoable_and_adds_only_two_grouped_objects(){
        let mut editor=Editor::default();
        let before=editor.doc.clone();
        editor.qr_text="https://example.org/shop".into();
        editor.create_vector_qr();
        assert_eq!(editor.doc.objects.len(),before.objects.len()+2);
        editor.doc.validate().unwrap();
        editor.undo();
        assert_eq!(editor.doc,before);
    }
    #[test] fn bulk_recolor_is_atomic_under_editor_undo(){
        let mut editor=Editor::default();
        let before=editor.doc.clone();
        editor.recolor_from="#244A87".into();
        editor.recolor_to="#F03245".into();
        editor.apply_recolor_action();
        assert_eq!(editor.doc.objects[0].style.fill,"#F03245");
        editor.undo();
        assert_eq!(editor.doc,before);
    }
    #[test] fn explorer_does_not_modify_document(){
        let mut editor=Editor::default();
        let before=editor.doc.clone();
        editor.find_object_text="ВАША".into();
        editor.run_object_search();
        assert!(editor.object_search_results.iter().any(|h|h.kind=="Текст"));
        assert_eq!(editor.doc,before);
    }
}
