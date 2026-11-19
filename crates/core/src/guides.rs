//! Persistent per-page guides and professional snap targets.
//! Guides are editor-only: SVG, PDF, DXF and cutting exports never include them.
use crate::{Document,Point};
use serde::{Deserialize,Serialize};
use std::collections::HashSet;

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum GuideAxis { X,Y }
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
pub struct Guide{
    /// X = vertical guide, Y = horizontal guide.
    pub axis:GuideAxis,
    pub position_mm:f32,
}
#[derive(Clone,Copy,Debug)]
pub struct SnapSettings{
    pub guides:bool,
    pub page_edges:bool,
    pub objects:bool,
    pub grid:bool,
    pub grid_mm:f32,
    pub tolerance_mm:f32,
}
impl Default for SnapSettings{
    fn default()->Self{Self{guides:true,page_edges:true,objects:true,
        grid:false,grid_mm:10.0,tolerance_mm:2.0}}
}
impl Document {
    pub fn validate_guides(&self)->Result<(),String>{
        if self.guides.len()>512{return Err("Page guide limit exceeded".into());}
        for guide in &self.guides{
            let extent=match guide.axis{
                GuideAxis::X=>self.page.width_mm,
                GuideAxis::Y=>self.page.height_mm
            };
            if !guide.position_mm.is_finite()
                || guide.position_mm<0.0 || guide.position_mm>extent{
                return Err("Guide exceeds page bounds".into());
            }
        }
        Ok(())
    }
    pub fn add_guide(&mut self,axis:GuideAxis,mm:f32)->Result<usize,String>{
        let limit=match axis{GuideAxis::X=>self.page.width_mm,
            GuideAxis::Y=>self.page.height_mm};
        if !mm.is_finite()||mm<0.0||mm>limit{
            return Err("Guide must be inside the page".into());
        }
        if self.guides.len()>=512{return Err("Too many page guides".into());}
        self.guides.push(Guide{axis,position_mm:mm});
        Ok(self.guides.len()-1)
    }
    pub fn edit_guide(&mut self,index:usize,mm:f32)->Result<(),String>{
        let Some(old)=self.guides.get(index) else{return Err("Unknown guide".into());};
        let limit=match old.axis{GuideAxis::X=>self.page.width_mm,
            GuideAxis::Y=>self.page.height_mm};
        if !mm.is_finite()||mm<0.0||mm>limit{
            return Err("Guide must be inside the page".into());
        }
        self.guides[index].position_mm=mm;
        Ok(())
    }
    pub fn remove_guide(&mut self,index:usize)->Result<(),String>{
        if index>=self.guides.len(){return Err("Unknown guide".into());}
        self.guides.remove(index);Ok(())
    }
    /// Snap x and y independently with priority: guides, objects/page bounds,
    /// then grid. Excluded IDs prevent selected objects snapping to themselves.
    pub fn snap_to_targets(&self,p:Point,options:SnapSettings,
        excluded:&HashSet<u64>)->Point {
        let tol=options.tolerance_mm.max(0.001);
        let mut x=(p.x,tol+0.001);
        let mut y=(p.y,tol+0.001);
        let test=|target:f32,source:f32,current:&mut (f32,f32)|{
            let delta=(target-source).abs();
            if delta<=tol && delta<current.1{*current=(target,delta);}
        };
        if options.guides{
            for guide in &self.guides{
                match guide.axis{
                    GuideAxis::X=>test(guide.position_mm,p.x,&mut x),
                    GuideAxis::Y=>test(guide.position_mm,p.y,&mut y),
                }
            }
        }
        if options.page_edges{
            for val in [0.0,self.page.width_mm/2.0,self.page.width_mm]{
                test(val,p.x,&mut x);
            }
            for val in [0.0,self.page.height_mm/2.0,self.page.height_mm]{
                test(val,p.y,&mut y);
            }
        }
        // Full-document scans can be expensive with 100k nodes; leave guides
        // and grid functioning rather than blocking the UI on huge documents.
        if options.objects && self.objects.len()<=12_000{
            for obj in self.visible_objects(){
                if excluded.contains(&obj.id){continue;}
                let b=obj.bounds();
                for val in [b.x,b.x+b.w/2.0,b.x+b.w]{test(val,p.x,&mut x);}
                for val in [b.y,b.y+b.h/2.0,b.y+b.h]{test(val,p.y,&mut y);}
            }
        }
        if options.grid && options.grid_mm.is_finite() && options.grid_mm>=0.01{
            if x.1>tol {x.0=(p.x/options.grid_mm).round()*options.grid_mm;}
            if y.1>tol {y.0=(p.y/options.grid_mm).round()*options.grid_mm;}
        }
        Point{x:x.0,y:y.0}
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Page,Shape,Style};
    #[test] fn guides_serialize_and_follow_their_pages(){
        let mut doc=Document::default();
        doc.add_guide(GuideAxis::X,42.0).unwrap();
        doc.add_guide(GuideAxis::Y,77.0).unwrap();
        let id=doc.add_page(Page{width_mm:210.,height_mm:297.}).unwrap();
        assert!(doc.guides.is_empty());
        doc.add_guide(GuideAxis::Y,11.0).unwrap();
        doc.switch_page(1).unwrap();
        assert_eq!(doc.guides.len(),2);
        doc.switch_page(id).unwrap();
        assert_eq!(doc.guides.len(),1);
        doc.validate().unwrap();
        let rehydrated:Document=serde_json::from_str(
            &serde_json::to_string(&doc).unwrap()).unwrap();
        assert_eq!(rehydrated.guides.len(),1);
        rehydrated.validate().unwrap();
    }
    #[test] fn rejects_invalid_guides_and_guide_edits(){
        let mut doc=Document::default();
        assert!(doc.add_guide(GuideAxis::X,f32::NAN).is_err());
        assert!(doc.add_guide(GuideAxis::X,-1.0).is_err());
        assert!(doc.add_guide(GuideAxis::X,100_000.0).is_err());
        let id=doc.add_guide(GuideAxis::Y,12.0).unwrap();
        assert!(doc.edit_guide(id,60_000.0).is_err());
        assert_eq!(doc.guides[id].position_mm,12.0);
        doc.remove_guide(id).unwrap();
        assert!(doc.remove_guide(id).is_err());
    }
    #[test] fn guides_have_priority_over_grid_and_objects(){
        let mut doc=Document::default();
        doc.add_guide(GuideAxis::X,19.0).unwrap();
        doc.add(Shape::Rectangle{x:40.,y:60.,w:20.,h:10.},Style::default());
        let settings=SnapSettings{
            guides:true,page_edges:false,objects:true,
            grid:true,grid_mm:10.0,tolerance_mm:2.0
        };
        let snapped=doc.snap_to_targets(Point{x:19.3,y:60.8},settings,&HashSet::new());
        assert_eq!(snapped,Point{x:19.0,y:60.0});
        let excluded=doc.objects.iter().map(|o|o.id).collect();
        let snapped=doc.snap_to_targets(Point{x:41.0,y:61.0},settings,&excluded);
        assert_eq!(snapped,Point{x:40.0,y:60.0});
    }
}
