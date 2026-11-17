//! Document-wide color replacement and HSL color harmonies.
//! Safe across inactive pages, clipping, gradients and legacy flat styles.
//! All bulk actions are preflighted, then recorded as exactly one undo step.
use super::Editor;
use eframe::egui::{self,RichText};
use opencorel_core::{valid_color,Document};
use std::collections::HashSet;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) enum HarmonyMode{
    Complement,Analogous,Triad,Square,SplitComplement,Monochrome,
    Warm,Cool,Pastel,Rich,Lighten,Darken,Grayscale,Invert,Hue45,Hue90,
}
impl HarmonyMode{
    pub const ALL:[Self;16]=[
        Self::Complement,Self::Analogous,Self::Triad,Self::Square,
        Self::SplitComplement,Self::Monochrome,Self::Warm,Self::Cool,
        Self::Pastel,Self::Rich,Self::Lighten,Self::Darken,
        Self::Grayscale,Self::Invert,Self::Hue45,Self::Hue90,
    ];
    pub fn label(self)->&'static str{
        match self{
            Self::Complement=>"Комплементарные цвета",
            Self::Analogous=>"Аналоговая гармония",
            Self::Triad=>"Триада",
            Self::Square=>"Квадрат из четырёх цветов",
            Self::SplitComplement=>"Раздельно-комплементарная",
            Self::Monochrome=>"Монохромные оттенки",
            Self::Warm=>"Тёплая гамма",
            Self::Cool=>"Холодная гамма",
            Self::Pastel=>"Пастельная гамма",
            Self::Rich=>"Насыщенные цвета",
            Self::Lighten=>"Осветлить цвет",
            Self::Darken=>"Затемнить цвет",
            Self::Grayscale=>"Оттенки серого",
            Self::Invert=>"Инверсия RGB",
            Self::Hue45=>"Цветовой сдвиг +45°",
            Self::Hue90=>"Цветовой сдвиг +90°",
        }
    }
}
#[derive(Clone,Copy,Debug)]
struct Hsl{h:f32,s:f32,l:f32}
fn parse_rgb(text:&str)->Result<[u8;3],String>{
    if !valid_color(text){return Err("Ожидается шестизначный цвет #RRGGBB".into());}
    let n=u32::from_str_radix(&text[1..],16)
        .map_err(|e|e.to_string())?;
    Ok([((n>>16)&255) as u8,((n>>8)&255) as u8,(n&255) as u8])
}
fn rgb_to_hsl(color:[u8;3])->Hsl{
    let r=color[0] as f32/255.0;
    let g=color[1] as f32/255.0;
    let b=color[2] as f32/255.0;
    let high=r.max(g.max(b));
    let low=r.min(g.min(b));
    let delta=high-low;
    let l=(high+low)/2.0;
    if delta<1e-7{return Hsl{h:0.,s:0.,l};}
    let s=delta/(1.0-(2.0*l-1.0).abs()).max(1e-7);
    let hue=if high==r{((g-b)/delta).rem_euclid(6.0)}
        else if high==g{(b-r)/delta+2.0}
        else{(r-g)/delta+4.0};
    Hsl{h:(60.0*hue).rem_euclid(360.0),s:s.clamp(0.,1.),l}
}
fn hsl_to_rgb(hsl:Hsl)->[u8;3]{
    let h=hsl.h.rem_euclid(360.0);
    let s=hsl.s.clamp(0.,1.);
    let l=hsl.l.clamp(0.,1.);
    let c=(1.0-(2.0*l-1.0).abs())*s;
    let x=c*(1.0-((h/60.0).rem_euclid(2.0)-1.0).abs());
    let m=l-c/2.0;
    let (r,g,b)=match h{
        h if h<60.0=>(c,x,0.0),
        h if h<120.0=>(x,c,0.0),
        h if h<180.0=>(0.0,c,x),
        h if h<240.0=>(0.0,x,c),
        h if h<300.0=>(x,0.0,c),
        _=>(c,0.0,x),
    };
    let to_byte=|v:f32|((v+m).clamp(0.,1.)*255.0).round() as u8;
    [to_byte(r),to_byte(g),to_byte(b)]
}
fn hex(color:[u8;3])->String{
    format!("#{:02X}{:02X}{:02X}",color[0],color[1],color[2])
}
fn color_harmony(base:&str,mode:HarmonyMode)->Result<Vec<String>,String>{
    let rgb=parse_rgb(base)?;
    let base=rgb_to_hsl(rgb);
    let offsets:&[f32]=match mode{
        HarmonyMode::Complement=>&[0.,180.],
        HarmonyMode::Analogous=>&[-40.,-20.,0.,20.,40.],
        HarmonyMode::Triad=>&[0.,120.,240.],
        HarmonyMode::Square=>&[0.,90.,180.,270.],
        HarmonyMode::SplitComplement=>&[0.,150.,210.],
        HarmonyMode::Monochrome=>&[0.,0.,0.,0.,0.],
        HarmonyMode::Warm=>&[-30.,-15.,0.,15.,30.],
        HarmonyMode::Cool=>&[-30.,-15.,0.,15.,30.],
        HarmonyMode::Pastel=>&[-30.,0.,30.,60.],
        HarmonyMode::Rich=>&[-30.,0.,30.,60.],
        HarmonyMode::Lighten=>&[0.,0.,0.],
        HarmonyMode::Darken=>&[0.,0.,0.],
        HarmonyMode::Grayscale=>&[0.,0.,0.],
        HarmonyMode::Invert=>&[0.],
        HarmonyMode::Hue45=>&[45.],
        HarmonyMode::Hue90=>&[90.],
    };
    let mut palette=Vec::with_capacity(offsets.len());
    for (index,delta) in offsets.iter().enumerate(){
        let n=index as f32;
        let modified=match mode{
            HarmonyMode::Monochrome=>Hsl{l:(0.22+n*0.14).min(0.86),..base},
            HarmonyMode::Warm=>Hsl{h:30.0+delta,
                s:base.s.max(0.4),l:base.l,..base},
            HarmonyMode::Cool=>Hsl{h:208.0+delta,
                s:base.s.max(0.4),l:base.l,..base},
            HarmonyMode::Pastel=>Hsl{h:base.h+delta,
                s:base.s.min(0.43),l:base.l.max(0.72),..base},
            HarmonyMode::Rich=>Hsl{h:base.h+delta,
                s:base.s.max(0.72),l:base.l.clamp(0.28,0.57),..base},
            HarmonyMode::Lighten=>Hsl{l:(base.l+0.14+n*0.12).min(0.95),..base},
            HarmonyMode::Darken=>Hsl{l:(base.l-0.12-n*0.1).max(0.08),..base},
            HarmonyMode::Grayscale=>Hsl{s:0.,l:(base.l-0.2+n*0.2).clamp(0.,1.),..base},
            HarmonyMode::Invert=>Hsl{h:base.h+180.,
                l:1.0-base.l,..base},
            _=>Hsl{h:base.h+delta,..base},
        };
        let final_color=if mode==HarmonyMode::Invert{
            [255-rgb[0],255-rgb[1],255-rgb[2]]
        }else{hsl_to_rgb(modified)};
        palette.push(hex(final_color));
    }
    Ok(palette)
}
/// Transform the fill (or stroke) of each chosen object in canvas order, cycling
/// through a generated harmonic palette. Invisible/locked selection rejected.
fn apply_harmony(doc:&mut Document,ids:&[u64],mode:HarmonyMode,
    base:&str,stroke:bool)->Result<usize,String>{
    let palette=color_harmony(base,mode)?;
    let selected:HashSet<u64>=ids.iter().copied().collect();
    if selected.is_empty(){return Err("Сначала выберите объекты".into());}
    if doc.objects.iter().filter(|o|selected.contains(&o.id)).count()!=selected.len(){
        return Err("Объект выделения отсутствует".into());
    }
    if doc.objects.iter().filter(|o|selected.contains(&o.id)).any(|o|
        !doc.layer(o.layer_id).is_some_and(|l|l.visible&&!l.locked)){
        return Err("В выделении есть заблокированные или скрытые слои".into());
    }
    let mut changed=0usize;
    for obj in doc.objects.iter_mut().filter(|o|selected.contains(&o.id)){
        let color=palette[changed%palette.len()].clone();
        if stroke{
            obj.style.stroke=color;
            if obj.style.stroke_width_mm==0.0{obj.style.stroke_width_mm=0.5;}
        }else{
            obj.style.fill=color;
            if let Some(app)=doc.appearances.get_mut(&obj.id){
                // Do not leave an existing gradient obscuring the new fill.
                app.gradient=None;
            }
        }
        changed+=1;
    }
    doc.validate()?;
    Ok(changed)
}
#[derive(Clone,Copy,Debug)]
struct ReplaceFlags{fill:bool,stroke:bool,gradients:bool}
fn eq_color(value:&str,target:&str)->bool{value.eq_ignore_ascii_case(target)}
fn count_matches(doc:&Document,needle:&str,flags:ReplaceFlags)->usize{
    let mut total=0usize;
    for o in &doc.objects{
        if flags.fill&&eq_color(&o.style.fill,needle){total+=1;}
        if flags.stroke&&eq_color(&o.style.stroke,needle){total+=1;}
    }
    if flags.gradients{
        for appearance in doc.appearances.values(){
            if let Some(g)=&appearance.gradient{
                total+=g.stops.iter().filter(|s|eq_color(&s.color,needle)).count();
            }
        }
    }
    total
}
fn replace_active_page(doc:&mut Document,find:&str,replacement:&str,
    flags:ReplaceFlags)->Result<usize,String>{
    // Before any mutation, ensure every affected object is editable.
    for o in &doc.objects{
        let regular=(flags.fill&&eq_color(&o.style.fill,find))||
            (flags.stroke&&eq_color(&o.style.stroke,find));
        let gradient=flags.gradients&&doc.appearances.get(&o.id)
            .and_then(|a|a.gradient.as_ref()).is_some_and(|g|
                g.stops.iter().any(|s|eq_color(&s.color,find)));
        if (regular||gradient)&&!doc.layer(o.layer_id)
            .is_some_and(|l|l.visible&&!l.locked){
            return Err(format!("Объект #{} на скрытом или заблокированном слое",o.id));
        }
    }
    let mut changes=0usize;
    for o in &mut doc.objects{
        if flags.fill&&eq_color(&o.style.fill,find){
            o.style.fill=replacement.into();changes+=1;
        }
        if flags.stroke&&eq_color(&o.style.stroke,find){
            o.style.stroke=replacement.into();changes+=1;
        }
    }
    if flags.gradients{
        for appearance in doc.appearances.values_mut(){
            if let Some(g)=&mut appearance.gradient{
                for stop in &mut g.stops{
                    if eq_color(&stop.color,find){
                        stop.color=replacement.into();changes+=1;
                    }
                }
            }
        }
    }
    Ok(changes)
}
fn replace_project_colors(doc:&mut Document,find:&str,replacement:&str,
    flags:ReplaceFlags)->Result<usize,String>{
    parse_rgb(find)?;parse_rgb(replacement)?;
    if !flags.fill&&!flags.stroke&&!flags.gradients{
        return Err("Выберите заливки, контуры или градиенты".into());
    }
    if eq_color(find,replacement){return Ok(0);}
    doc.validate()?;
    let original_index=doc.active_page_index;
    let mut changes=0usize;
    for index in 0..doc.page_count(){
        doc.switch_page(index)?;
        changes+=replace_active_page(doc,find,replacement,flags)?;
    }
    doc.switch_page(original_index)?;
    doc.validate()?;
    Ok(changes)
}
impl Editor{
    fn commit_color_change(&mut self,preview:Document,description:String){
        if preview==self.doc{self.status="Цвета не изменились".into();return;}
        self.checkpoint();self.doc=preview;
        self.fill_cache.entries.clear();self.vector_cache.clear();
        self.compound_cache.clear();self.bezier_preview.clear();
        self.raster_cache.clear();self.text_preview.clear();
        self.status=description;
    }
    pub(super) fn sample_current_fill(&mut self){
        let Some(id)=self.selected else{return;};
        if let Some(color)=self.doc.objects.iter().find(|o|o.id==id)
            .map(|o|&o.style.fill).filter(|c|valid_color(c)){
            self.palette_base_color=color.clone();
            self.status=format!("Образец цвета: {}",self.palette_base_color);
        }else{self.status="У первого выделенного объекта нет сплошной заливки".into();}
    }
    pub(super) fn apply_color_harmony(&mut self,mode:HarmonyMode){
        let mut preview=self.doc.clone();
        match apply_harmony(&mut preview,&self.selection_ids(),mode,
            &self.palette_base_color,self.palette_use_stroke){
            Ok(count)=>self.commit_color_change(preview,
                format!("{}: обработано {} объектов. Ctrl+Z — отмена.",
                    mode.label(),count)),
            Err(e)=>self.status=format!("Цветовая схема: {e}"),
        }
    }
    pub(super) fn replace_project_color(&mut self){
        let flags=ReplaceFlags{
            fill:self.palette_replace_fill,stroke:self.palette_replace_stroke,
            gradients:self.palette_replace_gradients,
        };
        self.palette_match_count=None;
        let mut preview=self.doc.clone();
        match replace_project_colors(&mut preview,
            &self.palette_find_color,&self.palette_replacement_color,flags){
            Ok(count)=>self.commit_color_change(preview,
                format!("Замена по документу: {} вхождений на {} страницах. Ctrl+Z — отмена.",
                    count,self.doc.page_count())),
            Err(e)=>self.status=format!("Замена цвета отклонена: {e}"),
        }
    }
    pub(super) fn color_studio_panel(&mut self,ui:&mut egui::Ui){
        ui.heading("Цветовые схемы");
        ui.horizontal(|ui|{
            ui.label("Базовый #RRGGBB");
            ui.text_edit_singleline(&mut self.palette_base_color);
        });
        if ui.small_button("Взять цвет заливки выделенного").clicked(){
            self.sample_current_fill();
        }
        ui.checkbox(&mut self.palette_use_stroke,
            "Применять к обводке вместо заливки");
        let palette=HarmonyMode::ALL;
        for chunk in palette.chunks(4){
            ui.horizontal_wrapped(|ui|{
                for &mode in chunk{
                    if ui.button(mode.label()).clicked(){
                        self.apply_color_harmony(mode);
                    }
                }
            });
        }
        ui.separator();
        ui.heading("Глобальная замена фирменных цветов");
        ui.label("Меняет совпадения на всех страницах (включая неактивные)");
        ui.horizontal(|ui|{
            ui.label("Найти");
            ui.text_edit_singleline(&mut self.palette_find_color);
        });
        ui.horizontal(|ui|{
            ui.label("Заменить");
            ui.text_edit_singleline(&mut self.palette_replacement_color);
        });
        ui.horizontal_wrapped(|ui|{
            ui.checkbox(&mut self.palette_replace_fill,"Заливки");
            ui.checkbox(&mut self.palette_replace_stroke,"Контуры");
            ui.checkbox(&mut self.palette_replace_gradients,"Градиенты");
        });
        if ui.small_button("Подсчитать совпадения на всех страницах").clicked(){
            if valid_color(&self.palette_find_color){
                let flags=ReplaceFlags{fill:self.palette_replace_fill,
                    stroke:self.palette_replace_stroke,
                    gradients:self.palette_replace_gradients};
                let mut count=0usize;
                for i in 0..self.doc.page_count(){
                    if let Ok(page)=self.doc.page_document(i){
                        count+=count_matches(&page,&self.palette_find_color,flags);
                    }
                }
                self.palette_match_count=Some(count);
            }else{
                self.status="Укажите цвет поиска в формате #RRGGBB".into();
            }
        }
        if let Some(count)=self.palette_match_count{
            ui.label(format!("Совпадений при последнем подсчёте: {count}"));
        }
        if ui.button("Заменить цвет во всём проекте").clicked(){
            self.replace_project_color();
        }
        ui.small("Скрытые и заблокированные слои не изменяются: вся операция отклоняется, если найдены совпадения на таких слоях.");
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Shape,Style,appearance::{
        Appearance,Gradient,GradientKind,GradientStop}};
    #[test]
    fn hsl_roundtrip_is_close_to_original_rgb(){
        for color in ["#FF0000","#00FF00","#0000FF","#FFFFFF",
            "#000000","#ABCDEF","#7339BE","#999999"]{
            let bytes=parse_rgb(color).unwrap();
            let back=hsl_to_rgb(rgb_to_hsl(bytes));
            assert!(back.iter().zip(bytes).all(|(a,b)|a.abs_diff(b)<=1));
        }
    }
    #[test]
    fn all_sixteen_modes_produce_valid_colors(){
        for mode in HarmonyMode::ALL{
            let palette=color_harmony("#AC36DF",mode).unwrap();
            assert!(!palette.is_empty());
            assert!(palette.iter().all(|c|valid_color(c)));
        }
        assert_eq!(color_harmony("#AABBCC",HarmonyMode::Invert).unwrap(),
            vec!["#554433"]);
        assert_eq!(color_harmony("#FF0000",HarmonyMode::Complement).unwrap(),
            ["#FF0000","#00FFFF"]);
    }
    #[test]
    fn selected_color_harmony_is_undoable_and_keeps_unselected(){
        let mut e=Editor::default();
        e.doc=Document::default();
        let id=e.doc.add(Shape::Rectangle{x:2.,y:2.,w:7.,h:7.},
            Style::default());
        let ignored=e.doc.add(Shape::Rectangle{x:17.,y:2.,w:7.,h:7.},
            Style::default());
        e.selected=Some(id);
        let original=e.doc.clone();
        e.apply_color_harmony(HarmonyMode::Triad);
        assert_eq!(e.doc.get_mut(ignored).unwrap().style.fill,
            original.objects[1].style.fill);
        e.undo();assert_eq!(e.doc,original);
    }
    #[test]
    fn project_color_replace_changes_all_pages_and_gradients(){
        let mut d=Document::default();
        let id=d.add(Shape::Rectangle{x:0.,y:0.,w:8.,h:8.},
            Style{fill:"#ABABAB".into(),stroke:"#ABABAB".into(),
                stroke_width_mm:1.});
        d.appearances.insert(id,Appearance{
            gradient:Some(Gradient{kind:GradientKind::Linear,
                angle_degrees:0.,stops:vec![
                    GradientStop{offset:0.,color:"#ABABAB".into()},
                    GradientStop{offset:1.,color:"#FFFFFF".into()},
                ]}),..Appearance::default()
        });
        d.append_page(true).unwrap();
        let previous=d.active_page_index;
        let flags=ReplaceFlags{fill:true,stroke:true,gradients:true};
        assert_eq!(replace_project_colors(&mut d,"#ababab","#112233",flags).unwrap(),6);
        assert_eq!(d.active_page_index,previous);
        for i in 0..d.page_count(){
            let page=d.page_document(i).unwrap();
            assert_eq!(page.objects[0].style.fill,"#112233");
            assert_eq!(page.objects[0].style.stroke,"#112233");
            assert_eq!(page.appearances.get(&id).unwrap().gradient.as_ref()
                .unwrap().stops[0].color,"#112233");
        }
    }
    #[test]
    fn locked_matching_layer_rejects_all_pages_atomically_via_preview(){
        let mut d=Document::default();
        d.add(Shape::Rectangle{x:0.,y:0.,w:5.,h:5.},
            Style{fill:"#FFFFFF".into(),stroke:"none".into(),stroke_width_mm:0.});
        d.append_page(true).unwrap();
        d.layers[0].locked=true;
        let original=d.clone();
        let mut candidate=d.clone();
        assert!(replace_project_colors(&mut candidate,"#FFFFFF","#000000",
            ReplaceFlags{fill:true,stroke:false,gradients:true}).is_err());
        assert_eq!(d,original);
    }
}
