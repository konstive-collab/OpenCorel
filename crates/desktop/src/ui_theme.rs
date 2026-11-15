//! Consistent compact professional palette; independent visual design.
use eframe::egui::{self,Color32,Stroke,Vec2};

const ACCENT:Color32=Color32::from_rgb(32,201,170);
pub fn apply(ctx:&egui::Context,light:bool){
    let mut visuals=if light{egui::Visuals::light()}else{egui::Visuals::dark()};
    if light{
        visuals.panel_fill=Color32::from_rgb(247,249,252);
        visuals.window_fill=Color32::from_rgb(254,254,255);
        visuals.extreme_bg_color=Color32::from_rgb(237,240,246);
        visuals.faint_bg_color=Color32::from_rgb(242,245,250);
        visuals.widgets.inactive.bg_fill=Color32::from_rgb(239,243,247);
        visuals.widgets.hovered.bg_fill=Color32::from_rgb(222,239,239);
        visuals.widgets.active.bg_fill=Color32::from_rgb(207,238,232);
        visuals.selection.bg_fill=Color32::from_rgb(0,145,129);
        visuals.selection.stroke=Stroke::new(1.0,Color32::WHITE);
    }else{
        visuals.panel_fill=Color32::from_rgb(24,27,34);
        visuals.window_fill=Color32::from_rgb(27,31,39);
        visuals.extreme_bg_color=Color32::from_rgb(17,20,26);
        visuals.faint_bg_color=Color32::from_rgb(30,35,44);
        visuals.widgets.noninteractive.bg_fill=Color32::from_rgb(29,33,41);
        visuals.widgets.inactive.bg_fill=Color32::from_rgb(37,42,52);
        visuals.widgets.hovered.bg_fill=Color32::from_rgb(48,65,72);
        visuals.widgets.active.bg_fill=Color32::from_rgb(45,88,82);
        visuals.widgets.noninteractive.fg_stroke.color=Color32::from_rgb(196,204,218);
        visuals.widgets.inactive.fg_stroke.color=Color32::from_rgb(224,230,240);
        visuals.selection.bg_fill=Color32::from_rgb(27,129,113);
        visuals.selection.stroke=Stroke::new(1.0,ACCENT);
    }
    ctx.set_visuals(visuals);
    let mut style=(*ctx.style()).clone();
    style.spacing.item_spacing=Vec2::new(8.0,6.0);
    style.spacing.button_padding=Vec2::new(10.0,6.0);
    ctx.set_style(style);
}
