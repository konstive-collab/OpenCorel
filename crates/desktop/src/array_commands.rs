//! Production repeat-grid layout. Copies existing editable objects into
//! rows/columns without rasterization, loss of typography or effect metadata.
use super::Editor;
use std::collections::HashSet;
use opencorel_core::Object;

const MAX_NEW_OBJECTS:usize=2000;
const MAX_NEW_ESTIMATED_BYTES:usize=96*1024*1024;

impl Editor{
    pub(super) fn align_selection_precise(&mut self,alignment:opencorel_core::Alignment){
        let ids=self.selection_ids();
        if ids.len()<2{
            self.status="Для выравнивания выберите не менее двух объектов".into();return;
        }
        if !self.layout_selection_editable(&ids){return;}
        // Preview the change before checkpointing; reject non-finite positions.
        let mut preview=self.doc.clone();
        if preview.align_objects(&ids,alignment)!=ids.len() ||
            preview.validate().is_err(){
            self.status="Выравнивание отклонено из-за ограничений геометрии".into();
            return;
        }
        if preview==self.doc{return;}
        self.checkpoint();
        self.doc=preview;
        self.clear_layout_caches();
        self.status=format!("Выровнено объектов: {}. Ctrl+Z — отмена",ids.len());
    }

    pub(super) fn distribute_selection_precise(&mut self,dir:opencorel_core::Distribution){
        let ids=self.selection_ids();
        if ids.len()<3{
            self.status="Для распределения нужно хотя бы три объекта".into();return;
        }
        if !self.layout_selection_editable(&ids){return;}
        let mut preview=self.doc.clone();
        if preview.distribute_objects(&ids,dir)!=ids.len() ||
            preview.validate().is_err(){
            self.status="Распределение отклонено из-за ограничений геометрии".into();
            return;
        }
        if preview==self.doc{return;}
        self.checkpoint();
        self.doc=preview;
        self.clear_layout_caches();
        self.status=format!("Равномерно распределено объектов: {}. Ctrl+Z — отмена",ids.len());
    }

    fn layout_selection_editable(&mut self,ids:&[u64])->bool{
        let chosen:HashSet<u64>=ids.iter().copied().collect();
        if self.doc.objects.iter().filter(|o|chosen.contains(&o.id)).count()!=chosen.len() ||
            self.doc.objects.iter().filter(|o|chosen.contains(&o.id)).any(|o|
                o.group_id.is_some() ||
                !self.doc.layer(o.layer_id).is_some_and(|l|l.visible&&!l.locked)) ||
            self.doc.clip_links.iter().any(|l|
                chosen.contains(&l.source_id)||chosen.contains(&l.target_id)){
            self.status="Выравнивание: нужны доступные объекты без групп и обтравки".into();
            return false;
        }
        true
    }

    fn clear_layout_caches(&mut self){
        self.fill_cache.entries.clear();
        self.compound_cache.clear();
        self.vector_cache.clear();
        self.bezier_preview.clear();
        self.raster_cache.clear();
    }
    pub(super) fn repeat_selection_as_grid(&mut self){
        let cols=self.array_columns as usize;
        let rows=self.array_rows as usize;
        if cols==0||rows==0||cols>50||rows>50||
            (cols==1&&rows==1){
            self.status="Сетка: от 1 до 50 строк и столбцов, минимум 2 ячейки".into();return;
        }
        if !self.array_gap_x_mm.is_finite()||!self.array_gap_y_mm.is_finite()
            || self.array_gap_x_mm.abs()>10_000.
            || self.array_gap_y_mm.abs()>10_000.{
            self.status="Некорректные отступы между объектами".into();return;
        }
        let selected:HashSet<u64>=self.selection_ids().into_iter().collect();
        if selected.is_empty(){
            self.status="Выберите объекты для размножения по сетке".into();return;
        }
        let sources:Vec<Object>=self.doc.objects.iter()
            .filter(|o|selected.contains(&o.id)).cloned().collect();
        if sources.len()!=selected.len(){
            self.status="Один или несколько выделенных объектов не найдены".into();return;
        }
        if sources.iter().any(|o|o.group_id.is_some() ||
            !self.doc.layer(o.layer_id).is_some_and(|l|l.visible&&!l.locked)){
            self.status="Сетка: сначала разгруппируйте объекты и разблокируйте слои".into();
            return;
        }
        if self.doc.clip_links.iter().any(|l|
            selected.contains(&l.source_id)||selected.contains(&l.target_id)){
            self.status="Сетка: сначала удалите связи обтравки у выбранных объектов".into();
            return;
        }
        let Some(cells)=cols.checked_mul(rows) else{
            self.status="Слишком много ячеек".into();return;
        };
        let Some(extra)=cells.checked_sub(1).and_then(|n|n.checked_mul(sources.len())) else{
            self.status="Переполнение количества копий".into();return;
        };
        if extra>MAX_NEW_OBJECTS || self.doc.objects.len().saturating_add(extra)>100_000
            || self.doc.next_id.checked_add(extra as u64).is_none_or(|v|v==u64::MAX){
            self.status="Сетка: превышен предел 2000 новых объектов или ID документа".into();
            return;
        }
        let source_bytes=sources.iter().fold(0usize,|sum,o|{
            sum.saturating_add(256)
                .saturating_add(o.name.len())
                .saturating_add(o.text_format.family.len())
                .saturating_add(match &o.shape{
                    opencorel_core::Shape::Raster{data_base64,..}=>data_base64.len(),
                    opencorel_core::Shape::EmbeddedSvg{svg,..}=>svg.len(),
                    opencorel_core::Shape::Text{value,..}=>value.len(),
                    opencorel_core::Shape::Bezier{segments,..}=>segments.len()*24,
                    opencorel_core::Shape::Polyline{points,..}=>points.len()*8,
                    opencorel_core::Shape::CompoundPath{contours}=>
                        contours.iter().map(|c|32+c.segments.len()*24).sum(),
                    _=>64,
                })
        });
        if source_bytes.saturating_mul(cells.saturating_sub(1))>MAX_NEW_ESTIMATED_BYTES{
            self.status="Сетка: объём копий превышает безопасный предел 96 МБ".into();
            return;
        }
        let (mut min_x,mut min_y,mut max_x,mut max_y)=
            (f32::INFINITY,f32::INFINITY,f32::NEG_INFINITY,f32::NEG_INFINITY);
        for obj in &sources{
            let b=obj.bounds();
            min_x=min_x.min(b.x);min_y=min_y.min(b.y);
            max_x=max_x.max(b.x+b.w);max_y=max_y.max(b.y+b.h);
        }
        let step_x=max_x-min_x+self.array_gap_x_mm;
        let step_y=max_y-min_y+self.array_gap_y_mm;
        if !step_x.is_finite()||!step_y.is_finite()||
            step_x<=0. || step_y<=0.{
            self.status="Сетка: шаг должен быть положительным".into();return;
        }
        let dx=step_x*(cols-1) as f32;
        let dy=step_y*(rows-1) as f32;
        if ![min_x,min_y,max_x,max_y,dx,dy].iter().all(|v|v.is_finite())||
            min_x.min(min_x+dx)< -100_000. || max_x.max(max_x+dx)>100_000. ||
            min_y.min(min_y+dy)< -100_000. || max_y.max(max_y+dy)>100_000.{
            self.status="Сетка выходит за допустимые координаты документа".into();
            return;
        }
        // Validate every translated shape before the only undo checkpoint.
        // This avoids a half-written layout on invalid geometry or overflow.
        let mut generated=Vec::with_capacity(extra);
        for row in 0..rows{
            for col in 0..cols{
                if row==0 && col==0{continue;}
                let shift_x=step_x*col as f32;
                let shift_y=step_y*row as f32;
                for source in &sources{
                    let mut object=source.clone();
                    object.id=0;
                    object.group_id=None;
                    object.name=String::new();
                    object.shape.translate(shift_x,shift_y);
                    generated.push((source.id,object));
                }
            }
        }
        // No document changes before checkpoint. Appearance and text formats
        // follow source IDs; all new instances are independent editable objects.
        self.checkpoint();
        let mut new_ids=Vec::with_capacity(extra);
        for (source_id,mut obj) in generated{
            let id=self.doc.next_id;
            self.doc.next_id+=1;
            obj.id=id;
            self.doc.objects.push(obj);
            if let Some(a)=self.doc.appearances.get(&source_id).cloned(){
                self.doc.appearances.insert(id,a);
            }
            new_ids.push(id);
        }
        self.selected=new_ids.first().copied();
        self.multi_selected=new_ids.into_iter().skip(1).collect();
        self.active_node=None;self.node_drag=None;self.node_drag_checkpointed=false;
        self.fill_cache.entries.clear();self.compound_cache.clear();
        self.vector_cache.clear();self.bezier_preview.clear();self.raster_cache.clear();
        self.status=format!("Сетка: {} × {}, создано {} новых объектов. Ctrl+Z — отмена",
            cols,rows,extra);
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Document,Shape,Style};
    #[test]
    fn alignment_preserves_other_objects_and_undo(){
        let mut e=Editor::default();e.doc=Document::default();
        let a=e.doc.add(Shape::Rectangle{x:10.,y:4.,w:10.,h:8.},Style::default());
        let b=e.doc.add(Shape::Rectangle{x:50.,y:15.,w:20.,h:8.},Style::default());
        let untouched=e.doc.add(Shape::Rectangle{x:130.,y:40.,w:12.,h:12.},
            Style::default());
        e.selected=Some(a);e.multi_selected=vec![b];
        let before=e.doc.clone();
        e.align_selection_precise(opencorel_core::Alignment::Left);
        assert_eq!(e.doc.objects[0].bounds().x,10.);
        assert_eq!(e.doc.objects[1].bounds().x,10.);
        assert_eq!(e.doc.get_mut(untouched).unwrap().bounds().x,130.);
        assert_eq!(e.undo.len(),1);
        e.undo();assert_eq!(e.doc,before);
    }
    #[test]
    fn distribution_keeps_endpoints_and_undoes(){
        let mut e=Editor::default();e.doc=Document::default();
        let ids:Vec<_>=[0.,11.,50.].into_iter().map(|x|
            e.doc.add(Shape::Rectangle{x,y:0.,w:10.,h:10.},Style::default())
        ).collect();
        e.selected=Some(ids[0]);e.multi_selected=ids[1..].to_vec();
        e.distribute_selection_precise(opencorel_core::Distribution::HorizontalCenters);
        assert_eq!(e.doc.objects.iter().map(|o|o.bounds().x).collect::<Vec<_>>(),
            vec![0.,25.,50.]);
        assert_eq!(e.undo.len(),1);
        e.doc.validate().unwrap();
    }
    #[test]
    fn repeat_grid_places_objects_in_exact_mm_and_undoes_as_one_action(){
        let mut e=Editor::default();e.doc=Document::default();
        let id=e.doc.add(Shape::Rectangle{x:10.,y:20.,w:30.,h:15.},Style::default());
        e.selected=Some(id);e.array_columns=3;e.array_rows=2;
        e.array_gap_x_mm=5.;e.array_gap_y_mm=10.;
        let original=e.doc.clone();
        e.repeat_selection_as_grid();
        assert_eq!(e.doc.objects.len(),6);
        assert_eq!(e.undo.len(),1);
        let positions:Vec<_>=e.doc.objects.iter().map(|o|{
            let b=o.bounds();(b.x,b.y)
        }).collect();
        assert_eq!(positions,vec![
            (10.,20.),(45.,20.),(80.,20.),
            (10.,45.),(45.,45.),(80.,45.)
        ]);
        e.doc.validate().unwrap();
        e.undo();assert_eq!(e.doc,original);
        e.redo();e.doc.validate().unwrap();
    }
    #[test]
    fn repeat_grid_rejects_locked_or_invalid_without_checkpoint(){
        let mut e=Editor::default();e.doc=Document::default();
        let id=e.doc.add(Shape::Rectangle{x:0.,y:0.,w:10.,h:10.},Style::default());
        e.selected=Some(id);e.doc.layers[0].locked=true;
        let before=e.doc.clone();
        e.repeat_selection_as_grid();
        assert_eq!(e.doc,before);assert!(e.undo.is_empty());
        e.doc.layers[0].locked=false;
        e.array_columns=50;e.array_rows=50;
        e.repeat_selection_as_grid();
        assert!(e.undo.is_empty());
        e.array_columns=3;e.array_rows=2;e.array_gap_x_mm=f32::NAN;
        e.repeat_selection_as_grid();
        assert!(e.undo.is_empty());
    }
    #[test]
    fn repeat_grid_retains_independent_text_and_style(){
        let mut e=Editor::default();e.doc=Document::default();
        let id=e.doc.add(Shape::Text{x:0.,y:0.,value:"Привет".into(),size_mm:5.},
            Style::default());
        e.selected=Some(id);e.array_columns=2;e.array_rows=1;
        e.repeat_selection_as_grid();
        assert_eq!(e.doc.objects.len(),2);
        assert_eq!(e.doc.objects[0].style,e.doc.objects[1].style);
        assert_eq!(e.doc.objects[0].text_format,e.doc.objects[1].text_format);
        assert_ne!(e.doc.objects[0].id,e.doc.objects[1].id);
        e.doc.validate().unwrap();
    }
}
