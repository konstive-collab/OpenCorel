//! Discoverable editing workflows and undoable productivity operations.
use super::{Editor,Tool};
use eframe::egui::{self,RichText};
use opencorel_core::{Bounds,Document,Object,Point,Shape,Style};
use opencorel_core::transform::{self,QuarterTurn};
use std::collections::{HashSet,HashMap};

const CLIPBOARD_LIMIT:usize=24*1024*1024;
pub(super) fn tool_label(tool:Tool)->&'static str{
    match tool{
        Tool::Select=>"Выделение",
        Tool::Nodes=>"Редактирование узлов",
        Tool::Rect=>"Прямоугольник",
        Tool::Ellipse=>"Эллипс",
        Tool::Text=>"Текст",
        Tool::Curve=>"Кривая Безье",
        Tool::Freehand=>"Свободное рисование",
        Tool::Line=>"Линия",
        Tool::Polygon=>"Шестиугольник",
        Tool::Smart=>"Профессиональные фигуры", 
    }
}
pub(super) fn tool_hint(tool:Tool)->&'static str{
    match tool{
        Tool::Select=>"Щёлкните и перетащите объект. На пустом месте нарисуйте рамку: слева направо — объекты целиком, справа налево — пересекающиеся. Shift добавляет к выделению. Углы рамки изменяют размер.",
        Tool::Nodes=>"Выберите линию или кривую и потяните её опорную точку или рычаг. Двойной щелчок по сегменту добавляет узел, Del удаляет выбранный опорный узел, Esc возвращает выделение. Для текста сначала используйте «Преобразовать в кривые»; группы перед редактированием разгруппируйте.",
        Tool::Rect=>"Протяните мышь по листу, чтобы нарисовать прямоугольник. Щелчок создаёт фигуру стандартного размера.",
        Tool::Ellipse=>"Протяните мышь, чтобы создать эллипс. После создания выберите его инструментом «Выделение».",
        Tool::Text=>"Напечатайте надпись в поле ниже, затем щёлкните по листу. Шрифт и размер меняются справа, во вкладке «Свойства».",
        Tool::Curve=>"Щёлкайте точки кривой на листе. Двойной щелчок завершает работу, Esc возвращает выделение.",
        Tool::Freehand=>"Удерживайте кнопку мыши и рисуйте свободную линию. После отпускания она станет редактируемой кривой Безье. Точность и сглаживание — справа.",
        Tool::Line=>"Протяните курсор от начала до конца линии. Линии можно назначить толщину и цвет обводки.",
        Tool::Polygon=>"Протяните мышь по листу, чтобы создать шестиугольник. Цвет и размеры изменяются после выделения.",
        Tool::Smart=>"93 редактируемые векторные фигуры. Выберите вариант в панели справа, затем протяните по холсту или добавьте в центр листа.",
    }
}
fn object_bytes(object:&Object)->usize{
    std::mem::size_of::<Object>()+object.style.fill.len()+object.style.stroke.len()
    +object.text_format.family.len()+
    match &object.shape{
        Shape::Raster{data_base64,..}=>data_base64.len(),
        Shape::EmbeddedSvg{svg,..}=>svg.len(),
        Shape::Text{value,..}=>value.len(),
        Shape::Polyline{points,..}=>points.len()*std::mem::size_of::<Point>(),
        Shape::Bezier{segments,..}=>segments.len()*std::mem::size_of::<opencorel_core::CubicSegment>(),
        Shape::CompoundPath{contours}=>contours.iter()
            .map(|c|std::mem::size_of::<opencorel_core::Contour>()+
                c.segments.len()*std::mem::size_of::<opencorel_core::CubicSegment>()).sum(),
        _=>0,
    }
}
fn remap_groups(doc:&mut Document,sources:&[Object],new_ids:&[u64]){
    let mut grouped:HashMap<u64,Vec<u64>>=HashMap::new();
    for (original,id) in sources.iter().zip(new_ids.iter()){
        if let Some(group)=original.group_id{
            grouped.entry(group).or_default().push(*id);
        }
    }
    for members in grouped.values(){
        if members.len()>=2{
            // New IDs point only at copied objects; never reuse source group IDs.
            let _=doc.group_objects(members);
        }
    }
}
fn plain(fill:&str)->Style{
    Style{fill:fill.into(),stroke:"none".into(),stroke_width_mm:0.0}
}
impl Editor{
    pub(super) fn copy_selection(&mut self)->bool{
        let ids:HashSet<u64>=self.selection_ids().into_iter().collect();
        let visible:HashSet<u64>=self.doc.layers.iter()
            .filter(|layer|layer.visible).map(|l|l.id).collect();
        let chosen:Vec<_>=self.doc.objects.iter().filter(|obj|
            ids.contains(&obj.id) && visible.contains(&obj.layer_id)).collect();
        if chosen.is_empty(){
            self.status="Сначала выделите объекты для копирования".into();
            return false;
        }
        let bytes=chosen.iter().fold(0usize,|sum,o|sum.saturating_add(object_bytes(o)));
        if bytes>CLIPBOARD_LIMIT{
            self.status="Выделение слишком велико для внутреннего буфера (24 МБ)".into();
            return false;
        }
        self.internal_clipboard=chosen.into_iter().cloned().collect();
        self.paste_number=0;
        self.status=format!("Скопировано {} объектов в буфер OpenCorel (Ctrl+V)",
            self.internal_clipboard.len());
        true
    }
    pub(super) fn cut_selection(&mut self){
        let selected=self.selection_ids();
        let editable:HashSet<u64>=self.doc.layers.iter()
            .filter(|layer|layer.visible&&!layer.locked).map(|l|l.id).collect();
        if selected.iter().any(|id|self.doc.objects.iter()
            .find(|obj|obj.id==*id)
            .map(|obj|!editable.contains(&obj.layer_id)).unwrap_or(true)){
            self.status="Нельзя вырезать объекты из заблокированных или скрытых слоёв".into();
            return;
        }
        if self.copy_selection(){
            self.delete_selection();
            self.status="Объекты вырезаны: Ctrl+V вставляет их обратно".into();
        }
    }
    pub(super) fn paste_selection(&mut self){
        if self.internal_clipboard.is_empty(){
            self.status="Буфер OpenCorel пуст. Сначала нажмите Ctrl+C".into();return;
        }
        if !self.active_layer_editable(){
            self.status="Текущий слой скрыт или заблокирован".into();return;
        }
        if self.doc.objects.len()+self.internal_clipboard.len()>100_000{
            self.status="Лимит 100 000 объектов превышен".into();return;
        }
        let copied=self.internal_clipboard.clone();
        let offset=10.0*(self.paste_number+1).min(100) as f32;
        self.checkpoint();
        let ids:Vec<u64>=copied.iter()
            .map(|obj|self.doc.copy_object(obj,self.active_layer,offset,offset)).collect();
        remap_groups(&mut self.doc,&copied,&ids);
        self.selected=ids.first().copied();
        self.multi_selected=ids.into_iter().skip(1).collect();
        self.paste_number=self.paste_number.saturating_add(1);
        self.status=format!("Вставлено {} объектов, с сохранением шрифтов и стилей",
            self.selection_ids().len());
    }
    pub(super) fn duplicate_preserving_styles(&mut self){
        let ids:HashSet<u64>=self.selection_ids().into_iter().collect();
        let editable:HashSet<u64>=self.doc.layers.iter()
            .filter(|l|l.visible&&!l.locked).map(|l|l.id).collect();
        let copies:Vec<_>=self.doc.objects.iter().filter(|obj|
            ids.contains(&obj.id)&&editable.contains(&obj.layer_id)).cloned().collect();
        if copies.is_empty(){
            self.status="Выделите редактируемые объекты для дублирования".into();return;
        }
        if self.doc.objects.len()+copies.len()>100_000{
            self.status="Лимит 100 000 объектов превышен".into();return;
        }
        self.checkpoint();
        let new_ids:Vec<_>=copies.iter().map(|o|self.doc.copy_object(o,o.layer_id,10.0,10.0)).collect();
        remap_groups(&mut self.doc,&copies,&new_ids);
        self.selected=new_ids.first().copied();
        self.multi_selected=new_ids.into_iter().skip(1).collect();
        self.status=format!("Создано копий: {}",self.selection_ids().len());
    }
    pub(super) fn group_selection(&mut self){
        let ids=self.selection_ids();
        if ids.len()<2{
            self.status="Для группировки выделите минимум два объекта с Shift".into();return;
        }
        let selected:HashSet<u64>=ids.iter().copied().collect();
        let editable:HashSet<u64>=self.doc.layers.iter()
            .filter(|layer|layer.visible&&!layer.locked).map(|layer|layer.id).collect();
        if self.doc.objects.iter().filter(|o|selected.contains(&o.id)).count()!=selected.len()
            || self.doc.objects.iter().filter(|o|selected.contains(&o.id))
            .any(|o|o.group_id.is_some()||!editable.contains(&o.layer_id)){
            self.status="Сначала разгруппируйте прежние группы и разблокируйте слои".into();
            return;
        }
        self.checkpoint();
        match self.doc.group_objects(&ids){
            Ok(group)=>self.status=format!("Группа #{group} создана ({} объектов). Ctrl+Shift+G — разгруппировать",ids.len()),
            Err(e)=>self.status=e,
        }
    }
    pub(super) fn ungroup_selection(&mut self){
        let ids=self.selection_ids();
        let groups:HashSet<u64>=self.doc.objects.iter()
            .filter(|obj|ids.contains(&obj.id)).filter_map(|obj|obj.group_id).collect();
        if groups.is_empty(){self.status="В выделении нет групп".into();return;}
        let editable:HashSet<u64>=self.doc.layers.iter()
            .filter(|l|l.visible&&!l.locked).map(|l|l.id).collect();
        if self.doc.objects.iter().any(|o|o.group_id
            .map(|g|groups.contains(&g)).unwrap_or(false)
            && !editable.contains(&o.layer_id)){
            self.status="Группа содержит объект из заблокированного слоя".into();return;
        }
        self.checkpoint();
        match self.doc.ungroup_objects(&ids){
            Ok(count)=>self.status=format!("Разгруппировано объектов: {count}"),
            Err(e)=>self.status=e,
        }
    }
    pub(super) fn selection_bounds(&self)->Option<Bounds>{
        let ids:HashSet<u64>=self.selection_ids().into_iter().collect();
        let mut b:Option<Bounds>=None;
        for obj in self.doc.objects.iter().filter(|o|ids.contains(&o.id)){
            let r=obj.bounds();
            b=Some(match b{
                None=>r,
                Some(old)=>{
                    let left=old.x.min(r.x);let top=old.y.min(r.y);
                    let right=(old.x+old.w).max(r.x+r.w);
                    let bottom=(old.y+old.h).max(r.y+r.h);
                    Bounds{x:left,y:top,w:right-left,h:bottom-top}
                }
            });
        }
        b
    }
    /// Move every selected member together; never partially move a locked group.
    pub(super) fn center_selection_on_page(&mut self){
        if let Some(bounds)=self.selection_bounds(){
            self.set_selection_position(
                (self.doc.page.width_mm-bounds.w)*0.5,
                (self.doc.page.height_mm-bounds.h)*0.5,
            );
            self.status="Выделение центрировано относительно листа".into();
        }else{
            self.status="Сначала выделите объекты для центрирования".into();
        }
    }
    pub(super) fn set_selection_position(&mut self,new_x:f32,new_y:f32){
        if !new_x.is_finite()||!new_y.is_finite()||
            new_x.abs()>100_000.0||new_y.abs()>100_000.0{return;}
        let Some(bounds)=self.selection_bounds() else{return;};
        let ids:HashSet<u64>=self.selection_ids().into_iter().collect();
        let layers:HashSet<u64>=self.doc.layers.iter()
            .filter(|layer|layer.visible&&!layer.locked).map(|l|l.id).collect();
        if self.doc.objects.iter().any(|o|ids.contains(&o.id)&&!layers.contains(&o.layer_id)){
            self.status="Выделение включает заблокированный слой".into();return;
        }
        let dx=new_x-bounds.x;
        let dy=new_y-bounds.y;
        if dx.abs()<0.00001 && dy.abs()<0.00001{return;}
        self.checkpoint();
        for o in &mut self.doc.objects{
            if ids.contains(&o.id){o.shape.translate(dx,dy);}
        }
        self.fill_cache.entries.clear();
        self.compound_cache.clear();
    }
    pub(super) fn resize_single(&mut self,width:f32,height:f32){
        if !self.multi_selected.is_empty(){
            self.status="Изменять ширину/высоту можно у одного объекта".into();return;
        }
        let Some(id)=self.selected else{return;};
        let Some(index)=self.doc.objects.iter().position(|o|o.id==id) else{return;};
        let obj=&self.doc.objects[index];
        if !self.doc.layer(obj.layer_id).map(|layer|layer.visible&&!layer.locked)
            .unwrap_or(false){
            self.status="Слой заблокирован".into();return;
        }
        if !transform::resizable(&obj.shape){
            self.status="Для текста измените размер шрифта во вкладке свойств".into();return;
        }
        let b=obj.bounds();
        if (b.w-width).abs()<0.00001&&(b.h-height).abs()<0.00001{return;}
        let mut resized=obj.shape.clone();
        match transform::resize_shape(&mut resized,
            Bounds{x:b.x,y:b.y,w:width,h:height}){
            Ok(())=>{
                self.checkpoint();
                self.doc.objects[index].shape=resized;
                self.fill_cache.entries.clear();self.compound_cache.clear();
            }
            Err(e)=>self.status=e,
        }
    }
    pub(super) fn rotate_selection(&mut self,turn:QuarterTurn){
        self.transform_selection(|s,c|transform::rotate_90(s,c,turn),"Поворот на 90°");
    }
    pub(super) fn mirror_selection(&mut self,horizontal:bool){
        self.transform_selection(|s,c|transform::flip_shape(s,c,horizontal),
            if horizontal{"Зеркало по горизонтали"}else{"Зеркало по вертикали"});
    }
    pub(super) fn rotate_arbitrary(&mut self){
        let degrees=self.rotation_degrees;
        let pivot=self.transform_pivot();
        self.transform_selection_at(|shape,center|transform::rotate_shape(shape,center,degrees),
            "Поворот на произвольный угол",pivot);
    }
    pub(super) fn skew_arbitrary(&mut self){
        let (x,y)=(self.skew_x_degrees,self.skew_y_degrees);
        let pivot=self.transform_pivot();
        self.transform_selection_at(|shape,center|transform::skew_shape(shape,center,x,y),
            "Наклон",pivot);
    }
    pub(super) fn scale_selected(&mut self){
        let (x,y)=(self.scale_x_percent/100.0,self.scale_y_percent/100.0);
        let pivot=self.transform_pivot();
        self.transform_selection_at(|shape,center|transform::scale_shape(shape,center,x,y),
            "Масштабирование",pivot);
    }
    fn transform_pivot(&self)->Option<Point>{
        self.custom_pivot.then_some(Point{x:self.pivot_x,y:self.pivot_y})
    }
    fn transform_selection<F>(&mut self,transform_shape:F,label:&str)
    where F:FnMut(&mut Shape,Point)->Result<(),String>{
        self.transform_selection_at(transform_shape,label,None);
    }
    fn transform_selection_at<F>(&mut self,mut transform_shape:F,label:&str,
        pivot_override:Option<Point>)
    where F:FnMut(&mut Shape,Point)->Result<(),String>{
        let ids:HashSet<u64>=self.selection_ids().into_iter().collect();
        if ids.is_empty(){self.status="Сначала выберите объект".into();return;}
        let editable:HashSet<u64>=self.doc.layers.iter()
            .filter(|l|l.visible&&!l.locked).map(|l|l.id).collect();
        let Some(bounds)=self.selection_bounds() else{return;};
        let center=pivot_override.unwrap_or(Point{
            x:bounds.x+bounds.w*0.5,y:bounds.y+bounds.h*0.5
        });
        // Never deform part of a persistent group, even if selected IDs were
        // populated through a non-GUI route such as an automated action.
        if self.doc.objects.iter().any(|o|ids.contains(&o.id) &&
            o.group_id.is_some_and(|group|
                self.doc.objects.iter().any(|member|
                    member.group_id==Some(group)&&!ids.contains(&member.id)))){
            self.status="Выделена только часть группы. Выберите группу целиком".into();
            return;
        }
        let mut converted=Vec::new();
        for o in self.doc.objects.iter().filter(|o|ids.contains(&o.id)){
            if !editable.contains(&o.layer_id){
                self.status="Выделение содержит объекты из недоступных слоёв".into();return;
            }
            let mut shape=o.shape.clone();
            if let Err(e)=transform_shape(&mut shape,center){
                self.status=format!("{label}: {e}. Операция не выполнена.");return;
            }
            converted.push((o.id,shape));
        }
        if converted.is_empty(){return;}
        self.checkpoint();
        for (id,shape) in converted{
            if let Some(object)=self.doc.get_mut(id){object.shape=shape;}
        }
        self.fill_cache.entries.clear();
        self.compound_cache.clear();
        self.status=format!("{label} — выполнено");
    }
    pub(super) fn nudge_selection(&mut self,dx:f32,dy:f32){
        let ids:HashSet<u64>=self.selection_ids().into_iter().collect();
        let editable:HashSet<u64>=self.doc.layers.iter()
            .filter(|l|l.visible&&!l.locked).map(|l|l.id).collect();
        if self.doc.objects.iter().any(|o|
            ids.contains(&o.id)&&!editable.contains(&o.layer_id)){
            self.status="Сдвиг отменён: в выделении есть заблокированный объект".into();
            return;
        }
        if !self.doc.objects.iter().any(|o|ids.contains(&o.id)){return;}
        self.checkpoint();
        for o in self.doc.objects.iter_mut().filter(|o|
            ids.contains(&o.id)){
            o.shape.translate(dx,dy);
        }
        self.fill_cache.entries.clear();
        self.compound_cache.clear();
        self.status=format!("Сдвиг: X {dx:+.1} мм, Y {dy:+.1} мм");
    }
    pub(super) fn blank_page(&mut self,width:f32,height:f32){
        self.new_document();
        self.doc.name="Новый дизайн".into();
        self.doc.page.width_mm=width;
        self.doc.page.height_mm=height;
        self.file="Новый дизайн.kdr".into();
        self.show_welcome=false;
        self.status=format!("Пустой документ: {width:.0} × {height:.0} мм. Выберите инструмент слева.");
    }
    pub(super) fn sample_design(&mut self){
        self.new_document();
        self.doc.name="Учебный рекламный макет".into();
        self.doc.page.width_mm=420.0;
        self.doc.page.height_mm=297.0;
        self.file="Учебный макет.kdr".into();
        self.doc.layers[0].name="Фон".into();
        let front=self.doc.add_layer("Надписи и элементы".into());
        self.doc.add(Shape::Rectangle{x:0.,y:0.,w:420.,h:297.},
            plain("#152339"));
        self.doc.add(Shape::Rectangle{x:0.,y:0.,w:14.,h:297.},
            plain("#1DC5C0"));
        self.doc.add(Shape::Rectangle{x:267.,y:35.,w:126.,h:226.},
            plain("#253E5C"));
        self.doc.add(Shape::Rectangle{x:30.,y:198.,w:212.,h:68.},
            plain("#1F3350"));
        self.doc.add_to_layer(Shape::Rectangle{x:30.,y:28.,w:115.,h:2.},
            plain("#1DC5C0"),front);
        for (x,y,r,col) in [
            (283.,57.,30.,"#1DC5C0"),
            (329.,102.,42.,"#F4B852"),
            (287.,177.,36.,"#7195F7")
        ]{
            self.doc.add_to_layer(Shape::Ellipse{x,y,w:r,h:r},plain(col),front);
        }
        let texts=[
            (30.,39.,11.,"OPENCOREL / УЧЕБНЫЙ МАКЕТ","#B4E8E7",700),
            (30.,100.,27.,"СОЗДАВАЙТЕ","#FFFFFF",700),
            (30.,139.,27.,"СВОЙ ДИЗАЙН","#FFFFFF",700),
            (35.,209.,7.,"1. Выделите текст или фигуру","#FFFFFF",400),
            (35.,226.,7.,"2. Измените цвет в палитре","#FFFFFF",400),
            (35.,243.,7.,"3. Сохраните макет в KDR","#FFFFFF",400),
            (30.,278.,6.,"ПРИМЕР: все элементы можно менять по отдельности","#AAC0D7",400),
        ];
        for (x,y,size,value,fill,weight) in texts{
            let id=self.doc.add_to_layer(Shape::Text{
                x,y,value:value.into(),size_mm:size
            },plain(fill),front);
            self.doc.get_mut(id).unwrap().text_format.weight=weight;
        }
        self.active_layer=front;
        self.inspector_tab=super::workspace::InspectorTab::Objects;
        self.show_welcome=false;
        self.status="Учебный макет открыт. Выделяйте объекты, меняйте текст, цвет и размер. F1 — инструкция.".into();
    }
    pub(super) fn welcome_window(&mut self,ctx:&egui::Context){
        if !self.show_welcome{return;}
        let mut opened=self.show_welcome;
        let mut action=0;
        egui::Window::new("Добро пожаловать в OpenCorel")
            .open(&mut opened)
            .collapsible(false)
            .resizable(true)
            .default_width(540.0)
            .show(ctx,|ui|{
                ui.heading("С чего начать?");
                ui.label("OpenCorel — редактор рекламных и векторных макетов. Здесь вы можете создавать фигуры, добавлять надписи и фотографии, менять размеры, цвета и сохранять результат.");
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui|{
                    if ui.button("Открыть пример макета").clicked(){action=1;}
                    if ui.button("Пустой A4").clicked(){action=2;}
                    if ui.button("Баннер 3 × 2 м").clicked(){action=3;}
                    if ui.button("Открыть файл...").clicked(){action=4;}
                });
                ui.label(RichText::new("Быстрый старт из шаблона").strong());
                ui.horizontal_wrapped(|ui|{
                    if ui.button("Визитка").clicked(){action=6;}
                    if ui.button("Листовка A5").clicked(){action=7;}
                    if ui.button("Постер").clicked(){action=8;}
                    if ui.button("24 этикетки A4").clicked(){action=9;}
                });
                ui.separator();
                ui.label(RichText::new("Быстрая инструкция").strong());
                ui.label("1. Выберите слева «Прямоугольник», «Эллипс» или «Текст».");
                ui.label("2. Щёлкните или протяните мышь на странице.");
                ui.label("3. Выберите «Выделение», щёлкните на объекте и перетащите его.");
                ui.label("4. Меняйте заливку палитрой, а размер и текст — в панели справа.");
                ui.label("5. Сохраните .kdr через Ctrl+S или «Сохранить как…».");
                ui.label("6. Для объединения или вычитания фигур выберите две замкнутые фигуры с Shift и откройте меню «Контуры».");
                ui.label("7. Для правки узлов кривой используйте F10; Ctrl+Z отменяет изменения.");
                ui.separator();
                ui.collapsing("Основные горячие клавиши",|ui|{
                    for (key,action) in [
                        ("Ctrl+N","Новый документ"),("Ctrl+O","Открыть"),
                        ("Ctrl+S","Сохранить"),("Ctrl+C/X/V","Копировать / вырезать / вставить (внутри OpenCorel)"),
                        ("Ctrl+D","Дублировать"),("Ctrl+A","Выделить всё"),
                        ("Ctrl+Z / Ctrl+Y","Отменить / повторить"),
                        ("Стрелки / Shift+стрелки","Сдвиг на 1 / 10 мм"),
                        ("Del","Удалить"),("F4","Страница целиком"),
                        ("F6 / F7 / F8","Прямоугольник / эллипс / текст"),
                        ("F10","Редактировать узлы выбранной кривой"),
                        ("L / P / B","Линия / шестиугольник / кривая"),
                        ("F1","Эта инструкция"),("Esc","Вернуться к выделению")
                    ]{
                        ui.horizontal(|ui|{
                            ui.label(RichText::new(key).strong());
                            ui.label(action);
                        });
                    }
                });
                ui.collapsing("Что пока не поддерживается",|ui|{
                    ui.label("CDR импортируется экспериментально, экспорт в CDR отсутствует. Профессиональный CMYK, PDF/X, многостраничность и полная совместимость с CorelDRAW пока не готовы.");
                    ui.label("Внутренний буфер Ctrl+C/X/V работает между объектами этого редактора, не между разными приложениями.");
                });
                ui.add_space(5.0);
                if ui.button("Продолжить работу").clicked(){action=5;}
            });
        self.show_welcome=opened;
        match action{
            1=>self.sample_design(),
            2=>self.blank_page(210.0,297.0),
            3=>self.blank_page(3000.0,2000.0),
            4=>{self.show_welcome=false;self.open_picker();},
            5=>self.show_welcome=false,
            6=>self.create_starter_template(super::starter_templates::StarterTemplate::BusinessCard),
            7=>self.create_starter_template(super::starter_templates::StarterTemplate::FlyerA5),
            8=>self.create_starter_template(super::starter_templates::StarterTemplate::SocialSquare),
            9=>self.create_starter_template(super::starter_templates::StarterTemplate::LabelSheetA4),
            _=>{},
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn centered_group_preserves_relative_geometry(){
        let mut e=Editor::default();
        e.select_all();
        let before=e.doc.objects[1].bounds().x-e.doc.objects[0].bounds().x;
        e.center_selection_on_page();
        let after=e.doc.objects[1].bounds().x-e.doc.objects[0].bounds().x;
        assert!((before-after).abs()<1e-4);
    }
    #[test]
    fn exact_move_and_resize_preserve_geometry(){
        let mut e=Editor::default();
        let id=e.doc.objects[0].id;
        e.select_object(Some(id),false);
        e.set_selection_position(20.0,30.0);
        assert!((e.doc.objects[0].bounds().x-20.0).abs()<0.01);
        e.resize_single(100.0,200.0);
        assert!((e.doc.objects[0].bounds().w-100.0).abs()<0.01);
        e.undo();
        assert!((e.doc.objects[0].bounds().w-700.0).abs()<0.01);
    }
    #[test]
    fn group_duplicate_creates_independent_linked_copies(){
        let mut e=Editor::default();
        e.select_all();
        e.group_selection();
        assert!(e.doc.objects[0].group_id.is_some());
        e.duplicate_preserving_styles();
        assert_eq!(e.doc.objects.len(),4);
        let original=e.doc.objects[0].group_id;
        let duplicate=e.doc.objects[2].group_id;
        assert_ne!(original,duplicate);
        assert_eq!(duplicate,e.doc.objects[3].group_id);
        e.select_object(Some(e.doc.objects[3].id),false);
        assert_eq!(e.selection_ids().len(),2);
        e.ungroup_selection();
        assert!(e.doc.objects[2].group_id.is_none());
        assert!(e.doc.objects[0].group_id.is_some());
        e.doc.validate().unwrap();
    }
    #[test]
    fn clipboard_retains_text_format_and_undo(){
        let mut editor=Editor::default();
        let id=editor.doc.objects[1].id;
        editor.select_object(Some(id),false);
        editor.doc.get_mut(id).unwrap().text_format.family="Arial".into();
        assert!(editor.copy_selection());
        editor.paste_selection();
        let copied=editor.doc.objects.last().unwrap();
        assert_eq!(copied.text_format.family,"Arial");
        assert_ne!(copied.id,id);
        editor.undo();
        assert_eq!(editor.doc.objects.len(),2);
    }
    #[test]
    fn sample_is_a_valid_editable_design(){
        let mut editor=Editor::default();
        editor.sample_design();
        assert!(editor.doc.objects.len()>10);
        editor.doc.validate().unwrap();
        assert_eq!(editor.doc.page.width_mm,420.0);
        assert!(editor.doc.layers.len()>=2);
    }
    #[test]
    fn copy_limit_refuses_huge_binary_payload(){
        let mut e=Editor::default();
        let id=e.doc.add(Shape::EmbeddedSvg{x:0.,y:0.,w:20.,h:20.,
            svg:"x".repeat(CLIPBOARD_LIMIT+1),source_format:"SVG".into()},plain("none"));
        e.select_object(Some(id),false);
        assert!(!e.copy_selection());
        assert!(e.internal_clipboard.is_empty());
    }
    #[test]
    fn rotate_atomic_when_selection_has_text(){
        let mut e=Editor::default();
        e.select_all();
        let before=e.doc.clone();
        e.rotate_selection(QuarterTurn::Clockwise);
        assert_eq!(e.doc,before);
        assert!(e.undo.is_empty());
    }
}
