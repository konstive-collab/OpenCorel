use eframe::egui::{self,Align2,Color32,FontId,Pos2,Rect,Sense,Stroke,Vec2};
use opencorel_core::{Bounds,CubicSegment,Document,Object,Point,Shape,Style,valid_color,valid_paint,Alignment,Distribution};
use std::{f32::consts::TAU,collections::{HashMap,HashSet},hash::{Hash,Hasher},collections::hash_map::DefaultHasher};
mod workspace;
mod vector_preview;
mod raster_preview;
use vector_preview::VectorCache;
use raster_preview::RasterCache;
use workspace::InspectorTab;

#[derive(Clone,Copy,PartialEq)]
enum Tool { Select, Rect, Ellipse, Text, Curve }

#[derive(Default)]
struct FillCache { entries:HashMap<u64,(u64,Vec<usize>)> }
impl FillCache {
    fn fill(&mut self,p:&egui::Painter,id:u64,pts:&[Point],v:View,color:Color32) {
        if color.a()==0 || pts.len()<3{return;}
        let mut hash=DefaultHasher::new();
        pts.len().hash(&mut hash);
        for pt in pts{
            pt.x.to_bits().hash(&mut hash);
            pt.y.to_bits().hash(&mut hash);
        }
        let key=hash.finish();
        if self.entries.get(&id).map(|(old,_)|*old!=key).unwrap_or(true) {
            let xy:Vec<f64>=pts.iter().flat_map(|p|[p.x as f64,p.y as f64]).collect();
            let tri=earcutr::earcut(&xy,&[],2).unwrap_or_default();
            self.entries.insert(id,(key,tri));
        }
        if let Some((_,indices))=self.entries.get(&id) {
            if indices.is_empty(){return;}
            let mut mesh=egui::Mesh::default();
            mesh.reserve_vertices(pts.len());
            mesh.reserve_triangles(indices.len()/3);
            for &pt in pts{mesh.colored_vertex(v.screen(pt),color);}
            for tri in indices.chunks_exact(3){
                mesh.add_triangle(tri[0] as u32,tri[1] as u32,tri[2] as u32);
            }
            p.add(egui::Shape::mesh(mesh));
        }
    }
}
struct Editor{
    doc:Document,
    selected:Option<u64>,
    multi_selected:Vec<u64>,
    tool:Tool,
    file:String,
    text:String,
    status:String,
    zoom:f32,
    // Keep only last pointer position and object ID; never clone megabytes of
    // embedded artwork for every frame during drag.
    drag_start:Option<(Point,u64)>,
    creation_start:Option<(Point,u64)>,
    curve_start:Option<Point>,
    curve_draft:Option<u64>,
    active_layer:u64,
    pan:Vec2,
    undo:Vec<Document>,
    redo:Vec<Document>,
    inspector_tab:InspectorTab,
    show_grid:bool,
    snap_grid:bool,
    show_rulers:bool,
    light_theme:bool,
    cursor_mm:Option<Point>,
    fill_cache:FillCache,
    vector_cache:VectorCache,
    raster_cache:RasterCache,
    proof_dpi:u32,
    keep_bitmap_ratio:bool,
}
impl Default for Editor{
    fn default()->Self{
        let mut doc=Document::default();
        doc.name="Рекламный макет".into();
        doc.add(Shape::Rectangle{x:70.0,y:110.0,w:700.0,h:350.0},
            Style{fill:"#244A87".into(),stroke:"#244A87".into(),stroke_width_mm:0.0});
        doc.add(Shape::Text{x:120.0,y:200.0,value:"ВАША РЕКЛАМА".into(),size_mm:42.0},
            Style{fill:"#FFFFFF".into(),stroke:"#FFFFFF".into(),stroke_width_mm:0.0});
        Self{doc,selected:None,multi_selected:Vec::new(),tool:Tool::Select,file:"project.kdr".into(),
            text:"Новый текст".into(),status:"Экспериментальный прототип".into(),
            zoom:1.0,drag_start:None,creation_start:None,
            curve_start:None,curve_draft:None,active_layer:1,pan:Vec2::ZERO,
            undo:Vec::new(),redo:Vec::new(),
            inspector_tab:InspectorTab::Objects,show_grid:false,snap_grid:false,
            show_rulers:true,light_theme:true,cursor_mm:None,
            fill_cache:FillCache::default(),vector_cache:VectorCache::default(),
            raster_cache:RasterCache::default(),proof_dpi:96,
            keep_bitmap_ratio:true}
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
    if s=="none"{return Color32::TRANSPARENT;}
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
fn paint(p:&egui::Painter,obj:&Object,v:View,cache:&mut FillCache,
    vectors:&mut VectorCache,rasters:&mut RasterCache,ctx:&egui::Context){
    let fill=color(&obj.style.fill);
    let stroke=if obj.style.stroke=="none" {
        Stroke::new(0.0,Color32::TRANSPARENT)
    } else {
        Stroke::new(obj.style.stroke_width_mm*v.scale,color(&obj.style.stroke))
    };
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
            let mut contour=vec![*start];
            let mut previous=*start;
            let cap=(20_000usize/segments.len().max(1)).clamp(4,48);
            for seg in segments {
                let approx=((seg.end.x-previous.x).hypot(seg.end.y-previous.y)
                    +(seg.control1.x-previous.x).hypot(seg.control1.y-previous.y)
                    +(seg.control2.x-seg.end.x).hypot(seg.control2.y-seg.end.y))*v.scale;
                let steps=((approx/10.0).ceil() as usize).clamp(4,cap);
                for i in 1..=steps{
                    contour.push(cubic(previous,seg.control1,seg.control2,seg.end,
                        i as f32/steps as f32));
                }
                previous=seg.end;
            }
            if *closed {cache.fill(p,obj.id,&contour,v,fill);}
            if stroke.width>0.0 {
                let mut vertices:Vec<Pos2>=contour.iter().map(|&pt|v.screen(pt)).collect();
                if *closed{vertices.push(v.screen(*start));}
                if vertices.len()>1{p.add(egui::Shape::line(vertices,stroke));}
            }
        }
        Shape::EmbeddedSvg{x,y,w,h,svg,..}=>{
            vectors.paint(ctx,p,obj.id,svg,
                v.bounds(Bounds{x:*x,y:*y,w:*w,h:*h}));
        }
        Shape::Raster{x,y,w,h,data_base64,..}=>{
            rasters.paint(ctx,p,obj.id,data_base64,
                v.bounds(Bounds{x:*x,y:*y,w:*w,h:*h}));
        }
        Shape::Polyline{points,closed}=>{
            if *closed{cache.fill(p,obj.id,points,v,fill);}
            if stroke.width>0.0{
                let mut vertices:Vec<Pos2>=points.iter().map(|&pt|v.screen(pt)).collect();
                if *closed && !vertices.is_empty(){vertices.push(vertices[0]);}
                if vertices.len()>1{p.add(egui::Shape::line(vertices,stroke));}
            }
        }
    }
}
fn trim_history(history:&mut Vec<Document>) {
    const MAX_HISTORY_BYTES:usize=96*1024*1024;
    while history.len()>1 {
        let size=history.iter().fold(0usize,|acc,doc|
            acc.saturating_add(doc.estimated_size_bytes()));
        if history.len()<=50 && size<=MAX_HISTORY_BYTES{break;}
        history.remove(0);
    }
}
impl Editor{
    fn selection_ids(&self)->Vec<u64>{
        let mut ids=Vec::with_capacity(self.multi_selected.len()+1);
        if let Some(id)=self.selected{ids.push(id);}
        ids.extend(self.multi_selected.iter().copied());
        ids
    }
    fn is_selected(&self,id:u64)->bool{
        self.selected==Some(id)||self.multi_selected.contains(&id)
    }
    fn select_object(&mut self,id:Option<u64>,additive:bool){
        if !additive{
            self.selected=id;self.multi_selected.clear();return;
        }
        let Some(id)=id else{return;};
        if self.selected==Some(id){
            self.selected=self.multi_selected.pop();
        }else if let Some(i)=self.multi_selected.iter().position(|v|*v==id){
            self.multi_selected.remove(i);
        }else if self.selected.is_none(){
            self.selected=Some(id);
        }else{
            self.multi_selected.push(id);
        }
    }
    fn select_all(&mut self){
        let editable:HashSet<u64>=self.doc.layers.iter().filter(|l|l.visible&&!l.locked)
            .map(|l|l.id).collect();
        let ids:Vec<u64>=self.doc.objects.iter()
            .filter(|o|editable.contains(&o.layer_id)).map(|o|o.id).collect();
        self.selected=ids.first().copied();
        self.multi_selected=ids.into_iter().skip(1).collect();
        self.status=format!("Выбрано объектов: {}",self.selection_ids().len());
    }
    fn align_selection(&mut self,mode:Alignment){
        let ids=self.selection_ids();
        if ids.len()<2{self.status="Выберите минимум два объекта с Shift".into();return;}
        self.checkpoint();
        let count=self.doc.align_objects(&ids,mode);
        self.status=format!("Выровнено объектов: {count}");
    }
    fn distribute_selection(&mut self,mode:Distribution){
        let ids=self.selection_ids();
        if ids.len()<3{self.status="Для распределения нужны три объекта".into();return;}
        self.checkpoint();
        let count=self.doc.distribute_objects(&ids,mode);
        self.status=format!("Распределено объектов: {count}");
    }
    fn checkpoint(&mut self){
        self.undo.push(self.doc.clone());
        trim_history(&mut self.undo);
        self.redo.clear();
    }
    fn undo(&mut self){
        if let Some(prev)=self.undo.pop(){
            self.redo.push(std::mem::replace(&mut self.doc,prev));
            trim_history(&mut self.redo);
            self.fill_cache.entries.clear();
            self.vector_cache.clear();
            self.raster_cache.clear();
            self.selected=None;self.multi_selected.clear();
            self.curve_draft=None;self.curve_start=None;
            self.status="Отменено".into();
        }
    }
    fn redo(&mut self){
        if let Some(next)=self.redo.pop(){
            self.undo.push(std::mem::replace(&mut self.doc,next));
            trim_history(&mut self.undo);
            self.fill_cache.entries.clear();
            self.vector_cache.clear();
            self.raster_cache.clear();
            self.selected=None;self.multi_selected.clear();
            self.curve_draft=None;self.curve_start=None;
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
        self.multi_selected.clear();
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
    fn import_svg(&mut self){self.open();}
    fn save(&mut self){
        self.status=match opencorel_formats::save_document(&self.doc,&self.file){
            Ok(())=>format!("Сохранено: {}",self.file),
            Err(e)=>format!("Ошибка сохранения: {e}"),
        };
    }
    fn place_bitmap_picker(&mut self){
        if let Some(path)=rfd::FileDialog::new().add_filter(
            "Изображения", &["png","jpg","jpeg","webp","gif","bmp","tif","tiff"]
        ).pick_file(){
            self.place_bitmap(&path);
        }
    }
    fn place_bitmap(&mut self,path:&std::path::Path){
        match opencorel_formats::import_raster_shape(path){
            Ok(mut shape)=>{
                if let Shape::Raster{x,y,w,h,..}=&mut shape {
                    let scale=(self.doc.page.width_mm*0.70/ *w)
                        .min(self.doc.page.height_mm*0.70/ *h)
                        .min(1.0);
                    *w*=scale;*h*=scale;
                    *x=(self.doc.page.width_mm-*w)*0.5;
                    *y=(self.doc.page.height_mm-*h)*0.5;
                }
                let style=Style{fill:"none".into(),stroke:"none".into(),
                    stroke_width_mm:0.0};
                if self.add_shape(shape,style).is_some(){
                    self.inspector_tab=InspectorTab::Properties;
                    self.status=format!("Изображение добавлено: {}",path.display());
                }
            }
            Err(e)=>self.status=format!("Ошибка изображения: {e}"),
        }
    }
    fn open_picker(&mut self){
        if let Some(path)=rfd::FileDialog::new()
            .add_filter("Документы и графика",&[
                "kdr","ocd","svg","svgz","cdr","cmx","pdf","ai","eps","ps",
                "odg","fodg","dxf","wmf","emf","wpg","vsd","vsdx","sxd",
                "png","jpg","jpeg","webp","gif","bmp","tif","tiff"
            ]).pick_file(){
            let previous=self.file.clone();
            self.file=path.to_string_lossy().into_owned();
            self.open();
            if self.status.starts_with("Ошибка"){self.file=previous;}
        }
    }
    fn save_as_picker(&mut self){
        if let Some(path)=rfd::FileDialog::new()
            .add_filter("Kodi Draw (.kdr)",&["kdr"])
            .add_filter("Старый OpenCorel (.ocd)",&["ocd"])
            .set_file_name("project.kdr")
            .save_file(){
            let chosen=if path.extension().is_none(){path.with_extension("kdr")}else{path};
            self.file=chosen.to_string_lossy().into_owned();
            self.save();
        }
    }
    fn import_svg_picker(&mut self){
        if let Some(path)=rfd::FileDialog::new().add_filter("SVG / SVGZ",&["svg","svgz"]).pick_file(){
            self.file=path.to_string_lossy().into_owned();
            self.open();
        }
    }
    fn open(&mut self){
        match opencorel_formats::open_any(&self.file){
            Ok(result)=>{
                self.checkpoint();
                self.doc=result.document;
                self.selected=None;self.multi_selected.clear();self.active_layer=1;
                self.fill_cache.entries.clear();self.vector_cache.clear();
                self.raster_cache.clear();
                self.curve_start=None;self.curve_draft=None;
                let source=opencorel_formats::extension(std::path::Path::new(&self.file));
                if source!="kdr"&&source!="ocd"{
                    self.file=std::path::Path::new(&self.file)
                        .with_extension("kdr").to_string_lossy().into_owned();
                }
                self.status=if result.warning.is_empty(){
                    "Проект открыт".into()
                }else{format!("Импорт: {}",result.warning)};
            }
            Err(e)=>self.status=format!("Ошибка открытия: {e}")
        }
    }
    fn raster_proof(&mut self,ext:&str){
        let path=std::path::Path::new(&self.file).with_extension(ext);
        self.status=match opencorel_export::raster_export::export_raster(
            &self.doc,&path,self.proof_dpi){
            Ok(())=>format!("{} ({} DPI) экспортирован: {}",ext.to_uppercase(),
                self.proof_dpi,path.display()),
            Err(e)=>format!("Ошибка экспорта {ext}: {e}"),
        };
    }
    fn pdf(&mut self){
        let path=std::path::Path::new(&self.file).with_extension("pdf");
        self.status=match opencorel_export::export_pdf(&self.doc,&path){
            Ok(())=>format!("PDF (RGB, не PDF/X) экспортирован: {}",path.display()),
            Err(e)=>format!("Ошибка PDF: {e}"),
        };
    }
    fn svgz(&mut self){
        let path=std::path::Path::new(&self.file).with_extension("svgz");
        self.status=match opencorel_formats::save_document(&self.doc,&path){
            Ok(())=>format!("SVGZ экспортирован: {}",path.display()),
            Err(e)=>format!("Ошибка SVGZ: {e}"),
        };
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
    fn snapped(&self,p:Point)->Point {
        if self.snap_grid {
            Point{x:(p.x/10.0).round()*10.0,y:(p.y/10.0).round()*10.0}
        }else{p}
    }
    fn draw_grid_rulers(&self,painter:&egui::Painter, page:&egui::Painter,
        v:View, area:Rect){
        if self.show_grid {
            let mut step=10.0f32;
            while step*v.scale<14.0 {step*=2.0;}
            let nx=(self.doc.page.width_mm/step).ceil().min(1200.0) as usize;
            let ny=(self.doc.page.height_mm/step).ceil().min(1200.0) as usize;
            let grid_color=Color32::from_gray(223);
            for i in 0..=nx {
                let x=v.screen(Point{x:i as f32*step,y:0.0}).x;
                if x>=v.rect.left()&&x<=v.rect.right() {
                    page.line_segment([Pos2::new(x,v.rect.top()),Pos2::new(x,v.rect.bottom())],
                        Stroke::new(0.6,grid_color));
                }
            }
            for i in 0..=ny {
                let y=v.screen(Point{x:0.0,y:i as f32*step}).y;
                if y>=v.rect.top()&&y<=v.rect.bottom() {
                    page.line_segment([Pos2::new(v.rect.left(),y),Pos2::new(v.rect.right(),y)],
                        Stroke::new(0.6,grid_color));
                }
            }
        }
        if self.show_rulers {
            let top=Rect::from_min_max(area.min,Pos2::new(area.max.x,area.min.y+24.0));
            let left=Rect::from_min_max(Pos2::new(area.min.x,area.min.y+24.0),
                Pos2::new(area.min.x+25.0,area.max.y));
            painter.rect_filled(top,0.0,Color32::from_gray(210));
            painter.rect_filled(left,0.0,Color32::from_gray(210));
            let mut step=10.0f32;
            while step*v.scale<35.0 {step*=2.0;}
            let xcount=(self.doc.page.width_mm/step).ceil().min(1200.0) as usize;
            let ycount=(self.doc.page.height_mm/step).ceil().min(1200.0) as usize;
            for i in 0..=xcount{
                let x=v.screen(Point{x:i as f32*step,y:0.0}).x;
                if x>=left.right()&&x<=top.right() {
                    painter.line_segment([Pos2::new(x,top.bottom()),Pos2::new(x,top.bottom()-7.0)],
                        Stroke::new(1.0,Color32::DARK_GRAY));
                    painter.text(Pos2::new(x+2.0,top.top()+3.0),Align2::LEFT_TOP,
                        format!("{:.0}",i as f32*step),FontId::monospace(9.0),Color32::DARK_GRAY);
                }
            }
            for i in 0..=ycount{
                let y=v.screen(Point{x:0.0,y:i as f32*step}).y;
                if y>=left.top()&&y<=left.bottom() {
                    painter.line_segment([Pos2::new(left.right()-7.0,y),Pos2::new(left.right(),y)],
                        Stroke::new(1.0,Color32::DARK_GRAY));
                    painter.text(Pos2::new(left.left()+1.0,y+2.0),Align2::LEFT_TOP,
                        format!("{:.0}",i as f32*step),FontId::monospace(9.0),Color32::DARK_GRAY);
                }
            }
        }
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
            let working=if self.show_rulers{
                Rect::from_min_max(response.rect.min+Vec2::new(25.0,24.0),response.rect.max)
            }else{response.rect};
            let v=View::new(working,&self.doc,self.zoom,self.pan);
            self.cursor_mm=response.hover_pos().map(|p|v.mm(p));
            painter.rect_filled(v.rect,0.0,Color32::WHITE);
            outline(&painter,v.rect,Color32::GRAY,1.0);
            let page_painter=painter.with_clip_rect(v.rect.intersect(working));
            self.draw_grid_rulers(&painter,&page_painter,v,response.rect);
            for obj in self.doc.visible_objects(){
                if v.bounds(obj.shape.bounds()).intersects(v.rect.intersect(response.rect)){
                    paint(&page_painter,obj,v,&mut self.fill_cache,
                        &mut self.vector_cache,&mut self.raster_cache,ctx);
                }
            }
            let selected:HashSet<u64>=self.selection_ids().into_iter().collect();
            for o in self.doc.objects.iter().filter(|o|selected.contains(&o.id)){
                if self.doc.layer(o.layer_id).map(|l|l.visible).unwrap_or(false){
                    outline(&page_painter,v.bounds(o.shape.bounds()).expand(3.0),Color32::BLUE,2.0);
                }
            }
            if response.drag_started_by(egui::PointerButton::Primary){
                if let Some(pos)=response.interact_pointer_pos(){
                    if v.rect.contains(pos){
                        let p=self.snapped(v.mm(pos));
                        match self.tool{
                            Tool::Select=>{
                                // Shift-click toggles selection, without starting a drag.
                                if !ctx.input(|i|i.modifiers.shift){
                                    let hit=self.doc.hit_test(p,2.0);
                                    if let Some(id)=hit{
                                        if !self.is_selected(id){self.select_object(Some(id),false);}
                                        self.checkpoint();
                                        self.drag_start=Some((p,id));
                                    }else{self.select_object(None,false);}
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
                    let now=self.snapped(v.mm(pos));
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
                    }else if let Some((previous,_id))=self.drag_start {
                        let selected:HashSet<u64>=self.selection_ids().into_iter().collect();
                        let editable:HashSet<u64>=self.doc.layers.iter()
                            .filter(|l|l.visible&&!l.locked).map(|l|l.id).collect();
                        for object in self.doc.objects.iter_mut(){
                            if selected.contains(&object.id)&&editable.contains(&object.layer_id){
                                object.shape.translate(now.x-previous.x,now.y-previous.y);
                            }
                        }
                        self.drag_start=Some((now,_id));
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
                        let p=self.snapped(v.mm(pos));
                        let shape=match self.tool{
                            Tool::Select=>{
                                let id=self.doc.hit_test(p,2.0);
                                let shift=ctx.input(|i|i.modifiers.shift);
                                if shift{self.select_object(id,true);}
                                else if !id.map(|id|self.is_selected(id)).unwrap_or(false){
                                    self.select_object(id,false);
                                }
                                return;
                            },
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
        let save_as=ctx.input(|i|i.modifiers.command&&i.modifiers.shift
            && i.key_pressed(egui::Key::S));
        if save_as{self.save_as_picker();}
        else if ctx.input(|i|i.modifiers.command&&i.key_pressed(egui::Key::S)){self.save();}
        if ctx.input(|i|i.modifiers.command&&i.key_pressed(egui::Key::O)){self.open_picker();}
        if !ctx.wants_keyboard_input(){
            let dropped=ctx.input(|i|i.raw.dropped_files.clone());
            for file in dropped{
                if let Some(path)=file.path {
                    if matches!(opencorel_formats::extension(&path).as_str(),
                        "png"|"jpg"|"jpeg"|"webp"|"gif"|"bmp"|"tif"|"tiff"){
                        self.place_bitmap(&path);
                    }else{
                        let old=self.file.clone();
                        self.file=path.to_string_lossy().into_owned();
                        self.open();
                        if self.status.starts_with("Ошибка"){self.file=old;}
                    }
                }
            }
        }
        if !ctx.wants_keyboard_input() && ctx.input(|i|i.key_pressed(egui::Key::Delete)){
            self.delete_selection();
        }
        if ctx.input(|i|i.key_pressed(egui::Key::Escape)){
            self.curve_start=None;self.curve_draft=None;self.tool=Tool::Select;
        }
        if ctx.style().visuals.dark_mode==self.light_theme {
            ctx.set_visuals(if self.light_theme{egui::Visuals::light()}else{egui::Visuals::dark()});
        }
        if !ctx.wants_keyboard_input(){
            if ctx.input(|i|i.modifiers.command&&i.key_pressed(egui::Key::A)){self.select_all();}
            if ctx.input(|i|i.modifiers.command&&i.key_pressed(egui::Key::N)){self.new_document();}
            if ctx.input(|i|i.modifiers.command&&i.key_pressed(egui::Key::D)){self.duplicate_selection();}
            if ctx.input(|i|i.key_pressed(egui::Key::F4)){self.fit_page();}
            if ctx.input(|i|i.key_pressed(egui::Key::F6)){self.change_tool(Tool::Rect);}
            if ctx.input(|i|i.key_pressed(egui::Key::F7)){self.change_tool(Tool::Ellipse);}
            if ctx.input(|i|i.key_pressed(egui::Key::F8)){self.change_tool(Tool::Text);}
        }
        self.top_workspace(ctx);
        self.tool_sidebar(ctx);
        self.color_palette(ctx);
        self.inspector(ctx);
        self.workspace_footer(ctx);
        self.canvas(ctx);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn additive_selection_toggles_objects(){
        let mut editor=Editor::default();
        let first=editor.doc.objects[0].id;
        let second=editor.doc.objects[1].id;
        editor.select_object(Some(first),false);
        editor.select_object(Some(second),true);
        assert_eq!(editor.selection_ids().len(),2);
        editor.select_object(Some(second),true);
        assert_eq!(editor.selection_ids(),vec![first]);
        editor.select_all();
        assert_eq!(editor.selection_ids().len(),2);
    }
    #[test]
    fn ctrl_a_like_selection_supports_batch_delete(){
        let mut editor=Editor::default();
        editor.select_all();
        assert_eq!(editor.selection_ids().len(),2);
        editor.delete_selection();
        assert_eq!(editor.doc.objects.len(),0);
        editor.undo();
        assert_eq!(editor.doc.objects.len(),2);
    }
    #[test]
    fn history_limit_prevents_unbounded_text_snapshots(){
        let mut history=Vec::new();
        for i in 0..80 {
            let mut doc=Document::default();
            doc.name=format!("{i}").repeat(100_000);
            history.push(doc);
        }
        trim_history(&mut history);
        assert!(history.len()<=50);
    }
    #[test]
    fn grid_snapping_uses_millimetres(){
        let mut editor=Editor::default();
        editor.snap_grid=true;
        assert_eq!(editor.snapped(Point{x:14.0,y:26.0}),
            Point{x:10.0,y:30.0});
        editor.snap_grid=false;
        assert_eq!(editor.snapped(Point{x:14.0,y:26.0}),
            Point{x:14.0,y:26.0});
    }
    #[test]
    fn undo_redo_recovers_geometry(){
        let mut editor=Editor::default();
        let original=editor.doc.objects.len();
        editor.add_shape(Shape::Rectangle{x:1.0,y:1.0,w:10.0,h:10.0},Style::default());
        assert_eq!(editor.doc.objects.len(),original+1);
        editor.undo();
        assert_eq!(editor.doc.objects.len(),original);
        editor.redo();
        assert_eq!(editor.doc.objects.len(),original+1);
    }
    #[test]
    fn concave_polygon_fill_is_triangulated(){
        let xy:Vec<f64>=vec![0.0,0.0, 20.0,0.0, 20.0,20.0, 10.0,5.0, 0.0,20.0];
        let indices=earcutr::earcut(&xy,&[],2).unwrap();
        assert_eq!(indices.len(),9);
        let mut area=0.0;
        for t in indices.chunks_exact(3){
            let a=(xy[2*t[0]],xy[2*t[0]+1]);
            let b=(xy[2*t[1]],xy[2*t[1]+1]);
            let c=(xy[2*t[2]],xy[2*t[2]+1]);
            area+=((b.0-a.0)*(c.1-a.1)-(b.1-a.1)*(c.0-a.0)).abs()*0.5;
        }
        assert!((area-250.0).abs()<0.001);
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
