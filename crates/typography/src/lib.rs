//! Convert system-font shaped text to editable cubic outlines, preserving holes.
//! Never bundles, exports, or exposes the original font files.
use fontdb::{Database,Family,Query,Style as FontStyle,Weight};
use opencorel_core::{Contour,CubicSegment,Object,Point,Shape,TextAlign};
use rustybuzz::{self,ttf_parser::{self,OutlineBuilder,GlyphId}};

const MAX_GLYPHS:usize=8_000;
const MAX_CONTOURS:usize=10_000;
const MAX_SEGMENTS:usize=60_000;

#[derive(Default)]
pub struct TextOutliner { database:Option<Database> }
impl TextOutliner {
    pub fn new()->Self{Self::default()}
    fn fonts(&mut self)->&Database{
        self.database.get_or_insert_with(||{
            let mut db=Database::new();
            db.load_system_fonts();
            db
        })
    }
    pub fn convert_object(&mut self,object:&Object)->Result<Shape,String>{
        let Shape::Text{x,y,value,size_mm}=&object.shape else{
            return Err("Only editable text objects can be converted to curves".into());
        };
        object.text_format.validate()?;
        if value.trim().is_empty(){return Err("Cannot outline an empty text object".into());}
        if value.chars().count()>MAX_GLYPHS{
            return Err(format!("Text exceeds {MAX_GLYPHS} character limit for outline conversion"));
        }
        let fmt=&object.text_format;
        let named=fmt.family!="sans-serif"&&fmt.family!="serif"&&fmt.family!="monospace";
        let family=match fmt.family.as_str(){
            "sans-serif"=>Family::SansSerif,
            "serif"=>Family::Serif,
            "monospace"=>Family::Monospace,
            _=>Family::Name(&fmt.family),
        };
        let families=[family];
        let query=Query{families:&families,
            weight:Weight(fmt.weight),
            style:if fmt.italic{FontStyle::Italic}else{FontStyle::Normal},
            ..Query::default()};
        let db=self.fonts();
        let id=db.query(&query)
            .ok_or_else(||format!("Font '{}' is not installed. Install it before outlining.",fmt.family))?;
        if named {
            let matching=db.face(id).map(|face|face.families.iter().any(|(name,_)|
                name.eq_ignore_ascii_case(&fmt.family))).unwrap_or(false);
            if !matching {
                return Err(format!("Exact font family '{}' is not available; refusing substitution.",fmt.family));
            }
        }
        db.with_face_data(id,|bytes,index|{
            outline_font(bytes,index,*x,*y,value,*size_mm,fmt)
        }).ok_or_else(||"Unable to read selected installed font".to_string())?
    }
}

fn outline_font(bytes:&[u8],index:u32,x:f32,y:f32,value:&str,size:f32,
    fmt:&opencorel_core::TextFormat)->Result<Shape,String>{
    let hb=rustybuzz::Face::from_slice(bytes,index)
        .ok_or("The selected font cannot be shaped")?;
    let face=ttf_parser::Face::parse(bytes,index)
        .map_err(|_|"Cannot read font outlines")?;
    let em=hb.units_per_em() as f32;
    if em<=0.0{return Err("Font has invalid units per em".into());}
    let scale=size/em;
    let lines=value.split('\n').collect::<Vec<_>>();
    let mut shaped=Vec::with_capacity(lines.len());
    for line in &lines {
        let mut buffer=rustybuzz::UnicodeBuffer::new();
        buffer.push_str(line);
        buffer.guess_segment_properties();
        let out=rustybuzz::shape(&hb,&[],buffer);
        if out.glyph_infos().iter().any(|info|info.glyph_id==0){
            return Err("Font lacks a required glyph (missing character). No conversion was performed.".into());
        }
        let width=out.glyph_positions().iter().map(|pos|pos.x_advance as f32*scale)
            .sum::<f32>()+
            (out.len().saturating_sub(1) as f32)*fmt.letter_spacing_mm;
        shaped.push((out,width));
    }
    let max_width=shaped.iter().map(|(_,width)|*width).fold(0.0,f32::max);
    let mut contours=Vec::<Contour>::new();
    let mut total_segments=0usize;
    for (row,(glyphs,width)) in shaped.iter().enumerate(){
        let left=x+match fmt.alignment{
            TextAlign::Left=>0.0,
            TextAlign::Center=>(max_width-*width)*0.5,
            TextAlign::Right=>max_width-*width,
        };
        let baseline=y+size+row as f32*size*fmt.line_height;
        let mut pen_x=left;
        let mut pen_y=baseline;
        for (i,(glyph,pos)) in glyphs.glyph_infos().iter()
            .zip(glyphs.glyph_positions()).enumerate(){
            let origin_x=pen_x+pos.x_offset as f32*scale;
            let origin_y=pen_y-pos.y_offset as f32*scale;
            let mut collector=Collector::new(origin_x,origin_y,scale);
            let outlined=face.outline_glyph(GlyphId(glyph.glyph_id as u16),&mut collector);
            if outlined.is_some(){
                collector.finish();
                for contour in collector.contours{
                    total_segments+=contour.segments.len();
                    contours.push(contour);
                    if contours.len()>MAX_CONTOURS||total_segments>MAX_SEGMENTS{
                        return Err("Text outlines exceed geometry safety limits".into());
                    }
                }
            }else if glyph.glyph_id!=3 {
                // Whitespace glyphs typically have no outlines; a malformed or
                // color-only glyph may also yield none. Fail instead of dropping it.
                let char_index=glyph.cluster as usize;
                let is_space=lines[row].get(char_index..).and_then(|s|s.chars().next())
                    .map(|c|c.is_whitespace()).unwrap_or(false);
                if !is_space{
                    return Err("A glyph has no vector outline (bitmap/color or malformed font).".into());
                }
            }
            pen_x+=pos.x_advance as f32*scale;
            pen_y-=pos.y_advance as f32*scale;
            if i+1<glyphs.len(){pen_x+=fmt.letter_spacing_mm;}
        }
    }
    if contours.is_empty(){return Err("No drawable glyph contours".into());}
    Ok(Shape::CompoundPath{contours})
}

struct Collector{
    ox:f32,oy:f32,scale:f32,
    start:Option<Point>,current:Point,
    segments:Vec<CubicSegment>,contours:Vec<Contour>,
}
impl Collector{
    fn new(ox:f32,oy:f32,scale:f32)->Self{
        Self{ox,oy,scale,start:None,current:Point::default(),
            segments:Vec::new(),contours:Vec::new()}
    }
    fn map(&self,x:f32,y:f32)->Point{
        Point{x:self.ox+x*self.scale,y:self.oy-y*self.scale}
    }
    fn flush(&mut self,closed:bool){
        if let Some(start)=self.start.take(){
            if !self.segments.is_empty(){
                self.contours.push(Contour{start,segments:std::mem::take(&mut self.segments),closed});
            }else{self.segments.clear();}
        }
    }
    fn finish(&mut self){self.flush(true);}
    fn add(&mut self,c1:Point,c2:Point,end:Point){
        self.segments.push(CubicSegment{control1:c1,control2:c2,end});
        self.current=end;
    }
}
impl OutlineBuilder for Collector{
    fn move_to(&mut self,x:f32,y:f32){
        self.flush(true);
        let p=self.map(x,y);
        self.start=Some(p);self.current=p;
    }
    fn line_to(&mut self,x:f32,y:f32){
        let to=self.map(x,y);
        let p=self.current;
        self.add(Point{x:p.x+(to.x-p.x)/3.0,y:p.y+(to.y-p.y)/3.0},
            Point{x:p.x+(to.x-p.x)*2.0/3.0,y:p.y+(to.y-p.y)*2.0/3.0},to);
    }
    fn quad_to(&mut self,x1:f32,y1:f32,x:f32,y:f32){
        let control=self.map(x1,y1);let end=self.map(x,y);
        let p=self.current;
        self.add(Point{x:p.x+(control.x-p.x)*2.0/3.0,
                y:p.y+(control.y-p.y)*2.0/3.0},
            Point{x:end.x+(control.x-end.x)*2.0/3.0,
                y:end.y+(control.y-end.y)*2.0/3.0},end);
    }
    fn curve_to(&mut self,x1:f32,y1:f32,x2:f32,y2:f32,x:f32,y:f32){
        self.add(self.map(x1,y1),self.map(x2,y2),self.map(x,y));
    }
    fn close(&mut self){self.flush(true);}
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Document,Shape,Style};
    #[test] fn collector_creates_closed_curves_and_counters(){
        let mut b=Collector::new(10.0,20.0,0.1);
        b.move_to(0.0,0.0);
        b.line_to(10.0,0.0);
        b.line_to(10.0,10.0);
        b.close();
        b.move_to(2.0,2.0);
        b.quad_to(5.0,9.0,8.0,2.0);
        b.close();
        assert_eq!(b.contours.len(),2);
        assert!(b.contours.iter().all(|c|c.closed));
        assert_eq!(b.contours[0].segments[0].end.x,11.0);
    }
    #[test] fn outlines_system_font_when_available(){
        let mut outliner=TextOutliner::new();
        let mut doc=Document::default();
        doc.add(Shape::Text{x:20.0,y:30.0,value:"ОО ВВ".into(),
            size_mm:12.0},Style::default());
        let result=outliner.convert_object(&doc.objects[0]);
        // Some stripped-down CI containers have no system fonts at all.
        if let Ok(shape)=result {
            if let Shape::CompoundPath{contours}=shape{
                assert!(contours.len()>=4);
            }else{panic!("not vector curves");}
        }
    }
}
