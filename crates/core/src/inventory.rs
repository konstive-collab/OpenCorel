//! Whole-publication asset inventory for large KDR layout projects.
//! Counts every active/inactive page without cloning embedded image buffers.
use crate::{Document,Shape,Object};
use std::collections::BTreeMap;
#[derive(Clone,Debug,PartialEq,Eq,Default)]
pub struct Inventory{
    pub pages:usize,
    pub objects:usize,
    pub editable_vectors:usize,
    pub editable_text:usize,
    pub embedded_svg:usize,
    pub raster_images:usize,
    pub estimated_image_bytes:usize,
    pub gradients:usize,
    pub grouped_members:usize,
    pub clipped_targets:usize,
    pub font_families:BTreeMap<String,usize>,
    pub solid_colors:BTreeMap<String,usize>,
}
impl Inventory{
    fn include(&mut self,objects:&[Object],
        appearances:&std::collections::BTreeMap<u64,crate::appearance::Appearance>,
        clips:&[crate::ClipLink]){
        self.objects+=objects.len();
        self.clipped_targets+=clips.len();
        self.gradients+=appearances.values().filter(|a|a.gradient.is_some()).count();
        for a in appearances.values(){
            if let Some(gradient)=&a.gradient{
                for stop in &gradient.stops{
                    *self.solid_colors.entry(stop.color.to_uppercase()).or_default()+=1;
                }
            }
        }
        for obj in objects{
            self.grouped_members+=usize::from(obj.group_id.is_some());
            for color in [&obj.style.fill,&obj.style.stroke]{
                if color!="none"{
                    *self.solid_colors.entry(color.to_uppercase()).or_default()+=1;
                }
            }
            match &obj.shape{
                Shape::Rectangle{..}|Shape::Ellipse{..}|Shape::Polyline{..}|
                Shape::Bezier{..}|Shape::CompoundPath{..}=>self.editable_vectors+=1,
                Shape::Text{..}=>{
                    self.editable_text+=1;
                    *self.font_families.entry(obj.text_format.family.clone())
                        .or_default()+=1;
                }
                Shape::EmbeddedSvg{..}=>self.embedded_svg+=1,
                Shape::Raster{data_base64,..}=>{
                    self.raster_images+=1;
                    self.estimated_image_bytes+=data_base64.len()*3/4;
                }
            }
        }
    }
}
impl Document{
    pub fn inventory(&self)->Inventory{
        let mut result=Inventory{pages:self.pages().len(),..Default::default()};
        result.include(&self.objects,&self.appearances,&self.clip_links);
        for page in self.inactive_pages.values(){
            result.include(&page.objects,&page.appearances,&page.clip_links);
        }
        result
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Page,Shape,Style};
    #[test] fn inventory_covers_every_page_and_gradient_stops(){
        let mut d=Document::default();
        let id=d.add(Shape::Rectangle{x:1.,y:2.,w:10.,h:20.},Style::default());
        d.set_appearance(id,crate::appearance::Appearance{
            gradient:Some(crate::appearance::GradientPreset::Ocean.create()),
            ..Default::default()
        }).unwrap();
        d.add_page(Page{width_mm:210.,height_mm:297.}).unwrap();
        d.add(Shape::Text{x:2.,y:3.,size_mm:5.,value:"Привет".into()},Style::default());
        let inventory=d.inventory();
        assert_eq!(inventory.pages,2);
        assert_eq!(inventory.objects,2);
        assert_eq!(inventory.editable_vectors,1);
        assert_eq!(inventory.editable_text,1);
        assert_eq!(inventory.gradients,1);
        assert_eq!(inventory.font_families["sans-serif"],1);
    }
}
