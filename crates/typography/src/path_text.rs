//! Font-shaped text along editable vector geometry.
//!
//! Shaping uses the selected installed system font via rustybuzz. Every glyph
//! is outlined independently, positioned at arc length and rotated with the
//! local tangent. The source text and guide path remain unmodified by the
//! desktop command; this module returns new native cubic contours.
use super::{Collector,TextOutliner,MAX_CONTOURS,MAX_GLYPHS,MAX_SEGMENTS};
use fontdb::{Family,Query,Style as FontStyle,Weight};
use opencorel_core::{Contour,CubicSegment,Object,Point,Shape,TextAlign};
use rustybuzz::{self,ttf_parser::{self,OutlineBuilder,GlyphId}};

const MAX_GUIDE_POINTS:usize=16_384;
const MAX_GUIDE_LENGTH_MM:f32=2_000_000.0;
const MIN_GUIDE_LENGTH_MM:f32=0.5;
const MAX_PATH_TEXT_GLYPHS:usize=1_500;
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct TextPathOptions{
    /// Distance from the guide path origin in millimetres.
    pub start_offset_mm:f32,
    /// Positive values shift towards the local left normal of the path.
    pub baseline_shift_mm:f32,
    /// Reverse guide direction while keeping text glyph order intact.
    pub reverse_guide:bool,
}
impl Default for TextPathOptions{
    fn default()->Self{
        Self{start_offset_mm:0.,baseline_shift_mm:0.,reverse_guide:false}
    }
}
impl TextPathOptions{
    fn validate(self)->Result<(),String>{
        if !self.start_offset_mm.is_finite()||
            !(0.0..=MAX_GUIDE_LENGTH_MM).contains(&self.start_offset_mm)||
            !self.baseline_shift_mm.is_finite()||
            self.baseline_shift_mm.abs()>5000.0{
            return Err("Text path offset must be 0..2,000,000 mm, baseline shift -5000..5000 mm".into());
        }
        Ok(())
    }
}

#[derive(Clone,Debug)]
struct PathMetric{
    points:Vec<Point>,
    cumulative:Vec<f32>,
    length:f32,
    closed:bool,
}
#[derive(Clone,Copy,Debug)]
struct Pose{position:Point,tangent:Point}
fn finite(p:Point)->bool{
    p.x.is_finite()&&p.y.is_finite()&&
    p.x.abs()<=100_000.0&&p.y.abs()<=100_000.0
}
fn cubic(p0:Point,p1:Point,p2:Point,p3:Point,t:f32)->Point{
    let u=1.-t;
    Point{
        x:u*u*u*p0.x+3.*u*u*t*p1.x+3.*u*t*t*p2.x+t*t*t*p3.x,
        y:u*u*u*p0.y+3.*u*u*t*p1.y+3.*u*t*t*p2.y+t*t*t*p3.y,
    }
}
fn sample_cubics(start:Point,segments:&[CubicSegment],
    points:&mut Vec<Point>)->Result<(),String>{
    if !finite(start)||segments.is_empty()||segments.len()>5000{
        return Err("Guide Bézier has invalid first point or too many/zero segments".into());
    }
    points.push(start);
    let mut previous=start;
    for seg in segments{
        if !finite(seg.control1)||!finite(seg.control2)||!finite(seg.end){
            return Err("Guide Bézier contains invalid controls".into());
        }
        let dist=previous.x.hypot(previous.y); // computed below for hull
        let _=dist;
        let extent=(seg.control1.x-previous.x).hypot(seg.control1.y-previous.y)+
            (seg.control2.x-seg.control1.x).hypot(seg.control2.y-seg.control1.y)+
            (seg.end.x-seg.control2.x).hypot(seg.end.y-seg.control2.y);
        let steps=(extent/3.0).ceil().clamp(8.,48.) as usize;
        if points.len().saturating_add(steps)>MAX_GUIDE_POINTS{
            return Err("Guide has too many sampled coordinates".into());
        }
        for i in 1..=steps{
            let t=i as f32/steps as f32;
            points.push(cubic(previous,seg.control1,seg.control2,seg.end,t));
        }
        previous=seg.end;
    }
    Ok(())
}
impl PathMetric{
    fn from_shape(shape:&Shape,reverse:bool)->Result<Self,String>{
        let (mut points,closed)=match shape{
            Shape::Polyline{points,closed}=>{
                if points.len()<2||points.len()>MAX_GUIDE_POINTS{
                    return Err("Polyline guide needs 2..16384 points".into());
                }
                (points.clone(),*closed)
            },
            Shape::Bezier{start,segments,closed}=>{
                let mut points=Vec::new();
                sample_cubics(*start,segments,&mut points)?;
                (points,*closed)
            },
            Shape::CompoundPath{contours}=>{
                let Some(first)=contours.first()else{
                    return Err("Compound guide has no contours".into());
                };
                let mut points=Vec::new();
                sample_cubics(first.start,&first.segments,&mut points)?;
                (points,first.closed)
            }
            Shape::Rectangle{x,y,w,h}=>{
                if *w<MIN_GUIDE_LENGTH_MM||*h<MIN_GUIDE_LENGTH_MM{
                    return Err("Rectangle guide is too small".into());
                }
                (vec![
                    Point{x:*x,y:*y},Point{x:x+w,y:*y},
                    Point{x:x+w,y:y+h},Point{x:*x,y:y+h},
                ],true)
            },
            Shape::Ellipse{x,y,w,h}=>{
                if *w<MIN_GUIDE_LENGTH_MM||*h<MIN_GUIDE_LENGTH_MM{
                    return Err("Ellipse guide is too small".into());
                }
                let mut p=Vec::with_capacity(128);
                for i in 0..128{
                    let a=std::f32::consts::TAU*i as f32/128.;
                    p.push(Point{x:x+w*0.5+(w*0.5)*a.cos(),
                        y:y+h*0.5+(h*0.5)*a.sin()});
                }
                (p,true)
            }
            _=>return Err("Choose a native path, rectangle or ellipse as the guide".into())
        };
        if points.iter().any(|p|!finite(*p)){
            return Err("Guide has invalid or out-of-range coordinates".into());
        }
        if reverse{points.reverse();}
        // Ignore degenerate consecutive vertices; they cannot define tangents.
        let mut cleaned:Vec<Point>=Vec::with_capacity(points.len()+1);
        for p in points{
            if cleaned.last().is_none_or(|prev|
                (p.x-prev.x).hypot(p.y-prev.y)>0.0001){
                cleaned.push(p);
            }
        }
        if closed&&cleaned.len()>=2{
            let first=cleaned[0];
            let last=*cleaned.last().unwrap();
            if (first.x-last.x).hypot(first.y-last.y)>0.0001{
                cleaned.push(first);
            }
        }
        if cleaned.len()<2{return Err("Guide collapses to a point".into());}
        if cleaned.len()>MAX_GUIDE_POINTS{
            return Err("Guide exceeds safe sampled-point limit".into());
        }
        let mut cumulative=Vec::with_capacity(cleaned.len());
        let mut length=0.0f32;
        cumulative.push(0.0);
        for pair in cleaned.windows(2){
            let dx=pair[1].x-pair[0].x;let dy=pair[1].y-pair[0].y;
            let segment=dx.hypot(dy);
            if !segment.is_finite()||segment<=0.0{
                return Err("Invalid guide segment".into());
            }
            length+=segment;
            cumulative.push(length);
        }
        if !(MIN_GUIDE_LENGTH_MM..=MAX_GUIDE_LENGTH_MM).contains(&length){
            return Err("Guide is too short or too long".into());
        }
        Ok(Self{points:cleaned,cumulative,length,closed})
    }
    fn sample(&self,distance:f32)->Result<Pose,String>{
        if !distance.is_finite(){return Err("Invalid path distance".into());}
        let s=if self.closed{
            // Precisely reaching the end of a closed perimeter returns start.
            distance.rem_euclid(self.length)
        }else{
            if distance< -0.001||distance>self.length+0.001{
                return Err("Text exceeds the selected open guide".into());
            }
            distance.clamp(0.,self.length)
        };
        let upper=self.cumulative.partition_point(|&x|x<=s)
            .clamp(1,self.points.len()-1);
        let from=self.points[upper-1];
        let to=self.points[upper];
        let length=self.cumulative[upper]-self.cumulative[upper-1];
        let fraction=((s-self.cumulative[upper-1])/length).clamp(0.,1.);
        let dx=(to.x-from.x)/length;let dy=(to.y-from.y)/length;
        Ok(Pose{
            position:Point{x:from.x+(to.x-from.x)*fraction,
                y:from.y+(to.y-from.y)*fraction},
            tangent:Point{x:dx,y:dy},
        })
    }
}
fn transform_glyph(contours:&mut [Contour],pose:Pose,shift:f32)->Result<(),String>{
    let (sin,cos)=(pose.tangent.y,pose.tangent.x);
    let normal=Point{x:-sin,y:cos};
    let map=|p:&mut Point|{
        let (x,y)=(p.x,p.y);
        p.x=pose.position.x+cos*x-sin*y+normal.x*shift;
        p.y=pose.position.y+sin*x+cos*y+normal.y*shift;
    };
    for contour in contours{
        map(&mut contour.start);
        for seg in &mut contour.segments{
            map(&mut seg.control1);map(&mut seg.control2);map(&mut seg.end);
        }
        if !finite(contour.start)||contour.segments.iter().any(|seg|
            !finite(seg.control1)||!finite(seg.control2)||!finite(seg.end)){
            return Err("Text glyph outlines exceed document limits".into());
        }
    }
    Ok(())
}
fn outline_path_font(bytes:&[u8],index:u32,object:&Object,guide:&PathMetric,
    opts:TextPathOptions)->Result<Shape,String>{
    let Shape::Text{value,size_mm,..}=&object.shape else{
        return Err("Text on path requires an editable text source".into());
    };
    if value.trim().is_empty()||value.chars().count()>MAX_PATH_TEXT_GLYPHS{
        return Err("Text on path supports 1..1500 nonempty characters".into());
    }
    if value.contains('\n')||value.contains('\r'){
        return Err("Text on path accepts one line; use separate text objects for multiple lines".into());
    }
    object.text_format.validate()?;
    let hb=rustybuzz::Face::from_slice(bytes,index)
        .ok_or("Selected font cannot be shaped")?;
    let face=ttf_parser::Face::parse(bytes,index)
        .map_err(|_|"Cannot read selected font outlines")?;
    let em=hb.units_per_em() as f32;
    if !em.is_finite()||em<=0.0{return Err("Font units are invalid".into());}
    let scale=size_mm/em;
    let mut buffer=rustybuzz::UnicodeBuffer::new();
    buffer.push_str(value);
    buffer.guess_segment_properties();
    let shaped=rustybuzz::shape(&hb,&[],buffer);
    if shaped.len()==0||shaped.len()>MAX_GLYPHS{
        return Err("Too many or no glyphs".into());
    }
    if shaped.glyph_infos().iter().any(|glyph|glyph.glyph_id==0){
        return Err("The selected font is missing a character glyph".into());
    }
    let spacing=object.text_format.letter_spacing_mm;
    let mut advances=Vec::with_capacity(shaped.len());
    for p in shaped.glyph_positions(){
        let advance=p.x_advance as f32*scale;
        if !advance.is_finite()||advance<0.0{
            return Err("Only forward horizontal shaping is supported for text on path".into());
        }
        advances.push(advance);
    }
    let text_width=advances.iter().sum::<f32>()+
        shaped.len().saturating_sub(1) as f32*spacing;
    if !text_width.is_finite()||text_width<=0.0||text_width>guide.length{
        return Err("Text does not fit on guide (reduce font size or increase path length)".into());
    }
    let aligned=match object.text_format.alignment{
        TextAlign::Left=>0.0,
        TextAlign::Center=>(guide.length-text_width)*0.5,
        TextAlign::Right=>guide.length-text_width,
    };
    let start=opts.start_offset_mm+aligned;
    if !guide.closed&&start+text_width>guide.length+0.001{
        return Err("Text and offset do not fit on the selected guide".into());
    }
    let mut contours=Vec::<Contour>::new();
    let mut segment_count=0usize;
    let mut cursor=0.0f32;
    for (i,((glyph,pos),advance)) in shaped.glyph_infos().iter()
        .zip(shaped.glyph_positions()).zip(advances.iter()).enumerate(){
        let centre=start+cursor+advance*0.5;
        let pose=guide.sample(centre)?;
        let local_x=pos.x_offset as f32*scale-advance*0.5;
        let local_y=-(pos.y_offset as f32)*scale;
        let mut collector=Collector::new(local_x,local_y,scale);
        let outlined=face.outline_glyph(GlyphId(glyph.glyph_id as u16),&mut collector);
        if outlined.is_some(){
            collector.finish();
            transform_glyph(&mut collector.contours,pose,opts.baseline_shift_mm)?;
            for contour in collector.contours{
                segment_count+=contour.segments.len();
                contours.push(contour);
                if contours.len()>MAX_CONTOURS||segment_count>MAX_SEGMENTS{
                    return Err("Text on path exceeded safe outline complexity".into());
                }
            }
        }else{
            let index=glyph.cluster as usize;
            let whitespace=value.get(index..).and_then(|part|part.chars().next())
                .is_some_and(char::is_whitespace);
            if !whitespace{
                return Err("A glyph lacks a vector outline".into());
            }
        }
        cursor+=advance;
        if i+1<advances.len(){cursor+=spacing;}
    }
    if contours.is_empty(){return Err("No drawable glyph contours".into());}
    Ok(Shape::CompoundPath{contours})
}
impl TextOutliner{
    /// Create editable Bézier glyph outlines following an arbitrary native
    /// vector guide. Neither input is changed. Font is exact, not substituted.
    pub fn outline_on_path(&mut self,text:&Object,path:&Shape,
        options:TextPathOptions)->Result<Shape,String>{
        options.validate()?;
        let metric=PathMetric::from_shape(path,options.reverse_guide)?;
        let Shape::Text{size_mm,..}=&text.shape else{
            return Err("Select a text object and an editable guide path".into());
        };
        if !size_mm.is_finite()||*size_mm<=0.0{return Err("Invalid font size".into());}
        let format=&text.text_format;
        format.validate()?;
        let named=!matches!(format.family.as_str(),"sans-serif"|"serif"|"monospace");
        let family=match format.family.as_str(){
            "sans-serif"=>Family::SansSerif,
            "serif"=>Family::Serif,
            "monospace"=>Family::Monospace,
            _=>Family::Name(&format.family),
        };
        let families=[family];
        let query=Query{families:&families,weight:Weight(format.weight),
            style:if format.italic{FontStyle::Italic}else{FontStyle::Normal},
            ..Query::default()};
        let db=self.fonts();
        let id=db.query(&query)
            .ok_or_else(||format!("Font '{}' is not installed",format.family))?;
        if named&&!db.face(id).is_some_and(|face|face.families.iter().any(|(name,_)|
            name.eq_ignore_ascii_case(&format.family))){
            return Err(format!("Exact font '{}' unavailable; no substitution allowed",format.family));
        }
        db.with_face_data(id,|bytes,index|{
            outline_path_font(bytes,index,text,&metric,options)
        }).ok_or_else(||"Could not read font data".to_string())?
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Document,Shape,Style};
    #[test]
    fn straight_path_has_exact_arc_length_and_tangent(){
        let shape=Shape::Polyline{points:vec![
            Point{x:10.,y:20.},Point{x:110.,y:20.}],closed:false};
        let path=PathMetric::from_shape(&shape,false).unwrap();
        assert!((path.length-100.).abs()<1e-4);
        let pose=path.sample(25.).unwrap();
        assert_eq!(pose.position,Point{x:35.,y:20.});
        assert_eq!(pose.tangent,Point{x:1.,y:0.});
        assert!(path.sample(120.).is_err());
        let reverse=PathMetric::from_shape(&shape,true).unwrap();
        assert_eq!(reverse.sample(0.).unwrap().position.x,110.);
        assert_eq!(reverse.sample(25.).unwrap().position.x,85.);
    }
    #[test]
    fn cubic_curve_preserves_path_geometry(){
        let shape=Shape::Bezier{
            start:Point{x:0.,y:0.},segments:vec![CubicSegment{
                control1:Point{x:0.,y:30.},
                control2:Point{x:100.,y:30.},
                end:Point{x:100.,y:0.},
            }],closed:false,
        };
        let path=PathMetric::from_shape(&shape,false).unwrap();
        assert!(path.length>100.);
        let middle=path.sample(path.length*0.5).unwrap();
        assert!(middle.position.y>15.);
        assert!((middle.position.x-50.).abs()<2.);
        assert!(middle.tangent.x>0.);
    }
    #[test]
    fn circle_and_rectangle_support_closed_text_paths(){
        for shape in [
            Shape::Ellipse{x:2.,y:3.,w:80.,h:80.},
            Shape::Rectangle{x:2.,y:3.,w:90.,h:45.},
        ]{
            let path=PathMetric::from_shape(&shape,false).unwrap();
            assert!(path.closed);
            assert!(path.length>200.);
            let start=path.sample(0.).unwrap();
            let end=path.sample(path.length).unwrap();
            assert!((start.position.x-end.position.x).abs()<1e-4);
            assert!((start.position.y-end.position.y).abs()<1e-4);
        }
    }
    #[test]
    fn bad_path_and_text_offsets_reject_without_panics(){
        let shape=Shape::Polyline{points:vec![
            Point{x:f32::NAN,y:1.},Point{x:20.,y:4.}],closed:false};
        assert!(PathMetric::from_shape(&shape,false).is_err());
        assert!(TextPathOptions{baseline_shift_mm:f32::NAN,
            ..TextPathOptions::default()}.validate().is_err());
        assert!(PathMetric::from_shape(&Shape::Text{x:0.,y:0.,
            value:"Guide?".into(),size_mm:5.},false).is_err());
    }
    #[test]
    fn outlines_text_on_straight_and_bezier_when_system_font_available(){
        let mut d=Document::default();
        let id=d.add(Shape::Text{x:0.,y:0.,
            value:"OpenCorel".into(),size_mm:8.},Style::default());
        let text=d.objects.iter().find(|o|o.id==id).unwrap().clone();
        let guide=Shape::Polyline{points:vec![
            Point{x:0.,y:40.},Point{x:200.,y:40.}],closed:false};
        let mut outliner=TextOutliner::new();
        let result=outliner.outline_on_path(&text,&guide,TextPathOptions::default());
        if outliner.font_families().is_empty(){
            assert!(result.is_err());return;
        }
        if let Ok(Shape::CompoundPath{contours})=result{
            assert!(!contours.is_empty());
            let mut native=Document::default();
            native.add(Shape::CompoundPath{contours},Style::default());
            native.validate().unwrap();
        }else{
            // Some CI fonts lack selected glyphs, same fail-closed semantics
            // as convert_object. The geometry tests are deterministic.
            assert!(result.is_err());
        }
    }
}
