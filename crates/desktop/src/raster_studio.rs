//! Raster Studio: one explicit Apply = one undoable document operation.
use super::{Editor,Shape};
use eframe::egui;
use opencorel_core::raster_edit::RasterSettings;

#[derive(Default)]
pub(super) struct RasterPanel{
    target:Option<(u64,u64,RasterSettings)>,
    draft:RasterSettings,
    thumbnails:super::raster_preview::RasterCache,
}
impl Editor{
    fn apply_raster_studio(&mut self,id:u64,settings:RasterSettings){
        match self.doc.prepare_raster_edit(id,settings){
            Ok(shape)=>{
                if self.doc.get(id).is_some_and(|o|o.shape==shape){
                    self.status="Изображение не изменилось".into();return;
                }
                self.checkpoint();
                if let Some(object)=self.doc.get_mut(id){object.shape=shape;}
                self.raster_cache.clear();
                self.masked_canvas=Default::default();
                self.status="Растровая обработка применена. Исходник сохранён; Ctrl+Z — отменить.".into();
                self.raster_panel.target=None;
            }
            Err(error)=>self.status=format!("Обработка не выполнена: {error}"),
        }
    }
    pub(super) fn raster_studio(&mut self,ui:&mut egui::Ui){
        ui.heading("Растровая студия");
        let Some(id)=self.selected else{
            self.raster_panel.target=None;
            ui.label("Выберите одно растровое изображение на холсте.");return;
        };
        if self.selection_ids().len()!=1{
            self.raster_panel.target=None;
            ui.label("Выберите только одно изображение.");return;
        }
        let Some(object)=self.doc.get(id) else{return;};
        let Shape::Raster{pixel_width,pixel_height,mime,data_base64,edit,..}=&object.shape else{
            self.raster_panel.target=None;
            ui.label("Выбранный объект не является растром.");return;
        };
        if object.group_id.is_some(){
            ui.label("Для обработки сначала разгруппируйте или отсоедините символ.");
            return;
        }
        if !self.doc.layer(object.layer_id).is_some_and(|l|l.visible&&!l.locked){
            ui.label("Слой скрыт или заблокирован.");return;
        }
        let (sw,sh,applied)=edit.as_ref().map_or(
            (*pixel_width,*pixel_height,RasterSettings::default()),
            |e|(e.source_width,e.source_height,e.settings));
        let target=(self.doc.active_page_id,id,applied);
        if self.raster_panel.target!=Some(target){
            self.raster_panel.target=Some(target);
            self.raster_panel.draft=applied;
            self.raster_panel.thumbnails.clear();
        }

        ui.label(format!("Исходник: {sw} × {sh} px • текущий: {pixel_width} × {pixel_height} px"));
        // Small before/after comparison of the stored source and applied result.
        // Do not regenerate the full-resolution image on slider events.
        let source=edit.as_ref().map_or(data_base64.as_str(),|e|e.source_base64.as_str());
        ui.horizontal(|ui|{
            let width=(ui.available_width()*0.48).clamp(36.,160.);
            for (key,label,bytes,pw,ph) in [(1,"Исходник",source,sw,sh),
                (2,"Применено",data_base64.as_str(),*pixel_width,*pixel_height)]{
                ui.vertical(|ui|{
                    ui.small(label);
                    let (response,painter)=ui.allocate_painter(egui::vec2(width,96.),egui::Sense::hover());
                    let ratio=(pw as f32/ph.max(1) as f32).max(0.001);
                    let w=width.min(96.*ratio);let h=(w/ratio).min(96.);
                    let rect=egui::Rect::from_center_size(response.rect.center(),egui::vec2(w,h));
                    painter.rect_filled(rect,0.,egui::Color32::from_gray(120));
                    self.raster_panel.thumbnails.paint(ui.ctx(),&painter,key,bytes,rect);
                });
            }
        });
        let source_mime=edit.as_ref().map_or(mime.as_str(),|e|e.source_mime.as_str());
        let current_mime=mime.as_str();
        let mut extract=None;
        ui.horizontal_wrapped(|ui|{
            if ui.small_button("Извлечь исходник").clicked(){extract=Some(("original",source_mime.to_owned()));}
            if ui.small_button("Извлечь результат").clicked(){extract=Some(("adjusted",current_mime.to_owned()));}
        });
        if let Some((kind,format))=extract{
            let ext=if format=="image/jpeg"{"jpg"}else{"png"};
            if let Some(path)=rfd::FileDialog::new().add_filter("Image",&[ext])
                .set_file_name(format!("raster-{id}-{kind}.{ext}")).save_file(){
                self.status=match opencorel_core::raster_edit::extract_raster(&object.shape,kind,&path){
                    Ok(())=>format!("Изображение сохранено: {}",path.display()),
                    Err(e)=>format!("Извлечение не выполнено: {e}"),
                };
            }
        }
        ui.small("Кадр на листе остаётся прежним. Изменения рассчитываются из исходника, а не из предыдущего результата.");
        ui.separator();
        let s=&mut self.raster_panel.draft;
        ui.horizontal_wrapped(|ui|{
            if ui.button("Уровни 15–240").on_hover_text("Фиксированный пресет уровней: 15–240").clicked(){s.black=15;s.white=240;}
            if ui.button("Ч/Б").clicked(){*s=RasterSettings{grayscale:true,..Default::default()};}
            if ui.button("Винтаж").clicked(){*s=RasterSettings{sepia:true,contrast:10,..Default::default()};}
            if ui.button("Негатив").clicked(){*s=RasterSettings{invert:true,..Default::default()};}
            if ui.button("Сброс параметров").clicked(){*s=RasterSettings::default();}
        });
        ui.collapsing("Кадрирование и ориентация",|ui|{
            for (label,value) in [("Слева ‰",&mut s.left),("Сверху ‰",&mut s.top),
                ("Справа ‰",&mut s.right),("Снизу ‰",&mut s.bottom)]{
                ui.add(egui::Slider::new(value,0..=999).text(label));
            }
            ui.horizontal_wrapped(|ui|{
                if ui.button("↶ 90°").clicked(){s.turns=(s.turns+3)%4;}
                if ui.button("↷ 90°").clicked(){s.turns=(s.turns+1)%4;}
                if ui.button("180°").clicked(){s.turns=(s.turns+2)%4;}
                ui.label(format!("{}°",u16::from(s.turns)*90));
            });
            ui.checkbox(&mut s.flip_x,"Зеркально по горизонтали");
            ui.checkbox(&mut s.flip_y,"Зеркально по вертикали");
            ui.small("Поля обрезки относятся к исходнику до поворота. 100‰ = 10%.");
        });
        egui::CollapsingHeader::new("Цвет и тон").default_open(true).show(ui,|ui|{
            ui.add(egui::Slider::new(&mut s.brightness,-100..=100).text("Яркость"));
            ui.add(egui::Slider::new(&mut s.contrast,-100..=100).text("Контраст"));
            ui.add(egui::Slider::new(&mut s.saturation,-100..=100).text("Насыщенность"));
            ui.add(egui::Slider::new(&mut s.exposure,-300..=300).text("Экспозиция ×100 EV"));
            ui.add(egui::Slider::new(&mut s.gamma,20..=500).text("Гамма ×100"));
            ui.add(egui::Slider::new(&mut s.warmth,-100..=100).text("Температура"));
            ui.add(egui::Slider::new(&mut s.tint,-100..=100).text("Оттенок"));
            ui.add(egui::Slider::new(&mut s.black,0..=254).text("Чёрная точка"));
            ui.add(egui::Slider::new(&mut s.white,1..=255).text("Белая точка"));
        });
        ui.collapsing("Эффекты",|ui|{
            ui.checkbox(&mut s.grayscale,"Градации серого");
            ui.checkbox(&mut s.sepia,"Сепия");
            ui.checkbox(&mut s.invert,"Инверсия");
            ui.add(egui::Slider::new(&mut s.threshold,0..=255).text("Порог (0 — выкл.)"));
            let mut poster=s.posterize>0;
            if ui.checkbox(&mut poster,"Постеризация").changed(){s.posterize=if poster{8}else{0};}
            if poster{ui.add(egui::Slider::new(&mut s.posterize,2..=32).text("Уровней"));}
        });
        ui.collapsing("Прозрачность и удаление цвета",|ui|{
            ui.add(egui::Slider::new(&mut s.opacity,0..=100).text("Непрозрачность %"));
            ui.checkbox(&mut s.key_enabled,"Удаление выбранного цвета");
            ui.add_enabled_ui(s.key_enabled,|ui|{
                ui.color_edit_button_srgb(&mut s.key_color);
                ui.add(egui::Slider::new(&mut s.key_tolerance,0..=100).text("Допуск %"));
                ui.add(egui::Slider::new(&mut s.key_softness,0..=100).text("Мягкость %"));
            });
            ui.small("Удаляется весь близкий цвет, включая внутренние области. Это chroma key, не AI-сегментация.");
        });
        ui.collapsing("Разрешение",|ui|{
            ui.add(egui::DragValue::new(&mut s.max_side).range(0..=16000).prefix("Макс. сторона: ").suffix(" px"));
            ui.small("0 — исходное разрешение после обрезки. Уменьшение выполняется без изменения физического размера на листе.");
            if ui.button("2048 px").clicked(){s.max_side=2048;}
            if ui.button("1024 px").clicked(){s.max_side=1024;}
        });
        let size=s.output_size(sw,sh);
        match &size{
            Ok((w,h))=>{ui.label(format!("Результат: {w} × {h} px"));},
            Err(e)=>{ui.colored_label(egui::Color32::LIGHT_RED,e);},
        }
        let params=*s;
        ui.separator();
        let apply=ui.add_enabled(size.is_ok()&&params!=applied,
            egui::Button::new("Применить к изображению")).clicked();
        let reset=ui.add_enabled(applied!=RasterSettings::default(),
            egui::Button::new("Восстановить исходник")).clicked();
        if reset{self.apply_raster_studio(id,RasterSettings::default());}
        else if apply{self.apply_raster_studio(id,params);}
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Document,Style};
    #[test] fn studio_undo_restores_original_source(){
        let mut e=Editor::default();e.doc=Document::default();
        // Valid tiny PNG generated through the existing image dependency.
        let mut png=std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(2,2).write_to(&mut png,image::ImageFormat::Png).unwrap();
        use base64::Engine;
        let source=Shape::Raster{x:0.,y:0.,w:10.,h:10.,pixel_width:2,pixel_height:2,
            mime:"image/png".into(),data_base64:base64::engine::general_purpose::STANDARD.encode(png.into_inner()),edit:None};
        let id=e.doc.add(source.clone(),Style::default());
        e.apply_raster_studio(id,RasterSettings{invert:true,..Default::default()});
        assert_ne!(e.doc.get(id).unwrap().shape,source);
        e.undo();assert_eq!(e.doc.get(id).unwrap().shape,source);
        e.redo();assert_ne!(e.doc.get(id).unwrap().shape,source);
    }
}
