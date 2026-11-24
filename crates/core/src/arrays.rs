//! Native copies in repeat patterns. All repeats are prepared in a temporary
//! document, validated, then committed atomically. Undo is owned by the UI.
use crate::{Document,Object,Point,Shape,ClipLink,transform};
use std::collections::{BTreeMap,HashMap,HashSet};

#[derive(Clone,Copy,Debug)]
pub enum RepeatPattern{
    Linear{count:usize,dx_mm:f32,dy_mm:f32},
    Grid{columns:usize,rows:usize,dx_mm:f32,dy_mm:f32},
    Radial{count:usize,center:Point,sweep_degrees:f32},
}
#[derive(Clone,Copy,Debug)]
enum Placement{Translate(f32,f32),Rotate(Point,f32)}
fn placements(pattern:RepeatPattern)->Result<Vec<Placement>,String>{
    let mut offsets=Vec::new();
    match pattern{
        RepeatPattern::Linear{count,dx_mm,dy_mm}=>{
            if !(2..=400).contains(&count) || !dx_mm.is_finite()||!dy_mm.is_finite()
                || dx_mm.abs()>100_000. || dy_mm.abs()>100_000. {
                return Err("Invalid linear repeat configuration".into());
            }
            for i in 1..count{offsets.push(Placement::Translate(dx_mm*i as f32,dy_mm*i as f32));}
        }
        RepeatPattern::Grid{columns,rows,dx_mm,dy_mm}=>{
            if columns==0 || rows==0 || columns>40 || rows>40 || columns*rows<2 ||
                !dx_mm.is_finite() || !dy_mm.is_finite() ||
                dx_mm.abs()>100_000. || dy_mm.abs()>100_000.{
                return Err("Invalid grid repeat configuration".into());
            }
            for row in 0..rows{
                for col in 0..columns{
                    if row!=0||col!=0{
                        offsets.push(Placement::Translate(col as f32*dx_mm,row as f32*dy_mm));
                    }
                }
            }
        }
        RepeatPattern::Radial{count,center,sweep_degrees}=>{
            if !(2..=360).contains(&count) || ![center.x,center.y,sweep_degrees]
                .iter().all(|v|v.is_finite())
                || center.x.abs()>100_000. || center.y.abs()>100_000.
                || sweep_degrees.abs()>3600.{
                return Err("Invalid radial repeat configuration".into());
            }
            for i in 1..count{
                offsets.push(Placement::Rotate(center,sweep_degrees*i as f32/count as f32));
            }
        }
    }
    Ok(offsets)
}
fn placed(shape:&Shape,placement:Placement)->Result<Shape,String>{
    let mut result=shape.clone();
    match placement{
        Placement::Translate(x,y)=>result.translate(x,y),
        Placement::Rotate(center,angle)=>transform::rotate_shape(&mut result,center,angle)?,
    }
    let b=result.bounds();
    if ![b.x,b.y,b.w,b.h].iter().all(|v|v.is_finite())
        || b.x.abs()>100_000. || b.y.abs()>100_000.
        || (b.x+b.w).abs()>100_000. || (b.y+b.h).abs()>100_000.{
        return Err("Repeated object exceeds editable coordinate range".into());
    }
    Ok(result)
}
impl Document{
    /// Copies selected objects while preserving layers, appearance, group
    /// membership and fully selected clipping pairs, with no partial mutations.
    /// The first position is the original, never an overlapping duplicate.
    pub fn duplicate_pattern(&mut self,ids:&[u64],pattern:RepeatPattern)
        ->Result<Vec<u64>,String>{
        let wanted:HashSet<u64>=ids.iter().copied().collect();
        if wanted.is_empty(){return Err("Select one or more objects".into());}
        let sources:Vec<Object>=self.objects.iter()
            .filter(|o|wanted.contains(&o.id)).cloned().collect();
        if sources.len()!=wanted.len(){return Err("Selection contains unknown objects".into());}
        if sources.iter().any(|o|!self.layer(o.layer_id)
            .is_some_and(|l|l.visible&&!l.locked)){
            return Err("Cannot repeat hidden or locked objects".into());
        }
        if self.objects.iter().any(|o|o.group_id.is_some_and(|group|
            sources.iter().any(|s|s.group_id==Some(group))&&!wanted.contains(&o.id))){
            return Err("Select every member of grouped objects".into());
        }
        if self.clip_links.iter().any(|link|
            wanted.contains(&link.source_id)!=wanted.contains(&link.target_id)){
            return Err("Select both parts of a clipped object".into());
        }
        let symbol_sources:Vec<_>=self.symbol_instances.iter()
            .filter(|instance|instance.page_id==self.active_page_id&&
                instance.member_ids.iter().all(|id|wanted.contains(id)))
            .cloned().collect();
        let placements=placements(pattern)?;
        let new_count=sources.len().checked_mul(placements.len())
            .ok_or("Too many objects")?;
        if new_count>1500 || self.objects.len()+new_count>100_000{
            return Err("Repeat exceeds the 1500-new-object / 100000-document limit".into());
        }
        let estimated_source:usize=sources.iter().map(|o|match &o.shape{
            Shape::Raster{data_base64,..}=>data_base64.len(),
            Shape::EmbeddedSvg{svg,..}=>svg.len(),
            Shape::Polyline{points,..}=>points.len()*8,
            Shape::Bezier{segments,..}=>segments.len()*24,
            Shape::CompoundPath{contours}=>contours.iter().map(|c|c.segments.len()*24).sum(),
            Shape::Text{value,..}=>value.len(),_=>64
        }).sum();
        if estimated_source.saturating_mul(placements.len())>128*1024*1024{
            return Err("Repeat exceeds the 128 MiB estimated source-copy budget".into());
        }
        let mut draft=self.clone();
        let mut created=Vec::with_capacity(new_count);
        for placement in placements{
            let mut new_ids=BTreeMap::new();
            let mut group_ids=HashMap::new();
            for original in &sources{
                let shape=placed(&original.shape,placement)?;
                if draft.next_id>u64::MAX-3{return Err("Object identifier limit reached".into());}
                let id=draft.copy_object(original,original.layer_id,0.0,0.0);
                draft.get_mut(id).ok_or("New object missing")?.shape=shape;
                if let Some(old_group)=original.group_id{
                    let new_group=if let Some(value)=group_ids.get(&old_group){
                        *value
                    }else{
                        let value=draft.next_id;
                        draft.next_id+=1;
                        group_ids.insert(old_group,value);
                        value
                    };
                    draft.get_mut(id).unwrap().group_id=Some(new_group);
                }
                new_ids.insert(original.id,id);
                created.push(id);
            }
            for link in &self.clip_links{
                if let (Some(&source_id),Some(&target_id))=
                    (new_ids.get(&link.source_id),new_ids.get(&link.target_id)){
                    draft.clip_links.push(ClipLink{source_id,target_id});
                }
            }
            for instance in &symbol_sources{
                let group_id=group_ids.get(&instance.group_id).copied()
                    .ok_or("Repeated symbol group ID was not remapped")?;
                let member_ids=instance.member_ids.iter().map(|old|
                    new_ids.get(old).copied().ok_or("Repeated symbol member ID was not remapped"))
                    .collect::<Result<Vec<_>,_>>()?;
                draft.symbol_instances.push(crate::SymbolInstance{
                    symbol_id:instance.symbol_id,page_id:draft.active_page_id,
                    group_id,member_ids
                });
            }
        }
        draft.validate()?;
        *self=draft;
        Ok(created)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Style,Shape};
    #[test] fn grid_copies_preserve_styles_and_groups(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Rectangle{x:1.,y:2.,w:10.,h:10.},Style::default());
        let b=doc.add(Shape::Ellipse{x:12.,y:2.,w:8.,h:8.},Style::default());
        doc.group_objects(&[a,b]).unwrap();
        let ids=doc.duplicate_pattern(&[a,b],RepeatPattern::Grid{
            columns:3,rows:2,dx_mm:24.,dy_mm:20.
        }).unwrap();
        assert_eq!(ids.len(),10);
        assert_eq!(doc.objects.len(),12);
        assert_ne!(doc.get(ids[0]).unwrap().group_id,doc.get(a).unwrap().group_id);
        assert_eq!(doc.group_members(ids[0]).len(),2);
        doc.validate().unwrap();
    }
    #[test] fn repeated_symbol_instances_keep_their_link(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Rectangle{x:1.,y:2.,w:10.,h:10.},Style::default());
        let b=doc.add(Shape::Ellipse{x:12.,y:2.,w:8.,h:8.},Style::default());
        let symbol=doc.create_symbol(&[a,b],"Array mark","Test").unwrap();
        let copies=doc.duplicate_pattern(&[a,b],RepeatPattern::Linear{
            count:3,dx_mm:40.,dy_mm:0.
        }).unwrap();
        assert_eq!(copies.len(),4);
        assert_eq!(doc.symbol_instances.len(),3);
        assert!(doc.symbol_instances.iter().all(|item|item.symbol_id==symbol.id));
        doc.validate().unwrap();
    }
    #[test] fn repeat_rejects_partial_group_atomically(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Rectangle{x:1.,y:1.,w:2.,h:2.},Style::default());
        let b=doc.add(Shape::Rectangle{x:4.,y:1.,w:2.,h:2.},Style::default());
        doc.group_objects(&[a,b]).unwrap();
        let before=doc.clone();
        assert!(doc.duplicate_pattern(&[a],RepeatPattern::Linear{
            count:3,dx_mm:20.,dy_mm:0.
        }).is_err());
        assert_eq!(doc,before);
    }
    #[test] fn radial_rotations_stay_editable_and_original_unchanged(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Rectangle{x:20.,y:20.,w:10.,h:10.},Style::default());
        let original=doc.get(a).unwrap().clone();
        let copies=doc.duplicate_pattern(&[a],RepeatPattern::Radial{
            count:8,center:Point{x:50.,y:50.},sweep_degrees:360.
        }).unwrap();
        assert_eq!(copies.len(),7);
        assert_eq!(doc.get(a).unwrap(),&original);
        doc.validate().unwrap();
        let restored:Document=serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
        assert_eq!(restored,doc);
    }
    #[test] fn invalid_configuration_does_not_mutate(){
        let mut doc=Document::default();
        let id=doc.add(Shape::Text{x:1.,y:1.,value:"test".into(),size_mm:8.},Style::default());
        let original=doc.clone();
        assert!(doc.duplicate_pattern(&[id],RepeatPattern::Radial{
            count:5,center:Point{x:50.,y:50.},sweep_degrees:57.
        }).is_err());
        assert_eq!(doc,original);
        assert!(doc.duplicate_pattern(&[id],RepeatPattern::Linear{
            count:4000,dx_mm:3.,dy_mm:0.
        }).is_err());
        assert_eq!(doc,original);
    }
    #[test] fn linear_repeats_can_copy_clipped_pairs(){
        let mut doc=Document::default();
        let mask=doc.add(Shape::Rectangle{x:5.,y:5.,w:20.,h:20.},Style::default());
        let target=doc.add(Shape::Ellipse{x:0.,y:0.,w:30.,h:30.},Style::default());
        doc.set_clip(mask,target).unwrap();
        let ids=doc.duplicate_pattern(&[mask,target],RepeatPattern::Linear{
            count:2,dx_mm:50.,dy_mm:0.
        }).unwrap();
        assert_eq!(doc.clip_links.len(),2);
        assert_eq!(doc.clip_links[1].source_id,ids[0]);
        assert_eq!(doc.clip_links[1].target_id,ids[1]);
    }
}
