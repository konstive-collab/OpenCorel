//! Compact vector-tool dock. Draws icons locally so they work with the
//! bundled fonts on Windows/Linux (no operating-system icon font required).
//! The default is a ~62px rail; optional labels never hide essential tools.
use super::{Editor,Tool,productivity};
use eframe::egui::{self,Align2,Color32,FontId,Pos2,Rect,Stroke,Vec2};

pub(super) const COMPACT_WIDTH:f32=64.0;
pub(super) const EXPANDED_WIDTH:f32=182.0;
const PRIMARY:[Tool;4]=[Tool::Select,Tool::Nodes,Tool::Text,Tool::Curve];
const SHAPES:[Tool;4]=[Tool::Rect,Tool::Ellipse,Tool::Line,Tool::Polygon];

#[derive(Clone,Copy)]
enum Glyph{Select,Nodes,Text,Curve,Shapes,Image,Check,More,Help}
fn is_shape(tool:Tool)->bool{SHAPES.contains(&tool)}
fn shortcut(tool:Tool)->&'static str{
    match tool{
        Tool::Select=>"V",Tool::Nodes=>"F10",Tool::Text=>"F8",Tool::Curve=>"B",
        Tool::Rect=>"F6",Tool::Ellipse=>"F7",Tool::Line=>"L",Tool::Polygon=>"P",
    }
}
fn paint_icon(ui:&egui::Ui,rect:Rect,glyph:Glyph,expanded:bool,active:bool){
    let center=Pos2::new(if expanded{rect.left()+22.0}else{rect.center().x},rect.center().y);
    let tint=if active{Color32::from_rgb(231,255,249)}
        else if ui.visuals().dark_mode{Color32::from_rgb(207,219,229)}
        else{Color32::from_rgb(48,62,74)};
    let secondary=if active{tint}else{Color32::from_rgb(41,190,167)};
    let p=ui.painter();
    let at=|x:f32,y:f32|center+Vec2::new(x,y);
    let line=|a:(f32,f32),b:(f32,f32),c:Color32|{
        p.line_segment([at(a.0,a.1),at(b.0,b.1)],Stroke::new(1.8,c));
    };
    match glyph{
        Glyph::Select=>{
            // Render as simple convex primitives. egui::convex_polygon must
            // never receive a concave arrow silhouette.
            p.add(egui::Shape::convex_polygon(
                vec![at(-8.0,-10.0),at(-7.0,8.0),at(5.0,-0.5)],
                tint,Stroke::NONE));
            line((-2.0,2.0),(3.0,11.0),tint);
        }
        Glyph::Nodes=>{
            line((-8.0,6.0),(0.0,-7.0),tint);
            line((0.0,-7.0),(8.0,6.0),tint);
            for (x,y) in [(-8.0,6.0),(0.0,-7.0),(8.0,6.0)]{
                p.circle_filled(at(x,y),3.3,secondary);
                p.circle_stroke(at(x,y),3.3,Stroke::new(1.0,tint));
            }
        }
        Glyph::Text=>{p.text(center,Align2::CENTER_CENTER,"T",
            FontId::proportional(22.0),tint);}
        Glyph::Curve=>{
            // Cubic preview; its curvature is visible at 64px and does not
            // depend on an installed symbol font.
            let mut pts=Vec::with_capacity(17);
            for i in 0..=16{
                let t=i as f32/16.0;
                let u=1.0-t;
                pts.push(at(
                    u*u*u*(-10.0)+3.0*u*u*t*(-4.0)+
                    3.0*u*t*t*4.0+t*t*t*10.0,
                    u*u*u*7.0+3.0*u*u*t*(-9.0)+
                    3.0*u*t*t*(-9.0)+t*t*t*7.0,
                ));
            }
            p.add(egui::Shape::line(pts,Stroke::new(1.8,tint)));
            p.circle_filled(at(-10.0,7.0),2.4,secondary);
            p.circle_filled(at(10.0,7.0),2.4,secondary);
        }
        Glyph::Shapes=>{
            for (a,b) in [
                ((-10.0,-9.0),(1.0,-9.0)),((1.0,-9.0),(1.0,2.0)),
                ((1.0,2.0),(-10.0,2.0)),((-10.0,2.0),(-10.0,-9.0))
            ]{line(a,b,tint);}
            p.circle_stroke(at(5.0,5.0),5.0,Stroke::new(1.7,secondary));
        }
        Glyph::Image=>{
            for (a,b) in [
                ((-10.0,-8.0),(10.0,-8.0)),((10.0,-8.0),(10.0,8.0)),
                ((10.0,8.0),(-10.0,8.0)),((-10.0,8.0),(-10.0,-8.0))
            ]{line(a,b,tint);}
            p.circle_filled(at(5.0,-3.5),2.2,secondary);
            line((-8.0,6.0),(-1.0,-1.0),secondary);
            line((-1.0,-1.0),(6.0,6.0),secondary);
        }
        Glyph::Check=>{
            line((-8.0,0.0),(-2.0,6.0),secondary);
            line((-2.0,6.0),(9.0,-6.0),tint);
        }
        Glyph::More=>{
            for x in [-6.0,0.0,6.0]{p.circle_filled(at(x,0.0),2.0,tint);}
        }
        Glyph::Help=>{p.text(center,Align2::CENTER_CENTER,"?",
            FontId::proportional(19.0),tint);}
    }
}
impl Editor{
    fn rail_button(&mut self,ui:&mut egui::Ui,tool:Tool,width:f32,expanded:bool){
        let is_active=self.tool==tool;
        let response=ui.add_sized([width,43.0],
            egui::Button::new("").selected(is_active))
            .on_hover_text(format!("{} ({})\n{}",
                productivity::tool_label(tool),shortcut(tool),productivity::tool_hint(tool)));
        let glyph=match tool{
            Tool::Select=>Glyph::Select,Tool::Nodes=>Glyph::Nodes,
            Tool::Text=>Glyph::Text,Tool::Curve=>Glyph::Curve,
            _=>Glyph::Shapes,
        };
        paint_icon(ui,response.rect,glyph,expanded,is_active);
        if expanded{
            ui.painter().text(response.rect.left_center()+Vec2::new(38.0,0.0),
                Align2::LEFT_CENTER,productivity::tool_label(tool),
                FontId::proportional(13.0),
                if is_active{Color32::WHITE}else if ui.visuals().dark_mode{
                    Color32::from_rgb(215,224,230)
                }else{Color32::from_rgb(39,53,66)});
        }
        if response.clicked(){self.change_tool(tool);}
    }
    fn rail_action(&mut self,ui:&mut egui::Ui,glyph:Glyph,label:&str,
        tooltip:&str,width:f32,expanded:bool)->bool{
        let response=ui.add_sized([width,39.0],egui::Button::new(""))
            .on_hover_text(tooltip);
        paint_icon(ui,response.rect,glyph,expanded,false);
        if expanded{
            ui.painter().text(response.rect.left_center()+Vec2::new(38.0,0.0),
                Align2::LEFT_CENTER,label,FontId::proportional(13.0),
                if ui.visuals().dark_mode{Color32::from_rgb(213,220,231)}
                else{Color32::from_rgb(48,59,73)});
        }
        response.clicked()
    }
    /// This is the only permanent tool panel. Advanced commands live in the
    /// top menus and context-sensitive inspector instead of occupying canvas.
    pub(super) fn draw_tool_dock(&mut self,ctx:&egui::Context){
        let expanded=self.toolbox_expanded;
        egui::SidePanel::left("vertical_toolbox")
            .exact_width(if expanded{EXPANDED_WIDTH}else{COMPACT_WIDTH})
            .resizable(false)
            .show(ctx,|ui|{
                ui.spacing_mut().item_spacing=Vec2::new(4.0,6.0);
                egui::ScrollArea::vertical()
                    .id_salt("professional_tool_rail")
                    .auto_shrink([false,false])
                    .show(ui,|ui|{
                        let w=(ui.available_width()-2.0).max(42.0);
                        ui.vertical_centered(|ui|{
                            let compact=ui.add_sized([w,28.0],egui::Button::new(
                                if expanded{"«  Свернуть"}else{"»"}
                            ).frame(false)).on_hover_text(
                                "Переключить компактную / расширенную панель инструментов"
                            );
                            if compact.clicked(){self.toolbox_expanded=!self.toolbox_expanded;}
                            ui.add_space(2.0);
                            for tool in PRIMARY{
                                self.rail_button(ui,tool,w,expanded);
                            }
                            ui.separator();
                            // Less-used primitives are deliberately hidden in
                            // one flyout; the current shape remains discoverable.
                            let shape_label=if expanded{"         Фигуры ▾"}else{"           "};
                            let shapes=ui.menu_button(shape_label,|ui|{
                                ui.label(egui::RichText::new("БАЗОВЫЕ ФИГУРЫ").strong().small());
                                for tool in SHAPES{
                                    let selected=self.tool==tool;
                                    if ui.selectable_label(selected,
                                        format!("{}   {}",productivity::tool_label(tool),shortcut(tool)))
                                        .on_hover_text(productivity::tool_hint(tool)).clicked(){
                                        self.change_tool(tool);
                                        ui.close_menu();
                                    }
                                }
                                ui.separator();
                                ui.label("Протяните мышью по листу.");
                            }).response.on_hover_text(
                                "Фигуры: прямоугольник F6, эллипс F7, линия L, шестиугольник P"
                            );
                            paint_icon(ui,shapes.rect,Glyph::Shapes,expanded,is_shape(self.tool));
                            if is_shape(self.tool){
                                // A narrow turquoise marker makes the active
                                // flyout visible without a permanent button row.
                                let mark=Rect::from_min_size(
                                    shapes.rect.left_top()+Vec2::new(-2.0,1.0),
                                    Vec2::new(2.0,(shapes.rect.height()-2.0).max(0.0))
                                );
                                ui.painter().rect_filled(mark,1.0,
                                    Color32::from_rgb(32,201,170));
                            }
                            if self.rail_action(ui,Glyph::Image,"Изображение",
                                "Импорт фотографии или логотипа (PNG/JPEG/WebP и другие)",
                                w,expanded){
                                self.place_bitmap_picker();
                            }
                            ui.separator();
                            if self.rail_action(ui,Glyph::Check,"Проверить макет",
                                "Проверить макет перед печатью: размеры и разрешение",
                                w,expanded){self.run_preflight();}
                            let actions=ui.menu_button(
                                if expanded{"        Действия ▾"}else{"           "},|ui|{
                                    if ui.button("Открыть учебный макет").clicked(){
                                        self.sample_design();ui.close_menu();
                                    }
                                    if ui.button("Копировать  Ctrl+C").clicked(){
                                        self.copy_selection();ui.close_menu();
                                    }
                                    if ui.button("Вставить  Ctrl+V").clicked(){
                                        self.paste_selection();ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui.button("Маски, обтравка").clicked(){
                                        self.inspector_tab=super::workspace::InspectorTab::Objects;
                                        self.status="Маски: верхнее меню «Маски». Сначала выделите контур, затем содержимое с Shift.".into();
                                        ui.close_menu();
                                    }
                                }).response;
                            paint_icon(ui,actions.rect,Glyph::More,expanded,false);
                            actions.on_hover_text("Редкие действия: пример, копирование, вставка, маски");
                            if self.rail_action(ui,Glyph::Help,"Справка  F1",
                                "Полная инструкция с горячими клавишами (F1)",
                                w,expanded){self.show_welcome=true;}
                            ui.add_space(4.0);
                        });
                    });
            });
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn compact_rail_has_only_essential_top_level_tools(){
        assert_eq!(PRIMARY.len(),4);
        assert_eq!(SHAPES.len(),4);
        assert!(!PRIMARY.contains(&Tool::Rect));
        assert!(!PRIMARY.contains(&Tool::Ellipse));
        assert!(SHAPES.contains(&Tool::Rect));
        assert!(SHAPES.contains(&Tool::Line));
        assert!(COMPACT_WIDTH<72.0);
    }
    #[test]
    fn compact_rail_is_default_and_has_accessible_expansion(){
        let mut editor=Editor::default();
        assert!(!editor.toolbox_expanded);
        editor.toolbox_expanded=true;
        assert!(EXPANDED_WIDTH>COMPACT_WIDTH);
    }
}
