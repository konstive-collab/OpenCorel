use eframe::egui::{self,Align2,Color32,FontId,Pos2,Rect,Sense,Stroke,Vec2};
use opencorel_core::{Bounds,CubicSegment,Document,Object,Point,Shape,Style,valid_color,valid_paint,Alignment,Distribution};
use std::{f32::consts::TAU,collections::{HashMap,HashSet},hash::{Hash,Hasher},collections::hash_map::DefaultHasher};
mod workspace;
mod tool_dock;
mod ui_theme;
mod productivity;
mod starter_templates;
mod page_commands;
mod array_commands;
use opencorel_core::transform;
use opencorel_core::node_edit::{self,NodeAddress};
mod node_interaction;
mod boolean_commands;
mod compound_commands;
mod clip_commands;
mod masked_canvas;
mod appearance_commands;
use masked_canvas::MaskedCanvasCache;
mod vector_preview;
mod raster_preview;
mod compound_render;
mod text_preview;
mod text_edit_ops;
mod text_studio;
mod pattern_studio;
mod vector_effects;
mod color_studio;
mod smart_shape_studio;
mod smart_shape_catalog;
mod text_path_commands;
mod bezier_preview;
mod bezier_pen;
mod snap_targets;
mod freehand_interaction;
mod selection_interaction;
mod guide_offset;
use opencorel_core::guides::{Guide,GuideAxis};
use compound_render::CompoundCache;
use text_preview::TextPreview;
use bezier_preview::BezierPreviewCache;
use opencorel_typography::TextOutliner;
use vector_preview::VectorCache;
use raster_preview::RasterCache;
use workspace::InspectorTab;

#[derive(Clone,Copy,PartialEq)]
enum Tool { Select, Nodes, Rect, Ellipse, Text, Curve, Freehand, Line, Polygon, Smart }
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

#[derive(Clone,Copy,Debug)]
struct GuideDrag{
    axis:GuideAxis,
    index:Option<usize>,
    position_mm:f32,
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
/// Temporary text editing state. The document is modified only on Apply,
/// so a whole typing session becomes one undoable transaction.
struct InlineTextEdit{
    object_id:u64,
    draft:String,
    initial:String,
    screen_pos:Pos2,
    request_focus:bool,
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
    selection_drag_checkpointed:bool,
    marquee_drag:Option<selection_interaction::MarqueeDrag>,
    creation_start:Option<(Point,u64)>,
    curve_start:Option<Point>,
    curve_outgoing:Option<Point>,
    curve_drag_anchor:Option<Point>,
    curve_draft:Option<u64>,
    /// True only while resuming an existing curve before its first new segment.
    curve_resumed:bool,
    // In-progress pointer samples: no document snapshots during drawing.
    freehand_samples:Vec<Point>,
    freehand_overflow:bool,
    freehand_precision_px:f32,
    freehand_smoothness:f32,
    freehand_closed:bool,
    refit_tolerance_mm:f32,
    array_columns:u32,
    array_rows:u32,
    array_gap_x_mm:f32,
    array_gap_y_mm:f32,
    pattern_copies:u32,
    pattern_radius_mm:f32,
    pattern_step_mm:f32,
    pattern_amplitude_mm:f32,
    pattern_cycles:f32,
    pattern_start_angle:f32,
    pattern_sweep_angle:f32,
    pattern_rotate_objects:bool,
    effect_offset_x_mm:f32,
    effect_offset_y_mm:f32,
    effect_depth:u32,
    effect_width_mm:f32,
    effect_opacity:f32,
    effect_primary:String,
    effect_secondary:String,
    palette_base_color:String,
    palette_find_color:String,
    palette_replacement_color:String,
    palette_use_stroke:bool,
    palette_replace_fill:bool,
    palette_replace_stroke:bool,
    palette_replace_gradients:bool,
    palette_match_count:Option<usize>,
    smart_kind:opencorel_core::smart_shapes::SmartShape,
    smart_search:String,
    smart_inset:f32,
    smart_detail:u8,
    smart_rotation_degrees:f32,
    smart_width_mm:f32,
    smart_height_mm:f32,
    text_path_start_mm:f32,
    text_path_shift_mm:f32,
    text_path_reverse:bool,
    active_layer:u64,
    pan:Vec2,
    undo:Vec<Document>,
    redo:Vec<Document>,
    inspector_tab:InspectorTab,
    show_inspector:bool,
    show_grid:bool,
    snap_grid:bool,
    snap_nodes:bool,
    snap_tolerance_px:f32,
    join_tolerance_mm:f32,
    show_guides:bool,
    snap_guides:bool,
    guide_drag:Option<GuideDrag>,
    /// Coalesces rapid numeric guide edits into one undo step.
    guide_numeric_session:Option<usize>,
    offset_distance_mm:f32,
    grid_mm:f32,
    show_rulers:bool,
    light_theme:bool,
    cursor_mm:Option<Point>,
    fill_cache:FillCache,
    vector_cache:VectorCache,
    raster_cache:RasterCache,
    compound_cache:CompoundCache,
    bezier_preview:BezierPreviewCache,
    masked_canvas:MaskedCanvasCache,
    rotation_degrees:f32,
    skew_x_degrees:f32,
    skew_y_degrees:f32,
    scale_x_percent:f32,
    scale_y_percent:f32,
    appearance_opacity:f32,
    appearance_gradient_mode:u8,
    appearance_gradient_start:String,
    appearance_gradient_end:String,
    appearance_gradient_angle:f32,
    appearance_dash_mm:f32,
    appearance_cap:u8,
    appearance_join:u8,
    custom_pivot:bool,
    pivot_x:f32,
    pivot_y:f32,
    text_outliner:TextOutliner,
    text_preview:TextPreview,
    font_search:String,
    text_find:String,
    text_replacement:String,
    text_studio_wrap_columns:u32,
    text_studio_indent_spaces:u32,
    inline_text:Option<InlineTextEdit>,
    proof_dpi:u32,
    keep_bitmap_ratio:bool,
    show_preflight:bool,
    page_name_edit:String,
    page_rename_mode:bool,
    preflight_lines:Vec<String>,
    minimum_image_dpi:f32,
    show_safe_area:bool,
    safe_margin_mm:f32,
    print_bleed_mm:f32,
    print_crop_marks:bool,
    internal_clipboard:Vec<Object>,
    paste_number:usize,
    show_welcome:bool,
    toolbox_expanded:bool,
    resize_drag:Option<(u64,ResizeCorner)>,
    resize_drag_base:Option<Bounds>,
    resize_drag_checkpointed:bool,
    object_filter:String,
    active_node:Option<(u64,NodeAddress)>,
    node_drag:Option<(u64,NodeAddress)>,
    node_drag_checkpointed:bool,
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
            zoom:1.0,drag_start:None,
            selection_drag_checkpointed:false,marquee_drag:None,
            creation_start:None,
            curve_start:None,curve_draft:None,curve_outgoing:None,curve_drag_anchor:None,
            curve_resumed:false,
            freehand_samples:Vec::new(),freehand_overflow:false,
            freehand_precision_px:2.0,freehand_smoothness:0.85,
            freehand_closed:false,
            refit_tolerance_mm:0.8,
            array_columns:3,array_rows:2,
            array_gap_x_mm:5.0,array_gap_y_mm:5.0,
            pattern_copies:12,pattern_radius_mm:50.0,pattern_step_mm:25.0,
            pattern_amplitude_mm:12.0,pattern_cycles:1.0,
            pattern_start_angle:0.0,pattern_sweep_angle:360.0,
            pattern_rotate_objects:false,
            effect_offset_x_mm:3.0,effect_offset_y_mm:3.0,
            effect_depth:8,effect_width_mm:2.0,effect_opacity:0.65,
            effect_primary:"#192539".into(),effect_secondary:"#32CFB4".into(),
            palette_base_color:"#447BD4".into(),
            palette_find_color:"#FFFFFF".into(),
            palette_replacement_color:"#000000".into(),
            palette_use_stroke:false,palette_replace_fill:true,
            palette_replace_stroke:true,palette_replace_gradients:true,
            palette_match_count:None,
            smart_kind:opencorel_core::smart_shapes::SmartShape::Star5,
            smart_search:String::new(),
            smart_inset:0.42,smart_detail:8,smart_rotation_degrees:0.0,
            smart_width_mm:60.0,smart_height_mm:60.0,
            text_path_start_mm:0.0,text_path_shift_mm:0.0,text_path_reverse:false,
            active_layer:1,pan:Vec2::ZERO,
            undo:Vec::new(),redo:Vec::new(),
            inspector_tab:InspectorTab::Objects,show_inspector:true,
            show_grid:false,snap_grid:false,
            snap_nodes:true,snap_tolerance_px:10.0,
            join_tolerance_mm:2.0,
            show_guides:true,snap_guides:true,guide_drag:None,
            guide_numeric_session:None,
            offset_distance_mm:2.0,
            grid_mm:10.0,
            show_rulers:true,light_theme:false,cursor_mm:None,
            fill_cache:FillCache::default(),vector_cache:VectorCache::default(),
            raster_cache:RasterCache::default(),
            compound_cache:CompoundCache::default(),
            bezier_preview:BezierPreviewCache::default(),
            masked_canvas:MaskedCanvasCache::default(),
            rotation_degrees:30.0,skew_x_degrees:15.0,skew_y_degrees:0.0,
            scale_x_percent:100.0,scale_y_percent:100.0,
            appearance_opacity:1.0,appearance_gradient_mode:0,
            appearance_gradient_start:"#447BD4".into(),
            appearance_gradient_end:"#FFFFFF".into(),
            appearance_gradient_angle:0.0,appearance_dash_mm:0.0,
            appearance_cap:0,appearance_join:0,
            custom_pivot:false,pivot_x:0.0,pivot_y:0.0,
            text_outliner:TextOutliner::new(),text_preview:TextPreview::default(),font_search:String::new(),text_find:String::new(),text_replacement:String::new(),
            text_studio_wrap_columns:48,text_studio_indent_spaces:4,
            inline_text:None,proof_dpi:96,
            keep_bitmap_ratio:true,
            show_preflight:false,preflight_lines:Vec::new(),
            page_name_edit:String::new(),page_rename_mode:false,
            minimum_image_dpi:150.0,
            show_safe_area:false,safe_margin_mm:5.0,
            print_bleed_mm:3.0,print_crop_marks:true,
            internal_clipboard:Vec::new(),paste_number:0,
            show_welcome:true,toolbox_expanded:false,resize_drag:None,
            resize_drag_base:None,resize_drag_checkpointed:false,
            object_filter:String::new(),active_node:None,node_drag:None,
            node_drag_checkpointed:false}
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
fn paint(p:&egui::Painter,obj:&Object,v:View,cache:&mut FillCache,
    vectors:&mut VectorCache,rasters:&mut RasterCache,
    compounds:&mut CompoundCache,bezier_preview:&mut BezierPreviewCache,
    text_preview:&mut TextPreview,
    text_outliner:&mut TextOutliner,ctx:&egui::Context){
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
            if let Some(contours)=text_preview.contours(obj,text_outliner){
                compounds.paint(p,obj.id,contours,v,fill,stroke);
                return;
            }
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
            // Pan/hover redraws reuse sampled geometry. Only shape edits or a
            // zoom-scale change require new adaptive Bezier subdivision.
            let contour=bezier_preview.points(obj.id,*start,segments,v.scale);
            if *closed {cache.fill(p,obj.id,contour,v,fill);}
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
    fn begin_inline_text_edit(&mut self,id:u64,pos:Pos2){
        let Some(object)=self.doc.objects.iter().find(|o|o.id==id) else{return;};
        let editable=self.doc.layer(object.layer_id)
            .is_some_and(|layer|layer.visible&&!layer.locked);
        if !editable {
            self.status="Текст находится на скрытом или заблокированном слое".into();
            return;
        }
        let Shape::Text{value,..}=&object.shape else{return;};
        self.selected=Some(id);
        self.multi_selected.clear();
        self.inline_text=Some(InlineTextEdit{
            object_id:id,draft:value.clone(),initial:value.clone(),
            screen_pos:pos,request_focus:true,
        });
    }
    fn apply_inline_text_edit(&mut self,session:InlineTextEdit){
        if session.draft==session.initial{return;}
        if session.draft.len()>text_edit_ops::MAX_EDITED_TEXT_BYTES{
            self.status="Текст слишком длинный для редактирования".into();
            return;
        }
        let Some(object)=self.doc.objects.iter().find(|o|o.id==session.object_id) else{
            self.status="Объект больше не существует".into();return;
        };
        let editable=self.doc.layer(object.layer_id)
            .is_some_and(|layer|layer.visible&&!layer.locked);
        if !editable {
            self.status="Нельзя изменять текст на заблокированном слое".into();return;
        }
        let Shape::Text{value,..}=&object.shape else{return;};
        if value!=&session.initial{
            self.status="Объект был изменён в другом месте; повторите редактирование".into();
            return;
        }
        let index=self.doc.objects.iter().position(|o|o.id==session.object_id).unwrap();
        self.checkpoint();
        if let Shape::Text{value,..}=&mut self.doc.objects[index].shape {
            *value=session.draft;
        }
        self.text_preview.clear();
        self.status="Текст изменён. Ctrl+Z отменит всю правку".into();
    }
    fn inline_text_window(&mut self,ctx:&egui::Context){
        let Some(mut session)=self.inline_text.take() else{return;};
        let mut apply=false;
        let mut cancel=false;
        let mut open=true;
        let pos=session.screen_pos;
        egui::Window::new("Редактирование текста")
            .id(egui::Id::new("inline_text_editor"))
            .default_pos(pos)
            .default_width(380.0)
            .resizable(true)
            .open(&mut open)
            .show(ctx,|ui|{
                ui.label("Выделяйте текст мышью или клавиатурой. Ввод поддерживает Unicode.");
                let response=ui.add(egui::TextEdit::multiline(&mut session.draft)
                    .desired_width(f32::INFINITY).desired_rows(6));
                if session.request_focus {
                    response.request_focus();
                    session.request_focus=false;
                }
                ui.horizontal(|ui|{
                    if ui.button("Применить").clicked(){apply=true;}
                    if ui.button("Отмена").clicked(){cancel=true;}
                });
                ui.label("Применить — одна запись в истории. Отмена не меняет документ.");
            });
        if ctx.input(|i|i.key_pressed(egui::Key::Escape)){cancel=true;}
        if apply{self.apply_inline_text_edit(session);}
        else if open&&!cancel{self.inline_text=Some(session);}
    }
    fn run_preflight(&mut self){
        let issues=opencorel_core::preflight::check(&self.doc,
            opencorel_core::preflight::PreflightOptions{
                minimum_image_dpi:self.minimum_image_dpi,
                safe_margin_mm:self.safe_margin_mm,
            });
        let warning_count=issues.iter().filter(|i|
            i.severity!=opencorel_core::preflight::Severity::Notice).count();
        self.preflight_lines=issues.iter().map(|issue|format!(
            "{:?} [{}] {}{}",
            issue.severity,issue.code,
            issue.object_id.map(|id|format!("Объект #{id}: ")).unwrap_or_default(),
            issue.message
        )).collect();
        self.status=format!("Проверка макета: замечаний {}, информационных {}",
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
    /// The canvas uses the same measured glyph envelope for visual selection
    /// and picking. Core still owns stacking, clipping and layer permissions.
    fn hit_test_at(&self,p:Point,tolerance:f32)->Option<u64>{
        self.doc.hit_test_with_bounds_filtered(
            p,tolerance,
            |object|self.text_preview.ink_bounds(object)
                .unwrap_or_else(||object.bounds()),
            opencorel_core::hit_testing::hit_object
        )
    }
    fn pick_tolerance_mm(&self,v:View)->f32{
        (6.0/v.scale.max(0.001)).clamp(0.05,20.0)
    }
    fn corner_at(&self,v:View,pos:Pos2)->Option<(u64,ResizeCorner)>{
        if self.tool!=Tool::Select || !self.multi_selected.is_empty(){return None;}
        let id=self.selected?;
        let o=self.doc.objects.iter().find(|o|o.id==id)?;
        let editable=self.doc.layer(o.layer_id)
            .map(|l|l.visible&&!l.locked).unwrap_or(false);
        if !editable||!transform::resizable(&o.shape){return None;}
        let b=self.text_preview.ink_bounds(o).unwrap_or_else(||o.bounds());
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
        if !pointer.x.is_finite()||!pointer.y.is_finite()||
            pointer.x.abs()>100_000.0||pointer.y.abs()>100_000.0{
            self.status="Недопустимая координата изменения размера".into();
            return;
        }
        if !self.doc.layer(self.doc.objects[index].layer_id)
            .is_some_and(|l|l.visible&&!l.locked) ||
            !transform::resizable(&self.doc.objects[index].shape){
            self.status="Размер заблокированного объекта изменить нельзя".into();
            return;
        }
        // A stable source rectangle keeps the opposite corner and Shift ratio
        // unchanged across frames; current-shape bounds cause resize jitter.
        let b=self.resize_drag_base.unwrap_or_else(||self.doc.objects[index].bounds());
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
        let current=self.doc.objects[index].bounds();
        if target.x==current.x&&target.y==current.y&&
            target.w==current.w&&target.h==current.h{return;}
        // Only the first edit needs a candidate clone for transactional
        // validation before the Undo checkpoint. Later frames use in-place
        // resizing; the core transform pre-validates before mutation.
        if self.resize_drag.is_some()&&!self.resize_drag_checkpointed{
            let mut candidate=self.doc.objects[index].shape.clone();
            if let Err(e)=transform::resize_shape(&mut candidate,target){
                self.status=e;return;
            }
            self.checkpoint();
            self.resize_drag_checkpointed=true;
            self.doc.objects[index].shape=candidate;
        }else if let Err(e)=transform::resize_shape(
            &mut self.doc.objects[index].shape,target){
            self.status=e;return;
        }
        self.fill_cache.entries.clear();
        self.vector_cache.clear();
        self.compound_cache.clear();
        self.bezier_preview.clear();
        self.raster_cache.clear();
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
    fn checkpoint(&mut self){
        self.guide_numeric_session=None;
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
            self.bezier_preview.clear();
            self.raster_cache.clear();
            self.selected=None;self.multi_selected.clear();self.active_layer=1;
            self.page_rename_mode=false;
            self.curve_draft=None;self.curve_start=None;
            self.curve_resumed=false;
            self.curve_outgoing=None;self.curve_drag_anchor=None;
            self.freehand_samples.clear();self.freehand_overflow=false;
            self.guide_drag=None;
            self.guide_numeric_session=None;
            self.resize_drag=None;self.node_drag=None;self.active_node=None;
            self.resize_drag_base=None;self.resize_drag_checkpointed=false;
            self.drag_start=None;self.selection_drag_checkpointed=false;
            self.marquee_drag=None;
            self.node_drag_checkpointed=false;
            self.status="Отменено".into();
        }
    }
    fn redo(&mut self){
        if let Some(next)=self.redo.pop(){
            self.undo.push(std::mem::replace(&mut self.doc,next));
            trim_history(&mut self.undo);
            self.fill_cache.entries.clear();
            self.vector_cache.clear();
            self.bezier_preview.clear();
            self.raster_cache.clear();
            self.selected=None;self.multi_selected.clear();self.active_layer=1;
            self.page_rename_mode=false;
            self.curve_draft=None;self.curve_start=None;
            self.curve_resumed=false;
            self.curve_outgoing=None;self.curve_drag_anchor=None;
            self.freehand_samples.clear();self.freehand_overflow=false;
            self.guide_drag=None;
            self.guide_numeric_session=None;
            self.resize_drag=None;self.node_drag=None;self.active_node=None;
            self.resize_drag_base=None;self.resize_drag_checkpointed=false;
            self.drag_start=None;self.selection_drag_checkpointed=false;
            self.marquee_drag=None;
            self.node_drag_checkpointed=false;
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
        if !p.x.is_finite()||!p.y.is_finite()||
            p.x.abs()>100_000.0||p.y.abs()>100_000.0{
            self.status="Координаты кривой выходят за допустимые пределы".into();
            return;
        }
        // Once drawing starts, the curve belongs to its original layer even
        // if the user changes the active layer from the layer panel.
        let layer_id=self.curve_draft
            .and_then(|id|self.doc.objects.iter().find(|o|o.id==id))
            .map(|o|o.layer_id).unwrap_or(self.active_layer);
        if !self.doc.layer(layer_id).is_some_and(|layer|layer.visible&&!layer.locked){
            self.status="Слой кривой скрыт или заблокирован".into();return;
        }
        let Some(start)=self.curve_start else {
            self.curve_outgoing=None;
            self.curve_start=Some(p);
            self.status="Начало кривой: укажите следующую точку".into();
            return;
        };
        let previous=match self.curve_draft.and_then(|id|self.doc.objects.iter().find(|o|o.id==id)){
            Some(Object{shape:Shape::Bezier{segments,..},..})=>
                segments.last().map(|seg|seg.end).unwrap_or(start),
            _=>start,
        };
        if self.curve_resumed && p==previous {
            // Pressing the saved last anchor can start a tangent drag without
            // creating a degenerate zero-length segment in the existing path.
            self.status="Последний узел выбран: потяните для касательной либо щёлкните дальше".into();
            return;
        }
        let seg=CubicSegment{
            control1:self.curve_outgoing.take().unwrap_or(Point{
                x:previous.x+(p.x-previous.x)/3.0,
                y:previous.y+(p.y-previous.y)/3.0,
            }),
            control2:Point{
                x:previous.x+2.0*(p.x-previous.x)/3.0,
                y:previous.y+2.0*(p.y-previous.y)/3.0,
            },
            end:p,
        };
        if let Some(id)=self.curve_draft{
            // Existing curves checkpoint on the first actual new segment.
            // A newly-created curve was already checkpointed by add_shape.
            let appendable=self.doc.objects.iter().find(|obj|obj.id==id)
                .is_some_and(|obj|matches!(&obj.shape,
                    Shape::Bezier{segments,closed:false,..}
                    if segments.len()<10_000));
            if !appendable {
                self.curve_start=None;self.curve_draft=None;
                self.curve_resumed=false;
                self.status="Кривая недоступна, замкнута или содержит слишком много узлов".into();
                return;
            }
            if self.curve_resumed {
                self.checkpoint();
                self.curve_resumed=false;
            }
            if let Some(obj)=self.doc.get_mut(id){
                if let Shape::Bezier{segments,..}=&mut obj.shape{
                    segments.push(seg);
                }
            }
        }else{
            let style=Style{fill:"#000000".into(),stroke:"#202020".into(),stroke_width_mm:0.8};
            self.curve_draft=self.add_shape(Shape::Bezier{start,segments:vec![seg],closed:false},style);
        }
        self.status="Точка добавлена; двойной щелчок завершает кривую".into();
    }
    /// Dragging an anchor controls its incoming and outgoing Bézier handles.
    /// One history checkpoint is still taken when the first segment is added.
    fn curve_drag_to(&mut self,cursor:Point){
        let Some(anchor)=self.curve_drag_anchor else{return;};
        let Some((incoming,outgoing))=bezier_pen::symmetric_tangent(anchor,cursor) else{
            self.status="Касательная выходит за пределы допустимых координат".into();
            return;
        };
        if let Some(id)=self.curve_draft{
            let Some(object)=self.doc.objects.iter().find(|o|o.id==id) else{return;};
            if !self.doc.layer(object.layer_id)
                .is_some_and(|layer|layer.visible&&!layer.locked){return;}
            let affects_existing=matches!(&object.shape,Shape::Bezier{segments,..}
                if segments.last().is_some_and(|last|
                    last.end==anchor && last.control2!=incoming));
            if affects_existing && self.curve_resumed{
                self.checkpoint();
                self.curve_resumed=false;
            }
            let Some(object)=self.doc.get_mut(id) else{return;};
            if let Shape::Bezier{segments,..}=&mut object.shape{
                if let Some(last)=segments.last_mut(){
                    if last.end==anchor {
                        last.control2=incoming;
                    }
                }
            }
        }
        self.curve_outgoing=Some(outgoing);
    }

    fn curve_finish(&mut self,close:bool){
        if close{
            if let Some(id)=self.curve_draft{
                let can_close=self.doc.objects.iter().find(|o|o.id==id)
                    .is_some_and(|o|self.doc.layer(o.layer_id)
                        .is_some_and(|l|l.visible&&!l.locked)
                        && matches!(&o.shape,Shape::Bezier{segments,closed:false,..}
                            if segments.len()>=2));
                if can_close {
                    if self.curve_resumed {
                        self.checkpoint();
                        self.curve_resumed=false;
                    }
                    if let Some(obj)=self.doc.get_mut(id){
                        if let Shape::Bezier{closed,..}=&mut obj.shape{
                            *closed=true;
                        }
                    }
                }
            }
        }
        self.curve_start=None;
        self.curve_draft=None;
        self.curve_resumed=false;
        self.curve_outgoing=None;
        self.curve_drag_anchor=None;
        self.tool=Tool::Select;
        self.status=if close{"Контур замкнут"}else{"Кривая завершена"}.into();
    }
    fn import_svg(&mut self){self.open();}
    fn save(&mut self){
        // New documents have only a suggested filename. Ask where to save
        // instead of silently writing into an unpredictable working directory.
        if !std::path::Path::new(&self.file).is_absolute(){
            self.save_as_picker();return;
        }
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
                    format!("Сохранено: {}",chosen.display())
                },
                Err(e)=>format!("Ошибка сохранения: {e}"),
            };
        }
    }
    /// Export each page as its own numbered vector file, without touching
    /// the native KDR and without silently dropping inactive pages.
    fn export_all_pages_dialog(&mut self,ext:&str){
        if !matches!(ext,"svg"|"pdf"){return;}
        let mut documents=Vec::with_capacity(self.doc.page_count());
        for page_index in 0..self.doc.page_count(){
            match self.doc.page_document(page_index){
                Ok(doc)=>documents.push(doc),
                Err(e)=>{
                    self.status=format!("Ошибка страницы {}: {e}",page_index+1);
                    return;
                }
            }
        }
        let Some(folder)=rfd::FileDialog::new()
            .set_title("Выберите папку для экспорта всех страниц")
            .pick_folder() else{return;};
        let stem=std::path::Path::new(&self.file).file_stem()
            .and_then(|v|v.to_str()).unwrap_or("OpenCorel");
        let safe_stem:String=stem.chars()
            .filter(|c|c.is_alphanumeric()||matches!(*c,'-'|'_'|' '))
            .take(48).collect();
        let folder_name=if safe_stem.trim().is_empty(){
            "OpenCorel - страницы".to_string()
        }else{format!("{safe_stem} - страницы")};
        let target=folder.join(folder_name);
        if let Err(e)=std::fs::create_dir(&target){
            self.status=format!("Не удалось создать отдельную папку экспорта: {e}. Выберите другое место.");
            return;
        }
        for (i,document) in documents.iter().enumerate(){
            let path=target.join(format!("page-{:03}.{ext}",i+1));
            if let Err(e)=opencorel_formats::save_document(document,&path){
                self.status=format!("Экспорт остановлен на странице {}: {e}. Ранее созданные файлы остались в {}",
                    i+1,target.display());
                return;
            }
        }
        self.status=format!("Экспортированы все {} страниц в {} ({}). Каждый файл — отдельная страница, не единый PDF.",
            documents.len(),target.display(),ext.to_uppercase());
    }

    fn export_print_sheet_dialog(&mut self){
        let opts=opencorel_core::print_sheet::PrintSheetOptions{
            bleed_mm:self.print_bleed_mm,
            crop_marks:self.print_crop_marks,
        };
        if let Err(e)=opts.validate(){
            self.status=format!("Ошибка параметров печатного SVG: {e}");
            return;
        }
        let stem=std::path::Path::new(&self.file).file_stem()
            .and_then(|s|s.to_str()).unwrap_or("Макет");
        if let Some(chosen)=rfd::FileDialog::new()
            .add_filter("SVG для печатного листа",&["svg"])
            .set_file_name(format!("{stem} - печать.svg"))
            .save_file(){
            let path=chosen.with_extension("svg");
            self.status=match opencorel_core::print_sheet::export_print_svg(
                &self.doc,&path,opts){
                Ok(())=>format!("Печатный SVG экспортирован: {}. Цвет RGB, не PDF/X",
                    path.display()),
                Err(e)=>format!("Ошибка экспорта печатного SVG: {e}"),
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
                _=>opencorel_formats::save_document(&self.doc,&path),
            };
            self.status=match result{
                Ok(())=>format!("Экспортировано: {} ({}). {}",
                    path.display(),
                    if ext=="pdf"{"PDF RGB, не PDF/X"}else{"контрольный файл"},
                    if self.doc.page_count()>1{
                        if ext=="pdf"{format!("Все {} страницы в одном PDF.",self.doc.page_count())}
                        else{format!("Экспортирована только активная страница {} из {}.",
                            self.doc.active_page_index+1,self.doc.page_count())}
                    }else{String::new()}),
                Err(e)=>format!("Ошибка экспорта: {e}"),
            };
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
                self.resize_drag=None;self.node_drag=None;self.active_node=None;
            self.resize_drag_base=None;self.resize_drag_checkpointed=false;
            self.drag_start=None;self.selection_drag_checkpointed=false;
            self.marquee_drag=None;
            self.node_drag_checkpointed=false;
                self.fill_cache.entries.clear();self.vector_cache.clear();
                self.compound_cache.clear();
                self.raster_cache.clear();
                self.curve_start=None;self.curve_draft=None;
                self.curve_resumed=false;
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
        if self.snap_grid && self.grid_mm.is_finite() && self.grid_mm>0.0 {
            Point{x:(p.x/self.grid_mm).round()*self.grid_mm,
                y:(p.y/self.grid_mm).round()*self.grid_mm}
        }else{p}
    }
    /// Pixel-consistent anchor snap, with the grid as a fallback. The object
    /// being edited is excluded so its nearby handles cannot trap a drag.
    fn snap_canvas_point(&self,p:Point,scale:f32,excluded_id:Option<u64>)->Point{
        if scale.is_finite() && scale>0.0{
            let tolerance=(self.snap_tolerance_px/scale).min(500.0);
            if self.snap_nodes{
                if let Some(hit)=snap_targets::nearest_anchor(
                    &self.doc,p,tolerance,excluded_id){
                    return hit.point;
                }
            }
            if self.show_guides && self.snap_guides{
                let guided=opencorel_core::guides::snap(p,&self.doc.guides,tolerance);
                let (x,y)=opencorel_core::guides::snapped_axes(
                    p,&self.doc.guides,tolerance);
                if x||y{
                    let grid=self.snapped(p);
                    return Point{x:if x{guided.x}else{grid.x},
                        y:if y{guided.y}else{grid.y}};
                }
            }
        }
        self.snapped(p)
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
    /// Preserve the document position under the pointer while wheel-zooming.
    /// Avoids the page-center zoom jump that made precision drawing difficult.
    fn zoom_canvas_at(&mut self,working:Rect,cursor:Pos2,scroll:f32){
        if !scroll.is_finite()||scroll.abs()<0.01||!working.contains(cursor){
            return;
        }
        let before=View::new(working,&self.doc,self.zoom,self.pan);
        let anchor=before.mm(cursor);
        let factor=(1.0+scroll*0.002).clamp(0.8,1.25);
        let next_zoom=(self.zoom*factor).clamp(0.2,8.0);
        if next_zoom==self.zoom{return;}
        self.zoom=next_zoom;
        let after=View::new(working,&self.doc,self.zoom,self.pan);
        self.pan+=cursor-after.screen(anchor);
    }
    fn canvas(&mut self,ctx:&egui::Context){
        egui::CentralPanel::default().show(ctx,|ui|{
            let response=ui.allocate_response(ui.available_size(),Sense::click_and_drag());
            let painter=ui.painter_at(response.rect);
            painter.rect_filled(response.rect,0.0,
                if self.light_theme{Color32::from_rgb(222,227,234)}
                else{Color32::from_rgb(15,19,26)});
            let working=if self.show_rulers{
                Rect::from_min_max(response.rect.min+Vec2::new(25.0,24.0),response.rect.max)
            }else{response.rect};
            if response.dragged_by(egui::PointerButton::Middle){
                self.pan+=ctx.input(|i|i.pointer.delta());
            }
            if response.hovered(){
                let scroll=ctx.input(|i|i.smooth_scroll_delta.y);
                if let Some(cursor)=response.hover_pos(){
                    self.zoom_canvas_at(working,cursor,scroll);
                }
            }
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
                            &mut self.compound_cache,&mut self.bezier_preview,
                            &mut self.text_preview,
                            &mut self.text_outliner,ctx);
                    }
                }
            }else{
                // Cached SVG proof uses the same clip paths as PDF/raster output.
                self.masked_canvas.paint(ctx,&page_painter,&self.doc,v.rect);
            }
            // Construction guides stay visible above artwork and are never exported.
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
                    let bounds=self.text_preview.ink_bounds(o).unwrap_or_else(||o.bounds());
                    outline(&page_painter,v.bounds(bounds).expand(3.0),
                        Color32::from_rgb(22,175,148),1.5);
                    if self.tool==Tool::Select && self.multi_selected.is_empty() &&
                        transform::resizable(&o.shape) &&
                        bounds.w>0.1 && bounds.h>0.1 {
                        for corner in [ResizeCorner::TopLeft,ResizeCorner::TopRight,
                            ResizeCorner::BottomLeft,ResizeCorner::BottomRight]{
                            let p=v.screen(corner.point(bounds));
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
            if self.tool==Tool::Select{
                self.paint_marquee(&page_painter,v);
            }
            if self.tool==Tool::Nodes{
                self.paint_node_handles(&page_painter,v);
            }
            if self.tool==Tool::Curve {
                if let Some(start)=self.curve_start {
                    let last=self.curve_draft
                        .and_then(|id|self.doc.objects.iter().find(|o|o.id==id))
                        .and_then(|o|match &o.shape{
                            Shape::Bezier{segments,..}=>segments.last().map(|seg|seg.end),
                            _=>None,
                        }).unwrap_or(start);
                    let anchor=v.screen(last);
                    page_painter.circle_filled(anchor,3.0,
                        Color32::from_rgb(20,161,139));
                    if let Some(handle)=self.curve_outgoing{
                        page_painter.line_segment([anchor,v.screen(handle)],
                            Stroke::new(1.0,Color32::from_rgb(137,123,218)));
                        page_painter.circle_filled(v.screen(handle),3.5,
                            Color32::from_rgb(137,123,218));
                    }
                    if let Some(cursor)=response.hover_pos().filter(|p|v.rect.contains(*p)){
                        let next=self.snap_canvas_point(v.mm(cursor),v.scale,
                            self.curve_draft);
                        let vertices=bezier_pen::preview_segment(
                            last,self.curve_outgoing,next)
                            .into_iter().map(|point|v.screen(point)).collect();
                        page_painter.add(egui::Shape::line(vertices,
                            Stroke::new(1.2,Color32::from_rgb(20,161,139))));
                        page_painter.circle_stroke(v.screen(next),3.0,
                            Stroke::new(1.0,Color32::from_rgb(20,161,139)));
                    }
                }
            }
            if self.tool==Tool::Freehand && self.freehand_samples.len()>=2 {
                let vertices:Vec<Pos2>=self.freehand_samples.iter()
                    .map(|p|v.screen(*p)).collect();
                page_painter.add(egui::Shape::line(vertices,
                    Stroke::new(1.5,Color32::from_rgb(23,177,161))));
            }
            // A small diamond marks the actual snapped anchor so mouse input
            // has visible feedback at any zoom. Hidden layers are ignored.
            if self.snap_nodes && matches!(self.tool,Tool::Curve|Tool::Nodes){
                if let Some(cursor)=response.hover_pos().filter(|p|v.rect.contains(*p)){
                    let excluded=if self.tool==Tool::Curve{
                        self.curve_draft
                    }else{
                        self.selected
                    };
                    if let Some(hit)=snap_targets::nearest_anchor(
                        &self.doc,v.mm(cursor),
                        (self.snap_tolerance_px/v.scale.max(0.001)).min(500.0),
                        excluded){
                        let center=v.screen(hit.point);
                        let diamond=vec![
                            center+Vec2::new(0.,-6.),
                            center+Vec2::new(6.,0.),
                            center+Vec2::new(0.,6.),
                            center+Vec2::new(-6.,0.),
                            center+Vec2::new(0.,-6.),
                        ];
                        page_painter.add(egui::Shape::line(diamond,
                            Stroke::new(1.8,Color32::from_rgb(219,133,46))));
                    }
                }
            }
            // A draft is open: prevent clicks behind the popup from modifying
            // the document, while the canvas continues to render.
            if self.inline_text.is_some(){return;}
            if response.drag_started_by(egui::PointerButton::Primary){
                if let Some(pos)=response.interact_pointer_pos(){
                    let press=ctx.input(|i|i.pointer.press_origin()).unwrap_or(pos);
                    let guide_started=self.begin_guide_drag(
                        v,response.rect,press,ctx.input(|i|i.modifiers.alt));
                    if !guide_started && v.rect.contains(pos){
                        let p=if matches!(self.tool,
                            Tool::Line|Tool::Rect|Tool::Ellipse|Tool::Polygon|Tool::Smart){
                            self.snap_canvas_point(v.mm(pos),v.scale,None)
                        }else{
                            self.snapped(v.mm(pos))
                        };
                        match self.tool{
                            Tool::Nodes=>self.begin_node_drag(v,pos),
                            Tool::Freehand=>{
                                let press=ctx.input(|i|i.pointer.press_origin())
                                    .unwrap_or(pos);
                                let start=self.snap_canvas_point(v.mm(press),
                                    v.scale,None);
                                self.freehand_begin(start);
                                self.freehand_extend(v.mm(pos),v.scale);
                            },
                            Tool::Curve=>{
                                let press=ctx.input(|i|i.pointer.press_origin())
                                    .unwrap_or(pos);
                                if v.rect.contains(press){
                                    let anchor=self.snap_canvas_point(v.mm(press),
                                        v.scale,self.curve_draft);
                                    self.curve_click(anchor);
                                    let valid_draft=self.curve_draft
                                        .and_then(|id|self.doc.objects.iter().find(|o|o.id==id))
                                        .is_some_and(|o|match &o.shape{
                                            Shape::Bezier{segments,..}=>segments.last()
                                                .is_some_and(|segment|segment.end==anchor),
                                            _=>false,
                                        });
                                    if valid_draft ||
                                        (self.curve_draft.is_none()
                                         && self.curve_start==Some(anchor)){
                                        self.curve_drag_anchor=Some(anchor);
                                    }
                                }
                            },
                            Tool::Select=>{
                                if let Some((id,corner))=self.corner_at(v,press){
                                    self.resize_drag=Some((id,corner));
                                    self.resize_drag_base=self.doc.objects.iter()
                                        .find(|o|o.id==id).map(|o|o.bounds());
                                    self.resize_drag_checkpointed=false;
                                }else{
                                    let additive=ctx.input(|i|i.modifiers.shift);
                                    let hit=self.hit_test_at(v.mm(press),self.pick_tolerance_mm(v));
                                    if let Some(id)=hit{
                                        if !additive{
                                            if !self.is_selected(id){
                                                self.select_object(Some(id),false);
                                            }
                                            self.begin_move_selection(id,
                                                self.snapped(v.mm(press)));
                                        }
                                    }else{
                                        self.begin_marquee(v.mm(press),additive);
                                    }
                                }
                            }
                            Tool::Smart=>self.start_smart_creation(p),
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
                    let now=if self.tool==Tool::Nodes &&
                        self.node_drag.is_some_and(|(_,address)|!address.is_handle()){
                        self.snap_canvas_point(v.mm(pos),v.scale,self.selected)
                    }else if matches!(self.tool,
                        Tool::Line|Tool::Rect|Tool::Ellipse|Tool::Polygon|Tool::Smart) &&
                        self.creation_start.is_some(){
                        self.snap_canvas_point(v.mm(pos),v.scale,
                            self.creation_start.map(|(_,id)|id))
                    }else{self.snapped(v.mm(pos))};
                    if self.guide_drag.is_some(){
                        self.update_guide_drag(v.mm(pos));
                    }else if self.marquee_drag.is_some(){
                        self.update_marquee(v.mm(pos));
                    }else if self.tool==Tool::Freehand {
                        self.freehand_extend(v.mm(pos),v.scale);
                    }else if self.tool==Tool::Curve && self.curve_drag_anchor.is_some(){
                        self.curve_drag_to(now);
                    }else if self.node_drag.is_some(){
                        self.drag_node_to(now);
                    }else if let Some((id,corner))=self.resize_drag{
                        self.resize_from_corner(id,corner,now,
                            ctx.input(|i|i.modifiers.shift));
                    }else if let Some((p,id))=self.creation_start{
                        let x=p.x.min(now.x);let y=p.y.min(now.y);
                        let w=(p.x-now.x).abs().max(0.1);
                        let h=(p.y-now.y).abs().max(0.1);
                        if self.tool==Tool::Smart{
                            self.resize_smart_creation(id,Bounds{x,y,w,h});
                        }else if let Some(obj)=self.doc.get_mut(id){
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
                    }else if self.drag_start.is_some(){
                        self.move_selection_drag(now);
                    }
                }
            }
            if response.drag_stopped_by(egui::PointerButton::Primary){
                if self.marquee_drag.is_some(){self.finish_marquee();}
                if let Some(drag)=self.guide_drag.take(){
                    self.commit_guide_drag(drag);
                }else if self.tool==Tool::Freehand {
                    let last=response.interact_pointer_pos().map(|pos|
                        self.snap_canvas_point(v.mm(pos),v.scale,None));
                    self.freehand_finish(last,v.scale);
                }
                if self.tool==Tool::Curve && self.curve_drag_anchor.is_some(){
                    if let Some(pos)=response.interact_pointer_pos(){
                        self.curve_drag_to(self.snapped(v.mm(pos)));
                    }
                }
                self.curve_drag_anchor=None;
                self.drag_start=None;self.creation_start=None;
                self.selection_drag_checkpointed=false;self.marquee_drag=None;
                self.resize_drag=None;self.node_drag=None;
                self.resize_drag_base=None;self.resize_drag_checkpointed=false;
                self.node_drag_checkpointed=false;
            }
            if response.double_clicked(){
                if self.tool==Tool::Select{
                    if let Some(pos)=response.interact_pointer_pos(){
                        if v.rect.contains(pos){
                            if let Some(id)=self.hit_test_at(v.mm(pos),self.pick_tolerance_mm(v)){
                                self.begin_inline_text_edit(id,pos);
                            }
                        }
                    }
                }
                if self.tool==Tool::Nodes{
                    if let Some(pos)=response.interact_pointer_pos(){
                        if v.rect.contains(pos){self.insert_node_at_pointer(v,pos);}
                    }
                }
                if self.tool==Tool::Curve{
                    self.curve_finish(false);
                }
            }
            if response.clicked() && !response.double_clicked(){
                if let Some(pos)=response.interact_pointer_pos(){
                    if v.rect.contains(pos){
                        let p=self.snapped(v.mm(pos));
                        let shape=match self.tool{
                            Tool::Nodes=>{self.select_node_near(v,pos);return;},
                            Tool::Freehand=>{return;},
                            Tool::Select=>{
                                let id=self.hit_test_at(v.mm(pos),self.pick_tolerance_mm(v));
                                let shift=ctx.input(|i|i.modifiers.shift);
                                if shift{self.select_object(id,true);}
                                else{
                                    // Plain click collapses a previous multi-selection;
                                    // drag on a selected object still moves the group.
                                    self.select_object(id,false);
                                }
                                return;
                            },
                            Tool::Curve=>{
                                let p=self.snap_canvas_point(v.mm(pos),v.scale,
                                    self.curve_draft);
                                let closing=self.curve_start.is_some_and(|start|{
                                    let segments=self.curve_draft
                                        .and_then(|id|self.doc.objects.iter().find(|o|o.id==id))
                                        .and_then(|obj|match &obj.shape{
                                            Shape::Bezier{segments,..}=>Some(segments.len()),
                                            _=>None,
                                        }).unwrap_or(0);
                                    segments>=2 &&
                                        (v.screen(start)-pos).length_sq()<=100.0
                                });
                                if closing{self.curve_finish(true);}
                                else{self.curve_click(p);}
                                return;
                            },
                            Tool::Smart=>{
                                self.place_smart_at(p);
                                return;
                            },
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
        let undo=ctx.input(|i|i.modifiers.command&&i.key_pressed(egui::Key::Z)&&!i.modifiers.shift);
        let redo=ctx.input(|i|i.modifiers.command&&(i.key_pressed(egui::Key::Y) ||
            (i.key_pressed(egui::Key::Z)&&i.modifiers.shift)));
        if self.inline_text.is_none(){
            if undo{self.undo();}
            if redo{self.redo();}
        }
        let save_as=ctx.input(|i|i.modifiers.command&&i.modifiers.shift
            && i.key_pressed(egui::Key::S));
        if self.inline_text.is_none(){
            if save_as{self.save_as_picker();}
            else if ctx.input(|i|i.modifiers.command&&i.key_pressed(egui::Key::S)){self.save();}
            if ctx.input(|i|i.modifiers.command&&i.key_pressed(egui::Key::O)){self.open_picker();}
        }
        if self.inline_text.is_none() && !ctx.wants_keyboard_input(){
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
        if self.inline_text.is_none() && !ctx.wants_keyboard_input(){
            let ctrl=ctx.input(|i|i.modifiers.command);
            if ctrl&&ctx.input(|i|i.key_pressed(egui::Key::C)){self.copy_selection();}
            if ctrl&&ctx.input(|i|i.key_pressed(egui::Key::X)){self.cut_selection();}
            if ctrl&&ctx.input(|i|i.key_pressed(egui::Key::V)){self.paste_selection();}
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
        if ctx.input(|i|i.key_pressed(egui::Key::F1)){self.show_welcome=true;}
        if self.inline_text.is_none() && !ctx.wants_keyboard_input() &&
            ctx.input(|i|i.key_pressed(egui::Key::F9)){
            self.show_inspector=!self.show_inspector;
        }
        if self.inline_text.is_none() && !ctx.wants_keyboard_input() && ctx.input(|i|i.key_pressed(egui::Key::Delete)){
            if self.tool==Tool::Nodes {self.remove_active_node();}
            else{self.delete_selection();}
        }
        if self.inline_text.is_none() && !ctx.wants_keyboard_input() &&
            self.tool==Tool::Curve && ctx.input(|i|i.key_pressed(egui::Key::Enter)){
            self.curve_finish(false);
        }
        if self.inline_text.is_none() && ctx.input(|i|i.key_pressed(egui::Key::Escape)){
            self.curve_start=None;self.curve_draft=None;self.curve_resumed=false;
            self.tool=Tool::Select;
            self.resize_drag=None;self.node_drag=None;self.active_node=None;
            self.resize_drag_base=None;self.resize_drag_checkpointed=false;
            self.drag_start=None;self.selection_drag_checkpointed=false;
            self.marquee_drag=None;
            self.node_drag_checkpointed=false;
            self.curve_outgoing=None;self.curve_drag_anchor=None;
            self.freehand_samples.clear();self.freehand_overflow=false;
        }
        if ctx.style().visuals.dark_mode==self.light_theme {
            ui_theme::apply(ctx,self.light_theme);
        }
        if self.inline_text.is_none() && !ctx.wants_keyboard_input(){
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
            if ctx.input(|i|i.key_pressed(egui::Key::H)){self.change_tool(Tool::Freehand);}
            if ctx.input(|i|i.key_pressed(egui::Key::L)){self.change_tool(Tool::Line);}
            if ctx.input(|i|i.key_pressed(egui::Key::P)){self.change_tool(Tool::Polygon);}
            if ctx.input(|i|i.key_pressed(egui::Key::S)){
                self.change_tool(Tool::Smart);
            }
        }
        self.top_workspace(ctx);
        self.tool_sidebar(ctx);
        self.color_palette(ctx);
        if self.show_inspector{self.inspector(ctx);}
        self.workspace_footer(ctx);
        self.canvas(ctx);
        self.inline_text_window(ctx);
        self.welcome_window(ctx);
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
    fn wheel_zoom_preserves_document_coordinate_under_mouse(){
        let mut e=Editor::default();
        e.doc=Document::default();
        let canvas=Rect::from_min_size(Pos2::ZERO,Vec2::new(1200.,800.));
        let cursor=Pos2::new(335.,249.);
        let before=View::new(canvas,&e.doc,e.zoom,e.pan).mm(cursor);
        e.zoom_canvas_at(canvas,cursor,180.0);
        assert!(e.zoom>1.0);
        let after=View::new(canvas,&e.doc,e.zoom,e.pan).mm(cursor);
        assert!((before.x-after.x).abs()<0.0001);
        assert!((before.y-after.y).abs()<0.0001);
        let unchanged=(e.zoom,e.pan);
        e.zoom_canvas_at(canvas,cursor,f32::NAN);
        assert_eq!((e.zoom,e.pan),unchanged);
        e.zoom_canvas_at(canvas,Pos2::new(-10.,-10.),120.0);
        assert_eq!((e.zoom,e.pan),unchanged);
    }
    #[test]
    fn guide_intersection_snaps_both_axes_and_grid_fills_unsnapped_axis(){
        let mut e=Editor::default();
        e.doc=Document::default();
        e.snap_nodes=false;
        e.snap_guides=true;
        e.snap_grid=true;e.grid_mm=10.0;e.snap_tolerance_px=10.0;
        e.doc.guides.push(Guide::new(GuideAxis::Vertical,13.0).unwrap());
        e.doc.guides.push(Guide::new(GuideAxis::Horizontal,21.0).unwrap());
        assert_eq!(e.snap_canvas_point(Point{x:12.8,y:20.4},10.0,None),
            Point{x:13.0,y:21.0});
        assert_eq!(e.snap_canvas_point(Point{x:12.8,y:17.1},10.0,None),
            Point{x:13.0,y:20.0});
        e.snap_guides=false;
        assert_eq!(e.snap_canvas_point(Point{x:12.8,y:20.4},10.0,None),
            Point{x:10.0,y:20.0});
    }
    #[test]
    fn hidden_guides_never_capture_mouse_or_snap(){
        let mut e=Editor::default();
        e.doc=Document::default();
        e.snap_nodes=false;e.snap_grid=false;
        e.snap_guides=true;e.show_guides=false;
        e.snap_tolerance_px=10.0;
        e.doc.guides.push(Guide::new(GuideAxis::Vertical,12.0).unwrap());
        let cursor=Point{x:12.3,y:35.0};
        assert_eq!(e.snap_canvas_point(cursor,10.0,None),cursor);
        e.show_guides=true;
        assert_eq!(e.snap_canvas_point(cursor,10.0,None),
            Point{x:12.0,y:35.0});
    }
    #[test]
    fn visible_anchor_priority_exceeds_nearby_guide(){
        let mut e=Editor::default();
        e.doc=Document::default();
        e.snap_nodes=true;e.snap_guides=true;e.snap_grid=false;
        e.snap_tolerance_px=10.0;
        e.doc.guides.push(Guide::new(GuideAxis::Vertical,13.0).unwrap());
        e.doc.add(Shape::Polyline{points:vec![
            Point{x:12.5,y:18.0},Point{x:30.0,y:20.0},
        ],closed:false},Style::default());
        assert_eq!(e.snap_canvas_point(Point{x:12.7,y:18.1},10.0,None),
            Point{x:12.5,y:18.0});
    }
    #[test]
    fn screen_tolerance_stays_constant_across_zoom_and_grid_falls_back(){
        let mut e=Editor::default();
        e.doc=Document::default();
        e.snap_nodes=true;e.snap_grid=true;
        e.grid_mm=10.0;e.snap_tolerance_px=10.0;
        let id=e.doc.add(Shape::Polyline{
            points:vec![Point{x:12.3,y:15.7},Point{x:35.,y:40.}],
            closed:false,
        },Style::default());
        let near=Point{x:12.6,y:15.7};
        assert_eq!(e.snap_canvas_point(near,5.0,None),Point{x:12.3,y:15.7});
        assert_eq!(e.snap_canvas_point(near,50.0,None),Point{x:10.,y:20.});
        assert_eq!(e.snap_canvas_point(near,5.0,Some(id)),Point{x:10.,y:20.});
        e.snap_grid=false;e.snap_nodes=false;
        assert_eq!(e.snap_canvas_point(near,5.0,None),near);
    }
    #[test]
    fn snapping_to_visible_locked_reference_is_allowed_but_not_hidden(){
        let mut e=Editor::default();
        e.doc=Document::default();
        e.snap_nodes=true;e.snap_grid=false;e.snap_tolerance_px=10.0;
        let target=Point{x:17.0,y:21.0};
        e.doc.add(Shape::Polyline{points:vec![target],closed:false},
            Style::default());
        e.doc.layers[0].locked=true;
        assert_eq!(e.snap_canvas_point(Point{x:17.4,y:21.2},5.0,None),target);
        e.doc.layers[0].visible=false;
        assert_eq!(e.snap_canvas_point(Point{x:17.4,y:21.2},5.0,None),
            Point{x:17.4,y:21.2});
    }
    #[test]
    fn bezier_drag_shapes_smooth_handles_without_extra_undo(){
        let mut editor=Editor::default();
        editor.change_tool(Tool::Curve);
        let p0=Point{x:10.0,y:20.0};
        let p1=Point{x:50.0,y:40.0};
        editor.curve_click(p0);
        editor.curve_drag_anchor=Some(p0);
        editor.curve_drag_to(Point{x:22.0,y:24.0});
        assert_eq!(editor.curve_outgoing,Some(Point{x:22.0,y:24.0}));
        editor.curve_drag_anchor=None;
        editor.curve_click(p1);
        let id=editor.curve_draft.unwrap();
        editor.curve_drag_anchor=Some(p1);
        editor.curve_drag_to(Point{x:58.0,y:50.0});
        let obj=editor.doc.objects.iter().find(|o|o.id==id).unwrap();
        let Shape::Bezier{segments,..}=&obj.shape else{panic!("curve")};
        assert_eq!(segments[0].control1,Point{x:22.0,y:24.0});
        assert_eq!(segments[0].control2,Point{x:42.0,y:30.0});
        assert_eq!(editor.curve_outgoing,Some(Point{x:58.0,y:50.0}));
        assert_eq!(editor.undo.len(),1);
        editor.curve_drag_anchor=None;
        editor.curve_click(Point{x:80.0,y:40.0});
        let Shape::Bezier{segments,..}=&editor.doc.objects.iter()
            .find(|o|o.id==id).unwrap().shape else{panic!("curve")};
        assert_eq!(segments[1].control1,Point{x:58.0,y:50.0});
        assert_eq!(editor.undo.len(),1);
        editor.doc.validate().unwrap();
    }
    #[test]
    fn bezier_click_starts_cusp_and_does_not_reuse_old_tangent(){
        let mut editor=Editor::default();
        editor.change_tool(Tool::Curve);
        editor.curve_click(Point{x:0.0,y:0.0});
        editor.curve_drag_anchor=Some(Point{x:0.0,y:0.0});
        editor.curve_drag_to(Point{x:9.0,y:0.0});
        editor.curve_drag_anchor=None;
        editor.curve_click(Point{x:30.0,y:0.0});
        assert!(editor.curve_outgoing.is_none());
        editor.curve_click(Point{x:60.0,y:30.0});
        let Shape::Bezier{segments,..}=&editor.doc.objects.last().unwrap().shape
            else{panic!("curve")};
        assert_eq!(segments[1].control1,Point{x:40.0,y:10.0});
    }
    #[test]
    fn bezier_close_sets_closed_flag_and_preserves_single_history_entry(){
        let mut editor=Editor::default();
        editor.change_tool(Tool::Curve);
        editor.curve_click(Point{x:0.0,y:0.0});
        editor.curve_click(Point{x:20.0,y:0.0});
        editor.curve_click(Point{x:20.0,y:20.0});
        editor.curve_finish(true);
        assert!(editor.tool==Tool::Select);
        assert!(editor.curve_outgoing.is_none());
        assert!(editor.curve_drag_anchor.is_none());
        let Shape::Bezier{closed,segments,..}=&editor.doc.objects.last().unwrap().shape
            else{panic!("curve")};
        assert!(*closed);
        assert_eq!(segments.len(),2);
        assert_eq!(editor.undo.len(),1);
        editor.undo();
        assert_eq!(editor.doc.objects.len(),2);
    }
    #[test]
    fn bezier_locked_layer_does_not_change_drag_tangent(){
        let mut editor=Editor::default();
        editor.change_tool(Tool::Curve);
        let p0=Point{x:1.0,y:1.0};
        editor.curve_click(p0);
        editor.curve_click(Point{x:20.0,y:20.0});
        let id=editor.curve_draft.unwrap();
        let before=editor.doc.objects.iter().find(|o|o.id==id).unwrap().shape.clone();
        editor.curve_drag_anchor=Some(Point{x:20.0,y:20.0});
        editor.doc.layers[0].locked=true;
        editor.curve_drag_to(Point{x:30.0,y:25.0});
        assert_eq!(editor.doc.objects.iter().find(|o|o.id==id).unwrap().shape,before);
        assert!(editor.curve_outgoing.is_none());
    }
    #[test]
    fn bezier_many_vertices_are_one_undo_transaction(){
        let mut editor=Editor::default();
        let original=editor.doc.clone();
        editor.change_tool(Tool::Curve);
        editor.curve_click(Point{x:10.0,y:10.0});
        assert!(editor.undo.is_empty());
        for index in 1..=250 {
            editor.curve_click(Point{x:10.0+index as f32,y:10.0});
        }
        assert_eq!(editor.undo.len(),1,
            "Adding vertices must not deep-clone the whole document");
        let curve_id=editor.curve_draft.expect("curve draft");
        let curve=editor.doc.get_mut(curve_id).unwrap();
        let Shape::Bezier{segments,..}=&curve.shape else{panic!("Bezier shape")};
        assert_eq!(segments.len(),250);
        editor.undo();
        assert_eq!(editor.doc,original);
        editor.redo();
        assert_eq!(editor.doc.objects.len(),original.objects.len()+1);
        editor.doc.validate().unwrap();
    }
    #[test]
    fn bezier_draft_respects_its_original_layer_lock(){
        let mut editor=Editor::default();
        editor.change_tool(Tool::Curve);
        editor.curve_click(Point{x:10.0,y:20.0});
        editor.curve_click(Point{x:20.0,y:30.0});
        let id=editor.curve_draft.unwrap();
        let before=editor.doc.objects.iter().find(|o|o.id==id).unwrap().shape.clone();
        editor.doc.layers[0].locked=true;
        editor.curve_click(Point{x:30.0,y:40.0});
        assert_eq!(editor.doc.objects.iter().find(|o|o.id==id).unwrap().shape,before);
        assert_eq!(editor.undo.len(),1);
        editor.doc.layers[0].locked=false;
        editor.active_layer=editor.doc.add_layer("Другой слой".into());
        editor.curve_click(Point{x:30.0,y:40.0});
        let obj=editor.doc.objects.iter().find(|o|o.id==id).unwrap();
        assert_eq!(obj.layer_id,1);
        let Shape::Bezier{segments,..}=&obj.shape else{panic!("Bezier required")};
        assert_eq!(segments.len(),2);
        assert_eq!(editor.undo.len(),1);
    }
    #[test]
    fn bezier_before_second_anchor_does_not_create_empty_shape(){
        let mut editor=Editor::default();
        let before=editor.doc.clone();
        editor.change_tool(Tool::Curve);
        editor.curve_click(Point{x:10.0,y:20.0});
        editor.change_tool(Tool::Select);
        assert_eq!(editor.doc,before);
        assert!(editor.undo.is_empty());
    }
    #[test]
    fn inline_text_apply_is_single_undoable_change(){
        let mut editor=Editor::default();
        let id=editor.doc.objects[1].id;
        let original=editor.doc.clone();
        editor.begin_inline_text_edit(id,Pos2::new(120.0,150.0));
        let mut session=editor.inline_text.take().expect("text editor must open");
        session.draft="Новый текст — мир!".into();
        editor.apply_inline_text_edit(session);
        assert_ne!(editor.doc,original);
        assert_eq!(editor.undo.len(),1);
        editor.undo();
        assert_eq!(editor.doc,original);
        editor.redo();
        let Shape::Text{value,..}=&editor.doc.objects[1].shape else{panic!("text expected")};
        assert_eq!(value,"Новый текст — мир!");
    }
    #[test]
    fn inline_text_cancel_and_no_op_do_not_create_undo(){
        let mut editor=Editor::default();
        let id=editor.doc.objects[1].id;
        let original=editor.doc.clone();
        editor.begin_inline_text_edit(id,Pos2::ZERO);
        let session=editor.inline_text.take().unwrap();
        // Simulate dismissing the editor: the working draft is dropped.
        assert_eq!(editor.doc,original);
        editor.apply_inline_text_edit(session);
        assert_eq!(editor.doc,original);
        assert!(editor.undo.is_empty());
    }
    #[test]
    fn inline_text_rechecks_layer_lock_before_commit(){
        let mut editor=Editor::default();
        let id=editor.doc.objects[1].id;
        let before=editor.doc.clone();
        editor.begin_inline_text_edit(id,Pos2::ZERO);
        let mut session=editor.inline_text.take().unwrap();
        session.draft="Заблокировано".into();
        editor.doc.layers[0].locked=true;
        editor.apply_inline_text_edit(session);
        assert_eq!(editor.doc.objects,before.objects);
        assert!(editor.undo.is_empty());
    }
    #[test]
    fn inline_text_rejects_conflicting_external_edits(){
        let mut editor=Editor::default();
        let id=editor.doc.objects[1].id;
        editor.begin_inline_text_edit(id,Pos2::ZERO);
        let mut session=editor.inline_text.take().unwrap();
        session.draft="Draft".into();
        if let Shape::Text{value,..}=&mut editor.doc.objects[1].shape{
            *value="Other writer".into();
        }
        editor.apply_inline_text_edit(session);
        assert!(editor.undo.is_empty());
        let Shape::Text{value,..}=&editor.doc.objects[1].shape else{panic!()};
        assert_eq!(value,"Other writer");
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
fn main()->eframe::Result<()>{
    let options=eframe::NativeOptions{
        viewport:egui::ViewportBuilder::default().with_inner_size([1280.0,780.0])
            .with_min_inner_size([780.0,520.0]),
        ..Default::default()
    };
    eframe::run_native(concat!("OpenCorel v",env!("CARGO_PKG_VERSION"),
        " — Редактор макетов"),options,Box::new(|cc|{
        ui_theme::apply(&cc.egui_ctx,false);
        Ok(Box::new(Editor::default()))
    }))
}
