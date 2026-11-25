use eframe::egui::{self,Align2,Color32,FontId,Pos2,Rect,Sense,Stroke,Vec2};
use opencorel_core::{Bounds,CubicSegment,Document,Object,Point,Shape,Style,valid_color,valid_paint,Alignment,Distribution};
use std::{f32::consts::TAU,collections::{HashMap,HashSet},hash::{Hash,Hasher},collections::hash_map::DefaultHasher,path::PathBuf,time::{Duration,Instant}};
mod workspace;
mod recovery;
mod tool_dock;
mod ui_theme;
mod productivity;
use opencorel_core::transform;
use opencorel_core::node_edit::{self,NodeAddress};
mod node_interaction;
mod device_tools;
mod page_workspace;
mod guide_tools;
mod layer_tools;
mod command_palette;
mod toolbar_icons;
mod drawing_tools;
mod dimension_tools;
mod pro_tools;
use pro_tools::ProToolsState;
mod boolean_commands;
mod clip_commands;
mod masked_canvas;
mod appearance_commands;
mod text_workspace;
mod path_workspace;
mod inventory_tools;
mod symbol_workspace;
mod production_tools;
use masked_canvas::MaskedCanvasCache;
mod vector_preview;
mod raster_preview;
mod raster_studio;
mod compound_render;
use compound_render::CompoundCache;
use opencorel_typography::TextOutliner;
use vector_preview::VectorCache;
use raster_preview::RasterCache;
use workspace::InspectorTab;

#[derive(Clone,Copy,PartialEq)]
enum Tool { Select, Nodes, Rect, Ellipse, Text, Curve, Line, Polygon, Freehand, Eyedropper }
#[derive(Debug)]
enum PendingAction{
    NewDocument,
    BlankPage{width:f32,height:f32},
    SampleDesign,
    Open(PathBuf),
    Close,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum ResizeCorner{TopLeft,TopRight,BottomLeft,BottomRight}
impl ResizeCorner{
    fn point(self,b:Bounds)->Point{
        match self{
            Self::TopLeft=>Point{x:b.x,y:b.y},
            Self::TopRight=>Point{x:b.x+b.w,y:b.y},
            Self::BottomLeft=>Point{x:b.x,y:b.y+b.h},
            Self::BottomRight=>Point{x:b.x+b.w,y:b.y+b.h}
        }
    }
}

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
    marquee_start:Option<Point>,
    marquee_end:Option<Point>,
    freehand_points:Vec<Point>,
    freehand_width_mm:f32,
    freehand_smoothing_mm:f32,
    freehand_color:String,
    style_clipboard:Option<opencorel_core::style_tools::StyleSample>,
    curve_start:Option<Point>,
    curve_draft:Option<u64>,
    curve_outgoing:Option<Point>,
    curve_drag_anchor:Option<Point>,
    active_layer:u64,
    pan:Vec2,
    last_canvas_size:Vec2,
    undo:Vec<Document>,
    redo:Vec<Document>,
    inspector_tab:InspectorTab,
    show_inspector:bool,
    show_grid:bool,
    snap_grid:bool,
    grid_mm:f32,
    show_rulers:bool,
    show_guides:bool,
    snap_guides:bool,
    snap_objects:bool,
    snap_page_edges:bool,
    snap_tolerance_mm:f32,
    new_guide_x:f32,
    new_guide_y:f32,
    light_theme:bool,
    cursor_mm:Option<Point>,
    fill_cache:FillCache,
    vector_cache:VectorCache,
    raster_cache:RasterCache,
    raster_panel:raster_studio::RasterPanel,
    compound_cache:CompoundCache,
    masked_canvas:MaskedCanvasCache,
    rotation_degrees:f32,
    skew_x_degrees:f32,
    skew_y_degrees:f32,
    scale_x_percent:f32,
    scale_y_percent:f32,
    appearance_opacity:f32,
    appearance_gradient_mode:u8,
    appearance_gradient_stops:Vec<opencorel_core::appearance::GradientStop>,
    appearance_new_stop_percent:f32,
    appearance_gradient_angle:f32,
    appearance_dash_mm:f32,
    appearance_cap:u8,
    appearance_join:u8,
    custom_pivot:bool,
    pivot_x:f32,
    pivot_y:f32,
    text_outliner:TextOutliner,
    proof_dpi:u32,
    plotter_options:opencorel_export::hpgl::PlotterOptions,
    dxf_options:opencorel_export::dxf::DxfOptions,
    keep_bitmap_ratio:bool,
    show_preflight:bool,
    preflight_lines:Vec<String>,
    inventory_lines:Vec<String>,
    minimum_image_dpi:f32,
    show_safe_area:bool,
    safe_margin_mm:f32,
    internal_clipboard:Vec<Object>,
    internal_symbol_clipboard:Vec<(opencorel_core::SymbolInstance,String)>,
    paste_number:usize,
    symbol_library:opencorel_core::SymbolLibrary,
    symbol_library_path:PathBuf,
    symbol_library_error:Option<String>,
    symbol_search:String,
    symbol_category_filter:String,
    symbol_new_name:String,
    symbol_new_category:String,
    selected_symbol_key:Option<String>,
    show_welcome:bool,
    toolbox_expanded:bool,
    resize_drag:Option<(u64,ResizeCorner)>,
    object_filter:String,
    show_command_palette:bool,
    show_qr_dialog:bool,
    qr_text:String,qr_size_mm:f32,qr_foreground:String,qr_background:String,
    qr_ecc:u8,qr_status:String,
    show_recolor_dialog:bool,
    recolor_from:String,recolor_to:String,
    recolor_fills:bool,recolor_strokes:bool,recolor_stops:bool,
    recolor_all_pages:bool,recolor_summary:String,
    show_object_finder:bool,
    find_object_text:String,find_object_layer:String,find_object_color:String,
    find_object_kind:usize,find_object_all_pages:bool,
    find_object_include_hidden:bool,
    object_search_results:Vec<opencorel_core::object_search::ObjectHit>,
    show_text_search:bool,
    text_find_query:String,
    text_replacement:String,
    text_case_sensitive:bool,
    text_all_pages:bool,
    text_hits:Vec<opencorel_core::text_workflow::TextHit>,
    command_query:String,
    command_focus:bool,
    pro_tools:ProToolsState,
    batch_format:opencorel_export::batch::PageExportFormat,
    batch_page_selection:Option<std::collections::HashSet<u64>>,
    property_aspect_locked:bool,
    trim_margin_mm:f32,
    path_simplify_tolerance_mm:f32,
    path_join_tolerance_mm:f32,
    active_node:Option<(u64,NodeAddress)>,
    node_drag:Option<(u64,NodeAddress)>,
    node_handle_mode:node_edit::HandleMode,
    is_dirty:bool,
    last_edit:Instant,
    last_autosave:Instant,
    recovery_path:PathBuf,
    recovery_checked:bool,
    recovery_candidate:Option<recovery::RecoveryCandidate>,
    show_recovery_dialog:bool,
    show_unsaved_dialog:bool,
    pending_action:Option<PendingAction>,
    run_pending_action:bool,
    close_confirmed:bool,
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
            marquee_start:None,marquee_end:None,
            freehand_points:Vec::new(),freehand_width_mm:1.0,
            freehand_smoothing_mm:0.2,freehand_color:"#117CC1".into(),
            style_clipboard:None,
            curve_start:None,curve_draft:None,
            curve_outgoing:None,curve_drag_anchor:None,
            active_layer:1,pan:Vec2::ZERO,
            last_canvas_size:Vec2::new(900.,600.),
            undo:Vec::new(),redo:Vec::new(),
            inspector_tab:InspectorTab::Objects,show_inspector:true,
            show_grid:false,snap_grid:false,
            grid_mm:10.0,
            show_rulers:true,show_guides:true,snap_guides:true,
            snap_objects:true,snap_page_edges:true,snap_tolerance_mm:2.0,
            new_guide_x:50.0,new_guide_y:50.0,
            light_theme:false,cursor_mm:None,
            fill_cache:FillCache::default(),vector_cache:VectorCache::default(),
            raster_cache:RasterCache::default(),raster_panel:raster_studio::RasterPanel::default(),
            compound_cache:CompoundCache::default(),
            masked_canvas:MaskedCanvasCache::default(),
            rotation_degrees:30.0,skew_x_degrees:15.0,skew_y_degrees:0.0,
            scale_x_percent:100.0,scale_y_percent:100.0,
            appearance_opacity:1.0,appearance_gradient_mode:0,
            appearance_gradient_stops:vec![
                opencorel_core::appearance::GradientStop{
                    offset:0.0,color:"#447BD4".into()
                },
                opencorel_core::appearance::GradientStop{
                    offset:1.0,color:"#FFFFFF".into()
                }
            ],
            appearance_new_stop_percent:50.0,
            appearance_gradient_angle:0.0,appearance_dash_mm:0.0,
            appearance_cap:0,appearance_join:0,
            custom_pivot:false,pivot_x:0.0,pivot_y:0.0,
            text_outliner:TextOutliner::new(),proof_dpi:96,
            plotter_options:opencorel_export::hpgl::PlotterOptions::default(),
            dxf_options:opencorel_export::dxf::DxfOptions::default(),
            keep_bitmap_ratio:true,
            show_preflight:false,preflight_lines:Vec::new(),
            inventory_lines:Vec::new(),
            minimum_image_dpi:150.0,
            show_safe_area:false,safe_margin_mm:5.0,
            internal_clipboard:Vec::new(),
            internal_symbol_clipboard:Vec::new(),paste_number:0,
            symbol_library:opencorel_core::SymbolLibrary::default(),
            symbol_library_path:opencorel_core::SymbolLibrary::default_path(),
            symbol_library_error:None,symbol_search:String::new(),
            symbol_category_filter:String::new(),
            symbol_new_name:"Новый символ".into(),
            symbol_new_category:"Общее".into(),selected_symbol_key:None,
            show_welcome:true,toolbox_expanded:false,resize_drag:None,
            object_filter:String::new(),show_command_palette:false,
            show_qr_dialog:false,qr_text:"https://example.org".into(),
            qr_size_mm:65.0,qr_foreground:"#000000".into(),
            qr_background:"#FFFFFF".into(),qr_ecc:1,qr_status:String::new(),
            show_recolor_dialog:false,recolor_from:"#447BD4".into(),
            recolor_to:"#058C7D".into(),recolor_fills:true,
            recolor_strokes:true,recolor_stops:true,recolor_all_pages:true,
            recolor_summary:String::new(),
            show_object_finder:false,find_object_text:String::new(),
            find_object_layer:String::new(),find_object_color:String::new(),
            find_object_kind:0,find_object_all_pages:true,
            find_object_include_hidden:false,object_search_results:Vec::new(),
            show_text_search:false,text_find_query:String::new(),
            text_replacement:String::new(),text_case_sensitive:false,
            text_all_pages:true,text_hits:Vec::new(),
            command_query:String::new(),command_focus:false,
            pro_tools:ProToolsState::default(),
            batch_format:opencorel_export::batch::PageExportFormat::Svg,
            batch_page_selection:None,property_aspect_locked:true,
            trim_margin_mm:12.0,
            path_simplify_tolerance_mm:0.30,path_join_tolerance_mm:0.15,
            active_node:None,node_drag:None,
            node_handle_mode:node_edit::HandleMode::Independent,
            is_dirty:false,last_edit:Instant::now(),last_autosave:Instant::now(),
            recovery_path:recovery::new_session_path(),recovery_checked:false,
            recovery_candidate:None,show_recovery_dialog:false,
            show_unsaved_dialog:false,pending_action:None,
            run_pending_action:false,close_confirmed:false}
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
fn hexagon(bounds:Bounds)->Vec<Point>{
    let (cx,cy)=(bounds.x+bounds.w*0.5,bounds.y+bounds.h*0.5);
    (0..6).map(|i|{
        let angle=i as f32/6.0*TAU-TAU*0.25;
        Point{x:cx+bounds.w*0.5*angle.cos(),
            y:cy+bounds.h*0.5*angle.sin()}
    }).collect()
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
    vectors:&mut VectorCache,rasters:&mut RasterCache,
    compounds:&mut CompoundCache,ctx:&egui::Context){
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
            let fmt=&obj.text_format;
            let family=fmt.family.to_ascii_lowercase();
            let monospaced=["mono","consolas","courier","menlo","code"]
                .iter().any(|needle|family.contains(needle));
            let font=if monospaced {
                FontId::monospace((size_mm*v.scale).clamp(1.0,400.0))
            }else{
                FontId::proportional((size_mm*v.scale).clamp(1.0,400.0))
            };
            let max_width=fmt.line_width_mm(value,*size_mm);
            for (i,line) in value.split('\n').take(1000).enumerate(){
                let line_width=fmt.line_width_mm(line,*size_mm);
                let x_offset=match fmt.alignment{
                    opencorel_core::TextAlign::Left=>0.0,
                    opencorel_core::TextAlign::Center=>(max_width-line_width)*0.5,
                    opencorel_core::TextAlign::Right=>max_width-line_width,
                };
                p.text(v.screen(Point{x:*x+x_offset,
                        y:*y+i as f32*fmt.line_height* *size_mm}),
                    Align2::LEFT_TOP,line,font.clone(),fill);
            }
        }
        Shape::CompoundPath{contours}=>{
            compounds.paint(p,obj.id,contours,v,fill,stroke);
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
    fn run_preflight(&mut self){
        let issues=opencorel_core::preflight::check_book(&self.doc,
            opencorel_core::preflight::PreflightOptions{
                minimum_image_dpi:self.minimum_image_dpi,
                safe_margin_mm:self.safe_margin_mm,
            });
        let warning_count=issues.iter().filter(|(_,i)|
            i.severity!=opencorel_core::preflight::Severity::Notice).count();
        let labels:std::collections::HashMap<_,_>=self.doc.pages()
            .into_iter().map(|p|(p.id,p.name)).collect();
        self.preflight_lines=issues.iter().map(|(page,issue)|format!(
            "{} · {:?} [{}] {}{}",
            labels.get(page).map(String::as_str).unwrap_or("Страница"),
            issue.severity,issue.code,
            issue.object_id.map(|id|format!("Объект #{id}: ")).unwrap_or_default(),
            issue.message
        )).collect();
        self.status=format!("Проверка {} стр.: замечаний {}, информационных {}",
            self.doc.pages().len(),
            warning_count,issues.len()-warning_count);
        self.show_preflight=true;
    }
    fn text_to_curves(&mut self){
        let ids=self.selection_ids();
        if ids.is_empty(){
            self.status="Выделите один или несколько текстовых объектов".into();
            return;
        }
        let editable:HashSet<u64>=self.doc.layers.iter()
            .filter(|l|l.visible&&!l.locked).map(|l|l.id).collect();
        let mut converted=Vec::new();
        // Atomic batch: an unavailable font or glyph must not convert only half
        // of a customer document.
        for id in ids{
            let Some(obj)=self.doc.objects.iter().find(|o|o.id==id) else{continue;};
            if !editable.contains(&obj.layer_id){continue;}
            if !matches!(obj.shape,Shape::Text{..}){continue;}
            match self.text_outliner.convert_object(obj){
                Ok(shape)=>converted.push((id,shape)),
                Err(e)=>{
                    self.status=format!("Не удалось преобразовать текст #{id}: {e}");
                    return;
                }
            }
        }
        if converted.is_empty(){
            self.status="Выделение не содержит доступного для преобразования текста".into();
            return;
        }
        self.checkpoint();
        let count=converted.len();
        for (id,shape) in converted{
            if let Some(obj)=self.doc.get_mut(id){
                obj.shape=shape;
            }
        }
        self.compound_cache.clear();
        self.status=format!("{} текстовых объектов преобразовано в кривые. Ctrl+Z вернёт текст.",count);
    }
    fn corner_at(&self,v:View,pos:Pos2)->Option<(u64,ResizeCorner)>{
        if self.tool!=Tool::Select || !self.multi_selected.is_empty(){return None;}
        let id=self.selected?;
        let o=self.doc.objects.iter().find(|o|o.id==id)?;
        let editable=self.doc.layer(o.layer_id)
            .map(|l|l.visible&&!l.locked).unwrap_or(false);
        if !editable||!transform::resizable(&o.shape){return None;}
        let b=o.bounds();
        if b.w<0.1||b.h<0.1{return None;}
        for corner in [ResizeCorner::TopLeft,ResizeCorner::TopRight,
            ResizeCorner::BottomLeft,ResizeCorner::BottomRight]{
            let screen=v.screen(corner.point(b));
            if (screen-pos).length_sq()<=100.0{return Some((id,corner));}
        }
        None
    }
    fn resize_from_corner(&mut self,id:u64,corner:ResizeCorner,
        pointer:Point,keep_ratio:bool){
        let Some(index)=self.doc.objects.iter().position(|o|o.id==id) else{return;};
        let b=self.doc.objects[index].bounds();
        let left=matches!(corner,ResizeCorner::TopLeft|ResizeCorner::BottomLeft);
        let top=matches!(corner,ResizeCorner::TopLeft|ResizeCorner::TopRight);
        let ax=if left{b.x+b.w}else{b.x};
        let ay=if top{b.y+b.h}else{b.y};
        let mut w=(if left{ax-pointer.x}else{pointer.x-ax}).clamp(0.1,100_000.0);
        let mut h=(if top{ay-pointer.y}else{pointer.y-ay}).clamp(0.1,100_000.0);
        if keep_ratio && b.w>0.1 && b.h>0.1{
            let ratio=b.w/b.h;
            if w/ratio>h{h=(w/ratio).clamp(0.1,100_000.0);}
            else{w=(h*ratio).clamp(0.1,100_000.0);}
        }
        let target=Bounds{x:if left{ax-w}else{ax},
            y:if top{ay-h}else{ay},w,h};
        if let Err(e)=transform::resize_shape(&mut self.doc.objects[index].shape,target){
            self.status=e;
        }else{
            self.fill_cache.entries.clear();
            self.compound_cache.clear();
        }
    }
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
        let Some(id)=id else {
            if !additive{self.selected=None;self.multi_selected.clear();}
            return;
        };
        let members=self.doc.group_members(id);
        if self.tool==Tool::Nodes&&self.doc.symbol_instance_for_object(id).is_some(){
            self.selected=Some(id);
            self.multi_selected.clear();
            return;
        }
        if !additive{
            self.selected=Some(id);
            self.multi_selected=members.into_iter().filter(|value|*value!=id).collect();
            return;
        }
        let existing:HashSet<u64>=self.selection_ids().into_iter().collect();
        let remove=members.iter().all(|value|existing.contains(value));
        let mut next:Vec<u64>=if remove{
            self.selection_ids().into_iter().filter(|value|!members.contains(value)).collect()
        }else{
            let mut current=self.selection_ids();
            for member in members{
                if !existing.contains(&member){current.push(member);}
            }
            current
        };
        self.selected=next.first().copied();
        self.multi_selected=next.into_iter().skip(1).collect();
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
        if !self.layout_selection_available(&ids){return;}
        self.checkpoint();
        let count=self.doc.align_objects(&ids,mode);
        self.status=format!("Выровнено объектов: {count}");
    }
    fn distribute_selection(&mut self,mode:Distribution){
        let ids=self.selection_ids();
        if ids.len()<3{self.status="Для распределения нужны три объекта".into();return;}
        if !self.layout_selection_available(&ids){return;}
        self.checkpoint();
        let count=self.doc.distribute_objects(&ids,mode);
        self.status=format!("Распределено объектов: {count}");
    }
    fn layout_selection_available(&mut self,ids:&[u64])->bool{
        let ids:HashSet<u64>=ids.iter().copied().collect();
        let editable:HashSet<u64>=self.doc.layers.iter()
            .filter(|l|l.visible&&!l.locked).map(|l|l.id).collect();
        if self.doc.objects.iter().any(|o|ids.contains(&o.id)
            && (!editable.contains(&o.layer_id)||o.group_id.is_some())){
            self.status="Выровнять можно только отдельные незаблокированные объекты. Группу сначала разгруппируйте.".into();
            return false;
        }
        true
    }
    fn request_action(&mut self,action:PendingAction){
        if self.show_unsaved_dialog{return;}
        self.pending_action=Some(action);
        if self.is_dirty{
            self.show_unsaved_dialog=true;
        }else{
            self.run_pending_action=true;
        }
    }
    fn request_open(&mut self,path:PathBuf){
        self.request_action(PendingAction::Open(path));
    }
    fn open_path(&mut self,path:PathBuf){
        let previous=self.file.clone();
        self.file=path.to_string_lossy().into_owned();
        self.open();
        if self.status.starts_with("Ошибка"){self.file=previous;}
    }
    fn process_pending_action(&mut self,ctx:&egui::Context){
        if !self.run_pending_action{return;}
        self.run_pending_action=false;
        let Some(action)=self.pending_action.take() else{return;};
        match action{
            PendingAction::NewDocument=>self.reset_document(),
            PendingAction::BlankPage{width,height}=>self.blank_page_now(width,height),
            PendingAction::SampleDesign=>self.sample_design_now(),
            PendingAction::Open(path)=>self.open_path(path),
            PendingAction::Close=>{
                self.cleanup_recovery();
                self.close_confirmed=true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
    fn cleanup_recovery(&mut self){
        recovery::discard(&self.recovery_path);
    }
    fn autosave_tick(&mut self,ctx:&egui::Context){
        ctx.request_repaint_after(Duration::from_secs(5));
        if !self.is_dirty||self.last_edit.elapsed()<Duration::from_secs(2)
            ||self.last_autosave.elapsed()<Duration::from_secs(45){return;}
        match recovery::save_snapshot(&self.doc,&self.recovery_path){
            Ok(())=>self.last_autosave=Instant::now(),
            Err(error)=>self.status=format!("Ошибка автосохранения: {error}"),
        }
    }
    fn check_recovery(&mut self){
        if self.recovery_checked{return;}
        self.recovery_checked=true;
        self.recovery_candidate=recovery::latest(&self.recovery_path);
        self.show_recovery_dialog=self.recovery_candidate.is_some();
    }
    fn recovery_dialog(&mut self,ctx:&egui::Context){
        if !self.show_recovery_dialog{return;}
        let mut choice=0u8;
        egui::Window::new("Восстановление проекта")
            .collapsible(false).resizable(false)
            .anchor(Align2::CENTER_CENTER,Vec2::ZERO)
            .show(ctx,|ui|{
                if let Some(candidate)=&self.recovery_candidate{
                    ui.label("Найдена резервная копия предыдущего сеанса.");
                    ui.heading(&candidate.document.name);
                    ui.label(format!("Объектов: {} · Страниц: {}",
                        candidate.document.all_object_count(),
                        candidate.document.pages().len()));
                    let age=candidate.modified.elapsed().unwrap_or_default().as_secs();
                    ui.label(format!("Создана примерно {} мин. назад",age/60));
                    ui.separator();
                    ui.label("Исходный файл не перезаписывается. Восстановленный проект откроется как отдельный документ.");
                    ui.horizontal(|ui|{
                        if ui.button("Восстановить").clicked(){choice=1;}
                        if ui.button("Удалить копию").clicked(){choice=2;}
                    });
                }else{choice=2;}
            });
        if choice==1{
            if let Some(candidate)=self.recovery_candidate.take(){
                self.doc=candidate.document;
                self.file="Восстановленный проект.kdr".into();
                recovery::discard(&candidate.path);
                self.is_dirty=true;
                self.last_edit=Instant::now();
                self.last_autosave=Instant::now();
                self.selected=None;self.multi_selected.clear();
                self.active_node=None;self.node_drag=None;
                self.fill_cache.entries.clear();self.vector_cache.clear();
                self.raster_cache.clear();self.compound_cache.clear();
                self.masked_canvas=Default::default();
                self.show_welcome=false;
                self.status="Проект восстановлен. Сохраните его под новым именем.".into();
            }
            self.show_recovery_dialog=false;
        }else if choice==2{
            if let Some(candidate)=self.recovery_candidate.take(){
                recovery::discard(&candidate.path);
            }
            self.show_recovery_dialog=false;
            self.status="Резервная копия предыдущего сеанса удалена".into();
        }
    }
    fn unsaved_dialog(&mut self,ctx:&egui::Context){
        if !self.show_unsaved_dialog{return;}
        let mut choice=0u8;
        egui::Window::new("Несохранённые изменения")
            .collapsible(false).resizable(false)
            .anchor(Align2::CENTER_CENTER,Vec2::ZERO)
            .show(ctx,|ui|{
                ui.label("В документе есть изменения, которые ещё не сохранены.");
                ui.label("Сохраните их перед продолжением?");
                ui.separator();
                ui.horizontal(|ui|{
                    if ui.button("Сохранить").clicked(){choice=1;}
                    if ui.button("Не сохранять").clicked(){choice=2;}
                    if ui.button("Отмена").clicked(){choice=3;}
                });
            });
        match choice{
            1=>{
                self.save();
                if !self.is_dirty{
                    self.show_unsaved_dialog=false;
                    self.run_pending_action=true;
                }
            }
            2=>{
                self.is_dirty=false;
                self.cleanup_recovery();
                self.show_unsaved_dialog=false;
                self.run_pending_action=true;
            }
            3=>{
                self.pending_action=None;
                self.show_unsaved_dialog=false;
            }
            _=>{}
        }
    }
    fn checkpoint(&mut self){
        self.is_dirty=true;
        self.last_edit=Instant::now();
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
            self.compound_cache.clear();
            self.compound_cache.clear();
            self.raster_cache.clear();
            self.selected=None;self.multi_selected.clear();
            self.curve_draft=None;self.curve_start=None;
            self.resize_drag=None;self.node_drag=None;self.active_node=None;
            self.marquee_start=None;self.marquee_end=None;
            self.freehand_points.clear();
            self.is_dirty=true;self.last_edit=Instant::now();
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
            self.resize_drag=None;self.node_drag=None;self.active_node=None;
            self.is_dirty=true;self.last_edit=Instant::now();
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
        self.curve_anchor(p,None);
    }
    /// Click = corner node; drag during creation = smooth node with a
    /// mirrored incoming handle. The outgoing tangent is reused by the
    /// following segment so the stroke is continuous without F10 repair.
    fn curve_anchor(&mut self,p:Point,outgoing:Option<Point>){
        if !self.active_layer_editable(){
            self.status="Слой скрыт или заблокирован".into();return;
        }
        let Some(start)=self.curve_start else {
            self.curve_start=Some(p);
            self.curve_outgoing=outgoing;
            self.status="Начало Безье: щёлкните следующий узел или перетащите мышь для касательных".into();
            return;
        };
        let previous=match self.curve_draft.and_then(|id|self.doc.objects.iter().find(|o|o.id==id)){
            Some(Object{shape:Shape::Bezier{segments,..},..})=>
                segments.last().map(|seg|seg.end).unwrap_or(start),
            _=>start,
        };
        let control1=if let Some(tangent)=self.curve_outgoing{
            Point{x:previous.x+tangent.x,y:previous.y+tangent.y}
        }else{
            Point{x:previous.x+(p.x-previous.x)/3.0,
                  y:previous.y+(p.y-previous.y)/3.0}
        };
        let control2=if let Some(tangent)=outgoing{
            Point{x:p.x-tangent.x,y:p.y-tangent.y}
        }else{
            Point{x:previous.x+2.0*(p.x-previous.x)/3.0,
                  y:previous.y+2.0*(p.y-previous.y)/3.0}
        };
        let seg=CubicSegment{control1,control2,end:p};
        if let Some(id)=self.curve_draft{
            self.checkpoint();
            if let Some(obj)=self.doc.get_mut(id){
                if let Shape::Bezier{segments,..}=&mut obj.shape{segments.push(seg);}
            }
        }else{
            let style=Style{fill:"#000000".into(),stroke:"#202020".into(),stroke_width_mm:0.8};
            self.curve_draft=self.add_shape(Shape::Bezier{start,segments:vec![seg],closed:false},style);
        }
        self.curve_outgoing=outgoing;
        self.status="Узел добавлен: протягивание создаёт плавные касательные; двойной щелчок завершает".into();
    }
    fn import_svg(&mut self){
        self.request_open(PathBuf::from(self.file.clone()));
    }
    fn save(&mut self){
        // New documents have only a suggested filename. Ask where to save
        // instead of silently writing into an unpredictable working directory.
        if !std::path::Path::new(&self.file).is_absolute(){
            self.save_as_picker();return;
        }
        match opencorel_formats::save_document(&self.doc,&self.file){
            Ok(())=>{
                self.is_dirty=false;self.last_autosave=Instant::now();
                self.cleanup_recovery();
                self.status=format!("Сохранено: {}",self.file);
            }
            Err(e)=>self.status=format!("Ошибка сохранения: {e}"),
        }
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
                    self.inspector_tab=InspectorTab::Raster;
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
            self.request_open(path);
        }
    }
    fn save_as_picker(&mut self){
        let suggested=std::path::Path::new(&self.file)
            .file_name().and_then(|s|s.to_str()).unwrap_or("project.kdr")
            .to_string();
        if let Some(path)=rfd::FileDialog::new()
            .add_filter("Kodi Draw (.kdr)",&["kdr"])
            .add_filter("Старый OpenCorel (.ocd)",&["ocd"])
            .set_file_name(&suggested)
            .save_file(){
            let chosen=if path.extension().is_none(){path.with_extension("kdr")}else{path};
            let result=opencorel_formats::save_document(&self.doc,&chosen);
            self.status=match result {
                Ok(())=>{
                    self.file=chosen.to_string_lossy().into_owned();
                    self.is_dirty=false;self.last_autosave=Instant::now();
                    self.cleanup_recovery();
                    format!("Сохранено: {}",chosen.display())
                },
                Err(e)=>format!("Ошибка сохранения: {e}"),
            };
        }
    }
    fn export_dialog(&mut self,ext:&str){
        let stem=std::path::Path::new(&self.file).file_stem()
            .and_then(|s|s.to_str()).unwrap_or("Макет");
        let suggested=format!("{stem}.{ext}");
        let caption=match ext{
            "svg"|"svgz"=>"SVG векторная графика",
            "pdf"=>"PDF для просмотра (RGB)",
            "png"|"jpg"=>"Изображение для согласования",
            "plt"=>"HP-GL/1 режущий плоттер",
            "dxf"=>"DXF R2000 — лазер / CNC",
            _=>return,
        };
        if let Some(chosen)=rfd::FileDialog::new()
            .add_filter(caption,&[ext])
            .set_file_name(&suggested)
            .save_file(){
            let path=chosen.with_extension(ext);
            let result=match ext{
                "png"|"jpg"=>opencorel_export::raster_export::export_raster(
                    &self.doc,&path,self.proof_dpi),
                "plt"=>opencorel_export::hpgl::export_hpgl(
                    &self.doc,&path,self.plotter_options),
                "dxf"=>opencorel_export::dxf::export_dxf(
                    &self.doc,&path,self.dxf_options),
                _=>opencorel_formats::save_document(&self.doc,&path),
            };
            self.status=match result{
                Ok(())=>format!("Экспортировано: {} ({}).",path.display(),
                    if ext=="pdf"{"PDF RGB, не PDF/X"}
                    else if ext=="plt"{"HP-GL/1, проверка на плоттере обязательна"}
                    else if ext=="dxf"{"DXF R2000, передача в CAD/CAM"}
                    else{"контрольный файл"}),
                Err(e)=>format!("Ошибка экспорта: {e}"),
            };
        }
    }
    fn import_svg_picker(&mut self){
        if let Some(path)=rfd::FileDialog::new().add_filter("SVG / SVGZ",&["svg","svgz"]).pick_file(){
            self.request_open(path);
        }
    }
    fn open(&mut self){
        match opencorel_formats::open_any(&self.file){
            Ok(result)=>{
                self.checkpoint();
                self.doc=result.document;
                self.is_dirty=false;self.last_edit=Instant::now();
                self.last_autosave=Instant::now();self.cleanup_recovery();
                self.batch_page_selection=None;
                self.selected=None;self.multi_selected.clear();self.active_layer=1;
                self.resize_drag=None;self.node_drag=None;self.active_node=None;
                self.fill_cache.entries.clear();self.vector_cache.clear();
                self.compound_cache.clear();
                self.raster_cache.clear();
                self.curve_start=None;self.curve_draft=None;
                let source=opencorel_formats::extension(std::path::Path::new(&self.file));
                if source!="kdr"&&source!="ocd"{
                    // Imported artwork must not automatically overwrite an existing
                    // neighbouring KDR file on the first Ctrl+S.
                    let source_name=std::path::Path::new(&self.file)
                        .file_stem().and_then(|s|s.to_str()).unwrap_or("Импорт");
                    self.file=format!("{source_name} - импорт.kdr");
                }
                self.show_welcome=false;
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
        let selected:HashSet<u64>=self.selection_ids().into_iter().collect();
        self.doc.snap_to_targets(p,opencorel_core::SnapSettings{
            guides:self.snap_guides,
            page_edges:self.snap_page_edges,
            objects:self.snap_objects,
            grid:self.snap_grid,grid_mm:self.grid_mm,
            tolerance_mm:self.snap_tolerance_mm,
        },&selected)
    }
    fn draw_grid_rulers(&self,painter:&egui::Painter, page:&egui::Painter,
        v:View, area:Rect){
        if self.show_grid {
            let mut step=self.grid_mm;
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
            let mut step=self.grid_mm;
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
            painter.rect_filled(response.rect,0.0,
                if self.light_theme{Color32::from_rgb(222,227,234)}
                else{Color32::from_rgb(15,19,26)});
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
            self.last_canvas_size=working.size();
            let v=View::new(working,&self.doc,self.zoom,self.pan);
            self.cursor_mm=response.hover_pos().map(|p|v.mm(p));
            painter.rect_filled(v.rect,0.0,Color32::WHITE);
            outline(&painter,v.rect,Color32::GRAY,1.0);
            let page_painter=painter.with_clip_rect(v.rect.intersect(working));
            self.draw_grid_rulers(&painter,&page_painter,v,response.rect);
            if self.doc.clip_links.is_empty() && self.doc.appearances.is_empty(){
                for obj in self.doc.visible_objects(){
                    if v.bounds(obj.bounds()).intersects(v.rect.intersect(response.rect)){
                        paint(&page_painter,obj,v,&mut self.fill_cache,
                            &mut self.vector_cache,&mut self.raster_cache,
                            &mut self.compound_cache,ctx);
                    }
                }
            }else{
                // Cached SVG proof uses the same clip paths as PDF/raster output.
                self.masked_canvas.paint(ctx,&page_painter,&self.doc,v.rect);
            }
            self.paint_guides(&page_painter,v);
            if self.show_safe_area {
                let margin=self.safe_margin_mm;
                if margin>0.0 && self.doc.page.width_mm>2.0*margin &&
                    self.doc.page.height_mm>2.0*margin {
                    let r=v.bounds(Bounds{x:margin,y:margin,
                        w:self.doc.page.width_mm-2.0*margin,
                        h:self.doc.page.height_mm-2.0*margin});
                    outline(&page_painter,r,Color32::from_rgb(222,129,48),1.0);
                }
            }
            let selected:HashSet<u64>=self.selection_ids().into_iter().collect();
            for o in self.doc.objects.iter().filter(|o|selected.contains(&o.id)){
                if self.doc.layer(o.layer_id).map(|l|l.visible).unwrap_or(false){
                    outline(&page_painter,v.bounds(o.bounds()).expand(3.0),
                        Color32::from_rgb(22,175,148),1.5);
                    if self.tool==Tool::Select && self.multi_selected.is_empty() &&
                        transform::resizable(&o.shape) &&
                        o.bounds().w>0.1 && o.bounds().h>0.1 {
                        for corner in [ResizeCorner::TopLeft,ResizeCorner::TopRight,
                            ResizeCorner::BottomLeft,ResizeCorner::BottomRight]{
                            let p=v.screen(corner.point(o.bounds()));
                            let handle=Rect::from_center_size(p,Vec2::splat(9.0));
                            page_painter.rect_filled(handle,1.0,Color32::WHITE);
                            outline(&page_painter,handle,
                                Color32::from_rgb(35,106,200),1.5);
                        }
                    }
                }
            }
            if selected.len()>1{
                if let Some(bounds)=self.selection_bounds(){
                    let rect=v.bounds(bounds).expand(5.0);
                    outline(&page_painter,rect,Color32::from_rgb(31,179,158),1.5);
                    page_painter.text(rect.left_top()+Vec2::new(0.0,-18.0),
                        Align2::LEFT_TOP,
                        format!("{} объектов",selected.len()),
                        FontId::proportional(11.0),Color32::from_rgb(39,209,177));
                }
            }
            if self.tool==Tool::Nodes{
                self.paint_node_handles(&page_painter,v);
            }
            self.paint_drawing_overlays(&page_painter,v);
            if let Some(anchor)=self.curve_drag_anchor{
                if let Some(pos)=ctx.input(|i|i.pointer.latest_pos()){
                    let shade=Color32::from_rgb(37,176,201);
                    page_painter.line_segment([v.screen(anchor),pos],Stroke::new(1.5,shade));
                    page_painter.circle_filled(v.screen(anchor),4.0,shade);
                }
            }
            if response.drag_started_by(egui::PointerButton::Primary){
                if let Some(pos)=response.interact_pointer_pos(){
                    if v.rect.contains(pos){
                        let p=self.snapped(v.mm(pos));
                        match self.tool{
                            Tool::Nodes=>self.begin_node_drag(v,pos),
                            Tool::Freehand=>self.begin_freehand(v.mm(pos)),
                            Tool::Curve=>{
                                let origin=ctx.input(|i|i.pointer.press_origin()).unwrap_or(pos);
                                if v.rect.contains(origin){
                                    self.curve_drag_anchor=Some(self.snapped(v.mm(origin)));
                                }
                            },
                            Tool::Select=>{
                                if let Some((id,corner))=self.corner_at(v,pos){
                                    self.checkpoint();
                                    self.resize_drag=Some((id,corner));
                                }else if !ctx.input(|i|i.modifiers.shift){
                                    let hit=self.doc.hit_test(p,2.0);
                                    if let Some(id)=hit{
                                        if !self.is_selected(id){self.select_object(Some(id),false);}
                                        self.checkpoint();
                                        self.drag_start=Some((p,id));
                                    }else{
                                        self.begin_marquee(v.mm(pos));
                                    }
                                }else{
                                    self.begin_marquee(v.mm(pos));
                                }
                            }
                            Tool::Rect|Tool::Ellipse|Tool::Line|Tool::Polygon=>{
                                let shape=match self.tool {
                                    Tool::Rect=>Shape::Rectangle{x:p.x,y:p.y,w:1.0,h:1.0},
                                    Tool::Ellipse=>Shape::Ellipse{x:p.x,y:p.y,w:1.0,h:1.0},
                                    Tool::Line=>Shape::Polyline{points:vec![p,p],closed:false},
                                    Tool::Polygon=>Shape::Polyline{
                                        points:hexagon(Bounds{x:p.x,y:p.y,w:1.0,h:1.0}),closed:true},
                                    _=>unreachable!()
                                };
                                let style=if self.tool==Tool::Line{
                                    Style{fill:"none".into(),stroke:"#262D3B".into(),
                                        stroke_width_mm:1.0}
                                }else{Style::default()};
                                if let Some(id)=self.add_shape(shape,style){
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
                    if self.marquee_start.is_some(){
                        self.update_marquee(v.mm(pos));
                    }else if !self.freehand_points.is_empty(){
                        self.extend_freehand(v.mm(pos));
                    }else if let Some(anchor)=self.curve_drag_anchor{
                        self.status=format!("Касательная Безье: X {:.1}, Y {:.1} мм",
                            v.mm(pos).x-anchor.x,v.mm(pos).y-anchor.y);
                    }else if self.node_drag.is_some(){
                        self.drag_node_to(now);
                    }else if let Some((id,corner))=self.resize_drag{
                        self.resize_from_corner(id,corner,now,
                            ctx.input(|i|i.modifiers.shift));
                    }else if let Some((p,id))=self.creation_start{
                        let x=p.x.min(now.x);let y=p.y.min(now.y);
                        let w=(p.x-now.x).abs().max(0.1);
                        let h=(p.y-now.y).abs().max(0.1);
                        if let Some(obj)=self.doc.get_mut(id){
                            match &mut obj.shape{
                                Shape::Rectangle{x:rx,y:ry,w:rw,h:rh}|
                                Shape::Ellipse{x:rx,y:ry,w:rw,h:rh}=>{
                                    *rx=x;*ry=y;*rw=w;*rh=h;
                                }
                                Shape::Polyline{points,closed} if self.tool==Tool::Line=>{
                                    *closed=false;
                                    *points=vec![p,now];
                                }
                                Shape::Polyline{points,closed} if self.tool==Tool::Polygon=>{
                                    *closed=true;
                                    *points=hexagon(Bounds{x,y,w,h});
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
                self.finish_marquee(ctx.input(|i|i.modifiers.shift));
                self.finish_freehand();
                if let Some(anchor)=self.curve_drag_anchor.take(){
                    let pointer=response.interact_pointer_pos().or_else(||
                        ctx.input(|i|i.pointer.latest_pos()));
                    if let Some(end)=pointer{
                        let raw=v.mm(end);
                        let delta=Point{x:raw.x-anchor.x,y:raw.y-anchor.y};
                        let tangent=if delta.x.hypot(delta.y)>0.15{
                            Some(delta)
                        }else{None};
                        self.curve_anchor(anchor,tangent);
                    }
                }
                self.drag_start=None;self.creation_start=None;
                self.resize_drag=None;self.node_drag=None;
            }
            if response.double_clicked(){
                if self.tool==Tool::Nodes{
                    if let Some(pos)=response.interact_pointer_pos(){
                        if v.rect.contains(pos){self.insert_node_at_pointer(v,pos);}
                    }
                }
                if self.tool==Tool::Curve{
                    self.curve_start=None;self.curve_draft=None;
                    self.curve_outgoing=None;self.curve_drag_anchor=None;
                    self.tool=Tool::Select;self.status="Кривая завершена".into();
                }
            }
            if response.clicked() && !response.double_clicked(){
                if let Some(pos)=response.interact_pointer_pos(){
                    if self.try_create_guide_from_ruler(pos,working,v){return;}
                    if v.rect.contains(pos){
                        let p=self.snapped(v.mm(pos));
                        let shape=match self.tool{
                            Tool::Nodes=>{self.select_node_near(v,pos);return;},
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
                            Tool::Freehand=>{return;},
                            Tool::Eyedropper=>{self.eyedropper_click(v.mm(pos));return;},
                            Tool::Rect=>Shape::Rectangle{x:p.x,y:p.y,w:60.0,h:40.0},
                            Tool::Ellipse=>Shape::Ellipse{x:p.x,y:p.y,w:60.0,h:40.0},
                            Tool::Line=>Shape::Polyline{
                                points:vec![p,Point{x:p.x+80.0,y:p.y}],closed:false},
                            Tool::Polygon=>Shape::Polyline{
                                points:hexagon(Bounds{x:p.x,y:p.y,w:60.0,h:60.0}),closed:true},
                            Tool::Text=>Shape::Text{x:p.x,y:p.y,value:self.text.clone(),size_mm:18.0},
                        };
                        let style=if self.tool==Tool::Text{
                            Style{fill:"#202020".into(),stroke:"#202020".into(),stroke_width_mm:0.0}
                        }else if self.tool==Tool::Line{
                            Style{fill:"none".into(),stroke:"#262D3B".into(),stroke_width_mm:1.0}
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
        if ctx.input(|i|i.viewport().close_requested())&&!self.close_confirmed{
            if self.is_dirty{
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.request_action(PendingAction::Close);
            }else{
                self.cleanup_recovery();
                self.close_confirmed=true;
            }
        }
        self.check_recovery();
        self.autosave_tick(ctx);
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
                        self.request_open(path);
                    }
                }
            }
        }
        if !ctx.wants_keyboard_input(){
            let ctrl=ctx.input(|i|i.modifiers.command);
            if ctrl&&ctx.input(|i|i.modifiers.shift&&i.key_pressed(egui::Key::C)){
                self.copy_style_selected();
            }else if ctrl&&ctx.input(|i|i.key_pressed(egui::Key::C)){
                self.copy_selection();
            }
            if ctrl&&ctx.input(|i|i.key_pressed(egui::Key::X)){self.cut_selection();}
            if ctrl&&ctx.input(|i|i.modifiers.shift&&i.key_pressed(egui::Key::V)){
                self.paste_style_selected();
            }else if ctrl&&ctx.input(|i|i.key_pressed(egui::Key::V)){
                self.paste_selection();
            }
            // Familiar page and z-order navigation used by vector editors.
            let page_up=ctx.input(|i|i.key_pressed(egui::Key::PageUp));
            let page_down=ctx.input(|i|i.key_pressed(egui::Key::PageDown));
            if page_up||page_down{
                if ctrl {
                    let pages=self.doc.pages();
                    let index=self.doc.page_index();
                    if page_up && index>0{self.activate_page(pages[index-1].id);}
                    if page_down && index+1<pages.len(){
                        self.activate_page(pages[index+1].id);
                    }
                }else{
                    self.z_step(page_up);
                }
            }

            if !ctrl {
                let step=if ctx.input(|i|i.modifiers.shift){10.0}else{1.0};
                let left=ctx.input(|i|i.key_pressed(egui::Key::ArrowLeft));
                let right=ctx.input(|i|i.key_pressed(egui::Key::ArrowRight));
                let up=ctx.input(|i|i.key_pressed(egui::Key::ArrowUp));
                let down=ctx.input(|i|i.key_pressed(egui::Key::ArrowDown));
                for (pressed,dx,dy) in [
                    (left,-step,0.0),(right,step,0.0),(up,0.0,-step),(down,0.0,step)
                ]{
                    if pressed{
                        if self.tool==Tool::Nodes {self.nudge_active_node(dx,dy);}
                        else{self.nudge_selection(dx,dy);}
                    }
                }
            }
        }
        if ctx.input(|i|i.modifiers.command&&
            (i.key_pressed(egui::Key::F)||i.key_pressed(egui::Key::H))){
            self.show_text_search=true;
        }
        if ctx.input(|i|i.modifiers.command&&i.key_pressed(egui::Key::K)){
            self.show_command_palette=!self.show_command_palette;
            self.command_focus=self.show_command_palette;
            if self.show_command_palette{self.command_query.clear();}
        }
        if ctx.input(|i|i.key_pressed(egui::Key::F1)){self.show_welcome=true;}
        if !ctx.wants_keyboard_input() &&
            ctx.input(|i|i.key_pressed(egui::Key::F9)){
            self.show_inspector=!self.show_inspector;
        }
        if !ctx.wants_keyboard_input() && ctx.input(|i|i.key_pressed(egui::Key::Delete)){
            if self.tool==Tool::Nodes {self.remove_active_node();}
            else{self.delete_selection();}
        }
        if ctx.input(|i|i.key_pressed(egui::Key::Escape)){
            self.curve_start=None;self.curve_draft=None;
            self.curve_outgoing=None;self.curve_drag_anchor=None;
            self.tool=Tool::Select;
            self.resize_drag=None;self.node_drag=None;self.active_node=None;
        }
        if ctx.style().visuals.dark_mode==self.light_theme {
            ui_theme::apply(ctx,self.light_theme);
        }
        if !ctx.wants_keyboard_input(){
            if ctx.input(|i|i.modifiers.command&&i.key_pressed(egui::Key::A)){self.select_all();}
            if ctx.input(|i|i.modifiers.command&&i.key_pressed(egui::Key::N)){self.new_document();}
            if ctx.input(|i|i.modifiers.command&&i.key_pressed(egui::Key::D)){self.duplicate_selection();}
            if ctx.input(|i|i.modifiers.command&&i.key_pressed(egui::Key::G)){
                if ctx.input(|i|i.modifiers.shift){self.ungroup_selection();}
                else{self.group_selection();}
            }
            if ctx.input(|i|i.key_pressed(egui::Key::F4)){self.fit_page();}
            if ctx.input(|i|i.key_pressed(egui::Key::F6)){self.change_tool(Tool::Rect);}
            if ctx.input(|i|i.key_pressed(egui::Key::F7)){self.change_tool(Tool::Ellipse);}
            if ctx.input(|i|i.key_pressed(egui::Key::F8)){self.change_tool(Tool::Text);}
            if ctx.input(|i|i.key_pressed(egui::Key::F10)){self.change_tool(Tool::Nodes);}
            if ctx.input(|i|i.key_pressed(egui::Key::V)){self.change_tool(Tool::Select);}
            if ctx.input(|i|i.key_pressed(egui::Key::B)){self.change_tool(Tool::Curve);}
            if ctx.input(|i|i.key_pressed(egui::Key::L)){self.change_tool(Tool::Line);}
            if ctx.input(|i|i.key_pressed(egui::Key::P)){self.change_tool(Tool::Polygon);}
            if !ctx.input(|i|i.modifiers.command)&&ctx.input(|i|i.key_pressed(egui::Key::F)){
                self.change_tool(Tool::Freehand);
            }
            if ctx.input(|i|i.key_pressed(egui::Key::I)){self.change_tool(Tool::Eyedropper);}
        }
        self.top_workspace(ctx);
        self.tool_sidebar(ctx);
        self.color_palette(ctx);
        if self.show_inspector{self.inspector(ctx);}
        self.workspace_footer(ctx);
        self.canvas(ctx);
        self.welcome_window(ctx);
        self.command_palette(ctx);
        self.text_search_dialog(ctx);
        self.qr_dialog(ctx);
        self.recolor_dialog(ctx);
        self.object_finder_dialog(ctx);
        self.recovery_dialog(ctx);
        self.unsaved_dialog(ctx);
        self.process_pending_action(ctx);
        if self.show_preflight{
            egui::Window::new("Проверка макета")
                .open(&mut self.show_preflight)
                .default_width(600.0)
                .resizable(true).show(ctx,|ui|{
                    ui.label(format!("Минимальное разрешение фотографий: {:.0} DPI",
                        self.minimum_image_dpi));
                    ui.label("Результат является предварительной проверкой, не сертификатом готовности к печати.");
                    ui.separator();
                    egui::ScrollArea::vertical().max_height(450.0).show(ui,|ui|{
                        for item in &self.preflight_lines{ui.label(item);}
                    });
                });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checkpoint_undo_and_redo_keep_document_marked_unsaved(){
        let mut editor=Editor::fresh_workspace();
        let id=editor.doc.add(Shape::Rectangle{
            x:1.,y:1.,w:10.,h:10.
        },Style::default());
        editor.selected=Some(id);
        editor.is_dirty=false;
        editor.checkpoint();
        editor.doc.get_mut(id).unwrap().shape.translate(5.,0.);
        assert!(editor.is_dirty);
        editor.undo();
        assert!(editor.is_dirty);
        editor.redo();
        assert!(editor.is_dirty);
    }
    #[test]
    fn destructive_new_document_action_waits_for_user_decision(){
        let mut editor=Editor::fresh_workspace();
        let before=editor.doc.clone();
        editor.is_dirty=true;
        editor.new_document();
        assert!(editor.show_unsaved_dialog);
        assert!(!editor.run_pending_action);
        assert!(matches!(editor.pending_action,Some(PendingAction::NewDocument)));
        assert_eq!(editor.doc,before);
    }
    #[test]
    fn clean_new_document_action_runs_and_keeps_history(){
        let mut editor=Editor::fresh_workspace();
        let previous=editor.doc.clone();
        editor.new_document();
        assert!(editor.run_pending_action);
        editor.process_pending_action(&egui::Context::default());
        assert!(editor.doc.objects.is_empty());
        assert!(!editor.is_dirty);
        assert_eq!(editor.undo.len(),1);
        editor.undo();
        assert_eq!(editor.doc,previous);
        assert!(editor.is_dirty);
    }
    #[test]
    fn successful_save_clears_dirty_and_recovery_snapshot(){
        let dir=tempfile::tempdir().unwrap();
        let mut editor=Editor::fresh_workspace();
        editor.file=dir.path().join("saved.kdr").to_string_lossy().into_owned();
        editor.is_dirty=true;
        editor.save();
        assert!(!editor.is_dirty);
        assert!(std::path::Path::new(&editor.file).exists());
        assert_eq!(opencorel_core::kdr::read_kdr(&editor.file).unwrap(),editor.doc);
    }
    #[test]
    fn production_workspace_opens_empty_a4_and_keeps_document_valid(){
        let e=Editor::fresh_workspace();
        assert_eq!(e.doc.objects.len(),0);
        assert_eq!(e.doc.pages().len(),1);
        assert_eq!(e.doc.page.width_mm,210.);
        assert_eq!(e.doc.page.height_mm,297.);
        e.doc.validate().unwrap();
    }
    #[test]
    fn align_does_not_corrupt_group_or_add_undo_entry(){
        let mut editor=Editor::default();
        editor.select_all();
        editor.group_selection();
        let before=editor.doc.clone();
        let history=editor.undo.len();
        editor.align_selection(Alignment::Left);
        assert_eq!(editor.doc,before);
        assert_eq!(editor.undo.len(),history);
        assert!(editor.status.contains("Группу"));
    }
    #[test]
    fn numeric_rotation_around_pivot_is_undoable(){
        let mut e=Editor::default();
        let id=e.doc.objects[0].id;
        let original=e.doc.clone();
        e.select_object(Some(id),false);
        e.rotation_degrees=45.0;
        e.custom_pivot=true;e.pivot_x=0.0;e.pivot_y=0.0;
        e.rotate_arbitrary();
        assert_ne!(e.doc,original);
        assert_eq!(e.undo.len(),1);
        assert!(matches!(&e.doc.objects[0].shape,Shape::Polyline{closed:true,..}));
        e.undo();
        assert_eq!(e.doc,original);
    }
    #[test]
    fn partial_group_transform_rejects_atomically(){
        let mut e=Editor::default();
        let a=e.doc.objects[0].id;
        let b=e.doc.add(Shape::Rectangle{x:10.0,y:10.0,w:20.0,h:20.0},Style::default());
        e.doc.group_objects(&[a,b]).unwrap();
        let before=e.doc.clone();
        e.selected=Some(a);e.multi_selected.clear();
        e.rotation_degrees=32.0;
        e.rotate_arbitrary();
        assert_eq!(e.doc,before);
        assert!(e.undo.is_empty());
        assert!(e.status.contains("часть группы"));
    }
    #[test]
    fn group_scale_is_atomic_and_undoable(){
        let mut e=Editor::default();
        let a=e.doc.objects[0].id;
        let b=e.doc.add(Shape::Rectangle{x:800.0,y:100.0,w:10.0,h:10.0},Style::default());
        e.doc.group_objects(&[a,b]).unwrap();
        let before=e.doc.clone();
        e.select_object(Some(a),false);
        e.scale_x_percent=110.0;e.scale_y_percent=110.0;
        e.scale_selected();
        assert_ne!(e.doc,before);
        e.doc.validate().unwrap();
        e.undo();
        assert_eq!(e.doc,before);
    }
    #[test]
    fn group_rotation_transforms_both_members(){
        let mut e=Editor::default();
        let a=e.doc.objects[0].id;
        let b=e.doc.add(Shape::Rectangle{x:5.0,y:5.0,w:20.0,h:20.0},Style::default());
        e.doc.group_objects(&[a,b]).unwrap();
        let before=e.doc.clone();
        e.select_object(Some(a),false);
        e.rotation_degrees=30.0;
        e.rotate_arbitrary();
        assert_ne!(e.doc.objects[0].shape,before.objects[0].shape);
        assert_ne!(e.doc.objects.last().unwrap().shape,before.objects.last().unwrap().shape);
        e.doc.validate().unwrap();
    }
    #[test]
    fn bezier_creation_drag_produces_smooth_cubic_handles(){
        let mut e=Editor::default();
        e.change_tool(Tool::Curve);
        e.curve_anchor(Point{x:10.0,y:10.0},Some(Point{x:10.0,y:0.0}));
        e.curve_anchor(Point{x:80.0,y:10.0},Some(Point{x:0.0,y:10.0}));
        let id=e.curve_draft.unwrap();
        let Shape::Bezier{segments,..}=&e.doc.get(id).unwrap().shape else{panic!()};
        assert_eq!(segments[0].control1,Point{x:20.0,y:10.0});
        assert_eq!(segments[0].control2,Point{x:80.0,y:0.0});
        assert_eq!(segments[0].end,Point{x:80.0,y:10.0});
        e.curve_click(Point{x:100.0,y:60.0});
        let Shape::Bezier{segments,..}=&e.doc.get(id).unwrap().shape else{panic!()};
        assert_eq!(segments[1].control1,Point{x:80.0,y:20.0});
        e.doc.validate().unwrap();
        e.undo();
        let Shape::Bezier{segments,..}=&e.doc.get(id).unwrap().shape else{panic!()};
        assert_eq!(segments.len(),1);
    }
    #[test]
    fn polygon_has_six_vertices(){
        let points=hexagon(Bounds{x:0.0,y:0.0,w:60.0,h:40.0});
        assert_eq!(points.len(),6);
        assert!(points.iter().all(|p|p.x>=0.0&&p.x<=60.0&&p.y>=0.0&&p.y<=40.0));
    }
    #[test]
    fn named_objects_are_kept_during_group_selection(){
        let mut e=Editor::default();
        e.doc.objects[0].name="Заголовок".into();
        let a=e.doc.objects[0].id;
        let b=e.doc.objects[1].id;
        e.doc.group_objects(&[a,b]).unwrap();
        e.select_object(Some(b),false);
        assert_eq!(e.selection_ids().len(),2);
        assert_eq!(e.doc.objects[0].name,"Заголовок");
        e.doc.validate().unwrap();
    }
    #[test]
    fn clicking_one_group_member_selects_entire_group(){
        let mut e=Editor::default();
        let a=e.doc.objects[0].id;
        let b=e.doc.objects[1].id;
        e.doc.group_objects(&[a,b]).unwrap();
        e.select_object(Some(a),false);
        assert_eq!(e.selection_ids().len(),2);
        e.select_object(Some(a),true);
        assert!(e.selection_ids().is_empty());
        e.select_object(Some(b),true);
        assert_eq!(e.selection_ids().len(),2);
    }
    #[test]
    fn resize_corner_keeps_opposite_edge(){
        let mut e=Editor::default();
        let id=e.doc.objects[0].id;
        e.resize_from_corner(id,ResizeCorner::BottomRight,
            Point{x:900.0,y:500.0},false);
        let b=e.doc.objects[0].bounds();
        assert_eq!(b.x,70.0);assert_eq!(b.y,110.0);
        assert_eq!(b.w,830.0);assert_eq!(b.h,390.0);
    }
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
    fn no_text_selection_fails_without_history_change(){
        let mut editor=Editor::default();
        editor.select_object(Some(editor.doc.objects[0].id),false);
        editor.text_to_curves();
        assert!(editor.undo.is_empty());
    }
    #[test]
    fn adjustable_grid_snaps_to_requested_precision(){
        let mut e=Editor::default();
        e.grid_mm=2.5;
        e.snap_grid=true;
        assert_eq!(e.snapped(Point{x:6.2,y:11.4}),Point{x:5.0,y:12.5});
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
impl Editor{
    /// Production startup: a neutral blank A4 document, not the two-shape
    /// fixture used by regression tests. Sample design stays one click away.
    fn fresh_workspace()->Self{
        let mut editor=Self::default();
        let mut doc=Document::default();
        doc.name="Безымянный документ".into();
        doc.page=opencorel_core::Page{width_mm:210.,height_mm:297.};
        editor.doc=doc;
        editor.file="Безымянный.kdr".into();
        editor.show_welcome=true;
        editor.status="Готово к работе · A4 · 210 × 297 мм".into();
        editor.load_symbol_library();
        editor
    }
}
fn main()->eframe::Result<()>{
    recovery::install_panic_hook();
    let options=eframe::NativeOptions{
        viewport:egui::ViewportBuilder::default().with_inner_size([1280.0,780.0])
            .with_min_inner_size([780.0,520.0]),
        ..Default::default()
    };
    eframe::run_native(concat!("OpenCorel v",env!("CARGO_PKG_VERSION"),
        " — Редактор макетов"),options,Box::new(|cc|{
        ui_theme::apply(&cc.egui_ctx,false);
        Ok(Box::new(Editor::fresh_workspace()))
    }))
}
