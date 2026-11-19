//! Page measurement utilities and selection-oriented zoom.
use super::Editor;
use opencorel_core::dimension::DimensionAxis;
use eframe::egui::Vec2;
impl Editor{
    pub(super) fn measure_selection(&mut self,axis:DimensionAxis){
        let ids=self.selection_ids();
        if ids.len()!=1{
            self.status="Размерная линия: выделите один объект".into();return;
        }
        let mut candidate=self.doc.clone();
        match candidate.annotate_dimension(ids[0],axis,10.)
            .and_then(|new_ids|{candidate.validate()?;Ok(new_ids)}){
            Ok(new_ids)=>{
                self.checkpoint();
                self.doc=candidate;
                self.selected=new_ids.first().copied();
                self.multi_selected=new_ids.into_iter().skip(1).collect();
                self.status="Размерная линия создана: обычные векторы + текст, сгруппированы; Ctrl+Z отменяет".into();
            }
            Err(e)=>self.status=format!("Размерная линия: {e}")
        }
    }
    pub(super) fn fit_selection(&mut self){
        let Some(bounds)=self.selection_bounds() else{
            self.status="Выберите объекты для приближения".into();return;
        };
        let page_w=self.doc.page.width_mm;
        let page_h=self.doc.page.height_mm;
        if bounds.w<0.01||bounds.h<0.01{return;}
        // Same scale as View::new, based on the last real canvas viewport.
        let usable=self.last_canvas_size-Vec2::splat(40.);
        let base=(usable.x/page_w).min(usable.y/page_h).max(0.001);
        self.zoom=((usable.x/(bounds.w*base)).min(usable.y/(bounds.h*base))*0.85)
            .clamp(0.20,8.0);
        let scale=base*self.zoom;
        self.pan=Vec2::new(
            (page_w/2.-bounds.x-bounds.w/2.)*scale,
            (page_h/2.-bounds.y-bounds.h/2.)*scale
        );
        self.status="Масштаб выделения установлен".into();
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn measurement_creation_is_one_undo_action(){
        let mut e=Editor::default();
        let id=e.doc.objects[0].id;
        e.selected=Some(id);
        let before=e.doc.clone();
        e.measure_selection(DimensionAxis::Width);
        assert_eq!(e.doc.objects.len(),before.objects.len()+4);
        e.undo();
        assert_eq!(e.doc,before);
    }
}
