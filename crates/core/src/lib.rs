//! OpenCorel document engine. Page coordinates are millimetres, not pixels.
//! Experimental schema v1; not a CorelDRAW CDR implementation.
use serde::{Deserialize, Serialize};
use base64::Engine;
use std::{collections::HashSet, fs, io::{self, Write}, path::Path};

pub mod svg_import;
pub mod kdr;

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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Object {
    pub id: u64,
    pub shape: Shape,
    pub style: Style,
    #[serde(default="default_layer_id")]
    pub layer_id: u64,
    #[serde(default)]
    pub text_format:TextFormat,
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
}
impl Default for Document {
    fn default() -> Self {
        Self { schema_version:SCHEMA_VERSION, name:"Новый макет".into(),
            page:Page {width_mm:841.0,height_mm:594.0},objects:Vec::new(),next_id:1,
            layers:default_layers(),next_layer_id:default_next_layer_id() }
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
                Shape::EmbeddedSvg{svg,..}=>svg.len(),
                Shape::Raster{data_base64,..}=>data_base64.len(),
                _=>0,
            };
            total=total.saturating_add(std::mem::size_of::<Object>()
                +obj.style.fill.len()+obj.style.stroke.len()+shape_bytes);
        }
        total
    }
    pub fn add(&mut self, shape:Shape, style:Style) -> u64 {
        self.add_to_layer(shape,style,1)
    }
    pub fn add_to_layer(&mut self,shape:Shape,style:Style,layer_id:u64)->u64 {
        let layer_id=if self.layers.iter().any(|l|l.id==layer_id){layer_id}else{1};
        let id=self.next_id;
        self.next_id+=1;
        self.objects.push(Object{id,shape,style,layer_id,
            text_format:TextFormat::default()});
        id
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
        self.objects.iter().filter(move |obj|visible.contains(&obj.layer_id))
    }
    pub fn get_mut(&mut self,id:u64) -> Option<&mut Object> {
        self.objects.iter_mut().find(|o|o.id==id)
    }
    pub fn remove(&mut self,id:u64) -> bool {
        let n=self.objects.len(); self.objects.retain(|o|o.id!=id); self.objects.len()!=n
    }
    /// Align the bounding boxes of eligible, unlocked objects.
    pub fn align_objects(&mut self,ids:&[u64],alignment:Alignment)->usize {
        let wanted:HashSet<u64>=ids.iter().copied().collect();
        let editable:HashSet<u64>=self.layers.iter()
            .filter(|l|l.visible&&!l.locked).map(|l|l.id).collect();
        let objects:Vec<_>=self.objects.iter().filter(|o|
            wanted.contains(&o.id)&&editable.contains(&o.layer_id))
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
        let mut positions:Vec<(u64,f32)>=self.objects.iter().filter(|o|
            wanted.contains(&o.id)&&editable.contains(&o.layer_id))
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
        let editable:HashSet<u64>=self.layers.iter()
            .filter(|layer|layer.visible&&!layer.locked).map(|layer|layer.id).collect();
        self.objects.iter().rev().find(|o|
            editable.contains(&o.layer_id) && o.bounds().contains(p,tolerance)
        ).map(|o|o.id)
    }
    pub fn validate(&self)->Result<(),String>{
        if self.schema_version!=SCHEMA_VERSION {return Err("Unknown document schema".into());}
        if ![self.page.width_mm,self.page.height_mm].iter().all(|v|v.is_finite() && (1.0..=100_000.0).contains(v)) {
            return Err("Invalid page dimensions".into());
        }
        if self.objects.len()>100_000{return Err("Object limit exceeded".into());}
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
        let mut ids=HashSet::new();
        for o in &self.objects {
            if o.id==0 || o.id>=self.next_id || !ids.insert(o.id) {return Err("Invalid object ID".into());}
            if !layer_ids.contains(&o.layer_id){return Err("Object references unknown layer".into());}
            if !valid_paint(&o.style.fill) || !valid_paint(&o.style.stroke)
                || !o.style.stroke_width_mm.is_finite()
                || !(0.0..=1000.0).contains(&o.style.stroke_width_mm) {
                return Err("Invalid style".into());
            }
            o.text_format.validate()?;
            let b=o.bounds();
            if ![b.x,b.y,b.w,b.h].iter().all(|v|v.is_finite()) || b.w<0.0 || b.h<0.0 {
                return Err("Invalid shape geometry".into());
            }
            match &o.shape{
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
        for obj in self.visible_objects() {
            let s=&obj.style;
            let common=format!("fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"",
                s.fill,s.stroke,s.stroke_width_mm);
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
                        fmt.letter_spacing_mm,s.fill);
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
            out.push_str(&format!("<g id=\"object-{}\">{element}</g>\n",obj.id));
        }
        out.push_str("</svg>\n");out
    }
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
