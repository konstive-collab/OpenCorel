use eframe::egui::{self,Align2,Color32,FontId,Pos2,Rect,Sense,Stroke,Vec2};
use opencorel_core::{Bounds,CubicSegment,Document,Object,Point,Shape,Style,valid_color};
use std::f32::consts::TAU;

#[derive(Clone,Copy,PartialEq)]
enum Tool { Select, Rect, Ellipse, Text, Curve }

struct Editor{
    doc:Document,
    selected:Option<u64>,
    tool:Tool,
    file:String,
    text:String,
    status:String,
    zoom:f32,
    drag_start:Option<(Point,Shape,u64)>,
    creation_start:Option<(Point,u64)>,
    curve_start:Option<Point>,
    curve_draft:Option<u64>,
    active_layer:u64,
    pan:Vec2,
    undo:Vec<Document>,
    redo:Vec<Document>,
}
impl Default for Editor{
    fn default()->Self{
        let mut doc=Document::default();
        doc.name="Рекламный макет".into();
        doc.add(Shape::Rectangle{x:70.0,y:110.0,w:700.0,h:350.0},
            Style{fill:"#244A87".into(),stroke:"#244A87".into(),stroke_width_mm:0.0});
        doc.add(Shape::Text{x:120.0,y:200.0,value:"ВАША РЕКЛАМА".into(),size_mm:42.0},
            Style{fill:"#FFFFFF".into(),stroke:"#FFFFFF".into(),stroke_width_mm:0.0});
        Self{doc,selected:None,tool:Tool::Select,file:"project.ocd".into(),
            text:"Новый текст".into(),status:"Экспериментальный прототип".into(),
            zoom:1.0,drag_start:None,creation_start:None,
            curve_start:None,curve_draft:None,active_layer:1,pan:Vec2::ZERO,
            undo:Vec::new(),redo:Vec::new()}
    }
}
#[derive(Clone,Copy)]
struct View {rect:Rect,scale:f32}
impl View{
    fn new(canvas:Rect,doc:&Document,zoom:f32,pan:Vec2)->Self{
        let scale=((canvas.width()-40.0)/doc.page.width_mm)
            .min((canvas.height()-40.0)/doc.page.height_mm).max(0.001)*zoom;
        let rect=Rect::from_center_size(canvas.center()+pan,
            Vec2::new(doc.page.width_mm*scale,doc.page.height_mm*scale));
        Self{rect,scale}
    }
    fn screen(self,p:Point)->Pos2{
        self.rect.min+Vec2::new(p.x*self.scale,p.y*self.scale)
    }
    fn mm(self,p:Pos2)->Point{
        let d=p-self.rect.min;Point{x:d.x/self.scale,y:d.y/self.scale}
    }
    fn bounds(self,b:Bounds)->Rect{
        Rect::from_min_size(self.screen(Point{x:b.x,y:b.y}),
            Vec2::new(b.w*self.scale,b.h*self.scale))
    }
}
fn color(s:&str)->Color32{
    if !valid_color(s){return Color32::GRAY;}
    let c=|a,b|u8::from_str_radix(&s[a..b],16).unwrap_or(0);
    Color32::from_rgb(c(1,3),c(3,5),c(5,7))
}
fn outline(p:&egui::Painter,r:Rect,c:Color32,width:f32){
    for (a,b) in [(r.left_top(),r.right_top()),(r.right_top(),r.right_bottom()),
        (r.right_bottom(),r.left_bottom()),(r.left_bottom(),r.left_top())]{
        p.line_segment([a,b],Stroke::new(width,c));
    }
}
fn cubic(p0:Point,p1:Point,p2:Point,p3:Point,t:f32)->Point {
    let u=1.0-t;
    let a=u*u*u;
    let b=3.0*u*u*t;
    let c=3.0*u*t*t;
    let d=t*t*t;
    Point{x:a*p0.x+b*p1.x+c*p2.x+d*p3.x,y:a*p0.y+b*p1.y+c*p2.y+d*p3.y}
}
fn paint(p:&egui::Painter,obj:&Object,v:View){
    let fill=color(&obj.style.fill);
    let stroke=Stroke::new(obj.style.stroke_width_mm*v.scale,color(&obj.style.stroke));
    match &obj.shape{
        Shape::Rectangle{x,y,w,h}=>{
            let r=v.bounds(Bounds{x:*x,y:*y,w:*w,h:*h});
            p.rect_filled(r,0.0,fill);
            if stroke.width>0.0{outline(p,r,stroke.color,stroke.width);}
        }
        Shape::Ellipse{x,y,w,h}=>{
            let center=v.screen(Point{x:*x+*w/2.0,y:*y+*h/2.0});
            let mut vertices=Vec::with_capacity(64);
            for i in 0..64{
                let a=(i as f32)/64.0*TAU;
                vertices.push(center+Vec2::new(*w*v.scale*a.cos()/2.0,*h*v.scale*a.sin()/2.0));
            }
            p.add(egui::Shape::convex_polygon(vertices,fill,stroke));
        }
        Shape::Text{x,y,value,size_mm}=>{
            p.text(v.screen(Point{x:*x,y:*y}),Align2::LEFT_TOP,value,
                FontId::proportional((size_mm*v.scale).clamp(1.0,400.0)),fill);
        }
        Shape::Bezier{start,segments,closed}=>{
            let mut vertices=vec![v.screen(*start)];
            let mut previous=*start;
            for segment in segments {
                for i in 1..=32 {
                    let t=(i as f32)/32.0;
                    vertices.push(v.screen(cubic(previous,segment.control1,segment.control2,segment.end,t)));
                }
                previous=segment.end;
            }
            if *closed{vertices.push(v.screen(*start));}
            if vertices.len()>1 {
                p.add(egui::Shape::line(vertices,Stroke::new(stroke.width.max(1.0),stroke.color)));
            }
        }
        Shape::Polyline{points,closed}=>{
            let mut ps:Vec<Pos2>=points.iter().map(|pt|v.screen(*pt)).collect();
            if *closed && !ps.is_empty(){ps.push(ps[0]);}
            if ps.len()>1{p.add(egui::Shape::line(ps,stroke));}
        }
    }
}
impl Editor{
    fn checkpoint(&mut self){
        self.undo.push(self.doc.clone());
        if self.undo.len()>50{self.undo.remove(0);}
        self.redo.clear();
    }
    fn undo(&mut self){
        if let Some(prev)=self.undo.pop(){
            self.redo.push(std::mem::replace(&mut self.doc,prev));
            self.selected=None;self.curve_draft=None;self.curve_start=None;
            self.status="Отменено".into();
        }
    }
    fn redo(&mut self){
        if let Some(next)=self.redo.pop(){
            self.undo.push(std::mem::replace(&mut self.doc,next));
            self.selected=None;self.curve_draft=None;self.curve_start=None;
            self.status="Повторено".into();
        }
    }
    fn active_layer_editable(&self)->bool {
        self.doc.layer(self.active_layer).map(|l|l.visible&&!l.locked).unwrap_or(false)
    }
    fn add_shape(&mut self,shape:Shape,style:Style)->Option<u64>{
        if !self.active_layer_editable(){
            self.status="Слой скрыт или заблокирован".into();
            return None;
        }
        self.checkpoint();
        let id=self.doc.add_to_layer(shape,style,self.active_layer);
        self.selected=Some(id);
        Some(id)
    }
    fn curve_click(&mut self,p:Point){
        if !self.active_layer_editable(){
            self.status="Слой скрыт или заблокирован".into();return;
        }
        let Some(start)=self.curve_start else {
            self.curve_start=Some(p);
            self.status="Начало кривой: укажите следующую точку".into();
            return;
        };
        let previous=match self.curve_draft.and_then(|id|self.doc.objects.iter().find(|o|o.id==id)){
            Some(Object{shape:Shape::Bezier{segments,..},..})=>
                segments.last().map(|seg|seg.end).unwrap_or(start),
            _=>start,
        };
        let seg=CubicSegment{
            control1:Point{x:previous.x+(p.x-previous.x)/3.0,y:previous.y+(p.y-previous.y)/3.0},
            control2:Point{x:previous.x+2.0*(p.x-previous.x)/3.0,
                           y:previous.y+2.0*(p.y-previous.y)/3.0},
            end:p
        };
        if let Some(id)=self.curve_draft{
            self.checkpoint();
            if let Some(obj)=self.doc.get_mut(id){
                if let Shape::Bezier{segments,..}=&mut obj.shape{segments.push(seg);}
            }
        }else{
            let style=Style{fill:"#000000".into(),stroke:"#202020".into(),stroke_width_mm:0.8};
            self.curve_draft=self.add_shape(Shape::Bezier{start,segments:vec![seg],closed:false},style);
        }
        self.status="Точка добавлена; двойной щелчок завершает кривую".into();
    }
    fn save(&mut self){
        self.status=match self.doc.write(&self.file){
            Ok(())=>format!("Сохранено: {}",self.file),
            Err(e)=>format!("Ошибка сохранения: {e}"),
        };
    }
    fn open(&mut self){
        match Document::read(&self.file){
            Ok(d)=>{self.doc=d;self.selected=None;self.active_layer=1;
                self.undo.clear();self.redo.clear();self.curve_start=None;self.curve_draft=None;
                self.status="Открыто".into();},
            Err(e)=>self.status=format!("Ошибка открытия: {e}"),
        }
    }
    fn svg(&mut self){
        let path=std::path::Path::new(&self.file).with_extension("svg");
        self.status=match self.doc.validate(){
            Ok(())=>match self.doc.export_svg(&path){
                Ok(())=>format!("Экспортировано: {}",path.display()),
                Err(e)=>format!("Ошибка экспорта: {e}"),
            },
            Err(e)=>format!("Неверный документ: {e}"),
        }
    }
    fn toolbar(&mut self,ctx:&egui::Context){
        egui::TopBottomPanel::top("top").show(ctx,|ui|{
            ui.horizontal_wrapped(|ui|{
                ui.heading("OpenCorel");
                ui.separator();
                if ui.button("Новый").clicked(){self.checkpoint();self.doc=Document::default();self.selected=None;
                    self.active_layer=1;self.curve_start=None;self.curve_draft=None;}
                if ui.button("Открыть").clicked(){self.open();}
                if ui.button("Сохранить").clicked(){self.save();}
                if ui.button("Экспорт SVG").clicked(){self.svg();}
                ui.separator();
                if ui.add_enabled(!self.undo.is_empty(),egui::Button::new("↶ Отменить")).clicked(){self.undo();}
                if ui.add_enabled(!self.redo.is_empty(),egui::Button::new("↷ Повторить")).clicked(){self.redo();}
                ui.separator();
                ui.label("Файл:");
                ui.add(egui::TextEdit::singleline(&mut self.file).desired_width(200.0));
                ui.label(format!("{:.0} × {:.0} мм",self.doc.page.width_mm,self.doc.page.height_mm));
                if ui.button("−").clicked(){self.zoom=(self.zoom/1.25).max(0.2);}
                ui.label(format!("{}%",(self.zoom*100.0) as i32));
                if ui.button("+").clicked(){self.zoom=(self.zoom*1.25).min(8.0);}
            });
        });
        egui::TopBottomPanel::bottom("status").show(ctx,|ui|{
            ui.label(format!("{} | Объектов: {}",self.status,self.doc.objects.len()));
        });
    }
    fn sidebars(&mut self,ctx:&egui::Context){
        egui::SidePanel::left("tools").default_width(160.0).show(ctx,|ui|{
            ui.heading("Инструменты");ui.separator();
            ui.selectable_value(&mut self.tool,Tool::Select,"Выделение");
            ui.selectable_value(&mut self.tool,Tool::Rect,"Прямоугольник");
            ui.selectable_value(&mut self.tool,Tool::Ellipse,"Эллипс");
            ui.selectable_value(&mut self.tool,Tool::Text,"Текст");
            if ui.selectable_value(&mut self.tool,Tool::Curve,"Кривая Безье").clicked(){
                self.curve_start=None;self.curve_draft=None;
            }
            if self.tool==Tool::Curve{ui.small("Точки: щелчок. Завершить: двойной щелчок / Esc");}
            if self.tool==Tool::Text{ui.text_edit_multiline(&mut self.text);}
            ui.separator();ui.heading("Страница");
            let mut w=self.doc.page.width_mm;
            let mut h=self.doc.page.height_mm;
            if ui.add(egui::DragValue::new(&mut w).range(1.0..=100_000.0).suffix(" мм")).changed(){
                self.checkpoint();self.doc.page.width_mm=w;
            }
            if ui.add(egui::DragValue::new(&mut h).range(1.0..=100_000.0).suffix(" мм")).changed(){
                self.checkpoint();self.doc.page.height_mm=h;
            }
            if ui.button("Баннер 3×2 м").clicked(){
                self.checkpoint();
                self.doc.page.width_mm=3000.0;self.doc.page.height_mm=2000.0;
            }
            ui.separator();
            ui.heading("Слои");
            let current=self.doc.layer(self.active_layer).map(|l|l.name.clone()).unwrap_or_default();
            egui::ComboBox::from_id_salt("active_layer").selected_text(current).show_ui(ui,|ui|{
                for layer in &self.doc.layers{
                    ui.selectable_value(&mut self.active_layer,layer.id,&layer.name);
                }
            });
            if ui.button("+ Добавить слой").clicked(){
                self.checkpoint();
                let name=format!("Слой {}",self.doc.layers.len()+1);
                self.active_layer=self.doc.add_layer(name);
            }
            let ids:Vec<u64>=self.doc.layers.iter().map(|l|l.id).collect();
            for id in ids{
                let Some(layer)=self.doc.layer(id).cloned() else {continue;};
                let mut visible=layer.visible;
                let mut locked=layer.locked;
                ui.horizontal(|ui|{
                    ui.label(&layer.name);
                    if ui.checkbox(&mut visible,"Вид.").changed() || ui.checkbox(&mut locked,"Зам.").changed(){
                        self.checkpoint();
                        if let Some(l)=self.doc.layers.iter_mut().find(|l|l.id==id){
                            l.visible=visible;l.locked=locked;
                        }
                    }
                });
            }
        });
        egui::SidePanel::right("properties").default_width(245.0).show(ctx,|ui|{
            ui.heading("Свойства");
            let Some(id)=self.selected else {ui.label("Выберите объект");return;};
            let Some(index)=self.doc.objects.iter().position(|o|o.id==id) else {
                self.selected=None;return;
            };
            let mut obj=self.doc.objects[index].clone();
            let mut changed=false;
            match &mut obj.shape{
                Shape::Rectangle{x,y,w,h}|Shape::Ellipse{x,y,w,h}=>{
                    for (label,value) in [("X",x),("Y",y),("Ширина",w),("Высота",h)] {
                        ui.horizontal(|ui|{ui.label(label);changed|=ui.add(egui::DragValue::new(value).speed(1.0)).changed();});
                    }
                }
                Shape::Text{x,y,value,size_mm}=>{
                    changed|=ui.text_edit_multiline(value).changed();
                    for (label,val) in [("X",x),("Y",y),("Размер",size_mm)] {
                        ui.horizontal(|ui|{ui.label(label);changed|=ui.add(egui::DragValue::new(val).speed(1.0)).changed();});
                    }
                }
                Shape::Polyline{points,..}=>{ui.label(format!("Узлов: {}",points.len()));}
                Shape::Bezier{start,segments,closed}=>{
                    ui.label(format!("Узлов кривой: {}",segments.len()+1));
                    changed|=ui.checkbox(closed,"Замкнуть кривую").changed();
                    changed|=ui.add(egui::DragValue::new(&mut start.x).prefix("X: ")).changed();
                    changed|=ui.add(egui::DragValue::new(&mut start.y).prefix("Y: ")).changed();
                    for (i,seg) in segments.iter_mut().enumerate(){
                        ui.collapsing(format!("Сегмент {}",i+1),|ui|{
                            for (label,point) in [("Ручка 1",&mut seg.control1),
                                                  ("Ручка 2",&mut seg.control2),("Конец",&mut seg.end)] {
                                ui.label(label);
                                changed|=ui.add(egui::DragValue::new(&mut point.x).prefix("X: ")).changed();
                                changed|=ui.add(egui::DragValue::new(&mut point.y).prefix("Y: ")).changed();
                            }
                        });
                    }
                }
            }
            ui.label("Заливка #RRGGBB");
            changed|=ui.text_edit_singleline(&mut obj.style.fill).changed();
            ui.label("Контур #RRGGBB");
            changed|=ui.text_edit_singleline(&mut obj.style.stroke).changed();
            changed|=ui.add(egui::DragValue::new(&mut obj.style.stroke_width_mm).speed(0.1).range(0.0..=1000.0)).changed();
            if changed{
                if valid_color(&obj.style.fill)&&valid_color(&obj.style.stroke){
                    self.checkpoint();self.doc.objects[index]=obj;
                }else{
                    self.status="Цвет должен быть в формате #RRGGBB".into();
                }
            }
            if ui.button("Удалить объект").clicked(){
                self.checkpoint();self.doc.remove(id);self.selected=None;
            }
        });
    }
    fn canvas(&mut self,ctx:&egui::Context){
        egui::CentralPanel::default().show(ctx,|ui|{
            let response=ui.allocate_response(ui.available_size(),Sense::click_and_drag());
            let painter=ui.painter_at(response.rect);
            painter.rect_filled(response.rect,0.0,Color32::from_rgb(37,41,49));
            if response.hovered(){
                let scroll=ctx.input(|i|i.smooth_scroll_delta.y);
                if scroll.abs()>0.01 {
                    self.zoom=(self.zoom*(1.0+scroll*0.002).clamp(0.8,1.25)).clamp(0.2,8.0);
                }
            }
            if response.dragged_by(egui::PointerButton::Middle){
                self.pan+=ctx.input(|i|i.pointer.delta());
            }
            let v=View::new(response.rect,&self.doc,self.zoom,self.pan);
            painter.rect_filled(v.rect,0.0,Color32::WHITE);
            outline(&painter,v.rect,Color32::GRAY,1.0);
            let page_painter=painter.with_clip_rect(v.rect.intersect(response.rect));
            for obj in self.doc.visible_objects(){
                if v.bounds(obj.shape.bounds()).intersects(v.rect.intersect(response.rect)){
                    paint(&page_painter,obj,v);
                }
            }
            if let Some(id)=self.selected{
                if let Some(o)=self.doc.objects.iter().find(|o|o.id==id){
                    if self.doc.layer(o.layer_id).map(|l|l.visible).unwrap_or(false){
                        outline(&page_painter,v.bounds(o.shape.bounds()).expand(3.0),Color32::BLUE,2.0);
                    }
                }
            }
            if response.drag_started_by(egui::PointerButton::Primary){
                if let Some(pos)=response.interact_pointer_pos(){
                    if v.rect.contains(pos){
                        let p=v.mm(pos);
                        match self.tool{
                            Tool::Select=>{
                                self.selected=self.doc.hit_test(p,2.0);
                                if let Some(id)=self.selected{
                                    if let Some(o)=self.doc.objects.iter().find(|o|o.id==id){
                                        let shape=o.shape.clone();
                                        self.checkpoint();
                                        self.drag_start=Some((p,shape,id));
                                    }
                                }
                            }
                            Tool::Rect|Tool::Ellipse=>{
                                let shape=if self.tool==Tool::Rect{
                                    Shape::Rectangle{x:p.x,y:p.y,w:1.0,h:1.0}
                                }else{
                                    Shape::Ellipse{x:p.x,y:p.y,w:1.0,h:1.0}
                                };
                                if let Some(id)=self.add_shape(shape,Style::default()){
                                    self.creation_start=Some((p,id));
                                }
                            }
                            _=>{}
                        }
                    }
                }
            }
            if response.dragged_by(egui::PointerButton::Primary){
                if let Some(pos)=response.interact_pointer_pos(){
                    let now=v.mm(pos);
                    if let Some((p,id))=self.creation_start{
                        let x=p.x.min(now.x);let y=p.y.min(now.y);
                        let w=(p.x-now.x).abs().max(0.1);
                        let h=(p.y-now.y).abs().max(0.1);
                        if let Some(obj)=self.doc.get_mut(id){
                            match &mut obj.shape{
                                Shape::Rectangle{x:rx,y:ry,w:rw,h:rh}|
                                Shape::Ellipse{x:rx,y:ry,w:rw,h:rh}=>{
                                    *rx=x;*ry=y;*rw=w;*rh=h;
                                }
                                _=>{}
                            }
                        }
                    }else if let Some((p,mut before,id))=self.drag_start.clone(){
                        before.translate(now.x-p.x,now.y-p.y);
                        if let Some(o)=self.doc.get_mut(id){o.shape=before;}
                    }
                }
            }
            if response.drag_stopped_by(egui::PointerButton::Primary){
                self.drag_start=None;self.creation_start=None;
            }
            if response.double_clicked(){
                if self.tool==Tool::Curve{
                    self.curve_start=None;self.curve_draft=None;
                    self.tool=Tool::Select;self.status="Кривая завершена".into();
                }
            }
            if response.clicked() && !response.double_clicked(){
                if let Some(pos)=response.interact_pointer_pos(){
                    if v.rect.contains(pos){
                        let p=v.mm(pos);
                        let shape=match self.tool{
                            Tool::Select=>{self.selected=self.doc.hit_test(p,2.0);return;},
                            Tool::Curve=>{self.curve_click(p);return;},
                            Tool::Rect=>Shape::Rectangle{x:p.x,y:p.y,w:60.0,h:40.0},
                            Tool::Ellipse=>Shape::Ellipse{x:p.x,y:p.y,w:60.0,h:40.0},
                            Tool::Text=>Shape::Text{x:p.x,y:p.y,value:self.text.clone(),size_mm:18.0},
                        };
                        let style=if self.tool==Tool::Text{
                            Style{fill:"#202020".into(),stroke:"#202020".into(),stroke_width_mm:0.0}
                        }else{Style::default()};
                        self.add_shape(shape,style);
                        if self.tool==Tool::Text{self.tool=Tool::Select;}
                    }
                }
            }
        });
    }
}
impl eframe::App for Editor{
    fn update(&mut self,ctx:&egui::Context,_:&mut eframe::Frame){
        let undo=ctx.input(|i|i.modifiers.command&&i.key_pressed(egui::Key::Z)&&!i.modifiers.shift);
        let redo=ctx.input(|i|i.modifiers.command&&(i.key_pressed(egui::Key::Y) ||
            (i.key_pressed(egui::Key::Z)&&i.modifiers.shift)));
        if undo{self.undo();}
        if redo{self.redo();}
        if ctx.input(|i|i.key_pressed(egui::Key::Escape)){
            self.curve_start=None;self.curve_draft=None;self.tool=Tool::Select;
        }
        self.toolbar(ctx);self.sidebars(ctx);self.canvas(ctx);
    }
}
fn main()->eframe::Result<()>{
    let options=eframe::NativeOptions{
        viewport:egui::ViewportBuilder::default().with_inner_size([1280.0,780.0])
            .with_min_inner_size([780.0,520.0]),
        ..Default::default()
    };
    eframe::run_native("OpenCorel — Vector Editor",options,Box::new(|cc|{
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        Ok(Box::new(Editor::default()))
    }))
}
