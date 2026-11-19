//! Non-destructive object appearance and style eyedropper commands.
use super::Editor;
use opencorel_core::appearance::{
    Appearance,Gradient,GradientKind,GradientStop,GradientPreset,StrokeCap,StrokeJoin
};
use std::collections::HashSet;
use eframe::egui::{self,Color32,Vec2,Rect,Pos2,RichText};

impl Editor{
    fn requested_appearance(&self)->Result<Appearance,String>{
        let gradient=match self.appearance_gradient_mode{
            0=>None,
            1|2=>Some(Gradient{
                kind:if self.appearance_gradient_mode==1{
                    GradientKind::Linear
                }else{GradientKind::Radial},
                angle_degrees:self.appearance_gradient_angle,
                stops:self.appearance_gradient_stops.clone(),
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
    /// An actual multi-stop appearance editor. The pending gradient stays in
    /// editor UI state until "Apply" commits a validated single undo step.
    pub(super) fn appearance_editor(&mut self,ui:&mut egui::Ui){
        ui.set_min_width(360.0);
        ui.label(RichText::new("ПРОФЕССИОНАЛЬНОЕ ОФОРМЛЕНИЕ").strong());
        if ui.add_enabled(self.selected.is_some(),egui::Button::new(
            "Загрузить оформление выбранного объекта"
        )).clicked(){
            if let Some(id)=self.selected{
                let appearance=self.doc.appearances.get(&id)
                    .cloned().unwrap_or_default();
                self.appearance_opacity=appearance.opacity;
                self.appearance_cap=match appearance.cap{
                    StrokeCap::Butt=>0,StrokeCap::Round=>1,StrokeCap::Square=>2
                };
                self.appearance_join=match appearance.join{
                    StrokeJoin::Miter=>0,StrokeJoin::Round=>1,StrokeJoin::Bevel=>2
                };
                self.appearance_dash_mm=appearance.dash_array
                    .first().copied().unwrap_or(0.0);
                if let Some(gradient)=appearance.gradient{
                    self.appearance_gradient_mode=match gradient.kind{
                        GradientKind::Linear=>1,GradientKind::Radial=>2
                    };
                    self.appearance_gradient_angle=gradient.angle_degrees;
                    self.appearance_gradient_stops=gradient.stops;
                }else{self.appearance_gradient_mode=0;}
                self.status="Оформление загружено для редактирования".into();
            }
        }
        ui.horizontal(|ui|{
            ui.label("Непрозрачность");
            ui.add(egui::Slider::new(&mut self.appearance_opacity,0.0..=1.0)
                .step_by(0.01).show_value(true));
        });
        ui.separator();
        ui.label("Тип заливки");
        ui.horizontal_wrapped(|ui|{
            ui.selectable_value(&mut self.appearance_gradient_mode,0,"Однотонная");
            ui.selectable_value(&mut self.appearance_gradient_mode,1,"Линейная");
            ui.selectable_value(&mut self.appearance_gradient_mode,2,"Радиальная");
        });
        if self.appearance_gradient_mode!=0{
            ui.label("Пресеты — редактируемые, сохраняются в KDR:");
            ui.horizontal_wrapped(|ui|{
                for preset in GradientPreset::ALL{
                    if ui.small_button(preset.label()).clicked(){
                        let gradient=preset.create();
                        self.appearance_gradient_mode=match gradient.kind{
                            GradientKind::Linear=>1,GradientKind::Radial=>2
                        };
                        self.appearance_gradient_angle=gradient.angle_degrees;
                        self.appearance_gradient_stops=gradient.stops;
                    }
                }
            });
            if self.appearance_gradient_mode==1{
                ui.horizontal(|ui|{
                    ui.label("Угол заливки");
                    ui.add(egui::DragValue::new(&mut self.appearance_gradient_angle)
                        .speed(1.0).range(-360.0..=360.0).suffix("°"));
                });
            }
            let preview=Gradient{
                kind:if self.appearance_gradient_mode==1{
                    GradientKind::Linear
                }else{GradientKind::Radial},
                angle_degrees:self.appearance_gradient_angle,
                stops:self.appearance_gradient_stops.clone(),
            };
            if preview.validate().is_ok(){
                let (rect,response)=ui.allocate_exact_size(
                    Vec2::new(ui.available_width().max(220.0),25.0),
                    egui::Sense::click()
                );
                let count=64;
                for i in 0..count{
                    let t=(i as f32+0.5)/count as f32;
                    let hex=preview.color_at(t).unwrap_or_else(|_|"#777777".into());
                    let rgb=|offset|u8::from_str_radix(&hex[offset..offset+2],16)
                        .unwrap_or(0);
                    let color=Color32::from_rgb(rgb(1),rgb(3),rgb(5));
                    let segment=Rect::from_min_max(
                        Pos2::new(rect.left()+rect.width()*i as f32/count as f32,rect.top()),
                        Pos2::new(rect.left()+rect.width()*(i+1) as f32/count as f32,rect.bottom())
                    );
                    ui.painter().rect_filled(segment,0.0,color);
                }
                if response.clicked()&&self.appearance_gradient_stops.len()<32{
                    if let Some(pos)=response.interact_pointer_pos(){
                        let t=((pos.x-rect.left())/rect.width()).clamp(0.0,1.0);
                        let mut candidate=preview;
                        if candidate.insert_stop(t).is_ok(){
                            self.appearance_gradient_stops=candidate.stops;
                        }
                    }
                }
                ui.small("Щёлкните по полосе градиента для добавления точки.");
            }else{
                ui.colored_label(Color32::from_rgb(235,120,80),
                    "Цветовые точки должны быть #RRGGBB, от 0 до 100%.");
            }
            let mut remove=None;
            for i in 0..self.appearance_gradient_stops.len(){
                ui.horizontal(|ui|{
                    ui.label(format!("{:02}",i+1));
                    let stop=&mut self.appearance_gradient_stops[i];
                    let mut percent=stop.offset*100.0;
                    if ui.add(egui::DragValue::new(&mut percent)
                        .range(0.0..=100.0).speed(0.5).suffix("%")).changed(){
                        stop.offset=percent/100.0;
                    }
                    ui.text_edit_singleline(&mut stop.color);
                    if ui.add_enabled(self.appearance_gradient_stops.len()>2,
                        egui::Button::new("×").small())
                        .on_hover_text("Удалить точку").clicked(){
                        remove=Some(i);
                    }
                });
            }
            if let Some(index)=remove{self.appearance_gradient_stops.remove(index);}
            self.appearance_gradient_stops
                .sort_by(|a,b|a.offset.total_cmp(&b.offset));
            ui.horizontal(|ui|{
                ui.label("Новая точка");
                ui.add(egui::DragValue::new(&mut self.appearance_new_stop_percent)
                    .range(0.0..=100.0).suffix("%"));
                if ui.add_enabled(self.appearance_gradient_stops.len()<32,
                    egui::Button::new("Добавить")).clicked(){
                    let mut candidate=Gradient{
                        kind:GradientKind::Linear,angle_degrees:0.0,
                        stops:self.appearance_gradient_stops.clone()
                    };
                    match candidate.insert_stop(self.appearance_new_stop_percent/100.0){
                        Ok(_)=>self.appearance_gradient_stops=candidate.stops,
                        Err(e)=>self.status=format!("Градиент: {e}")
                    }
                }
            });
            ui.small(format!("Цветовых точек: {} / 32",
                self.appearance_gradient_stops.len()));
        }
        ui.separator();
        ui.horizontal(|ui|{
            ui.label("Пунктир (мм; 0 = сплошной)");
            ui.add(egui::DragValue::new(&mut self.appearance_dash_mm)
                .speed(0.2).range(0.0..=100.0));
        });
        ui.label("Окончания линии");
        ui.horizontal(|ui|{
            ui.selectable_value(&mut self.appearance_cap,0,"Прямые");
            ui.selectable_value(&mut self.appearance_cap,1,"Круглые");
            ui.selectable_value(&mut self.appearance_cap,2,"Квадратные");
        });
        ui.label("Углы соединений");
        ui.horizontal(|ui|{
            ui.selectable_value(&mut self.appearance_join,0,"Острые");
            ui.selectable_value(&mut self.appearance_join,1,"Круглые");
            ui.selectable_value(&mut self.appearance_join,2,"Срез");
        });
        ui.separator();
        if ui.add_enabled(!self.selection_ids().is_empty(),
            egui::Button::new("Применить к выделению · Ctrl+Z — отмена"))
            .clicked(){
            self.apply_appearance_selection();
            ui.close_menu();
        }
        if ui.button("Сбросить оформление выделения").clicked(){
            self.clear_appearance_selection();ui.close_menu();
        }
        ui.small("RGB #RRGGBB. PDF выводится в RGB — без ICC/CMYK.");
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
        if !self.doc.layer(dest.layer_id).is_some_and(|l|l.visible&&!l.locked){
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
        e.appearance_gradient_stops[0].color="not a color".into();
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
