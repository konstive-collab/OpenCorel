//! Professional ruler guides and canvas snapping controls.
//! Guides never enter print/export geometry and remain independent per page.
use super::{Editor,View};
use eframe::egui::{self,Color32,Pos2,Stroke,Rect,RichText};
use opencorel_core::GuideAxis;
impl Editor {
    pub(super) fn add_guide_at(&mut self,axis:GuideAxis,value:f32){
        // Reject duplicates within a quarter millimetre to avoid accidental
        // hundreds of nearly coincident guides from repeated clicks.
        if self.doc.guides.iter().any(|g|g.axis==axis
            &&(g.position_mm-value).abs()<0.25){
            self.status="Направляющая уже существует в этой позиции".into();return;
        }
        let mut candidate=self.doc.clone();
        match candidate.add_guide(axis,value){
            Ok(_)=>{
                self.checkpoint();
                self.doc=candidate;
                self.status=match axis{
                    GuideAxis::X=>format!("Вертикальная направляющая: X {value:.2} мм"),
                    GuideAxis::Y=>format!("Горизонтальная направляющая: Y {value:.2} мм"),
                };
            }
            Err(e)=>self.status=format!("Направляющая: {e}")
        }
    }
    pub(super) fn remove_guide_at(&mut self,index:usize){
        if index>=self.doc.guides.len(){return;}
        self.checkpoint();
        self.doc.guides.remove(index);
        self.status="Направляющая удалена · Ctrl+Z — отмена".into();
    }
    pub(super) fn clear_guides(&mut self){
        if self.doc.guides.is_empty(){return;}
        self.checkpoint();
        self.doc.guides.clear();
        self.status="Направляющие на этой странице удалены".into();
    }
    pub(super) fn set_guide_at(&mut self,index:usize,value:f32){
        if index>=self.doc.guides.len()
            ||(self.doc.guides[index].position_mm-value).abs()<0.0001{return;}
        let mut candidate=self.doc.clone();
        match candidate.edit_guide(index,value){
            Ok(())=>{
                self.checkpoint();
                self.doc=candidate;
            }
            Err(e)=>self.status=format!("Направляющая: {e}")
        }
    }
    pub(super) fn guide_inspector(&mut self,ui:&mut egui::Ui){
        ui.heading("Направляющие");
        ui.horizontal_wrapped(|ui|{
            ui.checkbox(&mut self.show_guides,"Видимые");
            ui.checkbox(&mut self.snap_guides,"Привязка");
            ui.checkbox(&mut self.snap_page_edges,"К краям листа");
            ui.checkbox(&mut self.snap_objects,"К объектам");
        });
        ui.horizontal(|ui|{
            ui.checkbox(&mut self.snap_grid,"К сетке");
            ui.label("Допуск");
            ui.add(egui::DragValue::new(&mut self.snap_tolerance_mm)
                .range(0.2..=12.0).speed(0.2).suffix(" мм"));
        });
        ui.small("Щелчок по верхней линейке добавляет вертикальную, по левой — горизонтальную направляющую.");
        ui.separator();
        ui.horizontal(|ui|{
            ui.label("X, мм");
            ui.add(egui::DragValue::new(&mut self.new_guide_x)
                .range(0.0..=self.doc.page.width_mm).speed(1.0));
            if ui.small_button("＋ Вертикальная").clicked(){
                self.add_guide_at(GuideAxis::X,self.new_guide_x);
            }
        });
        ui.horizontal(|ui|{
            ui.label("Y, мм");
            ui.add(egui::DragValue::new(&mut self.new_guide_y)
                .range(0.0..=self.doc.page.height_mm).speed(1.0));
            if ui.small_button("＋ Горизонтальная").clicked(){
                self.add_guide_at(GuideAxis::Y,self.new_guide_y);
            }
        });
        if self.doc.guides.is_empty(){
            ui.small("Направляющих пока нет.");
        }
        let guides=self.doc.guides.clone();
        for (index,guide) in guides.into_iter().enumerate(){
            ui.horizontal(|ui|{
                let (axis,symbol)=match guide.axis{
                    GuideAxis::X=>("X","↕"),
                    GuideAxis::Y=>("Y","↔")
                };
                ui.label(format!("{symbol} {axis}"));
                let maximum=if guide.axis==GuideAxis::X{
                    self.doc.page.width_mm
                }else{self.doc.page.height_mm};
                let mut value=guide.position_mm;
                if ui.add(egui::DragValue::new(&mut value).speed(0.5)
                    .range(0.0..=maximum).suffix(" мм")).changed(){
                    self.set_guide_at(index,value);
                }
                if ui.small_button("×").on_hover_text("Удалить направляющую").clicked(){
                    self.remove_guide_at(index);
                }
            });
        }
        if ui.add_enabled(!self.doc.guides.is_empty(),
            egui::Button::new("Очистить направляющие")).clicked(){
            self.clear_guides();
        }
        ui.separator();
        ui.small(RichText::new(format!("Направляющих: {} · Ограничение 512 на лист",
            self.doc.guides.len())).color(Color32::GRAY));
        if self.doc.objects.len()>12_000 && self.snap_objects{
            ui.small("Привязка к объектам временно отключается при >12 000 объектах для сохранения отзывчивости.");
        }
    }
    pub(super) fn paint_guides(&self,p:&egui::Painter,v:View){
        if !self.show_guides{return;}
        let shade=Color32::from_rgba_premultiplied(31,129,224,180);
        let stroke=Stroke::new(1.1,shade);
        for guide in &self.doc.guides{
            match guide.axis{
                GuideAxis::X=>{
                    let x=v.screen(opencorel_core::Point{x:guide.position_mm,y:0.0}).x;
                    p.line_segment([Pos2::new(x,v.rect.top()),Pos2::new(x,v.rect.bottom())],stroke);
                }
                GuideAxis::Y=>{
                    let y=v.screen(opencorel_core::Point{x:0.0,y:guide.position_mm}).y;
                    p.line_segment([Pos2::new(v.rect.left(),y),Pos2::new(v.rect.right(),y)],stroke);
                }
            }
        }
    }
    /// Handles ruler clicks separately from artboard clicks. Returns true
    /// if a pointer action was consumed by a ruler.
    pub(super) fn try_create_guide_from_ruler(&mut self,pos:Pos2,
        work:Rect,view:View)->bool{
        if !self.show_rulers{return false;}
        let on_top=pos.y<work.top()&&pos.x>=work.left()&&pos.x<=work.right();
        let on_left=pos.x<work.left()&&pos.y>=work.top()&&pos.y<=work.bottom();
        if on_top{
            let mm=view.mm(pos).x;
            if (0.0..=self.doc.page.width_mm).contains(&mm){
                self.add_guide_at(GuideAxis::X,mm);
            }
        }else if on_left{
            let mm=view.mm(pos).y;
            if (0.0..=self.doc.page.height_mm).contains(&mm){
                self.add_guide_at(GuideAxis::Y,mm);
            }
        }
        on_top||on_left
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn adding_guides_is_undoable_and_deduplicated(){
        let mut e=Editor::default();
        e.add_guide_at(GuideAxis::X,50.0);
        assert_eq!(e.doc.guides.len(),1);
        e.add_guide_at(GuideAxis::X,50.1);
        assert_eq!(e.doc.guides.len(),1);
        e.undo();
        assert_eq!(e.doc.guides.len(),0);
    }
    #[test] fn editing_and_clearing_guides_is_undoable(){
        let mut e=Editor::default();
        e.add_guide_at(GuideAxis::Y,50.0);
        e.set_guide_at(0,60.0);
        assert_eq!(e.doc.guides[0].position_mm,60.0);
        e.clear_guides();
        assert!(e.doc.guides.is_empty());
        e.undo();
        assert_eq!(e.doc.guides[0].position_mm,60.0);
    }
}
