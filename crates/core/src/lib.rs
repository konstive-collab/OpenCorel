//! OpenCorel document engine. Page coordinates are millimetres, not pixels.
//! Experimental schema v1; not a CorelDRAW CDR implementation.
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, io, path::Path};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point { pub x: f32, pub y: f32 }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }

impl Bounds {
    pub fn contains(self, p: Point, tolerance: f32) -> bool {
        p.x >= self.x - tolerance && p.y >= self.y - tolerance
            && p.x <= self.x + self.w + tolerance && p.y <= self.y + self.h + tolerance
    }
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
}

impl Shape {
    pub fn bounds(&self) -> Bounds {
        match self {
            Self::Rectangle {x,y,w,h} | Self::Ellipse {x,y,w,h} => Bounds {x:*x,y:*y,w:*w,h:*h},
            Self::Text {x,y,value,size_mm} => Bounds {
                x:*x, y:*y, w:size_mm * (value.chars().count().max(1) as f32) * 0.6,
                h:size_mm * 1.25,
            },
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
            Self::Rectangle {x,y,..} | Self::Ellipse {x,y,..} | Self::Text {x,y,..} => {*x+=dx;*y+=dy;},
            Self::Polyline {points,..} => for p in points {p.x+=dx;p.y+=dy;},
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Object { pub id: u64, pub shape: Shape, pub style: Style }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub schema_version: u32,
    pub name: String,
    pub page: Page,
    pub objects: Vec<Object>,
    pub next_id: u64,
}
impl Default for Document {
    fn default() -> Self {
        Self { schema_version:SCHEMA_VERSION, name:"Новый макет".into(),
            page:Page {width_mm:841.0,height_mm:594.0},objects:Vec::new(),next_id:1 }
    }
}

impl Document {
    pub fn add(&mut self, shape:Shape, style:Style) -> u64 {
        let id=self.next_id;
        self.next_id += 1;
        self.objects.push(Object{id,shape,style});
        id
    }
    pub fn get_mut(&mut self,id:u64) -> Option<&mut Object> {
        self.objects.iter_mut().find(|o|o.id==id)
    }
    pub fn remove(&mut self,id:u64) -> bool {
        let n=self.objects.len(); self.objects.retain(|o|o.id!=id); self.objects.len()!=n
    }
    pub fn hit_test(&self,p:Point,tolerance:f32) -> Option<u64> {
        self.objects.iter().rev().find(|o|o.shape.bounds().contains(p,tolerance)).map(|o|o.id)
    }
    pub fn validate(&self)->Result<(),String>{
        if self.schema_version!=SCHEMA_VERSION {return Err("Unknown document schema".into());}
        if ![self.page.width_mm,self.page.height_mm].iter().all(|v|v.is_finite() && (1.0..=100_000.0).contains(v)) {
            return Err("Invalid page dimensions".into());
        }
        if self.objects.len()>100_000{return Err("Object limit exceeded".into());}
        let mut ids=HashSet::new();
        for o in &self.objects {
            if o.id==0 || o.id>=self.next_id || !ids.insert(o.id) {return Err("Invalid object ID".into());}
            if !valid_color(&o.style.fill) || !valid_color(&o.style.stroke)
                || !o.style.stroke_width_mm.is_finite()
                || !(0.0..=1000.0).contains(&o.style.stroke_width_mm) {
                return Err("Invalid style".into());
            }
            let b=o.shape.bounds();
            if ![b.x,b.y,b.w,b.h].iter().all(|v|v.is_finite()) || b.w<0.0 || b.h<0.0 {
                return Err("Invalid shape geometry".into());
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
        fs::write(path,serde_json::to_vec_pretty(self).map_err(|e|e.to_string())?)
            .map_err(|e|e.to_string())
    }
    pub fn export_svg(&self,path:impl AsRef<Path>)->io::Result<()>{
        fs::write(path,self.to_svg())
    }
    pub fn to_svg(&self)->String{
        let mut out=format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" viewBox=\"0 0 {} {}\">\n<title>{}</title>\n",
            self.page.width_mm,self.page.height_mm,self.page.width_mm,self.page.height_mm,
            xml_escape(&self.name));
        for obj in &self.objects {
            let s=&obj.style;
            let common=format!("fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"",
                s.fill,s.stroke,s.stroke_width_mm);
            let element=match &obj.shape {
                Shape::Rectangle{x,y,w,h} => format!("<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" {common}/>"),
                Shape::Ellipse{x,y,w,h} => format!("<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" {common}/>",
                    x+w/2.0,y+h/2.0,w/2.0,h/2.0),
                Shape::Text{x,y,value,size_mm} => format!("<text x=\"{x}\" y=\"{}\" font-size=\"{size_mm}\" fill=\"{}\">{}</text>",
                    y+size_mm,s.fill,xml_escape(value)),
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
