//! Editable layered vector effects: drop shadow, long shadow, echo,
//! sticker/outline, soft-step glow, risograph, bevel and neon.
//! These are real independent editable vector copies (not GPU blur or
//! nondestructive live filters). Source artwork is never altered.
use super::Editor;
use eframe::egui::{self,RichText};
use opencorel_core::{
    appearance::Appearance,valid_color,Document,Object,Shape,Style,
};
use std::collections::HashSet;

const MAX_EFFECT_OBJECTS:usize=1200;
const MAX_EFFECT_MEMORY:usize=80*1024*1024;
const EFFECT_MARKER:&str="OpenCorel effect of #";

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) enum EffectMode{
    HardShadow,LongShadow,Trail,StickerBorder,
    Contour,SoftGlow,TwoColorPrint,Emboss,Neon,
}
impl EffectMode{
    pub const ALL:[Self;9]=[
        Self::HardShadow,Self::LongShadow,Self::Trail,
        Self::StickerBorder,Self::Contour,Self::SoftGlow,
        Self::TwoColorPrint,Self::Emboss,Self::Neon,
    ];
    pub fn label(self)->&'static str{
        match self{
            Self::HardShadow=>"Жёсткая тень",
            Self::LongShadow=>"Длинная тень",
            Self::Trail=>"Эхо / движение",
            Self::StickerBorder=>"Стикер с окантовкой",
            Self::Contour=>"Внешняя цветная обводка",
            Self::SoftGlow=>"Ступенчатое свечение",
            Self::TwoColorPrint=>"Двухцветное смещение печати",
            Self::Emboss=>"Объёмная подсветка",
            Self::Neon=>"Неоновый векторный ореол",
        }
    }
    pub fn details(self)->&'static str{
        match self{
            Self::HardShadow=>"Одна теневая копия под оригиналом",
            Self::LongShadow=>"Последовательность копий с градацией прозрачности",
            Self::Trail=>"Смещение копий с постепенным ослаблением",
            Self::StickerBorder=>"Белая внешняя обводка и тонкая цветная кромка",
            Self::Contour=>"Цветной контур за исходной фигурой",
            Self::SoftGlow=>"Несколько концентрических обводок без растрового размытия",
            Self::TwoColorPrint=>"Красочный эффект несовмещения двух печатных слоёв",
            Self::Emboss=>"Контрастная светлая и тёмная подложка для объёма",
            Self::Neon=>"Яркая обводка с несколькими полупрозрачными ореолами",
        }
    }
    fn stroke_only(self)->bool{
        matches!(self,Self::StickerBorder|Self::Contour|
            Self::SoftGlow|Self::Neon)
    }
}
#[derive(Clone,Debug)]
struct EffectOptions{
    mode:EffectMode,
    offset_x:f32,
    offset_y:f32,
    depth:usize,
    width_mm:f32,
    opacity:f32,
    primary:String,
    secondary:String,
}
impl EffectOptions{
    fn validate(&self)->Result<(),String>{
        if self.depth==0||self.depth>24 ||
            ![self.offset_x,self.offset_y,self.width_mm,self.opacity].iter()
                .all(|v|v.is_finite())||
            self.offset_x.abs()>5000.||self.offset_y.abs()>5000.||
            !(0.1..=100.0).contains(&self.width_mm)||
            !(0.01..=1.0).contains(&self.opacity)||
            !valid_color(&self.primary)||!valid_color(&self.secondary){
            return Err("Ошибка параметров эффектов: цвет #RRGGBB, глубина 1–24, прозрачность 1–100 %".into());
        }
        Ok(())
    }
    fn layer_count(&self)->usize{
        match self.mode{
            EffectMode::HardShadow|EffectMode::Contour=>1,
            EffectMode::StickerBorder|EffectMode::TwoColorPrint|EffectMode::Emboss=>2,
            EffectMode::LongShadow|EffectMode::Trail=>self.depth,
            EffectMode::SoftGlow|EffectMode::Neon=>self.depth+1,
        }
    }
}
#[derive(Clone,Copy)]
struct EffectLayer{
    x:f32,y:f32,
    stroke_mm:f32,
    opacity:f32,
    color_choice:usize,
    outline:bool,
}
impl EffectOptions{
    fn recipe(&self)->Vec<EffectLayer>{
        let mut layers=Vec::with_capacity(self.layer_count());
        let default=EffectLayer{
            x:self.offset_x,y:self.offset_y,stroke_mm:self.width_mm,
            opacity:self.opacity,color_choice:0,outline:false,
        };
        match self.mode{
            EffectMode::HardShadow=>layers.push(default),
            EffectMode::LongShadow=>{
                for n in (1..=self.depth).rev(){
                    let frac=n as f32/self.depth as f32;
                    layers.push(EffectLayer{x:self.offset_x*frac,
                        y:self.offset_y*frac,opacity:self.opacity*frac,
                        ..default});
                }
            }
            EffectMode::Trail=>{
                for n in (1..=self.depth).rev(){
                    let frac=n as f32/self.depth as f32;
                    layers.push(EffectLayer{x:self.offset_x*n as f32,
                        y:self.offset_y*n as f32,
                        opacity:self.opacity*(1.0-frac*0.85),
                        ..default});
                }
            }
            EffectMode::Contour=>layers.push(EffectLayer{
                x:0.,y:0.,outline:true,..default
            }),
            EffectMode::StickerBorder=>{
                layers.push(EffectLayer{x:0.,y:0.,
                    color_choice:0,stroke_mm:self.width_mm*1.75,
                    outline:true,..default});
                layers.push(EffectLayer{x:0.,y:0.,
                    color_choice:1,stroke_mm:self.width_mm*0.6,
                    outline:true,..default});
            }
            EffectMode::SoftGlow=>{
                for n in (1..=self.depth).rev(){
                    let frac=n as f32/self.depth as f32;
                    layers.push(EffectLayer{x:0.,y:0.,outline:true,
                        stroke_mm:self.width_mm*(1.0+frac*4.0),
                        opacity:self.opacity*(1.0-frac)*0.35,
                        ..default});
                }
                layers.push(EffectLayer{x:0.,y:0.,outline:true,
                    stroke_mm:self.width_mm,opacity:self.opacity,
                    ..default});
            }
            EffectMode::TwoColorPrint=>{
                layers.push(EffectLayer{x:-self.offset_x,y:-self.offset_y,
                    color_choice:1,opacity:self.opacity*0.8,..default});
                layers.push(EffectLayer{x:self.offset_x,y:self.offset_y,
                    color_choice:0,opacity:self.opacity*0.8,..default});
            }
            EffectMode::Emboss=>{
                layers.push(EffectLayer{x:self.offset_x,y:self.offset_y,
                    color_choice:0,..default});
                layers.push(EffectLayer{x:-self.offset_x,y:-self.offset_y,
                    color_choice:1,opacity:self.opacity*0.65,..default});
            }
            EffectMode::Neon=>{
                for n in (1..=self.depth).rev(){
                    let frac=n as f32/self.depth as f32;
                    layers.push(EffectLayer{x:0.,y:0.,outline:true,
                        color_choice:0,stroke_mm:self.width_mm*(1.0+frac*5.0),
                        opacity:self.opacity*(1.0-frac)*0.25,
                        ..default});
                }
                layers.push(EffectLayer{x:0.,y:0.,outline:true,
                    color_choice:1,stroke_mm:self.width_mm*0.85,
                    opacity:self.opacity,..default});
            }
        }
        layers
    }
}
fn make_effect_copy(source:&Object,layer:EffectLayer,color:&str,
    idx:usize)->Object{
    let mut copy=source.clone();
    copy.id=0;copy.group_id=None;
    copy.name=format!("{EFFECT_MARKER}{} ({idx})",source.id);
    copy.shape.translate(layer.x,layer.y);
    let open_path=matches!(copy.shape,
        Shape::Polyline{closed:false,..}|Shape::Bezier{closed:false,..});
    if layer.outline||open_path{
        copy.style=Style{fill:"none".into(),stroke:color.to_owned(),
            stroke_width_mm:if layer.outline{
                layer.stroke_mm
            }else{layer.stroke_mm.max(source.style.stroke_width_mm)}};
    }else{
        copy.style=Style{fill:color.to_owned(),stroke:"none".into(),
            stroke_width_mm:0.0};
    }
    copy
}
fn generate_effect(doc:&mut Document,ids:&[u64],opts:&EffectOptions)->Result<Vec<u64>,String>{
    opts.validate()?;
    let selected:HashSet<_>=ids.iter().copied().collect();
    let originals:Vec<_>=doc.objects.iter().filter(|o|selected.contains(&o.id))
        .cloned().collect();
    if originals.is_empty()||originals.len()!=selected.len(){
        return Err("Сначала выберите редактируемые объекты".into());
    }
    if originals.iter().any(|o|o.group_id.is_some()||
        !doc.layer(o.layer_id).is_some_and(|l|l.visible&&!l.locked)){
        return Err("Эффекты недоступны для групп или закрытых слоёв".into());
    }
    if doc.clip_links.iter().any(|link|
        selected.contains(&link.source_id)||selected.contains(&link.target_id)){
        return Err("Эффекты требуют снять обтравку с исходных объектов".into());
    }
    if originals.iter().any(|o|matches!(o.shape,
        Shape::Raster{..}|Shape::EmbeddedSvg{..})||
        (opts.mode.stroke_only()&&matches!(o.shape,Shape::Text{..}))){
        return Err("Выбранный эффект поддерживает только собственные векторы; для текста доступны тени, эхо и цветовое смещение".into());
    }
    let layers=opts.recipe();
    let extra=layers.len().checked_mul(originals.len())
        .ok_or("Ошибка количества эффектов")?;
    if extra>MAX_EFFECT_OBJECTS||doc.objects.len().saturating_add(extra)>100_000
        ||doc.next_id.checked_add(extra as u64).is_none_or(|id|id>=u64::MAX){
        return Err("Слишком много векторных копий эффекта".into());
    }
    let bytes:usize=originals.iter().map(|o|
        256usize.saturating_add(match &o.shape{
            Shape::Text{value,..}=>value.len(),
            Shape::Polyline{points,..}=>points.len()*8,
            Shape::Bezier{segments,..}=>segments.len()*24,
            Shape::CompoundPath{contours}=>contours.iter()
                .map(|c|c.segments.len()*24+32).sum(),
            _=>64,
        })
    ).sum();
    if bytes.saturating_mul(layers.len())>MAX_EFFECT_MEMORY{
        return Err("Эффект превышает лимит памяти".into());
    }
    let mut new_ids=Vec::with_capacity(extra);
    for src in originals{
        let index=doc.objects.iter().position(|o|o.id==src.id)
            .ok_or("Исходный объект исчез")?;
        let mut batch=Vec::with_capacity(layers.len());
        for (n,layer) in layers.iter().copied().enumerate(){
            let color=if layer.color_choice==0{
                &opts.primary
            }else{&opts.secondary};
            let mut obj=make_effect_copy(&src,layer,color,n+1);
            let b=obj.bounds();
            if ![b.x,b.y,b.w,b.h].iter().all(|v|v.is_finite())||
                b.x< -100_000.||b.y< -100_000.||
                b.x+b.w>100_000.||b.y+b.h>100_000.{
                return Err("Эффект выходит за координаты макета".into());
            }
            let id=doc.next_id;doc.next_id+=1;obj.id=id;
            doc.appearances.insert(id,Appearance{
                opacity:layer.opacity,gradient:None,..Appearance::default()
            });
            batch.push(obj);
            new_ids.push(id);
        }
        doc.objects.splice(index..index,batch);
    }
    doc.validate()?;
    Ok(new_ids)
}
fn clean_effect_copies(doc:&mut Document,source_ids:&[u64])->Result<usize,String>{
    let labels:Vec<String>=source_ids.iter().map(|id|
        format!("{EFFECT_MARKER}{id} (")).collect();
    let matching:HashSet<u64>=doc.objects.iter()
        .filter(|o|labels.iter().any(|s|o.name.starts_with(s)))
        .map(|o|o.id).collect();
    if matching.is_empty(){return Ok(0);}
    for object in doc.objects.iter().filter(|o|matching.contains(&o.id)){
        if !doc.layer(object.layer_id).is_some_and(|l|l.visible&&!l.locked){
            return Err("Среди копий есть объекты на закрытых слоях".into());
        }
    }
    let count=matching.len();
    doc.objects.retain(|o|!matching.contains(&o.id));
    doc.appearances.retain(|id,_|!matching.contains(id));
    doc.validate()?;
    Ok(count)
}
impl Editor{
    pub(super) fn create_vector_effect(&mut self,mode:EffectMode){
        let options=EffectOptions{
            mode,offset_x:self.effect_offset_x_mm,offset_y:self.effect_offset_y_mm,
            depth:self.effect_depth as usize,width_mm:self.effect_width_mm,
            opacity:self.effect_opacity,primary:self.effect_primary.clone(),
            secondary:self.effect_secondary.clone(),
        };
        let mut candidate=self.doc.clone();
        match generate_effect(&mut candidate,&self.selection_ids(),&options){
            Ok(created)=>{
                self.checkpoint();
                self.doc=candidate;
                self.selected=created.first().copied();
                self.multi_selected=created.into_iter().skip(1).collect();
                self.fill_cache.entries.clear();self.vector_cache.clear();
                self.compound_cache.clear();self.bezier_preview.clear();
                self.raster_cache.clear();
                self.status=format!("{}: создано {} редактируемых векторных слоёв. Ctrl+Z — отмена.",
                    mode.label(),self.selection_ids().len());
            }
            Err(e)=>self.status=format!("Векторный эффект: {e}"),
        }
    }
    pub(super) fn remove_vector_effects(&mut self){
        let ids=self.selection_ids();
        if ids.is_empty(){self.status="Выделите исходные объекты эффекта".into();return;}
        let mut candidate=self.doc.clone();
        match clean_effect_copies(&mut candidate,&ids){
            Ok(0)=>self.status="Для выделенных оригиналов эффекты не найдены".into(),
            Ok(count)=>{
                self.checkpoint();self.doc=candidate;
                self.fill_cache.entries.clear();self.vector_cache.clear();
                self.compound_cache.clear();self.bezier_preview.clear();
                self.raster_cache.clear();
                self.status=format!("Удалено {} слоёв эффектов. Ctrl+Z — отмена.",count);
            }
            Err(e)=>self.status=format!("Ошибка удаления эффекта: {e}"),
        }
    }
    pub(super) fn vector_effects_panel(&mut self,ui:&mut egui::Ui){
        ui.label(RichText::new("Векторные эффекты").strong());
        ui.small("Генерирует редактируемые копии под оригиналом, не растровый фильтр");
        ui.horizontal(|ui|{
            ui.label("Смещение X / Y, мм");
            ui.add(egui::DragValue::new(&mut self.effect_offset_x_mm)
                .speed(0.5).range(-5000.0..=5000.0));
            ui.add(egui::DragValue::new(&mut self.effect_offset_y_mm)
                .speed(0.5).range(-5000.0..=5000.0));
        });
        ui.horizontal(|ui|{
            ui.label("Слоёв");
            ui.add(egui::DragValue::new(&mut self.effect_depth).range(1..=24));
            ui.label("Обводка");
            ui.add(egui::DragValue::new(&mut self.effect_width_mm)
                .speed(0.25).range(0.1..=100.0).suffix(" мм"));
        });
        ui.add(egui::Slider::new(&mut self.effect_opacity,0.01..=1.0)
            .text("Прозрачность"));
        ui.horizontal(|ui|{
            ui.label("Основной");
            ui.text_edit_singleline(&mut self.effect_primary);
        });
        ui.horizontal(|ui|{
            ui.label("Второй");
            ui.text_edit_singleline(&mut self.effect_secondary);
        });
        ui.separator();
        for mode in EffectMode::ALL{
            if ui.button(mode.label()).on_hover_text(mode.details()).clicked(){
                self.create_vector_effect(mode);
            }
        }
        ui.separator();
        if ui.button("Удалить копии эффектов выделенного оригинала").clicked(){
            self.remove_vector_effects();
        }
        ui.small("Связь не динамическая: изменение оригинала не обновляет созданные копии. После правки оригинала удалите прежние копии и повторите эффект.");
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Document,Shape,Style};
    fn settings(mode:EffectMode)->EffectOptions{
        EffectOptions{mode,offset_x:2.,offset_y:3.,depth:4,
            width_mm:1.5,opacity:0.7,
            primary:"#242424".into(),secondary:"#F2D47D".into()}
    }
    #[test]
    fn nine_recipes_have_correct_layer_counts_and_valid_opacity(){
        for mode in EffectMode::ALL{
            let config=settings(mode);
            assert_eq!(config.recipe().len(),config.layer_count());
            assert!(config.recipe().iter().all(|l|
                l.opacity>=0.0&&l.opacity<=1.0&&l.stroke_mm>=0.0));
        }
    }
    #[test]
    fn all_effects_keep_original_geometry_and_are_undoable(){
        for mode in EffectMode::ALL{
            let mut e=Editor::default();e.doc=Document::default();
            let id=e.doc.add(Shape::Rectangle{x:30.,y:40.,w:20.,h:10.},
                Style::default());
            e.selected=Some(id);
            let original=e.doc.clone();
            e.effect_depth=4;
            e.create_vector_effect(mode);
            assert_eq!(e.doc.objects.len(),1+settings(mode).layer_count());
            assert_eq!(e.doc.objects.last().unwrap().id,id);
            assert_eq!(e.doc.get_mut(id).unwrap().shape,original.objects[0].shape);
            e.doc.validate().unwrap();
            e.undo();assert_eq!(e.doc,original);
        }
    }
    #[test]
    fn generated_effects_can_be_removed_without_touching_source(){
        let mut doc=Document::default();
        let id=doc.add(Shape::Rectangle{x:1.,y:2.,w:9.,h:9.},Style::default());
        let source=doc.clone();
        generate_effect(&mut doc,&[id],&settings(EffectMode::LongShadow)).unwrap();
        assert_eq!(clean_effect_copies(&mut doc,&[id]).unwrap(),4);
        assert_eq!(doc.objects,source.objects);
        assert_eq!(doc.appearances,source.appearances);
        assert_eq!(doc.clip_links,source.clip_links);
        assert!(doc.next_id>source.next_id); // IDs are never reused after cleanup.
        doc.validate().unwrap();
    }
    #[test]
    fn reject_unsupported_raster_and_locked_object_without_mutating(){
        let mut doc=Document::default();
        let id=doc.add(Shape::Rectangle{x:0.,y:0.,w:5.,h:5.},Style::default());
        doc.layers[0].locked=true;
        let original=doc.clone();
        assert!(generate_effect(&mut doc,&[id],&settings(EffectMode::HardShadow)).is_err());
        assert_eq!(doc,original);
    }
    #[test]
    fn effect_copy_appearance_is_not_a_gradient_and_has_opacity(){
        let mut doc=Document::default();
        let id=doc.add(Shape::Text{x:1.,y:2.,value:"ABC".into(),
            size_mm:8.},Style::default());
        let ids=generate_effect(&mut doc,&[id],&settings(EffectMode::HardShadow)).unwrap();
        let app=doc.appearances.get(&ids[0]).unwrap();
        assert_eq!(app.opacity,0.7);
        assert!(app.gradient.is_none());
        assert!(doc.to_svg().contains("opacity=\"0.7\""));
    }
}
