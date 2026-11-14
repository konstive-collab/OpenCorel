use eframe::egui::{self,Align2,Color32,FontId,Pos2,Rect,Sense,Stroke,Vec2};
use opencorel_core::{Bounds,Document,Object,Point,Shape,Style,valid_color};
use std::f32::consts::TAU;

#[derive(Clone,Copy,PartialEq)]
enum Tool { Select, Rect, Ellipse, Text }

struct Editor{
    doc:Document,
    selected:Option<u64>,
    tool:Tool,
    file:String,
    text:String,
    status:String,
    zoom:f32,
    drag_start:Option<(Point,Shape,u64)>,
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
            zoom:1.0,drag_start:None}
    }
}
#[derive(Clone,Copy)]
struct View {rect:Rect,scale:f32}
impl View{
    fn new(canvas:Rect,doc:&Document,zoom:f32)->Self{
        let scale=((canvas.width()-40.0)/doc.page.width_mm)
            .min((canvas.height()-40.0)/doc.page.height_mm).max(0.001)*zoom;
        let rect=Rect::from_center_size(canvas.center(),
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
        Shape::Polyline{points,closed}=>{
            let mut ps:Vec<Pos2>=points.iter().map(|pt|v.screen(*pt)).collect();
            if *closed && !ps.is_empty(){ps.push(ps[0]);}
            if ps.len()>1{p.add(egui::Shape::line(ps,stroke));}
        }
    }
}
impl Editor{
    fn save(&mut self){
        self.status=match self.doc.write(&self.file){
            Ok(())=>format!("Сохранено: {}",self.file),
            Err(e)=>format!("Ошибка сохранения: {e}"),
        };
    }
    fn open(&mut self){
        match Document::read(&self.file){
            Ok(d)=>{self.doc=d;self.selected=None;self.status="Открыто".into();},
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
                if ui.button("Новый").clicked(){self.doc=Document::default();self.selected=None;}
                if ui.button("Открыть").clicked(){self.open();}
                if ui.button("Сохранить").clicked(){self.save();}
                if ui.button("Экспорт SVG").clicked(){self.svg();}
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
            if self.tool==Tool::Text{ui.text_edit_multiline(&mut self.text);}
            ui.separator();ui.heading("Страница");
            let mut w=self.doc.page.width_mm;
            let mut h=self.doc.page.height_mm;
            if ui.add(egui::DragValue::new(&mut w).range(1.0..=100_000.0).suffix(" мм")).changed(){
                self.doc.page.width_mm=w;
            }
            if ui.add(egui::DragValue::new(&mut h).range(1.0..=100_000.0).suffix(" мм")).changed(){
                self.doc.page.height_mm=h;
            }
            if ui.button("Баннер 3×2 м").clicked(){
                self.doc.page.width_mm=3000.0;self.doc.page.height_mm=2000.0;
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
            }
            ui.label("Заливка #RRGGBB");
            changed|=ui.text_edit_singleline(&mut obj.style.fill).changed();
            ui.label("Контур #RRGGBB");
            changed|=ui.text_edit_singleline(&mut obj.style.stroke).changed();
            changed|=ui.add(egui::DragValue::new(&mut obj.style.stroke_width_mm).speed(0.1).range(0.0..=1000.0)).changed();
            if changed{
                if valid_color(&obj.style.fill)&&valid_color(&obj.style.stroke){
                    self.doc.objects[index]=obj;
                }else{
                    self.status="Цвет должен быть в формате #RRGGBB".into();
                }
            }
            if ui.button("Удалить объект").clicked(){
                self.doc.remove(id);self.selected=None;
            }
        });
    }
    fn canvas(&mut self,ctx:&egui::Context){
        egui::CentralPanel::default().show(ctx,|ui|{
            let response=ui.allocate_response(ui.available_size(),Sense::click_and_drag());
            let painter=ui.painter_at(response.rect);
            painter.rect_filled(response.rect,0.0,Color32::from_rgb(37,41,49));
            let v=View::new(response.rect,&self.doc,self.zoom);
            painter.rect_filled(v.rect,0.0,Color32::WHITE);
            outline(&painter,v.rect,Color32::GRAY,1.0);
            let page_painter=painter.with_clip_rect(v.rect.intersect(response.rect));
            for obj in &self.doc.objects {paint(&page_painter,obj,v);}
            if let Some(id)=self.selected{
                if let Some(o)=self.doc.objects.iter().find(|o|o.id==id){
                    outline(&page_painter,v.bounds(o.shape.bounds()).expand(3.0),Color32::BLUE,2.0);
                }
            }
            if response.drag_started(){
                if let Some(pos)=response.interact_pointer_pos(){
                    if v.rect.contains(pos){
                        let p=v.mm(pos);
                        self.selected=self.doc.hit_test(p,2.0);
                        if let Some(id)=self.selected{
                            if let Some(o)=self.doc.objects.iter().find(|o|o.id==id){
                                self.drag_start=Some((p,o.shape.clone(),id));
                            }
                        }
                    }
                }
            }
            if response.dragged(){
                if let (Some(pos),Some((p,mut before,id)))=(response.interact_pointer_pos(),self.drag_start.clone()){
                    let now=v.mm(pos);
                    before.translate(now.x-p.x,now.y-p.y);
                    if let Some(o)=self.doc.get_mut(id){o.shape=before;}
                }
            }
            if response.drag_stopped(){self.drag_start=None;}
            if response.clicked(){
                if let Some(pos)=response.interact_pointer_pos(){
                    if v.rect.contains(pos){
                        let p=v.mm(pos);
                        let shape=match self.tool{
                            Tool::Select=>{self.selected=self.doc.hit_test(p,2.0);return;},
                            Tool::Rect=>Shape::Rectangle{x:p.x,y:p.y,w:60.0,h:40.0},
                            Tool::Ellipse=>Shape::Ellipse{x:p.x,y:p.y,w:60.0,h:40.0},
                            Tool::Text=>Shape::Text{x:p.x,y:p.y,value:self.text.clone(),size_mm:18.0},
                        };
                        let style=if self.tool==Tool::Text{
                            Style{fill:"#202020".into(),stroke:"#202020".into(),stroke_width_mm:0.0}
                        }else{Style::default()};
                        self.selected=Some(self.doc.add(shape,style));
                        if self.tool==Tool::Text{self.tool=Tool::Select;}
                    }
                }
            }
        });
    }
}
impl eframe::App for Editor{
    fn update(&mut self,ctx:&egui::Context,_:&mut eframe::Frame){
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
