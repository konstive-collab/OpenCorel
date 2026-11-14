//! Restricted SVG importer: independent subset; explicitly rejects unsupported constructs.
use crate::{CubicSegment,Document,Point,Shape,Style,valid_paint};
use roxmltree::{Node,ParsingOptions};
use svgtypes::{PathParser,PathSegment,PointsParser};

#[derive(Clone,Copy)]
struct ViewBox { x:f32, y:f32, sx:f32, sy:f32 }
impl ViewBox {
    fn point(self,p:Point)->Point{
        Point{x:(p.x-self.x)*self.sx,y:(p.y-self.y)*self.sy}
    }
}
fn point(x:f64,y:f64)->Point{Point{x:x as f32,y:y as f32}}
fn required_number(n:Node<'_,'_>,name:&str)->Result<f32,String>{
    let raw=n.attribute(name).ok_or_else(||format!("Missing SVG attribute: {name}"))?;
    let value=raw.parse::<f32>().map_err(|_|format!("Invalid SVG number {name}: {raw}"))?;
    if !value.is_finite(){return Err("Non-finite SVG coordinate".into());}
    Ok(value)
}
fn number_or(n:Node<'_,'_>,name:&str,default:f32)->Result<f32,String>{
    if n.attribute(name).is_some(){required_number(n,name)}else{Ok(default)}
}
fn svg_dimension(raw:&str)->Result<f32,String>{
    let raw=raw.trim();
    let (v,scale)=if let Some(v)=raw.strip_suffix("mm"){(v,1.0)}
        else if let Some(v)=raw.strip_suffix("cm"){(v,10.0)}
        else if let Some(v)=raw.strip_suffix("in"){(v,25.4)}
        else if let Some(v)=raw.strip_suffix("pt"){(v,25.4/72.0)}
        else if let Some(v)=raw.strip_suffix("px"){(v,25.4/96.0)}
        else {(raw,25.4/96.0)};
    let number=v.trim().parse::<f32>().map_err(|_|format!("Invalid SVG page dimension: {raw}"))?;
    if !number.is_finite() || number<=0.0 {return Err("Invalid SVG page dimension".into());}
    Ok(number*scale)
}
fn paint(raw:&str)->Result<String,String>{
    let raw=raw.trim();
    let out=match raw{
        "none"|"transparent"=>"none".to_string(),
        "black"=>"#000000".to_string(),
        "white"=>"#FFFFFF".to_string(),
        "red"=>"#FF0000".to_string(),
        "green"=>"#008000".to_string(),
        "blue"=>"#0000FF".to_string(),
        _ if raw.len()==4 && raw.starts_with('#') && raw[1..].bytes().all(|c|c.is_ascii_hexdigit())=>{
            let b=raw.as_bytes();format!("#{}{}{}{}{}{}",b[1] as char,b[1] as char,
                b[2] as char,b[2] as char,b[3] as char,b[3] as char)
        }
        _ if valid_paint(raw)=>raw.to_string(),
        _=>return Err(format!("Unsupported SVG paint: {raw}")),
    };
    Ok(out)
}
fn style_field(n:Node<'_,'_>,name:&str)->Option<String>{
    for el in n.ancestors().filter(|a|a.is_element()){
        if let Some(s)=el.attribute("style"){
            for field in s.split(';'){
                if let Some((key,value))=field.split_once(':'){
                    if key.trim()==name{return Some(value.trim().to_string());}
                }
            }
        }
        if let Some(s)=el.attribute(name){return Some(s.into());}
    }
    None
}
fn style_for(n:Node<'_,'_>,view:ViewBox)->Result<Style,String>{
    let fill=paint(&style_field(n,"fill").unwrap_or_else(||"#000000".into()))?;
    let stroke=paint(&style_field(n,"stroke").unwrap_or_else(||"none".into()))?;
    let stroke_width=style_field(n,"stroke-width").unwrap_or_else(||"1".into())
        .parse::<f32>().map_err(|_|"Unsupported SVG stroke-width".to_string())?;
    let width=if stroke=="none"{0.0}else{stroke_width*(view.sx.abs()+view.sy.abs())/2.0};
    if !width.is_finite()||width<0.0{return Err("Invalid SVG stroke width".into());}
    Ok(Style{fill,stroke,stroke_width_mm:width})
}
fn line_segment(a:Point,b:Point)->CubicSegment{
    CubicSegment{
        control1:Point{x:a.x+(b.x-a.x)/3.0,y:a.y+(b.y-a.y)/3.0},
        control2:Point{x:a.x+2.0*(b.x-a.x)/3.0,y:a.y+2.0*(b.y-a.y)/3.0},
        end:b
    }
}
fn quad_segment(a:Point,c:Point,b:Point)->CubicSegment{
    CubicSegment{
        control1:Point{x:a.x+(c.x-a.x)*2.0/3.0,y:a.y+(c.y-a.y)*2.0/3.0},
        control2:Point{x:b.x+(c.x-b.x)*2.0/3.0,y:b.y+(c.y-b.y)*2.0/3.0},
        end:b
    }
}
fn rel(a:Point,x:f64,y:f64,absolute:bool)->Point{
    let p=point(x,y);
    if absolute {p}else{Point{x:a.x+p.x,y:a.y+p.y}}
}
fn reflect(anchor:Point,control:Option<Point>)->Point{
    match control{
        Some(p)=>Point{x:2.0*anchor.x-p.x,y:2.0*anchor.y-p.y},
        None=>anchor
    }
}
fn flush_path(start:Option<Point>,parts:&mut Vec<CubicSegment>,closed:bool,
    view:ViewBox,result:&mut Vec<Shape>){
    if let Some(start)=start{
        if !parts.is_empty(){
            let segments=parts.drain(..).map(|seg|CubicSegment{
                control1:view.point(seg.control1),control2:view.point(seg.control2),
                end:view.point(seg.end)
            }).collect();
            result.push(Shape::Bezier{start:view.point(start),segments,closed});
        }
    }
    parts.clear();
}
fn parse_path(data:&str,view:ViewBox)->Result<Vec<Shape>,String>{
    let mut shapes=Vec::new();
    let mut start:Option<Point>=None;
    let mut current=Point{x:0.0,y:0.0};
    let mut parts=Vec::new();
    let (mut prev_cubic,mut prev_quad)=(None,None);
    let mut closed=false;
    for cmd in PathParser::from(data){
        let cmd=cmd.map_err(|e|format!("SVG path error: {e}"))?;
        match cmd{
            PathSegment::MoveTo{abs,x,y}=>{
                flush_path(start,&mut parts,closed,view,&mut shapes);
                current=rel(current,x,y,abs);start=Some(current);closed=false;
                prev_cubic=None;prev_quad=None;
            }
            PathSegment::LineTo{abs,x,y}=>{
                let end=rel(current,x,y,abs);
                parts.push(line_segment(current,end));current=end;
                prev_cubic=None;prev_quad=None;
            }
            PathSegment::HorizontalLineTo{abs,x}=>{
                let end=Point{x:if abs{x as f32}else{current.x+x as f32},y:current.y};
                parts.push(line_segment(current,end));current=end;
                prev_cubic=None;prev_quad=None;
            }
            PathSegment::VerticalLineTo{abs,y}=>{
                let end=Point{x:current.x,y:if abs{y as f32}else{current.y+y as f32}};
                parts.push(line_segment(current,end));current=end;
                prev_cubic=None;prev_quad=None;
            }
            PathSegment::CurveTo{abs,x1,y1,x2,y2,x,y}=>{
                let c1=rel(current,x1,y1,abs);let c2=rel(current,x2,y2,abs);
                let end=rel(current,x,y,abs);
                parts.push(CubicSegment{control1:c1,control2:c2,end});current=end;
                prev_cubic=Some(c2);prev_quad=None;
            }
            PathSegment::SmoothCurveTo{abs,x2,y2,x,y}=>{
                let c1=reflect(current,prev_cubic);
                let c2=rel(current,x2,y2,abs);
                let end=rel(current,x,y,abs);
                parts.push(CubicSegment{control1:c1,control2:c2,end});current=end;
                prev_cubic=Some(c2);prev_quad=None;
            }
            PathSegment::Quadratic{abs,x1,y1,x,y}=>{
                let control=rel(current,x1,y1,abs);
                let end=rel(current,x,y,abs);
                parts.push(quad_segment(current,control,end));current=end;
                prev_quad=Some(control);prev_cubic=None;
            }
            PathSegment::SmoothQuadratic{abs,x,y}=>{
                let control=reflect(current,prev_quad);
                let end=rel(current,x,y,abs);
                parts.push(quad_segment(current,control,end));current=end;
                prev_quad=Some(control);prev_cubic=None;
            }
            PathSegment::EllipticalArc{..}=>{
                return Err("SVG elliptical arcs are not supported yet".into());
            }
            PathSegment::ClosePath{..}=>{
                closed=true;
                if let Some(p)=start{current=p;}
                prev_cubic=None;prev_quad=None;
            }
        }
        if parts.len()>10000{return Err("SVG path exceeds 10000 segments".into());}
    }
    flush_path(start,&mut parts,closed,view,&mut shapes);
    Ok(shapes)
}
pub fn parse_svg(xml:&str)->Result<Document,String>{
    if xml.len()>32*1024*1024{return Err("SVG file exceeds 32MB".into());}
    let parsed=roxmltree::Document::parse_with_options(xml,ParsingOptions{
        allow_dtd:false,nodes_limit:300000,..Default::default()
    }).map_err(|e|format!("SVG XML error: {e}"))?;
    let root=parsed.root_element();
    if root.tag_name().name()!="svg"{return Err("Not an SVG document".into());}
    let width=svg_dimension(root.attribute("width").ok_or("SVG width is required")?)?;
    let height=svg_dimension(root.attribute("height").ok_or("SVG height is required")?)?;
    let vb=match root.attribute("viewBox"){
        Some(raw)=>{
            let nums:Vec<f32>=raw.split(|c:char|c==','||c.is_ascii_whitespace())
                .filter(|x|!x.is_empty())
                .map(str::parse::<f32>).collect::<Result<_,_>>()
                .map_err(|_|"Invalid SVG viewBox")?;
            if nums.len()!=4||nums[2]<=0.0||nums[3]<=0.0||
                !nums.iter().all(|v|v.is_finite()){return Err("Invalid SVG viewBox".into());}
            ViewBox{x:nums[0],y:nums[1],sx:width/nums[2],sy:height/nums[3]}
        },
        None=>ViewBox{x:0.0,y:0.0,sx:25.4/96.0,sy:25.4/96.0},
    };
    let mut doc=Document::default();
    doc.name="Imported SVG".into();
    doc.page.width_mm=width;doc.page.height_mm=height;
    for node in root.descendants().filter(|n|n.is_element()){
        if node.attribute("transform").is_some()
            || node.attribute("clip-path").is_some()
            || node.attribute("mask").is_some()
            || style_field(node,"opacity").is_some()
            || style_field(node,"fill-opacity").is_some()
            || style_field(node,"stroke-opacity").is_some()
            || style_field(node,"filter").is_some(){
            return Err(format!("Unsupported SVG effect on <{}>",node.tag_name().name()));
        }
        let name=node.tag_name().name();
        let styles=match name {
            "rect"|"ellipse"|"circle"|"line"|"polyline"|"polygon"|"text"|"path"=>Some(style_for(node,vb)?),
            _=>None
        };
        let mut shapes=Vec::new();
        match name {
            "svg"|"g"|"title"|"desc"|"metadata"=>{},
            "rect"=>{
                if node.attribute("rx").is_some() || node.attribute("ry").is_some(){
                    return Err("SVG rounded rectangles are not supported yet".into());
                }
                let x=number_or(node,"x",0.0)?;let y=number_or(node,"y",0.0)?;
                let w=required_number(node,"width")?;let h=required_number(node,"height")?;
                if w<0.0||h<0.0{return Err("Negative rectangle dimensions".into());}
                let p=vb.point(Point{x,y});
                shapes.push(Shape::Rectangle{x:p.x,y:p.y,w:w*vb.sx,h:h*vb.sy});
            }
            "ellipse"|"circle"=>{
                let cx=required_number(node,"cx")?;let cy=required_number(node,"cy")?;
                let rx=if name=="circle"{required_number(node,"r")?}else{required_number(node,"rx")?};
                let ry=if name=="circle"{rx}else{required_number(node,"ry")?};
                if rx<0.0||ry<0.0{return Err("Negative ellipse radius".into());}
                let p=vb.point(Point{x:cx-rx,y:cy-ry});
                shapes.push(Shape::Ellipse{x:p.x,y:p.y,w:rx*2.0*vb.sx,h:ry*2.0*vb.sy});
            }
            "line"=>{
                let p1=vb.point(Point{x:required_number(node,"x1")?,y:required_number(node,"y1")?});
                let p2=vb.point(Point{x:required_number(node,"x2")?,y:required_number(node,"y2")?});
                shapes.push(Shape::Polyline{points:vec![p1,p2],closed:false});
            }
            "polyline"|"polygon"=>{
                let points:Vec<Point>=PointsParser::from(node.attribute("points").ok_or("Missing SVG points")?)
                    .map(|(x,y)|vb.point(point(x,y))).collect();
                if points.len()<2{return Err("SVG polyline needs two points".into());}
                shapes.push(Shape::Polyline{points,closed:name=="polygon"});
            }
            "text"=>{
                if node.children().any(|c|c.is_element()){return Err("SVG tspan is not supported".into());}
                let size=style_field(node,"font-size").unwrap_or_else(||"12".into())
                    .parse::<f32>().map_err(|_|"Unsupported SVG font size".to_string())?;
                let x=number_or(node,"x",0.0)?;let y=number_or(node,"y",0.0)?;
                let p=vb.point(Point{x,y});
                shapes.push(Shape::Text{x:p.x,y:p.y-size*vb.sy,
                    value:node.text().unwrap_or("").to_string(),size_mm:size*vb.sy});
            }
            "path"=>{
                if styles.as_ref().map(|s|s.fill.as_str())!=Some("none"){
                    return Err("Filled SVG paths require a geometry fill renderer (not implemented)".into());
                }
                shapes=parse_path(node.attribute("d").ok_or("SVG path d missing")?,vb)?;
            }
            other=>return Err(format!("Unsupported SVG element: <{other}>")),
        }
        for shape in shapes {doc.add(shape,styles.clone().unwrap_or_default());}
    }
    doc.validate()?;
    Ok(doc)
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn import_shapes_from_export(){
        let mut source=Document::default();
        source.add(Shape::Rectangle{x:10.0,y:10.0,w:20.0,h:30.0},Style::default());
        source.add(Shape::Ellipse{x:40.0,y:40.0,w:20.0,h:20.0},Style::default());
        let doc=parse_svg(&source.to_svg()).unwrap();
        assert_eq!(doc.objects.len(),2);
        doc.validate().unwrap();
    }
    #[test] fn bezier_path_roundtrip(){
        let svg=r##"<svg width="100mm" height="100mm" viewBox="0 0 100 100"><path fill="none" stroke="#000000" d="M 10 10 C 20 20 30 20 40 10 L 50 50 Z"/></svg>"##;
        let doc=parse_svg(svg).unwrap();
        assert_eq!(doc.objects.len(),1);
        assert!(doc.to_svg().contains("C 20 20"));
    }
    #[test] fn reject_dtd_and_unsupported_arc(){
        assert!(parse_svg(r#"<!DOCTYPE svg [<!ENTITY x "hi">]><svg width="100mm" height="100mm">&x;</svg>"#).is_err());
        assert!(parse_svg(r#"<svg width="100mm" height="100mm"><path d="M0 0 A10 10 0 0 1 20 20"/></svg>"#).is_err());
    }
    #[test] fn map_px_viewbox_to_mm(){
        let doc=parse_svg(r#"<svg width="96px" height="96px" viewBox="0 0 96 96"><rect x="0" y="0" width="96" height="96"/></svg>"#).unwrap();
        assert!((doc.page.width_mm-25.4).abs()<0.001);
        assert!((doc.objects[0].shape.bounds().w-25.4).abs()<0.001);
    }
}