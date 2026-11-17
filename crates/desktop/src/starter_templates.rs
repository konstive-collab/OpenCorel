//! Editable, offline-first starter templates for real print and marketing layouts.
//! Templates contain native vector and text objects only; no baked-in images.
use super::Editor;
use opencorel_core::{Document,Shape,Style};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) enum StarterTemplate{
    BusinessCard,
    FlyerA5,
    SocialSquare,
    LabelSheetA4,
}
impl StarterTemplate{
    pub(super) const ALL:[Self;4]=[
        Self::BusinessCard,Self::FlyerA5,Self::SocialSquare,Self::LabelSheetA4,
    ];
    pub(super) fn title(self)->&'static str{
        match self{
            Self::BusinessCard=>"Визитка 90 × 50 мм",
            Self::FlyerA5=>"Листовка A5",
            Self::SocialSquare=>"Квадратный постер 210 × 210 мм",
            Self::LabelSheetA4=>"Этикетки A4 — 3 × 8",
        }
    }
    pub(super) fn file_stem(self)->&'static str{
        match self{
            Self::BusinessCard=>"Визитка",
            Self::FlyerA5=>"Листовка A5",
            Self::SocialSquare=>"Постер",
            Self::LabelSheetA4=>"Этикетки A4",
        }
    }
}
fn fill(color:&str)->Style{
    Style{fill:color.into(),stroke:"none".into(),stroke_width_mm:0.0}
}
fn rect(doc:&mut Document,layer:u64,x:f32,y:f32,w:f32,h:f32,color:&str){
    doc.add_to_layer(Shape::Rectangle{x,y,w,h},fill(color),layer);
}
fn text(doc:&mut Document,layer:u64,x:f32,y:f32,size:f32,
    value:&str,color:&str,bold:bool){
    let id=doc.add_to_layer(Shape::Text{
        x,y,value:value.into(),size_mm:size,
    },fill(color),layer);
    if let Some(obj)=doc.get_mut(id){
        obj.text_format.weight=if bold{700}else{400};
    }
}
pub(super) fn build_template(kind:StarterTemplate)->Document{
    let mut doc=Document::default();
    doc.name=kind.file_stem().into();
    let (w,h)=match kind{
        StarterTemplate::BusinessCard=>(90.,50.),
        StarterTemplate::FlyerA5=>(148.,210.),
        StarterTemplate::SocialSquare=>(210.,210.),
        StarterTemplate::LabelSheetA4=>(210.,297.),
    };
    doc.page.width_mm=w;
    doc.page.height_mm=h;
    doc.layers[0].name="Фон и оформление".into();
    let content=doc.add_layer("Редактируемый текст".into());
    match kind{
        StarterTemplate::BusinessCard=>{
            rect(&mut doc,1,0.,0.,90.,50.,"#13283D");
            rect(&mut doc,1,0.,0.,4.,50.,"#1AC9AF");
            rect(&mut doc,1,9.,9.,30.,1.,"#1AC9AF");
            rect(&mut doc,1,68.,0.,22.,50.,"#214760");
            text(&mut doc,content,9.,13.,7.,"ИМЯ ФАМИЛИЯ","#FFFFFF",true);
            text(&mut doc,content,9.,23.,3.5,"Должность / компания","#D8E5ED",false);
            text(&mut doc,content,9.,34.,3.,"+7 900 000-00-00","#FFFFFF",false);
            text(&mut doc,content,9.,40.,2.7,"hello@example.com","#FFFFFF",false);
        }
        StarterTemplate::FlyerA5=>{
            rect(&mut doc,1,0.,0.,148.,210.,"#F6F5F0");
            rect(&mut doc,1,0.,0.,148.,65.,"#122C42");
            rect(&mut doc,1,12.,11.,43.,1.5,"#1AC9AF");
            rect(&mut doc,1,12.,122.,124.,1.,"#D5DBDE");
            rect(&mut doc,1,12.,174.,124.,21.,"#1AC9AF");
            text(&mut doc,content,12.,22.,16.,"ВАШЕ СОБЫТИЕ","#FFFFFF",true);
            text(&mut doc,content,12.,45.,5.,"ДАТА / ВРЕМЯ / МЕСТО","#D6E5EC",false);
            text(&mut doc,content,12.,84.,9.,"Главная идея","#13283D",true);
            text(&mut doc,content,12.,105.,5.,"Добавьте свой текст и фотографии","#324B5A",false);
            text(&mut doc,content,12.,140.,5.,"Адрес, расписание, контакты","#324B5A",false);
            text(&mut doc,content,19.,179.,7.,"ПОДРОБНОСТИ / САЙТ","#092F35",true);
        }
        StarterTemplate::SocialSquare=>{
            rect(&mut doc,1,0.,0.,210.,210.,"#142136");
            rect(&mut doc,1,12.,12.,186.,186.,"#213D59");
            rect(&mut doc,1,22.,22.,48.,2.,"#35D5B6");
            doc.add_to_layer(Shape::Ellipse{x:120.,y:44.,w:66.,h:66.},
                fill("#35D5B6"),1);
            doc.add_to_layer(Shape::Ellipse{x:146.,y:66.,w:38.,h:38.},
                fill("#F2B656"),1);
            text(&mut doc,content,22.,59.,21.,"ВАША ИДЕЯ","#FFFFFF",true);
            text(&mut doc,content,22.,97.,7.,"Дизайн, который заметят","#E1EAF0",false);
            rect(&mut doc,1,22.,146.,92.,24.,"#35D5B6");
            text(&mut doc,content,29.,150.,9.,"УЗНАТЬ БОЛЬШЕ","#142136",true);
            text(&mut doc,content,22.,183.,5.,"ваш сайт / @ваш_аккаунт","#FFFFFF",false);
        }
        StarterTemplate::LabelSheetA4=>{
            rect(&mut doc,1,0.,0.,210.,297.,"#FFFFFF");
            // Standard demonstration layout: 3 columns × 8 rows, 5 mm
            // left/right gutters, 8 mm top/bottom margins.
            let (left,top,cell_w,cell_h,gap_x,gap_y)=
                (6.,8.,62.,32.,6.,4.);
            for row in 0..8{
                for col in 0..3{
                    let x=left+col as f32*(cell_w+gap_x);
                    let y=top+row as f32*(cell_h+gap_y);
                    let id=doc.add_to_layer(Shape::Rectangle{
                        x,y,w:cell_w,h:cell_h
                    },Style{fill:"#FFFFFF".into(),stroke:"#A8B6BD".into(),
                        stroke_width_mm:0.18},1);
                    if let Some(obj)=doc.get_mut(id){
                        obj.name=format!("Этикетка {}-{}",row+1,col+1);
                    }
                    text(&mut doc,content,x+4.,y+6.,6.,"НАЗВАНИЕ","#142B3C",true);
                    text(&mut doc,content,x+4.,y+17.,3.,"АРТИКУЛ / ЦЕНА","#497182",false);
                }
            }
        }
    }
    doc
}
impl Editor{
    pub(super) fn create_starter_template(&mut self,kind:StarterTemplate){
        let document=build_template(kind);
        if let Err(e)=document.validate(){
            self.status=format!("Шаблон не создан: {e}");
            return;
        }
        self.new_document();
        self.doc=document;
        self.active_layer=1;
        self.file=format!("{}.kdr",kind.file_stem());
        self.show_welcome=false;
        self.status=format!("Шаблон «{}» открыт: все элементы редактируемые. Ctrl+Z — отмена",
            kind.title());
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn every_template_is_valid_and_uses_native_editable_objects(){
        for kind in StarterTemplate::ALL{
            let doc=build_template(kind);
            doc.validate().unwrap();
            assert!(doc.objects.len()>=7);
            assert!(doc.objects.iter().all(|o|matches!(o.shape,
                Shape::Text{..}|Shape::Rectangle{..}|Shape::Ellipse{..})));
            assert!(doc.layers.len()>=2);
            assert!(!doc.objects.iter().any(|o|o.group_id.is_some()));
        }
    }
    #[test]
    fn label_template_makes_twenty_four_distinct_cells(){
        let doc=build_template(StarterTemplate::LabelSheetA4);
        assert_eq!(doc.page.width_mm,210.);
        assert_eq!(doc.page.height_mm,297.);
        let cells=doc.objects.iter().filter(|o|o.name.starts_with("Этикетка ")).count();
        assert_eq!(cells,24);
        assert_eq!(doc.objects.iter().filter(|o|matches!(o.shape,Shape::Text{..})).count(),48);
    }
    #[test]
    fn creating_template_is_one_undoable_document_change(){
        let mut e=Editor::default();
        let original=e.doc.clone();
        e.create_starter_template(StarterTemplate::BusinessCard);
        assert_eq!(e.doc.page.width_mm,90.);
        e.doc.validate().unwrap();
        e.undo();
        assert_eq!(e.doc,original);
    }
}
