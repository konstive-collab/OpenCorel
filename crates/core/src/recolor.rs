//! Document-wide palette replacement with atomic, layer-aware preflight.
//! Replaces RGB hex values in fills, strokes and multi-stop gradients across
//! current or all pages; never touches embedded SVG, raster bytes or text.
use crate::{Document,Object,Layer};
use crate::appearance::Appearance;
use std::collections::{BTreeMap,HashSet};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct ColorChannels{
    pub fills:bool,
    pub strokes:bool,
    pub gradient_stops:bool,
}
impl Default for ColorChannels{
    fn default()->Self{Self{fills:true,strokes:true,gradient_stops:true}}
}
#[derive(Clone,Debug,Default,PartialEq,Eq)]
pub struct RecolorReport{
    pub pages:usize,
    pub objects:usize,
    pub fill_slots:usize,
    pub stroke_slots:usize,
    pub gradient_slots:usize,
}
impl RecolorReport{
    pub fn total(&self)->usize{
        self.fill_slots+self.stroke_slots+self.gradient_slots
    }
}
fn same(s:&str,color:&str)->bool{s.eq_ignore_ascii_case(color)}
fn inspect_page(
    objects:&[Object],layers:&[Layer],
    appearances:&BTreeMap<u64,Appearance>,from:&str,channels:ColorChannels,
    report:&mut RecolorReport
)->Result<bool,String>{
    let mut changed=false;
    for object in objects{
        let fill=usize::from(channels.fills&&same(&object.style.fill,from));
        let stroke=usize::from(channels.strokes&&same(&object.style.stroke,from));
        let stops=if channels.gradient_stops{
            appearances.get(&object.id).and_then(|a|a.gradient.as_ref())
                .map(|g|g.stops.iter().filter(|s|same(&s.color,from)).count())
                .unwrap_or(0)
        }else{0};
        if fill+stroke+stops==0{continue;}
        if layers.iter().find(|l|l.id==object.layer_id)
            .is_none_or(|l|!l.visible||l.locked){
            return Err(format!(
                "Object #{} is on a hidden or locked layer; no color was replaced",
                object.id
            ));
        }
        report.objects+=1;
        report.fill_slots+=fill;
        report.stroke_slots+=stroke;
        report.gradient_slots+=stops;
        changed=true;
    }
    Ok(changed)
}
fn apply_page(
    objects:&mut [Object],
    appearances:&mut BTreeMap<u64,Appearance>,
    from:&str,to:&str,channels:ColorChannels
){
    if channels.fills||channels.strokes{
        for object in objects{
            if channels.fills&&same(&object.style.fill,from){
                object.style.fill=to.to_owned();
            }
            if channels.strokes&&same(&object.style.stroke,from){
                object.style.stroke=to.to_owned();
            }
        }
    }
    if channels.gradient_stops{
        for appearance in appearances.values_mut(){
            if let Some(gradient)=&mut appearance.gradient{
                for stop in &mut gradient.stops{
                    if same(&stop.color,from){stop.color=to.to_owned();}
                }
            }
        }
    }
}
impl Document{
    /// Pure preview: counts the exact slots to change without touching KDR.
    /// Any locked or hidden matching object aborts preview.
    pub fn preview_recolor(&self,from:&str,to:&str,
        channels:ColorChannels,all_pages:bool)->Result<RecolorReport,String>{
        if !crate::valid_color(from)||!crate::valid_color(to){
            return Err("Specify two RGB colors as #RRGGBB".into());
        }
        if !channels.fills&&!channels.strokes&&!channels.gradient_stops{
            return Err("Enable at least one color component".into());
        }
        let mut report=RecolorReport::default();
        if same(from,to){return Ok(report);}
        if inspect_page(&self.objects,&self.layers,&self.appearances,
            from,channels,&mut report)?{report.pages+=1;}
        if all_pages{
            for page in self.inactive_pages.values(){
                if inspect_page(&page.objects,&page.layers,&page.appearances,
                    from,channels,&mut report)?{report.pages+=1;}
            }
        }
        Ok(report)
    }
    /// Atomic palette replacement. Every page and every affected object's
    /// editability is checked before the first mutation. All new colors are
    /// validated RGB; object IDs, fill geometry, bitmap data and fonts survive.
    pub fn recolor_rgb(&mut self,from:&str,to:&str,
        channels:ColorChannels,all_pages:bool)->Result<RecolorReport,String>{
        let report=self.preview_recolor(from,to,channels,all_pages)?;
        if report.total()==0{return Ok(report);}
        let to=to.to_uppercase();
        apply_page(&mut self.objects,&mut self.appearances,from,&to,channels);
        if all_pages{
            for page in self.inactive_pages.values_mut(){
                apply_page(&mut page.objects,&mut page.appearances,from,&to,channels);
            }
        }
        Ok(report)
    }
    /// Return all eligible matching objects on the active page for
    /// "Select same color", honouring layer visibility and locks.
    pub fn objects_with_rgb(&self,color:&str,
        channels:ColorChannels)->Result<Vec<u64>,String>{
        if !crate::valid_color(color){return Err("RGB must be #RRGGBB".into());}
        let editable:HashSet<_>=self.layers.iter().filter(|l|
            l.visible&&!l.locked).map(|l|l.id).collect();
        Ok(self.objects.iter().filter(|obj|{
            if !editable.contains(&obj.layer_id){return false;}
            (channels.fills&&same(&obj.style.fill,color))
            ||(channels.strokes&&same(&obj.style.stroke,color))
            ||(channels.gradient_stops&&self.appearances.get(&obj.id)
                .and_then(|a|a.gradient.as_ref())
                .is_some_and(|g|g.stops.iter().any(|s|same(&s.color,color))))
        }).map(|o|o.id).collect())
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Page,Shape,Style};
    use crate::appearance::{Appearance,GradientPreset};
    #[test] fn recolors_fill_outline_and_stops_across_pages(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Rectangle{x:1.,y:2.,w:20.,h:30.},Style{
            fill:"#ff0000".into(),stroke:"#FF0000".into(),stroke_width_mm:1.0});
        doc.set_appearance(a,Appearance{
            gradient:Some(GradientPreset::Sunrise.create()),..Default::default()
        }).unwrap();
        // Add a matching gradient stop.
        doc.appearances.get_mut(&a).unwrap().gradient.as_mut()
            .unwrap().stops[1].color="#fF0000".into();
        let second=doc.add_page(Page{width_mm:210.,height_mm:297.}).unwrap();
        let b=doc.add(Shape::Ellipse{x:10.,y:20.,w:10.,h:10.},Style{
            fill:"#FF0000".into(),stroke:"none".into(),stroke_width_mm:0.0});
        let preview=doc.preview_recolor("#FF0000","#0A95FF",
            ColorChannels::default(),true).unwrap();
        assert_eq!(preview.pages,2);
        assert_eq!(preview.objects,2);
        assert_eq!(preview.total(),4);
        assert_eq!(doc.recolor_rgb("#ff0000","#0A95FF",
            ColorChannels::default(),true).unwrap(),preview);
        assert_eq!(doc.get(b).unwrap().style.fill,"#0A95FF");
        doc.switch_page(1).unwrap();
        assert_eq!(doc.get(a).unwrap().style.stroke,"#0A95FF");
        assert_eq!(doc.appearances[&a].gradient.as_ref().unwrap().stops[1].color,
            "#0A95FF");
        doc.switch_page(second).unwrap();
        doc.validate().unwrap();
    }
    #[test] fn locked_color_match_aborts_entire_recolor(){
        let mut doc=Document::default();
        doc.add(Shape::Rectangle{x:1.,y:1.,w:10.,h:10.},Style{
            fill:"#AABBCC".into(),stroke:"none".into(),stroke_width_mm:0.0});
        doc.add_page(Page{width_mm:210.,height_mm:297.}).unwrap();
        doc.add(Shape::Rectangle{x:1.,y:1.,w:10.,h:10.},Style{
            fill:"#AABBCC".into(),stroke:"none".into(),stroke_width_mm:0.0});
        doc.layers[0].locked=true;
        let before=doc.clone();
        assert!(doc.recolor_rgb("#AABBCC","#FFFFFF",
            ColorChannels::default(),true).is_err());
        assert_eq!(doc,before);
    }
    #[test] fn select_same_rgb_is_active_page_and_layer_aware(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Rectangle{x:0.,y:0.,w:10.,h:10.},Style{
            fill:"#111111".into(),stroke:"none".into(),stroke_width_mm:0.0});
        let b=doc.add(Shape::Ellipse{x:20.,y:0.,w:10.,h:10.},Style{
            fill:"#222222".into(),stroke:"#111111".into(),stroke_width_mm:1.0});
        let matched=doc.objects_with_rgb("#111111",ColorChannels::default()).unwrap();
        assert_eq!(matched,vec![a,b]);
        let fills=doc.objects_with_rgb("#111111",ColorChannels{
            fills:true,strokes:false,gradient_stops:false
        }).unwrap();
        assert_eq!(fills,vec![a]);
    }
}
