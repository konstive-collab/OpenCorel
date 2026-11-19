//! Object styling clipboard and true sampled appearance transfer.
//! This is editor state (not global clipboard), so original documents never
//! acquire hidden external dependencies.
use crate::{Document,Style,TextFormat,Shape};
use crate::appearance::Appearance;
use std::collections::HashSet;
#[derive(Clone,Debug,PartialEq)]
pub struct StyleSample{
    pub style:Style,
    pub appearance:Option<Appearance>,
    pub text:Option<TextFormat>,
}
impl Document{
    pub fn sample_style(&self,id:u64)->Result<StyleSample,String>{
        let obj=self.get(id).ok_or("Source object not found")?;
        Ok(StyleSample{
            style:obj.style.clone(),
            appearance:self.appearances.get(&id).cloned(),
            text:matches!(obj.shape,Shape::Text{..}).then(||obj.text_format.clone()),
        })
    }
    pub fn paste_style(&mut self,sample:&StyleSample,targets:&[u64])
        ->Result<usize,String>{
        let chosen:HashSet<_>=targets.iter().copied().collect();
        if chosen.is_empty(){return Err("No target objects selected".into());}
        if self.objects.iter().filter(|o|chosen.contains(&o.id)).count()!=chosen.len(){
            return Err("Target object not found".into());
        }
        for obj in self.objects.iter().filter(|o|chosen.contains(&o.id)){
            if self.layer(obj.layer_id).is_none_or(|l|!l.visible||l.locked){
                return Err("Cannot recolor hidden or locked objects".into());
            }
        }
        let mut changes=0;
        for obj in &mut self.objects{
            if !chosen.contains(&obj.id){continue;}
            let text_changed=sample.text.as_ref().is_some_and(|format|
                matches!(obj.shape,Shape::Text{..})&&obj.text_format!=*format);
            let new_appearance=self.appearances.get(&obj.id)!=sample.appearance.as_ref();
            if obj.style!=sample.style||new_appearance||text_changed{changes+=1;}
            obj.style=sample.style.clone();
            if let Some(font)=&sample.text{
                if matches!(obj.shape,Shape::Text{..}){obj.text_format=font.clone();}
            }
            match &sample.appearance{
                Some(appearance)=>{self.appearances.insert(obj.id,appearance.clone());}
                None=>{self.appearances.remove(&obj.id);}
            }
        }
        Ok(changes)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Shape,Style};
    #[test] fn sample_and_apply_gradient_and_text_style(){
        let mut d=Document::default();
        let first=d.add(Shape::Text{x:1.,y:1.,value:"a".into(),size_mm:5.},
            Style{fill:"#F00".into(),stroke:"none".into(),stroke_width_mm:0.});
        let next=d.add(Shape::Text{x:10.,y:10.,value:"b".into(),size_mm:4.},Style::default());
        d.get_mut(first).unwrap().text_format.weight=700;
        let sample=d.sample_style(first).unwrap();
        assert_eq!(d.paste_style(&sample,&[next]).unwrap(),1);
        assert_eq!(d.get(next).unwrap().style.fill,"#F00");
        assert_eq!(d.get(next).unwrap().text_format.weight,700);
        d.validate().unwrap();
    }
    #[test] fn locked_targets_are_atomic(){
        let mut d=Document::default();
        let a=d.add(Shape::Rectangle{x:0.,y:0.,w:2.,h:2.},Style::default());
        let layer=d.add_layer("Locked".into());
        let b=d.add_to_layer(Shape::Rectangle{x:10.,y:0.,w:2.,h:2.},Style::default(),layer);
        d.layers.iter_mut().find(|l|l.id==layer).unwrap().locked=true;
        let before=d.clone();
        let style=d.sample_style(a).unwrap();
        assert!(d.paste_style(&style,&[a,b]).is_err());
        assert_eq!(d,before);
    }
}
