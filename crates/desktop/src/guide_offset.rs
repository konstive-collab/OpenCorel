//! Guide UI/controller and non-destructive parallel-path creation.
//! Guide drags store only preview coordinates until release; one drag adds
//! or moves one guide with a single Undo snapshot.
use super::{Editor,GuideDrag,View};
use opencorel_core::{Point,Shape};
use opencorel_core::guides::{self,Guide,GuideAxis};
use eframe::egui::{self,Color32,Pos2,Rect,Stroke};

impl Editor{
    pub(super) fn add_guide(&mut self,axis:GuideAxis,value:f32)->bool{
        let guide=match Guide::new(axis,value){
            Ok(g)=>g,Err(reason)=>{self.status=reason;return false;}
        };
        if self.doc.guides.len()>=guides::MAX_GUIDES{
            self.status="Достигнут предел 256 направляющих".into();return false;
        }
        self.checkpoint();
        self.doc.guides.push(guide);
        self.status=format!("Добавлена направляющая на {:.2} мм. Ctrl+Z — отмена",value);
        true
    }
    pub(super) fn move_guide(&mut self,index:usize,value:f32)->bool{
        let guide=match Guide::new(
            self.doc.guides.get(index).map(|g|g.axis).unwrap_or(GuideAxis::Vertical),
            value){
            Ok(g)=>g,Err(reason)=>{self.status=reason;return false;}
        };
        if self.doc.guides.get(index).is_none(){
            self.status="Направляющая больше не существует".into();return false;
        }
        if self.doc.guides[index]==guide{return false;}
        self.checkpoint();
        self.doc.guides[index]=guide;
        self.status=format!("Направляющая перемещена: {:.2} мм",value);
        true
    }
    pub(super) fn remove_guide(&mut self,index:usize)->bool{
        if index>=self.doc.guides.len(){return false;}
        self.checkpoint();
        self.doc.guides.remove(index);
        self.status="Направляющая удалена. Ctrl+Z — отмена".into();
        true
    }
    pub(super) fn clear_guides(&mut self)->bool{
        if self.doc.guides.is_empty(){return false;}
        self.checkpoint();
        self.doc.guides.clear();
        self.status="Все направляющие удалены. Ctrl+Z — восстановить".into();
        true
    }
    pub(super) fn begin_guide_drag(
        &mut self,v:View,canvas:Rect,press:Pos2,alt:bool
    )->bool{
        if !self.show_guides{return false;}
        let mut drag=None;
        if self.show_rulers &&
            press.y>=canvas.top() && press.y<canvas.top()+24.0 &&
            press.x>=v.rect.left() && press.x<=v.rect.right(){
            drag=Some(GuideDrag{axis:GuideAxis::Vertical,index:None,
                position_mm:v.mm(press).x});
        }else if self.show_rulers &&
            press.x>=canvas.left() && press.x<canvas.left()+25.0 &&
            press.y>=v.rect.top() && press.y<=v.rect.bottom(){
            drag=Some(GuideDrag{axis:GuideAxis::Horizontal,index:None,
                position_mm:v.mm(press).y});
        }else if alt && v.rect.contains(press){
            // Alt+drag a guide to move it without hijacking normal object
            // selection and vector drawing clicks.
            let mut candidate=None;
            let mut best=7.0f32;
            for (i,g) in self.doc.guides.iter().enumerate(){
                let coordinate=v.screen(Point{
                    x:g.position_mm,y:g.position_mm,
                });
                let distance=match g.axis{
                    GuideAxis::Horizontal=>(press.y-coordinate.y).abs(),
                    GuideAxis::Vertical=>(press.x-coordinate.x).abs(),
                };
                if distance<best{
                    best=distance;
                    candidate=Some((i,*g));
                }
            }
            if let Some((i,g))=candidate{
                drag=Some(GuideDrag{axis:g.axis,index:Some(i),
                    position_mm:g.position_mm});
            }
        }
        self.guide_drag=drag;
        self.guide_drag.is_some()
    }
    pub(super) fn update_guide_drag(&mut self,p:Point){
        if let Some(drag)=&mut self.guide_drag{
            let coordinate=match drag.axis{
                GuideAxis::Horizontal=>p.y,GuideAxis::Vertical=>p.x,
            };
            if coordinate.is_finite()&&coordinate.abs()<=100_000.0{
                drag.position_mm=coordinate;
            }
        }
    }
    pub(super) fn commit_guide_drag(&mut self,drag:GuideDrag)->bool{
        if let Some(i)=drag.index{
            self.move_guide(i,drag.position_mm)
        }else{
            self.add_guide(drag.axis,drag.position_mm)
        }
    }
    pub(super) fn paint_guides(&self,page:&egui::Painter,v:View){
        if !self.show_guides{return;}
        let line=Stroke::new(1.0,Color32::from_rgb(59,164,202));
        for guide in &self.doc.guides{
            draw_guide(page,v,guide.axis,guide.position_mm,line);
        }
        if let Some(drag)=self.guide_drag{
            draw_guide(page,v,drag.axis,drag.position_mm,
                Stroke::new(1.5,Color32::from_rgb(242,132,53)));
        }
    }
    /// Creates a new native editable parallel path without replacing the
    /// source geometry or its attached node modes and appearance data.
    pub(super) fn offset_selected_path(&mut self){
        let Some(id)=self.selected else{
            self.status="Выберите одну кривую, линию, эллипс или прямоугольник".into();
            return;
        };
        if !self.multi_selected.is_empty(){
            self.status="Для смещения выберите ровно один объект".into();
            return;
        }
        let Some(source)=self.doc.objects.iter().find(|o|o.id==id).cloned()
            else{return;};
        if source.group_id.is_some() ||
            !self.doc.layer(source.layer_id)
                .is_some_and(|l|l.visible&&!l.locked){
            self.status="Смещение недоступно для групп и заблокированных слоёв".into();
            return;
        }
        if self.doc.clip_links.iter().any(|link|
            link.source_id==id || link.target_id==id){
            self.status="Удалите обтравочные связи перед созданием параллельного контура".into();
            return;
        }
        if self.doc.objects.len()>=100_000 || self.doc.next_id>=u64::MAX-1{
            self.status="Достигнут предел объектов".into();
            return;
        }
        let offset=match opencorel_core::offset::offset_shape(
            &source.shape,self.offset_distance_mm){
            Ok(out)=>out,
            Err(reason)=>{
                self.status=format!("Смещение не создано: {reason}");
                return;
            }
        };
        self.checkpoint();
        let new_id=self.doc.add_to_layer(offset,
            source.style.clone(),source.layer_id);
        if let Some(style)=self.doc.appearances.get(&id).cloned(){
            self.doc.appearances.insert(new_id,style);
        }
        if let Some(object)=self.doc.get_mut(new_id){
            object.name=if source.name.is_empty(){
                "Параллельный контур".into()
            }else{
                format!("{} — смещение",source.name)
                    .chars().take(200).collect()
            };
        }
        self.selected=Some(new_id);
        self.multi_selected.clear();
        self.active_node=None;
        self.fill_cache.entries.clear();self.vector_cache.clear();
        self.compound_cache.clear();self.bezier_preview.clear();
        self.status=format!("Создан редактируемый контур со смещением {:+.2} мм. Исходный сохранён. Ctrl+Z — отмена",
            self.offset_distance_mm);
    }
}
fn draw_guide(page:&egui::Painter,v:View,axis:GuideAxis,value:f32,stroke:Stroke){
    if !value.is_finite(){return;}
    let coordinate=v.screen(Point{x:value,y:value});
    match axis{
        GuideAxis::Horizontal=>{
            if coordinate.y>=v.rect.top() && coordinate.y<=v.rect.bottom(){
                page.line_segment([
                    Pos2::new(v.rect.left(),coordinate.y),
                    Pos2::new(v.rect.right(),coordinate.y),
                ],stroke);
            }
        },
        GuideAxis::Vertical=>{
            if coordinate.x>=v.rect.left() && coordinate.x<=v.rect.right(){
                page.line_segment([
                    Pos2::new(coordinate.x,v.rect.top()),
                    Pos2::new(coordinate.x,v.rect.bottom()),
                ],stroke);
            }
        },
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Document,Shape,Style};
    use eframe::egui::Vec2;
    #[test]
    fn adding_editing_and_deleting_guides_is_undoable(){
        let mut editor=Editor::default();
        editor.doc=Document::default();
        assert!(editor.add_guide(GuideAxis::Vertical,50.0));
        assert!(editor.add_guide(GuideAxis::Horizontal,70.0));
        editor.doc.validate().unwrap();
        let json=serde_json::to_string(&editor.doc).unwrap();
        assert!(json.contains("guides"));
        let restored:Document=serde_json::from_str(&json).unwrap();
        assert_eq!(restored,editor.doc);
        assert!(editor.move_guide(0,55.0));
        assert!(!editor.move_guide(0,55.0));
        assert!(editor.remove_guide(1));
        assert_eq!(editor.doc.guides.len(),1);
        editor.undo();
        assert_eq!(editor.doc.guides.len(),2);
        editor.undo();
        assert_eq!(editor.doc.guides[0].position_mm,50.0);
        editor.undo();
        editor.undo();
        assert!(editor.doc.guides.is_empty());
    }
    #[test]
    fn ruler_drag_creates_guide_with_one_checkpoint(){
        let mut editor=Editor::default();
        editor.doc=Document::default();
        editor.show_rulers=true;editor.show_guides=true;
        let canvas=Rect::from_min_size(Pos2::ZERO,Vec2::new(1000.,700.));
        let working=Rect::from_min_max(
            Pos2::new(25.,24.),canvas.max);
        let v=View::new(working,&editor.doc,1.0,Vec2::ZERO);
        let start=Pos2::new(v.rect.left()+25.0,10.0);
        assert!(editor.begin_guide_drag(v,canvas,start,false));
        assert_eq!(editor.undo.len(),0);
        let mut target=v.mm(start);
        target.x=105.0;
        editor.update_guide_drag(target);
        let drag=editor.guide_drag.take().unwrap();
        assert!(editor.commit_guide_drag(drag));
        assert_eq!(editor.doc.guides.len(),1);
        assert_eq!(editor.doc.guides[0].axis,GuideAxis::Vertical);
        assert_eq!(editor.doc.guides[0].position_mm,105.0);
        assert_eq!(editor.undo.len(),1);
        editor.undo();
        assert!(editor.doc.guides.is_empty());
    }
    #[test]
    fn offset_copy_is_one_undo_step_and_preserves_original(){
        let mut editor=Editor::default();
        editor.doc=Document::default();
        let id=editor.doc.add(Shape::Rectangle{x:0.,y:0.,w:20.,h:10.},
            Style::default());
        editor.selected=Some(id);
        editor.offset_distance_mm=-2.0;
        let previous=editor.doc.clone();
        editor.offset_selected_path();
        assert_eq!(editor.doc.objects.len(),2);
        assert_eq!(editor.undo.len(),1);
        assert_eq!(editor.doc.objects[0].shape,previous.objects[0].shape);
        assert!(matches!(editor.doc.objects[1].shape,Shape::Polyline{closed:true,..}));
        editor.doc.validate().unwrap();
        editor.undo();
        assert_eq!(editor.doc,previous);
    }
    #[test]
    fn offset_rejects_locked_layers_and_invalid_distance_without_history(){
        let mut editor=Editor::default();
        editor.doc=Document::default();
        let id=editor.doc.add(Shape::Rectangle{x:0.,y:0.,w:20.,h:10.},
            Style::default());
        editor.selected=Some(id);
        editor.doc.layers[0].locked=true;
        let old=editor.doc.clone();
        editor.offset_selected_path();
        assert_eq!(editor.doc,old);
        assert!(editor.undo.is_empty());
        editor.doc.layers[0].locked=false;
        editor.offset_distance_mm=f32::NAN;
        let old=editor.doc.clone();
        editor.offset_selected_path();
        assert_eq!(editor.doc,old);
        assert!(editor.undo.is_empty());
    }
}
