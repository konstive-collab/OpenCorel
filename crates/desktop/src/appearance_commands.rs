//! Non-destructive object appearance and style eyedropper commands.
use super::Editor;
use opencorel_core::appearance::{
    Appearance,Gradient,GradientKind,GradientStop,StrokeCap,StrokeJoin
};
use std::collections::HashSet;

impl Editor{
    fn requested_appearance(&self)->Result<Appearance,String>{
        let gradient=match self.appearance_gradient_mode{
            0=>None,
            1|2=>Some(Gradient{
                kind:if self.appearance_gradient_mode==1{
                    GradientKind::Linear
                }else{GradientKind::Radial},
                angle_degrees:self.appearance_gradient_angle,
                stops:vec![
                    GradientStop{offset:0.0,color:self.appearance_gradient_start.clone()},
                    GradientStop{offset:1.0,color:self.appearance_gradient_end.clone()},
                ],
            }),
            _=>return Err("Unknown gradient mode".into()),
        };
        let dash_array=if self.appearance_dash_mm==0.0{
            Vec::new()
        }else{
            vec![self.appearance_dash_mm,self.appearance_dash_mm*0.6]
        };
        let cap=match self.appearance_cap{
            0=>StrokeCap::Butt,1=>StrokeCap::Round,2=>StrokeCap::Square,
            _=>return Err("Unknown line cap".into()),
        };
        let join=match self.appearance_join{
            0=>StrokeJoin::Miter,1=>StrokeJoin::Round,2=>StrokeJoin::Bevel,
            _=>return Err("Unknown line join".into()),
        };
        let appearance=Appearance{
            opacity:self.appearance_opacity,gradient,dash_array,cap,join
        };
        appearance.validate()?;
        Ok(appearance)
    }
    pub(super) fn apply_appearance_selection(&mut self){
        let ids:HashSet<u64>=self.selection_ids().into_iter().collect();
        if ids.is_empty(){self.status="Выберите объект для изменения оформления".into();return;}
        let appearance=match self.requested_appearance(){
            Ok(a)=>a,
            Err(e)=>{self.status=format!("Недопустимое оформление: {e}");return;}
        };
        if !self.doc.objects.iter().filter(|o|ids.contains(&o.id)).all(|o|
            self.doc.layer(o.layer_id).is_some_and(|layer|layer.visible&&!layer.locked)){
            self.status="Оформление нельзя менять на заблокированном слое".into();
            return;
        }
        let changed=ids.iter().any(|id|
            self.doc.appearances.get(id).cloned().unwrap_or_default()!=appearance);
        if !changed{return;}
        self.checkpoint();
        for id in ids{
            self.doc.set_appearance(id,appearance.clone())
                .expect("Appearance and layer preflight succeeded");
        }
        self.status="Оформление применено. Ctrl+Z — отменить".into();
    }
    pub(super) fn clear_appearance_selection(&mut self){
        let ids:HashSet<u64>=self.selection_ids().into_iter().collect();
        if !ids.iter().any(|id|self.doc.appearances.contains_key(id)){return;}
        if !self.doc.objects.iter().filter(|o|ids.contains(&o.id)).all(|o|
            self.doc.layer(o.layer_id).is_some_and(|layer|layer.visible&&!layer.locked)){
            self.status="Не удалось сбросить оформление на заблокированном слое".into();
            return;
        }
        self.checkpoint();
        for id in ids{self.doc.appearances.remove(&id);}
        self.status="Оформление сброшено".into();
    }
    pub(super) fn copy_appearance_between_selected(&mut self){
        let ids=self.selection_ids();
        if ids.len()!=2{
            self.status="Пипетка: выделите исходный и целевой объекты с Shift".into();
            return;
        }
        let (source,target)=(ids[0],ids[1]);
        if source==target{return;}
        let from=match self.doc.objects.iter().find(|o|o.id==source){
            Some(o)=>o.style.clone(),
            None=>return
        };
        let dest=match self.doc.objects.iter().find(|o|o.id==target){
            Some(o)=>o,
            None=>return
        };
        if !self.doc.is_object_editable(dest.id){
            self.status="Цель пипетки находится на заблокированном слое".into();
            return;
        }
        let source_appearance=self.doc.appearances.get(&source).cloned();
        let old=self.doc.appearances.get(&target).cloned();
        if dest.style==from&&old==source_appearance{return;}
        self.checkpoint();
        self.doc.get_mut(target).unwrap().style=from;
        if let Some(value)=source_appearance{
            self.doc.appearances.insert(target,value);
        }else{
            self.doc.appearances.remove(&target);
        }
        self.fill_cache.entries.clear();
        self.status="Пипетка: оформление скопировано на второй объект".into();
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Shape,Style};
    #[test]
    fn applying_appearance_is_undoable(){
        let mut e=Editor::default();
        let id=e.doc.objects[0].id;
        e.selected=Some(id);
        e.appearance_opacity=0.6;
        e.appearance_gradient_mode=1;
        e.apply_appearance_selection();
        assert!(e.doc.to_svg().contains("<linearGradient"));
        assert_eq!(e.doc.appearances.get(&id).unwrap().opacity,0.6);
        e.undo();
        assert!(e.doc.appearances.is_empty());
    }
    #[test]
    fn malformed_gradient_does_not_mutate_document(){
        let mut e=Editor::default();
        e.selected=Some(e.doc.objects[0].id);
        e.appearance_gradient_mode=2;
        e.appearance_gradient_start="not a color".into();
        let original=e.doc.clone();
        e.apply_appearance_selection();
        assert_eq!(e.doc,original);
        assert!(e.undo.is_empty());
    }
    #[test]
    fn eyedropper_copies_style_and_gradient(){
        let mut e=Editor::default();
        let a=e.doc.objects[0].id;
        let b=e.doc.add(Shape::Rectangle{x:0.0,y:0.0,w:10.0,h:10.0},Style::default());
        e.selected=Some(a);
        e.appearance_gradient_mode=1;
        e.apply_appearance_selection();
        e.selected=Some(a);e.multi_selected=vec![b];
        e.copy_appearance_between_selected();
        assert_eq!(e.doc.appearances.get(&a),e.doc.appearances.get(&b));
        assert_eq!(
            e.doc.objects.iter().find(|o|o.id==a).unwrap().style,
            e.doc.objects.iter().find(|o|o.id==b).unwrap().style
        );
    }
}
