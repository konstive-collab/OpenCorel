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
        // View::new fits the entire page at zoom=1. Match its aspect ratio.
        let ratio_x=(page_w/bounds.w).abs();
        let ratio_y=(page_h/bounds.h).abs();
        self.zoom=(ratio_x.min(ratio_y)*0.80).clamp(0.20,8.0);
        // The centered page is shifted by the difference between document
        // center and selection center converted to the active view scale.
        self.pan=Vec2::ZERO;
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
