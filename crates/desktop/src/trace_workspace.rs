//! Editable raster tracing inspector, with non-destructive preview statistics.
use super::{Editor,Shape};
use eframe::egui;
use opencorel_core::vector_trace::{TraceMode,TraceSettings,trace_shape};

#[derive(Default)]
pub(super) struct TracePanel{
    settings:TraceSettings,
    replace:bool,
    report:String,
    preview_svg:Option<String>,
    preview_key:Option<(u64,u64,usize,usize,TraceSettings)>,
    preview_cache:super::vector_preview::VectorCache,
    preview_ratio:f32,
}
impl Editor{
    fn execute_trace(&mut self,id:u64,settings:TraceSettings,replace:bool){
        let mut draft=self.doc.clone();
        match draft.trace_raster(id,settings,replace){
            Ok((ids,result))=>{
                self.checkpoint();
                self.doc=draft;
                self.selected=ids.first().copied();
                self.multi_selected=ids.iter().copied().skip(1).collect();
                self.active_node=None;self.node_drag=None;
                self.fill_cache.entries.clear();self.compound_cache.clear();
                self.raster_cache.clear();self.masked_canvas=Default::default();
                self.trace_panel.preview_svg=None;
                self.trace_panel.preview_cache.clear();
                self.trace_panel.preview_key=None;
                self.trace_panel.report=format!("{} цветов, {} контуров, {} узлов; сетка {}×{}.",
                    ids.len(),result.contours,result.nodes,result.width,result.height);
                self.status=format!("Векторизация завершена: {}. Ctrl+Z — отмена.",self.trace_panel.report);
            }
            Err(e)=>{self.trace_panel.report=e.clone();self.status=format!("Векторизация: {e}");}
        }
    }
    pub(super) fn trace_inspector(&mut self,ui:&mut egui::Ui){
        ui.heading("Векторизация");
        ui.small("Растр → нативные редактируемые кривые. Включая внутренние отверстия.");
        let Some(id)=self.selected else{ui.label("Выберите растровое изображение.");return;};
        let Some(object)=self.doc.get(id) else{return;};
        if self.selection_ids().len()!=1||!matches!(object.shape,Shape::Raster{..}){
            ui.label("Выберите один растр.");return;
        }
        let allowed=object.group_id.is_none()&&
            self.doc.layer(object.layer_id).is_some_and(|l|l.visible&&!l.locked)&&
            !self.doc.clip_links.iter().any(|l|l.source_id==id||l.target_id==id);
        let s=&mut self.trace_panel.settings;
        ui.horizontal_wrapped(|ui|{
            ui.selectable_value(&mut s.mode,TraceMode::Dark,"Тёмный силуэт");
            ui.selectable_value(&mut s.mode,TraceMode::Light,"Светлый силуэт");
            ui.selectable_value(&mut s.mode,TraceMode::Color,"Цветной");
        });
        if s.mode==TraceMode::Color{
            ui.add(egui::Slider::new(&mut s.colors,2..=16).text("Цветов"));
        }else{ui.add(egui::Slider::new(&mut s.threshold,0..=255).text("Порог яркости"));}
        ui.add(egui::Slider::new(&mut s.alpha_min,1..=255).text("Порог альфа"));
        ui.add(egui::Slider::new(&mut s.max_side,32..=1024).text("Детализация px"));
        ui.add(egui::Slider::new(&mut s.min_area,0..=100).text("Мин. площадь px²"));
        if s.mode!=TraceMode::Light{ui.checkbox(&mut s.skip_white,"Исключить почти белый фон");}
        ui.checkbox(&mut self.trace_panel.replace,"Заменить исходное изображение");
        ui.horizontal_wrapped(|ui|{
            if ui.small_button("Логотип").clicked(){*s=TraceSettings{max_side:768,min_area:2,..Default::default()};}
            if ui.small_button("Иконка").clicked(){*s=TraceSettings{max_side:256,min_area:1,..Default::default()};}
            if ui.small_button("Цветная схема").clicked(){*s=TraceSettings{mode:TraceMode::Color,colors:8,max_side:768,..Default::default()};}
        });
        let settings=*s;
        let replace=self.trace_panel.replace;
        let Shape::Raster{data_base64,..}=&object.shape else{return;};
        let key=(self.doc.active_page_id,id,data_base64.as_ptr() as usize,data_base64.len(),settings);
        if self.trace_panel.preview_key!=Some(key){
            self.trace_panel.preview_svg=None;
            self.trace_panel.preview_cache.clear();
            self.trace_panel.preview_key=Some(key);
            self.trace_panel.report.clear();
        }
        if !allowed{ui.colored_label(egui::Color32::LIGHT_RED,"Нужен отдельный растр без маски на доступном слое.");}
        if ui.add_enabled(allowed,egui::Button::new("Построить предпросмотр")).clicked(){
            self.trace_panel.preview_cache.clear();
            self.trace_panel.preview_svg=None;
            self.trace_panel.report=match trace_shape(&object.shape,settings){
                Ok(r)=>{
                    let frame=object.bounds();
                    let mut doc=opencorel_core::Document::default();
                    doc.page.width_mm=frame.w;
                    doc.page.height_mm=frame.h;
                    for p in &r.parts{
                        let mut shape=p.shape.clone();shape.translate(-frame.x,-frame.y);
                        doc.add(shape,p.style.clone());
                    }
                    self.trace_panel.preview_svg=Some(doc.to_svg());
                    self.trace_panel.preview_ratio=frame.w/frame.h;
                    format!("{} цветов, {} контуров, {} узлов; сетка {}×{}.",r.parts.len(),r.contours,r.nodes,r.width,r.height)
                }
                Err(e)=>e,
            };
        }
        if let Some(svg)=self.trace_panel.preview_svg.as_ref(){
            let width=ui.available_width().clamp(80.,300.);
            let ratio=self.trace_panel.preview_ratio.clamp(0.001,1000.);
            let w=width.min(200.*ratio);let h=(w/ratio).min(200.);
            let (response,painter)=ui.allocate_painter(egui::vec2(width,h),egui::Sense::hover());
            let rect=egui::Rect::from_center_size(response.rect.center(),egui::vec2(w,h));
            painter.rect_filled(rect,0.,egui::Color32::WHITE);
            self.trace_panel.preview_cache.paint(ui.ctx(),&painter,1,svg,rect);
        }
        if ui.add_enabled(allowed,egui::Button::new("Преобразовать в кривые")).clicked(){
            self.execute_trace(id,settings,replace);
        }
        if !self.trace_panel.report.is_empty(){ui.label(&self.trace_panel.report);}
        ui.small("Точная граница по пиксельной сетке с удалением коллинеарных узлов. Автоматическая аппроксимация плавными кривыми пока не реализована.");
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Document,Style};
    use base64::Engine;
    #[test] fn trace_can_be_undone(){
        let mut e=Editor::default();e.doc=Document::default();
        let mut png=std::io::Cursor::new(Vec::new());
        let img=image::RgbaImage::from_pixel(8,8,image::Rgba([0,0,0,255]));
        image::DynamicImage::ImageRgba8(img).write_to(&mut png,image::ImageFormat::Png).unwrap();
        let id=e.doc.add(Shape::Raster{x:0.,y:0.,w:10.,h:10.,pixel_width:8,pixel_height:8,
            mime:"image/png".into(),data_base64:base64::engine::general_purpose::STANDARD.encode(png.into_inner()),edit:None},Style::default());
        let before=e.doc.clone();
        e.execute_trace(id,TraceSettings::default(),true);
        assert!(e.doc.get(id).is_none());
        assert!(matches!(e.doc.objects[0].shape,Shape::CompoundPath{..}));
        e.undo();assert_eq!(e.doc,before);
    }
}
