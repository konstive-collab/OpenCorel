//! Object Explorer indexes native objects by name, text, type, layer and
//! RGB color across active/inactive pages, without cloning raster payloads.
use crate::{Document,Layer,Object,Shape};
use crate::appearance::Appearance;
use std::collections::BTreeMap;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ObjectKind{Any,Text,Vector,Raster,Embedded,Grouped}
impl ObjectKind{
    pub const ALL:[Self;6]=[
        Self::Any,Self::Text,Self::Vector,Self::Raster,
        Self::Embedded,Self::Grouped
    ];
    pub fn label(self)->&'static str{match self{
        Self::Any=>"Все",Self::Text=>"Текст",Self::Vector=>"Контуры",
        Self::Raster=>"Растры",Self::Embedded=>"SVG / PDF",
        Self::Grouped=>"Группы"
    }}
}
#[derive(Clone,Debug)]
pub struct ObjectQuery{
    pub phrase:String,
    pub layer_phrase:String,
    pub rgb_color:Option<String>,
    pub kind:ObjectKind,
    pub all_pages:bool,
    pub include_hidden:bool,
}
impl Default for ObjectQuery{
    fn default()->Self{
        Self{phrase:String::new(),layer_phrase:String::new(),
            rgb_color:None,kind:ObjectKind::Any,
            all_pages:true,include_hidden:false}
    }
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct ObjectHit{
    pub page_id:u64,
    pub object_id:u64,
    pub name:String,
    pub layer_name:String,
    pub kind:&'static str,
    pub grouped:bool,
}
fn kind_of(shape:&Shape)->&'static str{
    match shape{
        Shape::Text{..}=>"Текст",
        Shape::Rectangle{..}=>"Прямоугольник",
        Shape::Ellipse{..}=>"Эллипс",
        Shape::Polyline{..}=>"Ломаная",
        Shape::Bezier{..}=>"Безье",
        Shape::CompoundPath{..}=>"Составной контур",
        Shape::Raster{..}=>"Растр",
        Shape::EmbeddedSvg{..}=>"Встроенный вектор"
    }
}
fn eligible(shape:&Shape,grouped:bool,kind:ObjectKind)->bool{
    match kind{
        ObjectKind::Any=>true,
        ObjectKind::Text=>matches!(shape,Shape::Text{..}),
        ObjectKind::Vector=>matches!(shape,Shape::Rectangle{..}|Shape::Ellipse{..}|
            Shape::Polyline{..}|Shape::Bezier{..}|Shape::CompoundPath{..}),
        ObjectKind::Raster=>matches!(shape,Shape::Raster{..}),
        ObjectKind::Embedded=>matches!(shape,Shape::EmbeddedSvg{..}),
        ObjectKind::Grouped=>grouped,
    }
}
fn scan_page(page_id:u64,objects:&[Object],layers:&[Layer],
    appearances:&BTreeMap<u64,Appearance>,
    q:&ObjectQuery,needle:&str,layer_needle:&str,
    out:&mut Vec<ObjectHit>){
    for obj in objects{
        if out.len()>=2000{return;}
        let Some(layer)=layers.iter().find(|l|l.id==obj.layer_id) else{continue;};
        if !q.include_hidden&&!layer.visible{continue;}
        if !eligible(&obj.shape,obj.group_id.is_some(),q.kind){continue;}
        if !layer.name.to_lowercase().contains(layer_needle){continue;}
        let title=if obj.name.is_empty(){kind_of(&obj.shape)}else{&obj.name};
        let value=if let Shape::Text{value,..}=&obj.shape{value.as_str()}else{""};
        if !title.to_lowercase().contains(needle)
            &&!value.to_lowercase().contains(needle){continue;}
        if let Some(color)=&q.rgb_color{
            let style_color=obj.style.fill.eq_ignore_ascii_case(color)||
                obj.style.stroke.eq_ignore_ascii_case(color);
            let gradient_color=appearances.get(&obj.id)
                .and_then(|a|a.gradient.as_ref())
                .is_some_and(|g|g.stops.iter().any(|s|
                    s.color.eq_ignore_ascii_case(color)));
            if !style_color&&!gradient_color{continue;}
        }
        out.push(ObjectHit{
            page_id,object_id:obj.id,
            name:title.chars().take(96).collect(),
            layer_name:layer.name.clone(),kind:kind_of(&obj.shape),
            grouped:obj.group_id.is_some(),
        });
    }
}
impl Document{
    pub fn find_objects(&self,q:&ObjectQuery)->Result<Vec<ObjectHit>,String>{
        if q.phrase.len()>256||q.layer_phrase.len()>256||
            !crate::valid_xml_text(&q.phrase)||
            !crate::valid_xml_text(&q.layer_phrase){
            return Err("Search or layer filter exceeds 256 valid UTF-8 bytes".into());
        }
        if q.rgb_color.as_ref().is_some_and(|color|!crate::valid_color(color)){
            return Err("Color filter must be #RRGGBB".into());
        }
        let needle=q.phrase.to_lowercase();
        let layer_needle=q.layer_phrase.to_lowercase();
        let mut found=Vec::new();
        if q.all_pages{
            for tab in self.pages(){
                if tab.id==self.active_page_id{
                    scan_page(tab.id,&self.objects,&self.layers,&self.appearances,
                        q,&needle,&layer_needle,&mut found);
                }else if let Some(p)=self.inactive_pages.get(&tab.id){
                    scan_page(tab.id,&p.objects,&p.layers,&p.appearances,
                        q,&needle,&layer_needle,&mut found);
                }
                if found.len()>=2000{break;}
            }
        }else{
            scan_page(self.active_page_id,&self.objects,&self.layers,&self.appearances,
                q,&needle,&layer_needle,&mut found);
        }
        Ok(found)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Page,Shape,Style};
    #[test] fn page_order_unicode_text_and_native_type_filters(){
        let mut d=Document::default();
        let first=d.add(Shape::Text{x:1.,y:2.,value:"Привет реклама".into(),size_mm:6.},
            Style::default());
        let page=d.add_page(Page{width_mm:200.,height_mm:300.}).unwrap();
        d.add(Shape::Text{x:1.,y:2.,value:"РЕКЛАМА вторая".into(),size_mm:6.},
            Style::default());
        let results=d.find_objects(&ObjectQuery{
            phrase:"реклама".into(),kind:ObjectKind::Text,..Default::default()
        }).unwrap();
        assert_eq!(results.len(),2);
        assert_eq!(results[0].object_id,first);
        assert_eq!(results[1].page_id,page);
        assert_eq!(d.active_page_id,page);
    }
    #[test] fn object_color_filter_and_hidden_layers(){
        let mut d=Document::default();
        let one=d.add(Shape::Rectangle{x:1.,y:1.,w:10.,h:10.},Style{
            fill:"#AA00BB".into(),stroke:"none".into(),stroke_width_mm:0.0});
        let query=ObjectQuery{rgb_color:Some("#aa00bb".into()),
            kind:ObjectKind::Vector,..Default::default()};
        assert_eq!(d.find_objects(&query).unwrap()[0].object_id,one);
        d.layers[0].visible=false;
        assert!(d.find_objects(&query).unwrap().is_empty());
        let visible=ObjectQuery{include_hidden:true,..query};
        assert_eq!(d.find_objects(&visible).unwrap().len(),1);
    }
}
