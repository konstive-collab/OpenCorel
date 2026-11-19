//! Native vector glyphs for the top quick-action toolbar.
//! All icons are drawn with egui primitives (not Unicode/symbol fonts).
use eframe::egui::{self,Align2,Color32,FontId,Pos2,Rect,Stroke,Vec2};
#[derive(Clone,Copy,Debug)]
pub(super) enum Icon{
    New,Open,Save,Undo,Redo,Image,Export,Search,ZoomIn,ZoomOut,Fit,Help,
}
pub(super) fn icon_button(ui:&mut egui::Ui,icon:Icon,tip:&str,enabled:bool)->bool{
    let response=ui.add_enabled(enabled,egui::Button::new("").min_size(Vec2::new(31.,31.)))
        .on_hover_text(tip);
    let center=response.rect.center();
    let color=if !enabled{Color32::from_gray(93)}
        else if ui.visuals().dark_mode{Color32::from_rgb(215,222,234)}
        else{Color32::from_rgb(43,57,70)};
    let secondary=if enabled{Color32::from_rgb(48,193,164)}else{color};
    let pt=|x:f32,y:f32|center+Vec2::new(x,y);
    let line=|a:(f32,f32),b:(f32,f32),tone:Color32|{
        ui.painter().line_segment([pt(a.0,a.1),pt(b.0,b.1)],
            Stroke::new(1.65,tone));
    };
    let frame=|x1:f32,y1:f32,x2:f32,y2:f32,tone:Color32|{
        for (a,b) in [
            ((x1,y1),(x2,y1)),((x2,y1),(x2,y2)),
            ((x2,y2),(x1,y2)),((x1,y2),(x1,y1))
        ]{line(a,b,tone);}
    };
    match icon{
        Icon::New=>{
            frame(-8.,-10.,7.,9.,color);
            line((1.,-10.),(7.,-4.),color);
            line((3.,2.),(3.,7.),secondary);
            line((0.,4.5),(6.,4.5),secondary);
        }
        Icon::Open=>{
            line((-10.,-5.),(-6.,-9.),color);
            line((-6.,-9.),(1.,-9.),color);
            line((1.,-9.),(4.,-6.),color);
            line((4.,-6.),(9.,-6.),color);
            line((9.,-6.),(6.,8.),color);
            line((6.,8.),(-10.,8.),color);
            line((-10.,8.),(-10.,-5.),color);
        }
        Icon::Save=>{
            frame(-9.,-9.,9.,9.,color);
            frame(-4.,-9.,5.,-2.,color);
            frame(-5.,2.,5.,9.,secondary);
            line((1.,-8.),(1.,-4.),color);
        }
        Icon::Undo|Icon::Redo=>{
            let dir=if matches!(icon,Icon::Undo){-1.0}else{1.0};
            let pts=vec![pt(dir*8.,7.),pt(dir*8.,-2.),
                pt(dir*4.,-7.),pt(dir*-5.,-7.),pt(dir*-9.,-2.)];
            ui.painter().add(egui::Shape::line(pts,Stroke::new(1.75,color)));
            line((dir*-4.,-7.),(dir*-9.,-2.),secondary);
            line((dir*-9.,-2.),(dir*-9.,-8.),secondary);
        }
        Icon::Image=>{
            frame(-10.,-8.,10.,8.,color);
            ui.painter().circle_filled(pt(5.,-3.),2.0,secondary);
            line((-8.,5.),(-2.,-1.),color);
            line((-2.,-1.),(3.,4.),color);
            line((3.,4.),(6.,1.),color);
            line((6.,1.),(9.,5.),color);
        }
        Icon::Export=>{
            frame(-10.,-7.,3.,9.,color);
            line((0.,-9.),(10.,-9.),secondary);
            line((10.,-9.),(10.,1.),secondary);
            line((-2.,3.),(10.,-9.),secondary);
        }
        Icon::Search=>{
            ui.painter().circle_stroke(pt(-2.,-2.),6.8,Stroke::new(1.8,color));
            line((3.,3.),(10.,10.),secondary);
        }
        Icon::ZoomIn|Icon::ZoomOut=>{
            ui.painter().circle_stroke(pt(-2.,-2.),7.0,Stroke::new(1.8,color));
            line((3.,3.),(10.,10.),color);
            line((-6.,-2.),(2.,-2.),secondary);
            if matches!(icon,Icon::ZoomIn){line((-2.,-6.),(-2.,2.),secondary);}
        }
        Icon::Fit=>{
            for (a,b) in [
                ((-9.,-2.),(-9.,-9.)),((-9.,-9.),(-2.,-9.)),
                ((2.,-9.),(9.,-9.)),((9.,-9.),(9.,-2.)),
                ((9.,2.),(9.,9.)),((9.,9.),(2.,9.)),
                ((-2.,9.),(-9.,9.)),((-9.,9.),(-9.,2.))
            ]{line(a,b,color);}
            frame(-4.,-4.,4.,4.,secondary);
        }
        Icon::Help=>{
            ui.painter().circle_stroke(center,10.,Stroke::new(1.6,color));
            ui.painter().text(center,Align2::CENTER_CENTER,"?",
                FontId::proportional(17.),secondary);
        }
    }
    response.clicked()
}
