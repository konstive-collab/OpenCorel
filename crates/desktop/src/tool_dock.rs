//! Compact vector-tool dock. Draws icons locally so they work with the
//! bundled fonts on Windows/Linux (no operating-system icon font required).
//! The default is a ~62px rail; optional labels never hide essential tools.
use super::{Editor,Tool,productivity};
use eframe::egui::{self,Align2,Color32,FontId,Pos2,Rect,Stroke,Vec2};

pub(super) const COMPACT_WIDTH:f32=64.0;
pub(super) const EXPANDED_WIDTH:f32=182.0;
const PRIMARY:[Tool;6]=[Tool::Select,Tool::Nodes,Tool::Text,Tool::Curve,Tool::Freehand,Tool::Eyedropper];
const SHAPES:[Tool;4]=[Tool::Rect,Tool::Ellipse,Tool::Line,Tool::Polygon];

#[derive(Clone,Copy)]
enum Glyph{Select,Nodes,Text,Curve,Freehand,Eyedropper,Rect,Ellipse,Line,Polygon,Shapes,Image,Check,More,Help}
fn is_shape(tool:Tool)->bool{SHAPES.contains(&tool)}
fn shortcut(tool:Tool)->&'static str{
    match tool{
        Tool::Select=>"V",Tool::Nodes=>"F10",Tool::Text=>"F8",Tool::Curve=>"B",
        Tool::Rect=>"F6",Tool::Ellipse=>"F7",Tool::Line=>"L",Tool::Polygon=>"P",
        Tool::Freehand=>"F",Tool::Eyedropper=>"I",
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
        Glyph::Freehand=>{
            let nodes=[(-10.0,6.0),(-6.0,-6.0),(0.0,5.0),(6.0,-8.0),(10.0,4.0)];
            for w in nodes.windows(2){line(w[0],w[1],tint);}
            p.circle_filled(at(10.0,4.0),2.2,secondary);
        }
        Glyph::Eyedropper=>{
            line((-9.0,7.0),(5.0,-7.0),tint);
            line((-5.0,10.0),(-9.0,7.0),tint);
            line((5.0,-7.0),(9.0,-11.0),tint);
            line((2.0,-10.0),(7.0,-5.0),secondary);
            p.circle_filled(at(-9.0,8.0),2.2,secondary);
        }
        Glyph::Rect=>{
            for (a,b) in [
                ((-9.0,-7.0),(9.0,-7.0)),((9.0,-7.0),(9.0,7.0)),
                ((9.0,7.0),(-9.0,7.0)),((-9.0,7.0),(-9.0,-7.0))
            ]{line(a,b,tint);}
        }
        Glyph::Ellipse=>{
            p.circle_stroke(center,9.0,Stroke::new(1.8,tint));
        }
        Glyph::Line=>{
            line((-10.0,8.0),(10.0,-8.0),tint);
            p.circle_filled(at(-10.0,8.0),2.6,secondary);
            p.circle_filled(at(10.0,-8.0),2.6,secondary);
        }
        Glyph::Polygon=>{
            let vertices:[(f32,f32);6]=[
                (0.0,-10.0),(8.7,-5.0),(8.7,5.0),
                (0.0,10.0),(-8.7,5.0),(-8.7,-5.0)];
            for i in 0..6{
                line(vertices[i],vertices[(i+1)%6],tint);
            }
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
        let dark=ui.visuals().dark_mode;
        let fill=if is_active{Color32::from_rgb(25,119,109)}
            else if dark{Color32::from_rgb(30,35,43)}
            else{Color32::from_rgb(235,241,247)};
        let response=ui.add_sized([width,43.0],
            egui::Button::new("").selected(is_active).fill(fill)
                .stroke(if is_active{Stroke::new(1.0,Color32::from_rgb(39,202,174))}
                    else{Stroke::NONE}))
            .on_hover_text(format!("{} ({})\n{}",
                productivity::tool_label(tool),shortcut(tool),productivity::tool_hint(tool)));
        let glyph=match tool{
            Tool::Select=>Glyph::Select,Tool::Nodes=>Glyph::Nodes,
            Tool::Text=>Glyph::Text,Tool::Curve=>Glyph::Curve,
            Tool::Freehand=>Glyph::Freehand,Tool::Eyedropper=>Glyph::Eyedropper,
            Tool::Rect=>Glyph::Rect,Tool::Ellipse=>Glyph::Ellipse,
            Tool::Line=>Glyph::Line,Tool::Polygon=>Glyph::Polygon,
        };
        if is_active{
            ui.painter().rect_filled(
                Rect::from_min_size(response.rect.left_top()+Vec2::new(1.0,4.0),
                    Vec2::new(2.5,(response.rect.height()-8.0).max(0.0))),
                1.0,Color32::from_rgb(51,236,199));
        }
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
        let neutral=if ui.visuals().dark_mode{
            Color32::from_rgb(30,35,43)
        }else{Color32::from_rgb(235,241,247)};
        let response=ui.add_sized([width,39.0],
            egui::Button::new("").fill(neutral).stroke(Stroke::NONE))
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
                            ui.label(egui::RichText::new(
                                if expanded{"ФИГУРЫ"}else{"—"}
                            ).small());
                            // Corel-style direct access: each fundamental
                            // drawing tool has a visible icon and shortcut.
                            for tool in SHAPES{
                                self.rail_button(ui,tool,w,expanded);
                            }
                            let shape_menu=ui.menu_button(
                                if expanded{"       19 ФИГУР ▾"}else{"          "},
                                |ui|self.procedural_menu(ui)
                            ).response.on_hover_text(
                                "19 редактируемых фигур: звезда, спираль, шестерня, сердце, кольцо..."
                            );
                            paint_icon(ui,shape_menu.rect,Glyph::Shapes,expanded,is_shape(self.tool));
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
        assert_eq!(PRIMARY.len(),6);
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
