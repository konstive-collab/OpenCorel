//! OpenCorel document engine. Page coordinates are millimetres, not pixels.
//! Experimental schema v1; not a CorelDRAW CDR implementation.
use serde::{Deserialize, Serialize};
use base64::Engine;
use std::{collections::{HashSet,BTreeMap}, fs, io::{self, Write}, path::Path};

pub mod svg_import;
pub mod object_manager;
pub mod kdr;
pub mod preflight;
pub mod print_sheet;
pub mod pages;
pub mod smart_shapes;
pub mod transform;
pub mod node_edit;
pub mod path_topology;
pub mod compound_ops;
pub mod freehand;
pub mod guides;
pub mod offset;
pub mod hit_testing;
pub mod boolean_ops;
pub mod appearance;
use appearance::Appearance;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point { pub x: f32, pub y: f32 }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Alignment { Left, CenterX, Right, Top, CenterY, Bottom }
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Distribution { HorizontalCenters, VerticalCenters }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }

impl Bounds {
    pub fn contains(self, p: Point, tolerance: f32) -> bool {
        p.x >= self.x - tolerance && p.y >= self.y - tolerance
            && p.x <= self.x + self.w + tolerance && p.y <= self.y + self.h + tolerance
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CubicSegment {
    pub control1: Point,
    pub control2: Point,
    pub end: Point,
}
/// A closed or open subpath; multiple contours form glyphs with counters/holes.
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
pub struct Contour {
    pub start:Point,
    pub segments:Vec<CubicSegment>,
    pub closed:bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: u64,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
}
fn default_layer_id() -> u64 { 1 }
fn default_next_layer_id() -> u64 { 2 }
fn default_layers() -> Vec<Layer> {
    vec![Layer{id:1,name:"Основной".into(),visible:true,locked:false}]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Page { pub width_mm: f32, pub height_mm: f32 }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Style { pub fill: String, pub stroke: String, pub stroke_width_mm: f32 }

impl Default for Style {
    fn default() -> Self {
        Self { fill: "#447BD4".into(), stroke: "#18283C".into(), stroke_width_mm: 0.7 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag="type", rename_all="snake_case")]
pub enum Shape {
    Rectangle { x: f32, y: f32, w: f32, h: f32 },
    Ellipse { x: f32, y: f32, w: f32, h: f32 },
    Text { x: f32, y: f32, value: String, size_mm: f32 },
    Polyline { points: Vec<Point>, closed: bool },
    Bezier { start: Point, segments: Vec<CubicSegment>, closed: bool },
    /// Multiple editable cubic contours with even-odd filling (font glyph counters).
    CompoundPath { contours:Vec<Contour> },
    // Imported vector artwork preserved as one repositionable/scalable object.
    // Its inner text/paths are NOT independently editable yet.
    EmbeddedSvg { x:f32,y:f32,w:f32,h:f32, svg:String, source_format:String },
    /// Self-contained bitmap artwork, using original PNG/JPEG encoded bytes.
    /// Dimensions in mm stay independent of source pixel dimensions.
    Raster { x:f32,y:f32,w:f32,h:f32, pixel_width:u32,pixel_height:u32,
        mime:String,data_base64:String },
}

impl Shape {
    pub fn bounds(&self) -> Bounds {
        match self {
            Self::Rectangle {x,y,w,h} | Self::Ellipse {x,y,w,h} |
            Self::EmbeddedSvg {x,y,w,h,..} |
            Self::Raster {x,y,w,h,..} => Bounds {x:*x,y:*y,w:*w,h:*h},
            Self::Text {x,y,value,size_mm} => {
                let widest=value.split('\n').map(|line|line.chars().count()).max().unwrap_or(0);
                let lines=value.split('\n').count().max(1);
                Bounds{x:*x,y:*y,w:*size_mm*widest as f32*0.6,
                    h:*size_mm*(1.0+(lines-1) as f32*1.2)}
            },
            Self::CompoundPath{contours}=>{
                let (mut x1,mut y1,mut x2,mut y2)=
                    (f32::INFINITY,f32::INFINITY,f32::NEG_INFINITY,f32::NEG_INFINITY);
                for contour in contours {
                    for p in std::iter::once(&contour.start)
                        .chain(contour.segments.iter().flat_map(|seg|
                            [&seg.control1,&seg.control2,&seg.end])) {
                        x1=x1.min(p.x);y1=y1.min(p.y);
                        x2=x2.max(p.x);y2=y2.max(p.y);
                    }
                }
                if contours.is_empty(){Bounds{x:0.0,y:0.0,w:0.0,h:0.0}}
                else{Bounds{x:x1,y:y1,w:x2-x1,h:y2-y1}}
            }
            Self::Bezier {start,segments,..} => {
                let (mut x1,mut y1,mut x2,mut y2)=(start.x,start.y,start.x,start.y);
                for seg in segments {
                    for p in [seg.control1,seg.control2,seg.end] {
                        x1=x1.min(p.x);y1=y1.min(p.y);x2=x2.max(p.x);y2=y2.max(p.y);
                    }
                }
                Bounds{x:x1,y:y1,w:x2-x1,h:y2-y1}
            }
            Self::Polyline {points,..} => {
                if points.is_empty() { return Bounds {x:0.0,y:0.0,w:0.0,h:0.0}; }
                let (mut x1,mut y1,mut x2,mut y2) =
                    (f32::INFINITY,f32::INFINITY,f32::NEG_INFINITY,f32::NEG_INFINITY);
                for p in points { x1=x1.min(p.x);y1=y1.min(p.y);x2=x2.max(p.x);y2=y2.max(p.y); }
                Bounds {x:x1,y:y1,w:x2-x1,h:y2-y1}
            }
        }
    }
    pub fn translate(&mut self, dx:f32, dy:f32) {
        match self {
            Self::Rectangle {x,y,..} | Self::Ellipse {x,y,..} |
            Self::Text {x,y,..} | Self::EmbeddedSvg {x,y,..} |
            Self::Raster {x,y,..} => {*x+=dx;*y+=dy;},
            Self::Polyline {points,..} => for p in points {p.x+=dx;p.y+=dy;},
            Self::CompoundPath{contours}=>{
                for contour in contours{
                    contour.start.x+=dx;contour.start.y+=dy;
                    for seg in &mut contour.segments{
                        for p in [&mut seg.control1,&mut seg.control2,&mut seg.end]{
                            p.x+=dx;p.y+=dy;
                        }
                    }
                }
            },
            Self::Bezier {start,segments,..} => {
                start.x+=dx;start.y+=dy;
                for seg in segments {
                    for p in [&mut seg.control1,&mut seg.control2,&mut seg.end] {
                        p.x+=dx;p.y+=dy;
                    }
                }
            }
        }
    }
}

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize,Default)]
#[serde(rename_all="snake_case")]
pub enum TextAlign{#[default] Left,Center,Right}

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(default)]
pub struct TextFormat{
    pub family:String,
    pub weight:u16,
    pub italic:bool,
    pub alignment:TextAlign,
    pub letter_spacing_mm:f32,
    pub line_height:f32,
}
impl Default for TextFormat{
    fn default()->Self{
        Self{family:"sans-serif".into(),weight:400,italic:false,
            alignment:TextAlign::Left,letter_spacing_mm:0.0,line_height:1.2}
    }
}
impl TextFormat{
    pub fn line_width_mm(&self,value:&str,size_mm:f32)->f32{
        let max_chars=value.split('\n').map(|line|line.chars().count()).max().unwrap_or(0);
        (size_mm*0.6*max_chars as f32 +
            (max_chars.saturating_sub(1) as f32)*self.letter_spacing_mm).max(0.0)
    }
    pub fn validate(&self)->Result<(),String>{
        if self.family.is_empty() || self.family.len()>128 ||
            self.family.chars().any(|c|c.is_control()) {
            return Err("Font family must contain 1–128 printable bytes".into());
        }
        if !(100..=900).contains(&self.weight) || self.weight%100!=0{
            return Err("Font weight must be 100–900 in steps of 100".into());
        }
        if !self.letter_spacing_mm.is_finite() ||
            !(-100.0..=100.0).contains(&self.letter_spacing_mm) ||
            !self.line_height.is_finite() ||
            !(0.7..=4.0).contains(&self.line_height) {
            return Err("Invalid text letter spacing or line height".into());
        }
        Ok(())
    }
}
fn default_object_visible()->bool{true}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Object {
    pub id: u64,
    pub shape: Shape,
    pub style: Style,
    /// Per-object visibility and edit protection; absent in legacy KDR means visible/unlocked.
    #[serde(default="default_object_visible")]
    pub visible:bool,
    #[serde(default)]
    pub locked:bool,
    #[serde(default="default_layer_id")]
    pub layer_id: u64,
    #[serde(default)]
    pub text_format:TextFormat,
    /// Optional human-readable object label. Empty means autogenerated UI title.
    #[serde(default)]
    pub name:String,
    /// Flat group membership. Zero/None means ungrouped; no nested groups.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub group_id:Option<u64>,
    /// Persisted interior cubic node tangent modes. Absent in legacy KDR files.
    /// Index remapping on node insertion/removal is handled by node_edit.
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub node_modes:Vec<node_edit::NodeModeBinding>,
}
impl Object{
    pub fn bounds(&self)->Bounds{
        if let Shape::Text{x,y,value,size_mm}=&self.shape{
            let line_count=value.split('\n').count().max(1);
            let h=*size_mm*(1.0+(line_count.saturating_sub(1) as f32)*self.text_format.line_height);
            Bounds{x:*x,y:*y,w:self.text_format.line_width_mm(value,*size_mm),h}
        }else{self.shape.bounds()}
    }
}

/// Non-destructive SVG-compatible vector clipping. The source is preserved as
/// an editable object, and can clip multiple targets without altering geometry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipLink { pub source_id: u64, pub target_id: u64 }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub schema_version: u32,
    pub name: String,
    pub page: Page,
    pub objects: Vec<Object>,
    pub next_id: u64,
    #[serde(default="default_layers")]
    pub layers: Vec<Layer>,
    #[serde(default="default_next_layer_id")]
    pub next_layer_id: u64,
    #[serde(default, skip_serializing_if="Vec::is_empty")]
    pub clip_links: Vec<ClipLink>,
    #[serde(default,skip_serializing_if="BTreeMap::is_empty")]
    pub appearances:BTreeMap<u64,Appearance>,
    /// Nonprinting horizontal and vertical construction guides in mm.
    /// Optional field keeps older .kdr documents compatible.
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub guides:Vec<guides::Guide>,
    /// Per-page persistent storage. The active slot is an empty placeholder;
    /// active geometry remains in the legacy top-level fields.
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub page_slots:Vec<pages::StoredPage>,
    #[serde(default,skip_serializing_if="pages::is_page_zero")]
    pub active_page_index:usize,
    #[serde(default="pages::default_page_title")]
    pub active_page_title:String,
}
impl Default for Document {
    fn default() -> Self {
        Self { schema_version:SCHEMA_VERSION, name:"Новый макет".into(),
            page:Page {width_mm:841.0,height_mm:594.0},objects:Vec::new(),next_id:1,
            layers:default_layers(),next_layer_id:default_next_layer_id(),clip_links:Vec::new(),appearances:BTreeMap::new(),guides:Vec::new(),
            page_slots:Vec::new(),active_page_index:0,active_page_title:pages::default_page_title() }
    }
}

impl Document {
    /// Approximate owned memory size, used to cap undo history on low-RAM PCs.
    /// Does not include allocator overhead or GPU textures.
    pub fn estimated_size_bytes(&self)->usize {
        let mut total=std::mem::size_of::<Document>()+self.name.len();
        for layer in &self.layers{
            total=total.saturating_add(std::mem::size_of::<Layer>()+layer.name.len());
        }
        for obj in &self.objects{
            let shape_bytes=obj.text_format.family.len()+match &obj.shape{
                Shape::Text{value,..}=>value.len(),
                Shape::Polyline{points,..}=>points.len()*std::mem::size_of::<Point>(),
                Shape::Bezier{segments,..}=>segments.len()*std::mem::size_of::<CubicSegment>(),
                Shape::CompoundPath{contours}=>contours.iter()
                    .map(|c|std::mem::size_of::<Contour>()+
                        c.segments.len()*std::mem::size_of::<CubicSegment>())
                    .sum(),
                Shape::EmbeddedSvg{svg,..}=>svg.len(),
                Shape::Raster{data_base64,..}=>data_base64.len(),
                _=>0,
            };
            total=total.saturating_add(std::mem::size_of::<Object>()
                +obj.style.fill.len()+obj.style.stroke.len()+obj.name.len()+shape_bytes
                +obj.node_modes.len()*std::mem::size_of::<node_edit::NodeModeBinding>());
        }
        total=total.saturating_add(self.inactive_size_bytes());
        total.saturating_add(self.guides.len()*std::mem::size_of::<guides::Guide>())
            .saturating_add(self.clip_links.len()*std::mem::size_of::<ClipLink>())
            .saturating_add(self.appearances.iter().map(|(_,a)|
                std::mem::size_of::<Appearance>()+
                a.dash_array.len()*std::mem::size_of::<f32>()+
                a.gradient.as_ref().map(|g|
                    g.stops.iter().map(|s|s.color.len()+std::mem::size_of::<appearance::GradientStop>())
                        .sum::<usize>()).unwrap_or_default()).sum::<usize>())
    }
    /// Apply a closed vector object as a reusable clipping shape.
    /// No geometry or styling is destroyed; source remains editable.
    pub fn check_clip(&self, source_id:u64, target_id:u64)->Result<(),String>{
        if source_id==target_id {return Err("A clipping source cannot clip itself".into());}
        let source=self.objects.iter().find(|o|o.id==source_id)
            .ok_or("Clipping source not found")?;
        let target=self.objects.iter().find(|o|o.id==target_id)
            .ok_or("Clipping target not found")?;
        if !clip_shape_supported(&source.shape) {
            return Err("Clipping source must be a closed vector shape".into());
        }
        if source.group_id.is_some() || target.group_id.is_some() {
            return Err("Grouped objects cannot be clipping operands".into());
        }
        let editable=|object:&Object|self.layer(object.layer_id)
            .is_some_and(|l|l.visible&&!l.locked)&&object.visible&&!object.locked;
        if !editable(source)||!editable(target) {
            return Err("Clipping operands must be on visible, unlocked layers".into());
        }
        if self.clip_links.iter().any(|c|c.target_id==source_id ||
            c.source_id==target_id) {
            return Err("Nested or cyclic clip links are unsupported".into());
        }
        if self.clip_links.iter().filter(|c|c.target_id!=target_id).count()>=10000 {
            return Err("Clipping link limit exceeded".into());
        }
        Ok(())
    }
    pub fn set_clip(&mut self, source_id:u64, target_id:u64)->Result<(),String>{
        self.check_clip(source_id,target_id)?;
        // Replacing an existing target link is atomic.
        self.clip_links.retain(|c|c.target_id!=target_id);
        self.clip_links.push(ClipLink{source_id,target_id});
        Ok(())
    }
    pub fn clear_clip(&mut self,target_id:u64)->bool{
        let n=self.clip_links.len();
        self.clip_links.retain(|c|c.target_id!=target_id);
        self.clip_links.len()!=n
    }
    pub fn set_appearance(&mut self,id:u64,appearance:Appearance)->Result<(),String>{
        appearance.validate()?;
        let obj=self.objects.iter().find(|o|o.id==id)
            .ok_or("Appearance target not found")?;
        if !self.layer(obj.layer_id).is_some_and(|l|l.visible&&!l.locked)||!obj.visible||obj.locked{
            return Err("Cannot style an object on a hidden or locked layer".into());
        }
        if appearance.is_default(){self.appearances.remove(&id);}
        else {self.appearances.insert(id,appearance);}
        Ok(())
    }
    pub fn clear_appearance(&mut self,id:u64)->bool{
        self.appearances.remove(&id).is_some()
    }
    pub fn add(&mut self, shape:Shape, style:Style) -> u64 {
        self.add_to_layer(shape,style,1)
    }
    pub fn add_to_layer(&mut self,shape:Shape,style:Style,layer_id:u64)->u64 {
        let layer_id=if self.layers.iter().any(|l|l.id==layer_id){layer_id}else{1};
        let id=self.next_id;
        self.next_id+=1;
        self.objects.push(Object{id,shape,style,layer_id,
            visible:true,locked:false,
            text_format:TextFormat::default(),name:String::new(),group_id:None,
            node_modes:Vec::new()});
        id
    }
    /// Create a flat, persistent group without changing the individual objects'
    /// geometry or layer. Group IDs share the ID allocator with object IDs.
    pub fn group_objects(&mut self,ids:&[u64])->Result<u64,String>{
        let wanted:HashSet<u64>=ids.iter().copied().collect();
        if wanted.len()<2{return Err("Select at least two objects to group".into());}
        let eligible:HashSet<u64>=self.layers.iter()
            .filter(|layer|layer.visible&&!layer.locked).map(|layer|layer.id).collect();
        let chosen:Vec<_>=self.objects.iter().filter(|o|wanted.contains(&o.id)).collect();
        if chosen.len()!=wanted.len() || chosen.iter().any(|o|
            !eligible.contains(&o.layer_id)||!o.visible||o.locked||o.group_id.is_some()){
            return Err("Objects must exist, be editable and not already grouped".into());
        }
        if self.next_id==u64::MAX{return Err("Group ID limit exceeded".into());}
        let group=self.next_id;self.next_id+=1;
        for obj in &mut self.objects{
            if wanted.contains(&obj.id){obj.group_id=Some(group);}
        }
        Ok(group)
    }
    /// Ungroup all members of groups referenced by the selected IDs.
    pub fn ungroup_objects(&mut self,ids:&[u64])->Result<usize,String>{
        let wanted:HashSet<u64>=ids.iter().copied().collect();
        let groups:HashSet<u64>=self.objects.iter().filter(|o|wanted.contains(&o.id))
            .filter_map(|o|o.group_id).collect();
        if groups.is_empty(){return Err("No grouped objects selected".into());}
        let eligible:HashSet<u64>=self.layers.iter().filter(|l|l.visible&&!l.locked)
            .map(|l|l.id).collect();
        if self.objects.iter().any(|o|o.group_id.map(|g|groups.contains(&g))
            .unwrap_or(false)&&(!eligible.contains(&o.layer_id)||!o.visible||o.locked)){
            return Err("Cannot ungroup objects on hidden or locked layers".into());
        }
        let mut count=0;
        for obj in &mut self.objects{
            if obj.group_id.map(|g|groups.contains(&g)).unwrap_or(false){
                obj.group_id=None;count+=1;
            }
        }
        Ok(count)
    }
    pub fn group_members(&self,id:u64)->Vec<u64>{
        let group=self.objects.iter().find(|o|o.id==id).and_then(|o|o.group_id);
        if let Some(group)=group{
            self.objects.iter().filter(|o|o.group_id==Some(group)).map(|o|o.id).collect()
        }else{vec![id]}
    }
    pub fn add_layer(&mut self,name:String)->u64{
        let id=self.next_layer_id;
        self.next_layer_id+=1;
        self.layers.push(Layer{id,name,visible:true,locked:false});
        id
    }
    pub fn layer(&self,id:u64)->Option<&Layer>{
        self.layers.iter().find(|layer|layer.id==id)
    }
    pub fn visible_objects(&self)->impl Iterator<Item=&Object>{
        let visible:HashSet<u64>=self.layers.iter().filter(|layer|layer.visible)
            .map(|layer|layer.id).collect();
        self.objects.iter().filter(move |obj|visible.contains(&obj.layer_id)&&obj.visible)
    }
    pub fn get_mut(&mut self,id:u64) -> Option<&mut Object> {
        self.objects.iter_mut().find(|o|o.id==id)
    }
    pub fn remove(&mut self,id:u64) -> bool {
        let n=self.objects.len(); self.objects.retain(|o|o.id!=id);
        if self.objects.len()!=n {
            self.clip_links.retain(|c|c.source_id!=id&&c.target_id!=id);
            self.appearances.remove(&id);
            true
        }else{false}
    }
    /// Align the bounding boxes of eligible, unlocked objects.
    pub fn align_objects(&mut self,ids:&[u64],alignment:Alignment)->usize {
        let wanted:HashSet<u64>=ids.iter().copied().collect();
        let editable:HashSet<u64>=self.layers.iter()
            .filter(|l|l.visible&&!l.locked).map(|l|l.id).collect();
        // Never align only part of a locked or grouped selection. Individual
        // group members must not be rearranged, breaking the group geometry.
        if self.objects.iter().any(|o|wanted.contains(&o.id)
            && (!editable.contains(&o.layer_id)||!o.visible||o.locked||o.group_id.is_some())){
            return 0;
        }
        let objects:Vec<_>=self.objects.iter().filter(|o|
            wanted.contains(&o.id))
            .map(|o|(o.id,o.bounds())).collect();
        if objects.len()<2{return 0;}
        let left=objects.iter().map(|(_,b)|b.x).fold(f32::INFINITY,f32::min);
        let right=objects.iter().map(|(_,b)|b.x+b.w).fold(f32::NEG_INFINITY,f32::max);
        let top=objects.iter().map(|(_,b)|b.y).fold(f32::INFINITY,f32::min);
        let bottom=objects.iter().map(|(_,b)|b.y+b.h).fold(f32::NEG_INFINITY,f32::max);
        let count=objects.len();
        for (id,b) in objects{
            let (dx,dy)=match alignment{
                Alignment::Left=>(left-b.x,0.0),
                Alignment::CenterX=>((left+right-b.w)*0.5-b.x,0.0),
                Alignment::Right=>(right-b.x-b.w,0.0),
                Alignment::Top=>(0.0,top-b.y),
                Alignment::CenterY=>(0.0,(top+bottom-b.h)*0.5-b.y),
                Alignment::Bottom=>(0.0,bottom-b.y-b.h),
            };
            if let Some(obj)=self.get_mut(id){obj.shape.translate(dx,dy);}
        }
        count
    }
    /// Space object centers uniformly between the first and last center.
    pub fn distribute_objects(&mut self,ids:&[u64],direction:Distribution)->usize {
        let wanted:HashSet<u64>=ids.iter().copied().collect();
        let editable:HashSet<u64>=self.layers.iter()
            .filter(|l|l.visible&&!l.locked).map(|l|l.id).collect();
        if self.objects.iter().any(|o|wanted.contains(&o.id)
            && (!editable.contains(&o.layer_id)||!o.visible||o.locked||o.group_id.is_some())){
            return 0;
        }
        let mut positions:Vec<(u64,f32)>=self.objects.iter().filter(|o|
            wanted.contains(&o.id))
            .map(|o|{
                let b=o.bounds();
                (o.id,match direction{
                    Distribution::HorizontalCenters=>b.x+b.w*0.5,
                    Distribution::VerticalCenters=>b.y+b.h*0.5,
                })
            }).collect();
        if positions.len()<3{return 0;}
        positions.sort_by(|a,b|a.1.total_cmp(&b.1));
        let step=(positions.last().unwrap().1-positions[0].1)/(positions.len()-1) as f32;
        let first=positions[0].1;
        for (index,(id,center)) in positions.iter().enumerate().skip(1).take(positions.len()-2){
            let delta=first+index as f32*step-*center;
            if let Some(obj)=self.get_mut(*id){
                match direction{
                    Distribution::HorizontalCenters=>obj.shape.translate(delta,0.0),
                    Distribution::VerticalCenters=>obj.shape.translate(0.0,delta),
                }
            }
        }
        positions.len()
    }
    pub fn hit_test(&self,p:Point,tolerance:f32) -> Option<u64> {
        self.hit_test_with_bounds(p,tolerance,|object|object.bounds())
    }

    /// Allows a viewport to supply measured bounds (for example, font glyph
    /// outlines) without changing persisted object geometry or bypassing
    /// visibility, lock and clipping semantics.
    pub fn hit_test_with_bounds<F>(&self,p:Point,tolerance:f32,bounds:F)->Option<u64>
    where F:Fn(&Object)->Bounds {
        self.hit_test_with_bounds_filtered(p,tolerance,bounds,|_,_,_|true)
    }
    /// Precise canvas picking while preserving visibility, lock, z-order,
    /// and clipping rules of the generic document hit-test. The optional
    /// shape predicate runs only after broad-phase bounds screening.
    pub fn hit_test_with_bounds_filtered<F,G>(
        &self,p:Point,tolerance:f32,bounds:F,shape_hit:G
    )->Option<u64>
    where F:Fn(&Object)->Bounds,G:Fn(&Object,Point,f32)->bool{
        if !p.x.is_finite() || !p.y.is_finite() ||
            !tolerance.is_finite() || tolerance<0.0{return None;}
        let editable:HashSet<u64>=self.layers.iter()
            .filter(|layer|layer.visible&&!layer.locked).map(|layer|layer.id).collect();
        let sources:HashSet<u64>=self.clip_links.iter().map(|c|c.source_id).collect();
        self.objects.iter().rev().find(|o|{
            if !editable.contains(&o.layer_id)||!o.visible||o.locked||sources.contains(&o.id)
                || !bounds(o).contains(p,tolerance) ||
                !shape_hit(o,p,tolerance){return false;}
            self.clip_links.iter().find(|link|link.target_id==o.id)
                .is_none_or(|link|self.objects.iter().find(|src|src.id==link.source_id)
                    .is_some_and(|src|clip_contains_point(&src.shape,p)))
        }).map(|o|o.id)
    }
    pub fn validate(&self)->Result<(),String>{
        if self.schema_version!=SCHEMA_VERSION {return Err("Unknown document schema".into());}
        self.validate_pages()?;
        guides::validate(&self.guides)?;
        if ![self.page.width_mm,self.page.height_mm].iter().all(|v|v.is_finite() && (1.0..=100_000.0).contains(v)) {
            return Err("Invalid page dimensions".into());
        }
        if self.objects.len()>100_000{return Err("Object limit exceeded".into());}
        let all_ids:HashSet<u64>=self.objects.iter().map(|o|o.id).collect();
        if self.appearances.len()>self.objects.len(){
            return Err("Appearance count exceeds object count".into());
        }
        for (id,appearance) in &self.appearances{
            if !all_ids.contains(id){
                return Err("Orphaned appearance reference".into());
            }
            appearance.validate()?;
        }
        if self.clip_links.len()>10000{return Err("Clipping link limit exceeded".into());}
        let mut targets=HashSet::new();
        for link in &self.clip_links{
            if link.source_id==link.target_id || !targets.insert(link.target_id) {
                return Err("Duplicate or self-referencing clipping link".into());
            }
            let source=self.objects.iter().find(|o|o.id==link.source_id)
                .ok_or("Clipping source missing")?;
            if !clip_shape_supported(&source.shape) {
                return Err("Invalid or open clipping source".into());
            }
            if !self.objects.iter().any(|o|o.id==link.target_id) {
                return Err("Clipping target missing".into());
            }
        }
        if self.clip_links.iter().any(|c|targets.contains(&c.source_id)) {
            return Err("Nested or cyclic clipping references are unsupported".into());
        }
        if self.next_id==0 || self.next_id==u64::MAX ||
            self.next_layer_id==0 || self.next_layer_id==u64::MAX {
            return Err("Invalid identifier allocation state".into());
        }
        if !valid_xml_text(&self.name) || self.name.len()>4096{
            return Err("Invalid document name".into());
        }
        if self.layers.is_empty()||self.layers.len()>1024{return Err("Invalid layer count".into());}
        let mut layer_ids=HashSet::new();
        for layer in &self.layers{
            if layer.id==0 || layer.id>=self.next_layer_id || !layer_ids.insert(layer.id)
                || layer.name.len()>256 {
                return Err("Invalid layer".into());
            }
        }
        if !layer_ids.contains(&1){return Err("Primary layer missing".into());}
        // Reserve every object ID before validating group references. The old
        // single-pass check could accept a group ID colliding with a later object.
        let mut ids=HashSet::new();
        for o in &self.objects {
            if o.id==0 || o.id>=self.next_id || !ids.insert(o.id) {return Err("Invalid object ID".into());}
            if !layer_ids.contains(&o.layer_id){return Err("Object references unknown layer".into());}
            if o.name.len()>256 || !valid_xml_text(&o.name){
                return Err("Invalid object label".into());
            }
            if let Some(group)=o.group_id{
                if group==0 || group>=self.next_id || all_ids.contains(&group){
                    return Err("Invalid object group reference".into());
                }
            }
            if !valid_paint(&o.style.fill) || !valid_paint(&o.style.stroke)
                || !o.style.stroke_width_mm.is_finite()
                || !(0.0..=1000.0).contains(&o.style.stroke_width_mm) {
                return Err("Invalid style".into());
            }
            o.text_format.validate()?;
            node_edit::validate_modes(&o.shape,&o.node_modes)?;
            let b=o.bounds();
            if ![b.x,b.y,b.w,b.h].iter().all(|v|v.is_finite()) || b.w<0.0 || b.h<0.0 {
                return Err("Invalid shape geometry".into());
            }
            match &o.shape{
                Shape::CompoundPath{contours}=>{
                    if contours.is_empty() || contours.len()>10000 ||
                        contours.iter().any(|c|c.segments.is_empty() ||
                            !finite_point(c.start) ||
                            c.segments.iter().any(|s|
                                !finite_point(s.control1) ||
                                !finite_point(s.control2) ||
                                !finite_point(s.end))) ||
                        contours.iter().map(|c|c.segments.len()).sum::<usize>()>60000 {
                        return Err("Invalid compound vector path".into());
                    }
                }
                Shape::Bezier{start,segments,..}=>{
                    if segments.is_empty() || segments.len()>10000 {
                        return Err("Invalid Bezier segment count".into());
                    }
                    if !finite_point(*start) || segments.iter().any(|seg|
                        !finite_point(seg.control1) || !finite_point(seg.control2) ||
                        !finite_point(seg.end)){
                        return Err("Non-finite Bezier coordinates".into());
                    }
                }
                Shape::Polyline{points,..}=>{
                    if points.len()<2 || points.len()>100_000 ||
                        points.iter().any(|p|!finite_point(*p)){
                        return Err("Invalid polyline geometry".into());
                    }
                }
                Shape::Text{value,..}=>{
                    if value.len()>1_000_000 || !valid_xml_text(value){
                        return Err("Invalid text content".into());
                    }
                }
                Shape::EmbeddedSvg{w,h,svg,source_format,..}=>{
                    if *w<=0.0||*h<=0.0 || svg.len()>20*1024*1024 ||
                        svg.trim().is_empty() || source_format.len()>32 ||
                        !source_format.chars().all(|c|c.is_ascii_alphanumeric()) {
                        return Err("Invalid embedded vector artwork".into());
                    }
                }
                Shape::Raster{w,h,pixel_width,pixel_height,mime,data_base64,..}=>{
                    if *w<=0.0||*h<=0.0 ||
                        *pixel_width==0 || *pixel_height==0 ||
                        *pixel_width>16_000 || *pixel_height>16_000 ||
                        u64::from(*pixel_width)*u64::from(*pixel_height)>40_000_000 ||
                        data_base64.len()>56*1024*1024 ||
                        !matches!(mime.as_str(),"image/png"|"image/jpeg") {
                        return Err("Invalid raster metadata".into());
                    }
                    let raw=base64::engine::general_purpose::STANDARD
                        .decode(data_base64.as_bytes())
                        .map_err(|_|"Invalid raster base64 data")?;
                    if raw.len()>40*1024*1024 || !raster_signature_matches(mime,&raw){
                        return Err("Invalid raster image header or size".into());
                    }
                }
                _=>{}
            }
            if let Shape::Text{size_mm,..}=&o.shape {
                if !size_mm.is_finite() || !(0.1..=10_000.0).contains(size_mm) {
                    return Err("Invalid text size".into());
                }
            }
        }
        Ok(())
    }
    pub fn read(path:impl AsRef<Path>)->Result<Self,String>{
        let path=path.as_ref();
        let size=fs::metadata(path).map_err(|e|e.to_string())?.len();
        if size>100*1024*1024{return Err("File exceeds 100MB".into());}
        let doc:Self=serde_json::from_slice(&fs::read(path).map_err(|e|e.to_string())?)
            .map_err(|e|e.to_string())?;
        doc.validate()?;Ok(doc)
    }
    pub fn write(&self,path:impl AsRef<Path>)->Result<(),String>{
        self.validate()?;
        let data=serde_json::to_vec_pretty(self).map_err(|e|e.to_string())?;
        atomic_write(path.as_ref(),&data).map_err(|e|e.to_string())
    }
    pub fn export_svg(&self,path:impl AsRef<Path>)->io::Result<()>{
        self.validate().map_err(io::Error::other)?;
        atomic_write(path.as_ref(),self.to_svg().as_bytes())
    }
    pub fn to_svg(&self)->String{
        let mut out=format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" viewBox=\"0 0 {} {}\">\n<title>{}</title>\n",
            self.page.width_mm,self.page.height_mm,self.page.width_mm,self.page.height_mm,
            xml_escape(&self.name));
        for (&id,style) in &self.appearances{
            if let Some(gradient)=&style.gradient{
                out.push_str("<defs>");
                out.push_str(&gradient.svg_definition(id));
                out.push_str("</defs>\n");
            }
        }
        let clip_sources:HashSet<u64>=self.clip_links.iter().map(|c|c.source_id).collect();
        for link in &self.clip_links{
            if let Some(source)=self.objects.iter().find(|o|o.id==link.source_id){
                if let Some(geometry)=clip_geometry_svg(&source.shape){
                    out.push_str(&format!("<defs><clipPath id=\"clip-target-{}\" clipPathUnits=\"userSpaceOnUse\">{geometry}</clipPath></defs>\n",link.target_id));
                }
            }
        }
        for obj in self.visible_objects() {
            if clip_sources.contains(&obj.id){continue;}
            let s=&obj.style;
            let fill=if self.appearances.get(&obj.id).is_some_and(|a|a.gradient.is_some()){
                format!("url(#appearance-{})",obj.id)
            }else{s.fill.clone()};
            let appearance_attrs=self.appearances.get(&obj.id)
                .map(|a|a.svg_attributes()).unwrap_or_default();
            let common=format!("fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"",
                fill,s.stroke,s.stroke_width_mm);
            let element=match &obj.shape {
                Shape::Rectangle{x,y,w,h} => format!("<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" {common}/>"),
                Shape::Ellipse{x,y,w,h} => format!("<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" {common}/>",
                    x+w/2.0,y+h/2.0,w/2.0,h/2.0),
                Shape::Text{x,y,value,size_mm} => {
                    let fmt=&obj.text_format;
                    let width=fmt.line_width_mm(value,*size_mm);
                    let (anchor,xbase)=match fmt.alignment{
                        TextAlign::Left=>("start",*x),
                        TextAlign::Center=>("middle",*x+width*0.5),
                        TextAlign::Right=>("end",*x+width),
                    };
                    let style=if fmt.italic{"italic"}else{"normal"};
                    let head=format!(
                        "<text x=\"{xbase}\" y=\"{}\" font-size=\"{size_mm}\" font-family=\"{}\" font-weight=\"{}\" font-style=\"{style}\" text-anchor=\"{anchor}\" letter-spacing=\"{}\" fill=\"{}\" xml:space=\"preserve\">",
                        y+size_mm,xml_escape(&fmt.family),fmt.weight,
                        fmt.letter_spacing_mm,fill);
                    if !value.contains('\n'){
                        format!("{head}{}</text>",xml_escape(value))
                    }else{
                        let mut body=head;
                        for (i,line) in value.split('\n').enumerate(){
                            let dy=if i==0{0.0}else{fmt.line_height* *size_mm};
                            body.push_str(&format!("<tspan x=\"{xbase}\" dy=\"{dy}\">{}</tspan>",
                                xml_escape(line)));
                        }
                        body.push_str("</text>");
                        body
                    }
                },
                Shape::CompoundPath{contours}=>{
                    let mut d=String::new();
                    for contour in contours{
                        d.push_str(&format!("M {} {}",
                            contour.start.x,contour.start.y));
                        for seg in &contour.segments {
                            d.push_str(&format!(" C {} {}, {} {}, {} {}",
                                seg.control1.x,seg.control1.y,
                                seg.control2.x,seg.control2.y,
                                seg.end.x,seg.end.y));
                        }
                        if contour.closed{d.push_str(" Z");}
                        d.push(' ');
                    }
                    format!("<path d=\"{d}\" fill-rule=\"evenodd\" {common}/>")
                }
                Shape::Bezier{start,segments,closed}=>{
                    let mut d=format!("M {} {}",start.x,start.y);
                    for seg in segments{
                        d.push_str(&format!(" C {} {}, {} {}, {} {}",
                            seg.control1.x,seg.control1.y,seg.control2.x,seg.control2.y,
                            seg.end.x,seg.end.y));
                    }
                    if *closed {d.push_str(" Z");}
                    format!("<path d=\"{d}\" {common}/>")
                }
                Shape::EmbeddedSvg{x,y,w,h,svg,..}=>{
                    let encoded=base64::engine::general_purpose::STANDARD.encode(svg.as_bytes());
                    format!("<image x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" href=\"data:image/svg+xml;base64,{encoded}\"/>")
                }
                Shape::Raster{x,y,w,h,mime,data_base64,..}=>{
                    format!("<image x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" preserveAspectRatio=\"none\" href=\"data:{mime};base64,{data_base64}\"/>")
                }
                Shape::Polyline{points,closed} => {
                    let coords=points.iter().map(|p|format!("{},{}",p.x,p.y)).collect::<Vec<_>>().join(" ");
                    if *closed {format!("<polygon points=\"{coords}\" {common}/>")
                    } else {format!("<polyline points=\"{coords}\" {common} fill=\"none\"/>")}
                }
            };
            if self.clip_links.iter().any(|c|c.target_id==obj.id) {
                out.push_str(&format!("<g id=\"object-{}\"{appearance_attrs} clip-path=\"url(#clip-target-{})\">{element}</g>\n",obj.id,obj.id));
            }else{
                out.push_str(&format!("<g id=\"object-{}\"{appearance_attrs}>{element}</g>\n",obj.id));
            }
        }
        out.push_str("</svg>\n");out
    }
}
/// SVG mask geometry is intentionally restricted to closed, filled vector paths.
fn clip_shape_supported(shape:&Shape)->bool{
    match shape{
        Shape::Rectangle{w,h,..}|Shape::Ellipse{w,h,..}=>*w>0.0&&*h>0.0,
        Shape::Polyline{points,closed}=>*closed&&points.len()>=3,
        Shape::Bezier{segments,closed,..}=>*closed&&!segments.is_empty(),
        Shape::CompoundPath{contours}=>!contours.is_empty()&&
            contours.iter().all(|c|c.closed&&!c.segments.is_empty()),
        _=>false,
    }
}
fn clip_geometry_svg(shape:&Shape)->Option<String>{
    if !clip_shape_supported(shape){return None;}
    let contour_path=|start:Point,segments:&[CubicSegment],closed:bool|{
        let mut d=format!("M {} {}",start.x,start.y);
        for seg in segments{
            d.push_str(&format!(" C {} {}, {} {}, {} {}",
                seg.control1.x,seg.control1.y,seg.control2.x,seg.control2.y,
                seg.end.x,seg.end.y));
        }
        if closed{d.push_str(" Z");}
        d
    };
    Some(match shape{
        Shape::Rectangle{x,y,w,h}=>format!("<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\"/>"),
        Shape::Ellipse{x,y,w,h}=>format!("<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"/>",x+w/2.0,y+h/2.0,w/2.0,h/2.0),
        Shape::Polyline{points,..}=>{
            let coords=points.iter().map(|p|format!("{},{}",p.x,p.y)).collect::<Vec<_>>().join(" ");
            format!("<polygon points=\"{coords}\"/>")
        }
        Shape::Bezier{start,segments,closed}=>format!("<path d=\"{}\"/>",contour_path(*start,segments,*closed)),
        Shape::CompoundPath{contours}=>{
            let d=contours.iter().map(|c|contour_path(c.start,&c.segments,c.closed))
                .collect::<Vec<_>>().join(" ");
            format!("<path d=\"{d}\" clip-rule=\"evenodd\"/>")
        }
        _=>return None
    })
}
/// Test visibility inside a clip path. This does not change the underlying
/// editable geometry and uses the same even-odd rule as SVG clip output.
fn clip_contains_point(shape:&Shape,p:Point)->bool{
    match shape{
        Shape::Rectangle{x,y,w,h}=>p.x>=*x&&p.y>=*y&&p.x<=*x+*w&&p.y<=*y+*h,
        Shape::Ellipse{x,y,w,h}=>{
            if *w<=0.0||*h<=0.0{return false;}
            let dx=(p.x-*x-*w*0.5)/(*w*0.5);
            let dy=(p.y-*y-*h*0.5)/(*h*0.5);
            dx*dx+dy*dy<=1.0
        }
        Shape::Polyline{points,closed}=>*closed&&polygon_contains(points,p),
        Shape::Bezier{start,segments,closed}=>{
            *closed&&polygon_contains(&flatten_contour(*start,segments),p)
        }
        Shape::CompoundPath{contours}=>contours.iter().filter(|c|
            c.closed&&polygon_contains(&flatten_contour(c.start,&c.segments),p)
        ).count()%2==1,
        _=>false,
    }
}
fn flatten_contour(start:Point,segments:&[CubicSegment])->Vec<Point>{
    let mut polygon=Vec::with_capacity(1+segments.len()*16);
    polygon.push(start);
    let mut from=start;
    for segment in segments{
        for step in 1..=16{
            let t=step as f32/16.0;
            let u=1.0-t;
            polygon.push(Point{
                x:u*u*u*from.x+3.0*u*u*t*segment.control1.x+
                    3.0*u*t*t*segment.control2.x+t*t*t*segment.end.x,
                y:u*u*u*from.y+3.0*u*u*t*segment.control1.y+
                    3.0*u*t*t*segment.control2.y+t*t*t*segment.end.y,
            });
        }
        from=segment.end;
    }
    polygon
}
fn polygon_contains(points:&[Point],p:Point)->bool{
    if points.len()<3{return false;}
    let mut inside=false;
    let mut previous=points[points.len()-1];
    for &current in points{
        if (current.y>p.y)!=(previous.y>p.y){
            let crossing=(previous.x-current.x)*(p.y-current.y)
                /(previous.y-current.y)+current.x;
            if p.x<crossing{inside=!inside;}
        }
        previous=current;
    }
    inside
}
fn raster_signature_matches(mime:&str,raw:&[u8])->bool{
    match mime{
        "image/png"=>raw.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg"=>raw.starts_with(&[0xFF,0xD8,0xFF]),
        _=>false,
    }
}
fn finite_point(p:Point)->bool{p.x.is_finite()&&p.y.is_finite()}
fn valid_xml_text(text:&str)->bool{
    text.chars().all(|c|!c.is_control()||matches!(c,'\t'|'\r'|'\n'))
}
fn atomic_write(path:&Path,content:&[u8])->io::Result<()> {
    let parent=path.parent().filter(|p|!p.as_os_str().is_empty())
        .unwrap_or_else(||Path::new("."));
    // Same-directory temporary file preserves original if writing or validation fails.
    let mut temp=tempfile::Builder::new().prefix(".opencorel-").tempfile_in(parent)?;
    temp.write_all(content)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e|e.error)?;
    Ok(())
}
pub fn valid_paint(paint:&str)->bool{paint=="none" || valid_color(paint)}
pub fn valid_color(color:&str)->bool{
    color.len()==7 && color.starts_with('#')
      && color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}
fn xml_escape(s:&str)->String{
    s.replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;")
      .replace('"',"&quot;").replace('\'',"&apos;")
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn old_document_schema_imports_as_primary_layer(){
        let json=r##"{"schema_version":1,"name":"Old","page":{"width_mm":50,"height_mm":50},"objects":[{"id":1,"shape":{"type":"rectangle","x":0,"y":0,"w":10,"h":10},"style":{"fill":"#FFFFFF","stroke":"#000000","stroke_width_mm":0}}],"next_id":2}"##;
        let old:Document=serde_json::from_str(json).unwrap();
        assert_eq!(old.layers.len(),1);
        assert_eq!(old.objects[0].layer_id,1);
        old.validate().unwrap();
    }
    #[test] fn hidden_and_locked_layers_are_skipped(){
        let mut d=Document::default();
        let layer=d.add_layer("Overlay".into());
        let id=d.add_to_layer(Shape::Rectangle{x:0.0,y:0.0,w:30.0,h:30.0},Style::default(),layer);
        assert_eq!(d.hit_test(Point{x:2.0,y:2.0},0.0),Some(id));
        d.layers[1].locked=true;
        assert_eq!(d.hit_test(Point{x:2.0,y:2.0},0.0),None);
        d.layers[1].visible=false;
        assert!(!d.to_svg().contains("object-1"));
    }
    #[test] fn cubic_curve_has_valid_svg_and_can_move(){
        let mut d=Document::default();
        let mut shape=Shape::Bezier{start:Point{x:10.0,y:10.0},
            segments:vec![CubicSegment{control1:Point{x:20.0,y:30.0},
            control2:Point{x:50.0,y:40.0},end:Point{x:60.0,y:10.0}}],closed:false};
        shape.translate(10.0,20.0);
        d.add(shape,Style::default());
        d.validate().unwrap();
        assert!(d.to_svg().contains("C 30 50, 60 60, 70 30"));
    }
    #[test] fn failed_write_does_not_destroy_saved_document(){
        let dir=tempfile::tempdir().unwrap();
        let path=dir.path().join("project.ocd");
        let good=Document::default();
        good.write(&path).unwrap();
        let mut broken=good.clone();
        broken.page.width_mm=f32::NAN;
        assert!(broken.write(&path).is_err());
        assert_eq!(Document::read(&path).unwrap(),good);
    }
    #[test] fn reject_nonfinite_path_points(){
        let mut doc=Document::default();
        doc.add(Shape::Bezier{
            start:Point{x:0.0,y:0.0},
            segments:vec![CubicSegment{control1:Point{x:f32::NAN,y:2.0},
                control2:Point{x:5.0,y:5.0},end:Point{x:10.0,y:10.0}}],
            closed:false
        },Style::default());
        assert!(doc.validate().unwrap_err().contains("Non-finite"));
    }
    #[test] fn reject_control_characters_in_text(){
        let mut doc=Document::default();
        doc.add(Shape::Text{x:1.0,y:1.0,value:String::from("hello\0"),size_mm:10.0},
            Style::default());
        assert!(doc.validate().is_err());
    }
    #[test] fn embedded_svg_can_move_and_export(){
        let mut doc=Document::default();
        let shape=Shape::EmbeddedSvg{x:2.0,y:3.0,w:100.0,h:200.0,
            svg:"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"/>".into(),
            source_format:"CDR".into()};
        doc.add(shape,Style::default());
        doc.validate().unwrap();
        assert!(doc.to_svg().contains("data:image/svg+xml;base64,"));
    }
    #[test] fn raster_roundtrip_and_svg_embedding(){
        let png=base64::engine::general_purpose::STANDARD
            .encode(b"\x89PNG\r\n\x1a\nplaceholder");
        let mut doc=Document::default();
        doc.add(Shape::Raster{x:5.0,y:6.0,w:15.0,h:20.0,
            pixel_width:1,pixel_height:1,mime:"image/png".into(),
            data_base64:png},Style::default());
        doc.validate().unwrap();
        assert!(doc.to_svg().contains("data:image/png;base64,"));
        let copy:Document=serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
        assert_eq!(doc,copy);
    }
    #[test] fn raster_rejects_wrong_mime_and_invalid_base64(){
        let mut doc=Document::default();
        let id=doc.add(Shape::Raster{x:0.0,y:0.0,w:10.0,h:10.0,
            pixel_width:100,pixel_height:100,mime:"image/jpeg".into(),
            data_base64:"invalid!".into()},Style::default());
        assert!(doc.validate().is_err());
        if let Shape::Raster{data_base64,..}=&mut doc.get_mut(id).unwrap().shape{
            *data_base64=base64::engine::general_purpose::STANDARD.encode(b"\x89PNG\r\n\x1a\n");
        }
        assert!(doc.validate().is_err());
    }
    #[test] fn aligns_shape_edges_and_distributes_centers(){
        let mut doc=Document::default();
        let mut ids=Vec::new();
        for x in [10.0,60.0,110.0]{
            ids.push(doc.add(Shape::Rectangle{x,y:20.0,w:10.0,h:15.0},Style::default()));
        }
        assert_eq!(doc.align_objects(&ids,Alignment::Top),3);
        assert_eq!(doc.distribute_objects(&ids,Distribution::HorizontalCenters),3);
        let b:Vec<_>=doc.objects.iter().map(|o|o.shape.bounds()).collect();
        assert_eq!(b[0].y,b[2].y);
        assert!((b[1].x-60.0).abs()<0.001);
        assert_eq!(doc.align_objects(&ids,Alignment::Left),3);
        assert!(doc.objects.iter().all(|o|(o.shape.bounds().x-10.0).abs()<0.001));
    }
    #[test] fn alignment_respects_locked_layer(){
        let mut doc=Document::default();
        let locked=doc.add_layer("Locked".into());
        let a=doc.add(Shape::Rectangle{x:0.0,y:0.0,w:10.0,h:10.0},Style::default());
        let b=doc.add_to_layer(Shape::Rectangle{x:100.0,y:20.0,w:10.0,h:10.0},Style::default(),locked);
        doc.layers[1].locked=true;
        assert_eq!(doc.align_objects(&[a,b],Alignment::Left),0);
        assert_eq!(doc.get_mut(b).unwrap().shape.bounds().x,100.0);
    }
    #[test] fn memory_estimate_accounts_for_assets(){
        let mut doc=Document::default();
        let before=doc.estimated_size_bytes();
        doc.add(Shape::EmbeddedSvg{x:0.0,y:0.0,w:10.0,h:10.0,
            source_format:"SVG".into(),svg:"a".repeat(5000)},Style::default());
        assert!(doc.estimated_size_bytes()>before+5000);
    }
    #[test] fn multiline_cyrillic_text_exports_preserving_layout(){
        let mut doc=Document::default();
        let id=doc.add(Shape::Text{x:10.0,y:20.0,
            value:"КАРТА И РЕКЛАМА\nВторая строка".into(),size_mm:8.0},
            Style::default());
        let obj=doc.get_mut(id).unwrap();
        obj.text_format=TextFormat{family:"Arial".into(),weight:700,
            italic:true,alignment:TextAlign::Center,letter_spacing_mm:0.25,line_height:1.5};
        doc.validate().unwrap();
        let svg=doc.to_svg();
        assert!(svg.contains("font-weight=\"700\""));
        assert!(svg.contains("text-anchor=\"middle\""));
        assert!(svg.contains("dy=\"12\""));
        assert!(svg.contains("КАРТА И РЕКЛАМА"));
        assert!(svg.contains("Вторая строка"));
        let copy:Document=serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
        assert_eq!(copy,doc);
        assert!(copy.objects[0].bounds().h>8.0);
    }
    #[test] fn legacy_objects_deserialize_default_text_format(){
        let json=r##"{"id":7,"shape":{"type":"text","x":1,"y":2,"value":"Hi","size_mm":4},"style":{"fill":"#000000","stroke":"none","stroke_width_mm":0},"layer_id":1}"##;
        let object:Object=serde_json::from_str(json).unwrap();
        assert_eq!(object.text_format,TextFormat::default());
    }
    #[test] fn malicious_text_format_values_rejected(){
        let mut doc=Document::default();
        let id=doc.add(Shape::Text{x:1.0,y:1.0,value:"Hello".into(),size_mm:5.0},Style::default());
        doc.get_mut(id).unwrap().text_format.family="\u{0000}".into();
        assert!(doc.validate().is_err());
    }
    #[test] fn compound_path_supports_holes_and_json_roundtrip(){
        let make=|x:f32,y:f32,w:f32,h:f32|Contour{
            start:Point{x,y},
            segments:vec![
                CubicSegment{control1:Point{x,y},control2:Point{x:x+w,y},end:Point{x:x+w,y}},
                CubicSegment{control1:Point{x:x+w,y},control2:Point{x:x+w,y:y+h},end:Point{x:x+w,y:y+h}},
                CubicSegment{control1:Point{x:x+w,y:y+h},control2:Point{x,y:y+h},end:Point{x,y:y+h}},
                CubicSegment{control1:Point{x,y:y+h},control2:Point{x,y},end:Point{x,y}},
            ],closed:true
        };
        let mut doc=Document::default();
        doc.add(Shape::CompoundPath{contours:vec![
            make(10.0,10.0,40.0,40.0),make(20.0,20.0,20.0,20.0)
        ]},Style::default());
        doc.validate().unwrap();
        let svg=doc.to_svg();
        assert!(svg.contains("fill-rule=\"evenodd\""));
        assert!(svg.contains("M 20 20"));
        let copied:Document=serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
        assert_eq!(doc,copied);
        assert_eq!(doc.objects[0].bounds().w,40.0);
    }
    #[test] fn groups_roundtrip_and_follow_all_members(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Rectangle{x:1.,y:2.,w:3.,h:4.},Style::default());
        let b=doc.add(Shape::Ellipse{x:5.,y:6.,w:7.,h:8.},Style::default());
        doc.get_mut(a).unwrap().name="Logo background".into();
        let group=doc.group_objects(&[a,b]).unwrap();
        assert!(group>2);
        assert_eq!(doc.group_members(a),vec![a,b]);
        doc.validate().unwrap();
        let copy:Document=serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
        assert_eq!(copy,doc);
        assert_eq!(copy.objects[0].name,"Logo background");
        assert_eq!(doc.ungroup_objects(&[a]).unwrap(),2);
        assert_eq!(doc.group_members(a),vec![a]);
    }
    #[test] fn group_rejects_locked_layers_and_partial_groups(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Rectangle{x:0.,y:0.,w:2.,h:2.},Style::default());
        let layer=doc.add_layer("locked".into());
        let b=doc.add_to_layer(Shape::Ellipse{x:3.,y:3.,w:2.,h:2.},Style::default(),layer);
        doc.layers[1].locked=true;
        assert!(doc.group_objects(&[a,b]).is_err());
        doc.layers[1].locked=false;
        doc.group_objects(&[a,b]).unwrap();
        assert!(doc.group_objects(&[a,b]).is_err());
        doc.layers[1].locked=true;
        assert!(doc.ungroup_objects(&[a]).is_err());
    }
    #[test] fn rejects_group_id_that_collides_with_later_object(){
        let mut doc=Document::default();
        let first=doc.add(Shape::Rectangle{x:0.,y:0.,w:10.,h:10.},Style::default());
        let second=doc.add(Shape::Rectangle{x:15.,y:0.,w:10.,h:10.},Style::default());
        doc.get_mut(first).unwrap().group_id=Some(second);
        assert!(doc.validate().unwrap_err().contains("group"));
    }
    #[test] fn group_alignment_and_distribution_are_atomic(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Rectangle{x:0.,y:0.,w:5.,h:5.},Style::default());
        let b=doc.add(Shape::Rectangle{x:20.,y:10.,w:5.,h:5.},Style::default());
        let c=doc.add(Shape::Rectangle{x:40.,y:0.,w:5.,h:5.},Style::default());
        let group=doc.group_objects(&[a,b]).unwrap();
        assert!(group>c);
        let prior=doc.objects.clone();
        assert_eq!(doc.align_objects(&[a,b],Alignment::Left),0);
        assert_eq!(doc.distribute_objects(&[a,b,c],Distribution::HorizontalCenters),0);
        assert_eq!(doc.objects,prior);
        doc.ungroup_objects(&[a]).unwrap();
        assert_eq!(doc.align_objects(&[a,b],Alignment::Top),2);
    }
    #[test] fn old_files_default_to_unlabelled_ungrouped_objects(){
        let mut doc=Document::default();
        doc.add(Shape::Rectangle{x:1.,y:1.,w:5.,h:5.},Style::default());
        let mut json:serde_json::Value=serde_json::to_value(&doc).unwrap();
        let obj=&mut json["objects"][0];
        obj.as_object_mut().unwrap().remove("name");
        obj.as_object_mut().unwrap().remove("group_id");
        let loaded:Document=serde_json::from_value(json).unwrap();
        assert!(loaded.objects[0].name.is_empty());
        assert_eq!(loaded.objects[0].group_id,None);
        loaded.validate().unwrap();
    }
    #[test] fn document_roundtrip(){
        let mut a=Document::default();
        a.add(Shape::Rectangle{x:2.0,y:3.0,w:4.0,h:5.0},Style::default());
        let b:Document=serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        assert_eq!(a,b); b.validate().unwrap();
    }
    #[test] fn svg_escapes_text(){
        let mut a=Document::default();
        a.add(Shape::Text{x:1.0,y:1.0,value:"<Hello>&".into(),size_mm:12.0},Style::default());
        assert!(a.to_svg().contains("&lt;Hello&gt;&amp;"));
    }
    #[test] fn overlapping_hit_test_picks_topmost(){
        let mut a=Document::default();
        a.add(Shape::Rectangle{x:0.0,y:0.0,w:10.0,h:10.0},Style::default());
        let id=a.add(Shape::Ellipse{x:0.0,y:0.0,w:10.0,h:10.0},Style::default());
        assert_eq!(a.hit_test(Point{x:5.0,y:5.0},0.0),Some(id));
    }
    #[test] fn oversized_sheet_is_legal(){
        let mut a=Document::default();a.page.width_mm=6000.0;a.page.height_mm=3000.0;
        assert!(a.validate().is_ok());
    }
    #[test] fn invalid_shape_is_rejected(){
        let mut a=Document::default();
        a.add(Shape::Rectangle{x:0.0,y:0.0,w:-1.0,h:2.0},Style::default());
        assert!(a.validate().is_err());
    }
}
