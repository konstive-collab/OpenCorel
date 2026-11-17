//! Creative vector pattern generator: radial, arc, spiral, line, wave,
//! zigzag and rotational mandala. Copies remain native editable objects.
//! One checkpoint per accepted generation, comprehensive geometry validation.
use super::Editor;
use eframe::egui::{self,RichText};
use opencorel_core::{Document,Bounds,Object,Shape,Point,transform};
use std::{collections::HashSet,f32::consts::TAU};

const MAX_PATTERN_NEW_OBJECTS:usize=1800;
const MAX_PATTERN_MEMORY:usize=90*1024*1024;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) enum PatternMode{Circle,Arc,Spiral,Line,Wave,Zigzag,Kaleidoscope}
impl PatternMode{
    pub const ALL:[Self;7]=[
        Self::Circle,Self::Arc,Self::Spiral,Self::Line,
        Self::Wave,Self::Zigzag,Self::Kaleidoscope
    ];
    pub fn label(self)->&'static str{
        match self{
            Self::Circle=>"Кольцевой массив",
            Self::Arc=>"Вдоль дуги",
            Self::Spiral=>"Спираль",
            Self::Line=>"Линейный массив под углом",
            Self::Wave=>"Синусоидальная волна",
            Self::Zigzag=>"Зигзаг",
            Self::Kaleidoscope=>"Розетка / вращение копий",
        }
    }
    pub fn hint(self)->&'static str{
        match self{
            Self::Circle=>"Копии в окружности радиуса R вокруг центра выделения",
            Self::Arc=>"Копии по дуге от начального угла до конечного",
            Self::Spiral=>"Плавно растущий радиус от центра до R",
            Self::Line=>"Копии на прямой с шагом D под выбранным углом",
            Self::Wave=>"Декоративный ряд с синусоидальным отклонением",
            Self::Zigzag=>"Смещённый ряд с чередованием верхнего и нижнего положения",
            Self::Kaleidoscope=>"Копии вращаются относительно общего центра и перекрываются",
        }
    }
}
#[derive(Clone,Copy,Debug)]
struct PatternOptions{
    mode:PatternMode,
    copies:usize,
    radius:f32,
    spacing:f32,
    amplitude:f32,
    cycles:f32,
    start_degrees:f32,
    sweep_degrees:f32,
    rotate_objects:bool,
}
impl PatternOptions{
    fn validate(self)->Result<(),String>{
        if !(2..=64).contains(&self.copies) ||
            ![self.radius,self.spacing,self.amplitude,self.cycles,
                self.start_degrees,self.sweep_degrees].iter().all(|v|v.is_finite()){
            return Err("Неверное количество копий или нечисловой параметр".into());
        }
        if self.radius<0.0||self.radius>20_000.0||
            self.spacing<0.0||self.spacing>20_000.0||
            self.amplitude.abs()>20_000.0||
            self.cycles.abs()>50.0||
            self.start_degrees.abs()>3600.0||
            self.sweep_degrees.abs()>3600.0{
            return Err("Параметры массива выходят за допустимый диапазон".into());
        }
        if self.mode==PatternMode::Kaleidoscope && !self.rotate_objects{
            return Err("Розетка требует включённого поворота копий".into());
        }
        Ok(())
    }
    fn layout(self,index:usize)->Result<(f32,f32,f32),String>{
        self.validate()?;
        let idx=index as f32;
        let count=self.copies as f32;
        let start=self.start_degrees.to_radians();
        let angle_for=|fraction:f32|{
            start+self.sweep_degrees.to_radians()*fraction
        };
        let (x,y,rotation)=match self.mode{
            PatternMode::Circle=>{
                let angle=start+TAU*idx/count;
                (self.radius*angle.cos(),self.radius*angle.sin(),angle.to_degrees()+90.)
            }
            PatternMode::Arc=>{
                let fraction=idx/(count-1.0);
                let angle=angle_for(fraction);
                (self.radius*angle.cos(),self.radius*angle.sin(),angle.to_degrees()+90.)
            }
            PatternMode::Spiral=>{
                let fraction=(idx+1.0)/count;
                let angle=angle_for(fraction);
                let radius=self.radius*fraction;
                (radius*angle.cos(),radius*angle.sin(),angle.to_degrees()+90.)
            }
            PatternMode::Line=>{
                let angle=start;
                let d=self.spacing*(idx+1.0);
                (d*angle.cos(),d*angle.sin(),angle.to_degrees())
            }
            PatternMode::Wave=>{
                let advance=self.spacing*(idx+1.0);
                let phase=TAU*self.cycles*(idx+1.0)/(count+1.0);
                let y=self.amplitude*phase.sin();
                let slope=if self.spacing>0.001{
                    self.amplitude*TAU*self.cycles/(count+1.0)/
                        self.spacing*phase.cos()
                }else{0.0};
                let (sin,cos)=start.sin_cos();
                (advance*cos-y*sin,advance*sin+y*cos,
                    start.to_degrees()+slope.atan().to_degrees())
            }
            PatternMode::Zigzag=>{
                let advance=self.spacing*(idx+1.0);
                let y=if index%2==0{self.amplitude}else{-self.amplitude};
                let (sin,cos)=start.sin_cos();
                (advance*cos-y*sin,advance*sin+y*cos,start.to_degrees())
            }
            PatternMode::Kaleidoscope=>{
                let angle=angle_for((idx+1.0)/count);
                (0.,0.,angle.to_degrees())
            }
        };
        if ![x,y,rotation].iter().all(|v|v.is_finite()){
            return Err("Координаты массива вышли за числовой диапазон".into());
        }
        Ok((x,y,rotation))
    }
}
fn selected_group_center(objects:&[Object])->Result<Point,String>{
    let mut min=Point{x:f32::INFINITY,y:f32::INFINITY};
    let mut max=Point{x:f32::NEG_INFINITY,y:f32::NEG_INFINITY};
    for obj in objects{
        let Bounds{x,y,w,h}=obj.bounds();
        if ![x,y,w,h].iter().all(|v|v.is_finite()){
            return Err("Некорректные границы исходной фигуры".into());
        }
        min.x=min.x.min(x);min.y=min.y.min(y);
        max.x=max.x.max(x+w);max.y=max.y.max(y+h);
    }
    if !min.x.is_finite()||!min.y.is_finite(){return Err("Нет исходных объектов".into());}
    Ok(Point{x:(min.x+max.x)*0.5,y:(min.y+max.y)*0.5})
}
/// Generate one batch, retaining shape type, colors, typography and appearance.
/// Group members and clipping operands are rejected: copying them separately
/// would produce dangling references or visually incorrect groups.
fn generate_pattern(doc:&mut Document,ids:&[u64],opts:PatternOptions)
    ->Result<Vec<u64>,String>{
    opts.validate()?;
    let selected:HashSet<_>=ids.iter().copied().collect();
    let originals:Vec<_>=doc.objects.iter().filter(|o|selected.contains(&o.id))
        .cloned().collect();
    if originals.is_empty()||originals.len()!=selected.len(){
        return Err("Выберите хотя бы один существующий объект".into());
    }
    if originals.iter().any(|o|o.group_id.is_some()||
        !doc.layer(o.layer_id).is_some_and(|l|l.visible&&!l.locked)){
        return Err("Массив требует незаблокированные несгруппированные объекты".into());
    }
    if doc.clip_links.iter().any(|link|
        selected.contains(&link.source_id)||selected.contains(&link.target_id)){
        return Err("Сначала разорвите связи обтравки у выбранных объектов".into());
    }
    if opts.rotate_objects && originals.iter().any(|o|
        matches!(o.shape,Shape::Text{..}|Shape::Raster{..}|Shape::EmbeddedSvg{..})){
        return Err("Для поворота текста и изображений нужны кривые; отключите вращение копий".into());
    }
    let extra=originals.len().checked_mul(opts.copies)
        .ok_or("Переполнение количества копий")?;
    if extra>MAX_PATTERN_NEW_OBJECTS||
        doc.objects.len().saturating_add(extra)>100_000 ||
        doc.next_id.checked_add(extra as u64).is_none_or(|id|id==u64::MAX){
        return Err("Массив превышает предел 1800 объектов".into());
    }
    // Bound expensive raster clones before touching the document.
    let source_size:usize=originals.iter().map(|o|{
        256usize.saturating_add(o.name.len())
            .saturating_add(match &o.shape{
                Shape::Raster{data_base64,..}=>data_base64.len(),
                Shape::EmbeddedSvg{svg,..}=>svg.len(),
                Shape::Text{value,..}=>value.len(),
                Shape::Bezier{segments,..}=>segments.len()*24,
                Shape::Polyline{points,..}=>points.len()*8,
                Shape::CompoundPath{contours}=>contours.iter()
                    .map(|c|32+c.segments.len()*24).sum(),
                _=>64,
            })
    }).sum();
    if source_size.saturating_mul(opts.copies)>MAX_PATTERN_MEMORY{
        return Err("Массив превысит безопасный лимит 90 МБ".into());
    }
    let center=selected_group_center(&originals)?;
    let mut generated=Vec::with_capacity(extra);
    for i in 0..opts.copies{
        let (dx,dy,rotation)=opts.layout(i)?;
        for source in &originals{
            let mut o=source.clone();
            o.id=0;o.group_id=None;
            o.name=String::new();
            if opts.rotate_objects{
                transform::rotate_shape(&mut o.shape,center,rotation)
                    .map_err(|e|format!("Не удалось повернуть копию: {e}"))?;
            }
            o.shape.translate(dx,dy);
            let b=o.bounds();
            if ![b.x,b.y,b.w,b.h].iter().all(|v|v.is_finite())||
                b.x< -100_000.||b.y< -100_000.||
                b.x+b.w>100_000.||b.y+b.h>100_000.{
                return Err("Копия выходит за допустимые координаты".into());
            }
            generated.push((source.id,o));
        }
    }
    let mut new_ids=Vec::with_capacity(extra);
    for (source_id,mut obj) in generated{
        let id=doc.next_id;
        doc.next_id+=1;obj.id=id;
        doc.objects.push(obj);
        if let Some(a)=doc.appearances.get(&source_id).cloned(){
            doc.appearances.insert(id,a);
        }
        new_ids.push(id);
    }
    doc.validate()?;
    Ok(new_ids)
}
impl Editor{
    pub(super) fn create_pattern(&mut self,mode:PatternMode){
        let opts=PatternOptions{
            mode,
            copies:self.pattern_copies as usize,
            radius:self.pattern_radius_mm,
            spacing:self.pattern_step_mm,
            amplitude:self.pattern_amplitude_mm,
            cycles:self.pattern_cycles,
            start_degrees:self.pattern_start_angle,
            sweep_degrees:self.pattern_sweep_angle,
            rotate_objects:self.pattern_rotate_objects||mode==PatternMode::Kaleidoscope,
        };
        let mut candidate=self.doc.clone();
        let ids=self.selection_ids();
        match generate_pattern(&mut candidate,&ids,opts){
            Ok(new_ids)=>{
                self.checkpoint();
                self.doc=candidate;
                self.selected=new_ids.first().copied();
                self.multi_selected=new_ids.into_iter().skip(1).collect();
                self.active_node=None;self.node_drag=None;self.node_drag_checkpointed=false;
                self.fill_cache.entries.clear();self.compound_cache.clear();
                self.vector_cache.clear();self.bezier_preview.clear();self.raster_cache.clear();
                self.status=format!("{}: создано {} копий. Исходные фигуры сохранены. Ctrl+Z — отмена.",
                    mode.label(),self.selection_ids().len());
            }
            Err(e)=>self.status=format!("Генератор массива: {e}"),
        }
    }
    pub(super) fn pattern_studio_panel(&mut self,ui:&mut egui::Ui){
        ui.label(RichText::new("Генератор векторных композиций").strong());
        ui.horizontal(|ui|{
            ui.label("Копий");
            ui.add(egui::DragValue::new(&mut self.pattern_copies).range(2..=64));
        });
        ui.horizontal(|ui|{
            ui.label("Радиус (мм)");
            ui.add(egui::DragValue::new(&mut self.pattern_radius_mm)
                .speed(1.0).range(0.0..=20_000.0));
        });
        ui.horizontal(|ui|{
            ui.label("Шаг (мм)");
            ui.add(egui::DragValue::new(&mut self.pattern_step_mm)
                .speed(1.0).range(0.0..=20_000.0));
        });
        ui.horizontal(|ui|{
            ui.label("Амплитуда (мм)");
            ui.add(egui::DragValue::new(&mut self.pattern_amplitude_mm)
                .speed(0.5).range(-20_000.0..=20_000.0));
        });
        ui.horizontal(|ui|{
            ui.label("Циклов");
            ui.add(egui::DragValue::new(&mut self.pattern_cycles)
                .speed(0.25).range(-50.0..=50.0));
        });
        ui.horizontal(|ui|{
            ui.label("Начальный угол");
            ui.add(egui::DragValue::new(&mut self.pattern_start_angle)
                .speed(1.0).range(-3600.0..=3600.0).suffix("°"));
        });
        ui.horizontal(|ui|{
            ui.label("Размах дуги");
            ui.add(egui::DragValue::new(&mut self.pattern_sweep_angle)
                .speed(1.0).range(-3600.0..=3600.0).suffix("°"));
        });
        ui.checkbox(&mut self.pattern_rotate_objects,
            "Поворачивать копии (только векторы)");
        ui.separator();
        for mode in PatternMode::ALL{
            if ui.button(mode.label()).on_hover_text(mode.hint()).clicked(){
                self.create_pattern(mode);
            }
        }
        ui.small("Исходные объекты остаются на месте. Нельзя копировать элементы обтравки и группы. До 1800 новых объектов за действие.");
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::Style;
    fn base(mode:PatternMode)->PatternOptions{
        PatternOptions{mode,copies:4,radius:20.,spacing:10.,
            amplitude:5.,cycles:1.,start_degrees:0.,
            sweep_degrees:180.,rotate_objects:false}
    }
    #[test]
    fn seven_modes_produce_finite_distinct_positions(){
        for mode in PatternMode::ALL{
            let mut options=base(mode);
            if mode==PatternMode::Kaleidoscope{options.rotate_objects=true;}
            for i in 0..options.copies{
                let (x,y,angle)=options.layout(i).unwrap();
                assert!(x.is_finite()&&y.is_finite()&&angle.is_finite());
            }
        }
    }
    #[test]
    fn circle_arc_spiral_and_wave_are_mm_exact(){
        let mut circle=base(PatternMode::Circle);
        let (x,y,_) = circle.layout(1).unwrap();
        assert!(x.abs()<0.001);assert!((y-20.).abs()<0.001);
        circle.mode=PatternMode::Arc;
        let (x,y,_)=circle.layout(3).unwrap();
        assert!((x+20.).abs()<0.001);assert!(y.abs()<0.001);
        circle.mode=PatternMode::Spiral;
        let (x,y,_)=circle.layout(3).unwrap();
        assert!((x+20.).abs()<0.001);assert!(y.abs()<0.001);
        circle.mode=PatternMode::Wave;
        let (x,_,_)=circle.layout(2).unwrap();
        assert_eq!(x,30.);
    }
    #[test]
    fn native_objects_and_appearance_stay_editable_and_one_undo_step(){
        let mut e=Editor::default();e.doc=Document::default();
        let id=e.doc.add(Shape::Rectangle{x:100.,y:100.,w:20.,h:10.},
            Style::default());
        e.selected=Some(id);e.pattern_copies=4;e.pattern_radius_mm=30.;
        let original=e.doc.clone();
        e.create_pattern(PatternMode::Circle);
        assert_eq!(e.doc.objects.len(),5);
        assert_eq!(e.undo.len(),1);
        assert!(e.doc.objects.iter().all(|o|matches!(o.shape,Shape::Rectangle{..})));
        e.doc.validate().unwrap();
        e.undo();assert_eq!(e.doc,original);
    }
    #[test]
    fn locked_clip_and_invalid_count_do_not_checkpoint(){
        let mut e=Editor::default();e.doc=Document::default();
        let id=e.doc.add(Shape::Rectangle{x:0.,y:0.,w:5.,h:5.},
            Style::default());
        e.selected=Some(id);e.doc.layers[0].locked=true;
        let old=e.doc.clone();
        e.create_pattern(PatternMode::Circle);
        assert_eq!(e.doc,old);assert!(e.undo.is_empty());
        e.doc.layers[0].locked=false;e.pattern_copies=500;
        e.create_pattern(PatternMode::Circle);
        assert!(e.undo.is_empty());
    }
    #[test]
    fn rotation_rejects_text_and_creates_editable_rosette_from_rect(){
        let mut e=Editor::default();e.doc=Document::default();
        let id=e.doc.add(Shape::Text{x:0.,y:0.,value:"Hi".into(),size_mm:5.},
            Style::default());
        e.selected=Some(id);
        e.create_pattern(PatternMode::Kaleidoscope);
        assert!(e.undo.is_empty());
        e.doc=Document::default();
        let id=e.doc.add(Shape::Rectangle{x:20.,y:20.,w:10.,h:15.},
            Style::default());
        e.selected=Some(id);
        e.pattern_copies=4;
        e.create_pattern(PatternMode::Kaleidoscope);
        assert_eq!(e.doc.objects.len(),5);
        e.doc.validate().unwrap();
    }
}
