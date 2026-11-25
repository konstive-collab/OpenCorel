//! Bounded raster-to-vector contour extraction. All output is native editable
//! cubic paths (straight cubic segments), with even-odd holes. No external
//! executable, AI service or embedded bitmap in the generated artwork.
use crate::{Bounds,Contour,CubicSegment,Document,Point,Shape,Style};
use image::{ImageReader,RgbaImage,imageops};
use base64::{Engine,engine::general_purpose::STANDARD};
use std::{collections::HashMap,io::Cursor};

const MAX_SOURCE_PIXELS:u64=40_000_000;
const MAX_EDGES:usize=300_000;
const MAX_NODES:usize=25_000;
const MAX_RINGS:usize=4_000;

#[derive(Clone,Copy,Debug,PartialEq,Eq,serde::Serialize,serde::Deserialize)]
#[serde(rename_all="snake_case")]
pub enum TraceMode{Dark,Light,Color}
#[derive(Clone,Copy,Debug,PartialEq,Eq,serde::Serialize,serde::Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct TraceSettings{
    pub mode:TraceMode,
    pub threshold:u8,
    pub alpha_min:u8,
    pub colors:u8,
    pub max_side:u16,
    pub min_area:u16,
    pub skip_white:bool,
    /// Optional corner rounding in percent of half the shortest incident edge.
    /// Disabled for palette tracing to prevent gaps between adjacent colors.
    pub round_corners:u8,
}
impl Default for TraceSettings{
    fn default()->Self{Self{mode:TraceMode::Dark,threshold:128,alpha_min:128,
        colors:6,max_side:512,min_area:3,skip_white:true,round_corners:0}}
}
impl TraceSettings{
    pub fn validate(self)->Result<(),String>{
        if !(2..=16).contains(&self.colors)||!(32..=1024).contains(&self.max_side)
            ||self.alpha_min==0||self.min_area>4096||self.round_corners>90{
            return Err("Invalid trace settings".into());
        }
        Ok(())
    }
}
#[derive(Clone,Debug,PartialEq)]
pub struct TracePart{pub shape:Shape,pub style:Style}
#[derive(Clone,Debug,PartialEq)]
pub struct TraceResult{
    pub parts:Vec<TracePart>,
    pub width:u32,pub height:u32,pub contours:usize,pub nodes:usize,
}
fn bucket(c:[u8;4])->usize{
    ((c[0] as usize>>4)<<8)|((c[1] as usize>>4)<<4)|(c[2] as usize>>4)
}
fn white(c:[u8;4])->bool{c[0]>=245&&c[1]>=245&&c[2]>=245}
fn eligible(c:[u8;4],s:TraceSettings)->bool{c[3]>=s.alpha_min&&(s.mode==TraceMode::Light||!s.skip_white||!white(c))}
fn classify(img:&RgbaImage,s:TraceSettings)->(Vec<u8>,Vec<[u8;3]>){
    if s.mode!=TraceMode::Color{
        let labels=img.pixels().map(|p|{
            let c=p.0;
            let y=(54*u32::from(c[0])+183*u32::from(c[1])+19*u32::from(c[2]))/256;
            let foreground=match s.mode{TraceMode::Dark=>y<=u32::from(s.threshold),
                TraceMode::Light=>y>=u32::from(s.threshold),TraceMode::Color=>false};
            u8::from(eligible(c,s)&&foreground)
        }).collect();
        let color=if s.mode==TraceMode::Light{[255;3]}else{[0;3]};
        return (labels,vec![color]);
    }
    // Deterministic histogram of 4-bit RGB buckets, represented by actual
    // mean source colors, then nearest palette assignment for all pixels.
    let mut histogram=vec![[0u64;4];4096];
    for pixel in img.pixels(){
        let c=pixel.0;
        if !eligible(c,s){continue;}
        let h=&mut histogram[bucket(c)];
        h[0]+=1;for i in 0..3{h[i+1]+=u64::from(c[i]);}
    }
    let mut ranked:Vec<_>=histogram.iter().enumerate().filter(|(_,h)|h[0]>0).collect();
    ranked.sort_unstable_by(|(ia,a),(ib,b)|b[0].cmp(&a[0]).then(ia.cmp(ib)));
    let mut palette=Vec::<[u8;3]>::new();
    for (_,h) in ranked{
        let color=std::array::from_fn(|i|(h[i+1]/h[0]) as u8);
        if palette.iter().all(|p|distance(*p,color)>=24*24){
            palette.push(color);
            if palette.len()==usize::from(s.colors){break;}
        }
    }
    let labels=img.pixels().map(|p|{
        let c=p.0;
        if !eligible(c,s){return 0;}
        palette.iter().enumerate().min_by_key(|(_,p)|distance(**p,[c[0],c[1],c[2]]))
            .map_or(0,|(i,_)|i as u8+1)
    }).collect();
    (labels,palette)
}
fn distance(a:[u8;3],b:[u8;3])->i32{
    a.into_iter().zip(b).map(|(a,b)|(i32::from(a)-i32::from(b)).pow(2)).sum()
}
/// Remove whole 4-connected foreground components, not individual rings:
/// dropping an outer ring while retaining its hole would create new artwork.
fn remove_small_islands(labels:&mut [u8],w:u32,h:u32,min_area:u16){
    if min_area<=1{return;}
    let width=w as usize;let height=h as usize;
    let mut seen=vec![false;labels.len()];
    let mut queue=Vec::new();
    for start in 0..labels.len(){
        if seen[start]||labels[start]==0{continue;}
        let label=labels[start];queue.clear();queue.push(start);seen[start]=true;
        let mut cursor=0;
        while cursor<queue.len(){
            let p=queue[cursor];cursor+=1;
            let x=p%width;let y=p/width;
            let neighbors=[(x>0).then(||p-1),(x+1<width).then(||p+1),
                (y>0).then(||p-width),(y+1<height).then(||p+width)];
            for n in neighbors.into_iter().flatten(){
                if !seen[n]&&labels[n]==label{seen[n]=true;queue.push(n);}
            }
        }
        if queue.len()<usize::from(min_area){for &p in &queue{labels[p]=0;}}
    }
}
type Vertex=(u32,u32);
#[derive(Clone,Copy)]
struct Edge{start:Vertex,end:Vertex,dir:u8}
fn edges_for(labels:&[u8],w:u32,h:u32,label:u8,budget:&mut usize)->Result<Vec<Edge>,String>{
    let at=|x:u32,y:u32|labels[(y*w+x) as usize];
    let mut edges=Vec::new();
    let mut add=|start,end,dir|->Result<(),String>{
        if *budget>=MAX_EDGES{return Err("Trace boundary budget exceeded; lower resolution or use fewer colors".into());}
        *budget+=1;edges.push(Edge{start,end,dir});Ok(())
    };
    for y in 0..h{for x in 0..w{
        if at(x,y)!=label{continue;}
        // Filled pixel lies on the right side of each directed edge.
        if y==0||at(x,y-1)!=label{add((x,y),(x+1,y),0)?;}
        if x+1==w||at(x+1,y)!=label{add((x+1,y),(x+1,y+1),1)?;}
        if y+1==h||at(x,y+1)!=label{add((x+1,y+1),(x,y+1),2)?;}
        if x==0||at(x-1,y)!=label{add((x,y+1),(x,y),3)?;}
    }}
    Ok(edges)
}
fn turn_rank(a:u8,b:u8)->u8{match (b+4-a)%4{1=>0,0=>1,3=>2,_=>3}}
fn rings(edges:&[Edge])->Result<Vec<Vec<Vertex>>,String>{
    let mut outgoing:HashMap<Vertex,[Option<usize>;4]>=HashMap::new();
    for (i,e) in edges.iter().enumerate(){
        let slots=outgoing.entry(e.start).or_default();
        if slots[e.dir as usize].replace(i).is_some(){return Err("Duplicate trace boundary".into());}
    }
    let mut used=vec![false;edges.len()];
    let mut result=Vec::new();
    for first in 0..edges.len(){
        if used[first]{continue;}
        let mut current=first;
        let mut vertices=Vec::new();
        loop{
            if used[current]{return Err("Open or ambiguous trace boundary".into());}
            used[current]=true;
            let e=edges[current];vertices.push(e.start);
            // At diagonal contacts, prefer the right turn, preserving separate
            // 4-connected islands instead of forming a self-crossing ring.
            let next=outgoing.get(&e.end).and_then(|slots|
                slots.iter().flatten().copied()
                    .min_by_key(|i|turn_rank(e.dir,edges[*i].dir)))
                .ok_or("Unclosed trace boundary")?;
            if next==first{break;}
            current=next;
        }
        let mut simplified=Vec::new();
        for i in 0..vertices.len(){
            let a=vertices[(i+vertices.len()-1)%vertices.len()];
            let b=vertices[i];let c=vertices[(i+1)%vertices.len()];
            if (a.0==b.0&&b.0==c.0)||(a.1==b.1&&b.1==c.1){continue;}
            simplified.push(b);
        }
        if simplified.len()>=3{
            result.push(simplified);
            if result.len()>MAX_RINGS{return Err("Too many trace contours; reduce resolution".into());}
        }
    }
    Ok(result)
}
fn line(a:Point,b:Point)->CubicSegment{
    CubicSegment{control1:Point{x:a.x+(b.x-a.x)/3.,y:a.y+(b.y-a.y)/3.},
        control2:Point{x:a.x+(b.x-a.x)*2./3.,y:a.y+(b.y-a.y)*2./3.},end:b}
}
fn lerp(a:Point,b:Point,t:f32)->Point{Point{x:a.x+(b.x-a.x)*t,y:a.y+(b.y-a.y)*t}}
fn rounded_ring(points:&[Point],amount:u8)->Contour{
    let fraction=f32::from(amount)/200.;
    let n=points.len();
    let corners:Vec<_>=(0..n).map(|i|{
        let previous=points[(i+n-1)%n];let current=points[i];let next=points[(i+1)%n];
        let r=(current.x-previous.x).hypot(current.y-previous.y)
            .min((next.x-current.x).hypot(next.y-current.y))*fraction;
        let a=r/(current.x-previous.x).hypot(current.y-previous.y).max(1e-9);
        let b=r/(next.x-current.x).hypot(next.y-current.y).max(1e-9);
        (lerp(current,previous,a),current,lerp(current,next,b))
    }).collect();
    let start=corners[0].0;
    let mut segments=Vec::with_capacity(n*2);
    for i in 0..n{
        let (a,b,c)=corners[i];
        // Exact quadratic-to-cubic conversion through the original corner.
        segments.push(CubicSegment{control1:lerp(a,b,2./3.),control2:lerp(c,b,2./3.),end:c});
        segments.push(line(c,corners[(i+1)%n].0));
    }
    Contour{start,segments,closed:true}
}
pub fn trace_pixels(image:&RgbaImage,frame:Bounds,s:TraceSettings)->Result<TraceResult,String>{
    s.validate()?;
    if image.width()==0||image.height()==0||u64::from(image.width())*u64::from(image.height())>MAX_SOURCE_PIXELS{
        return Err("Invalid source dimensions".into());
    }
    if ![frame.x,frame.y,frame.w,frame.h,frame.x+frame.w,frame.y+frame.h]
        .iter().all(|v|v.is_finite()&&v.abs()<=100_000.)||frame.w<=0.||frame.h<=0.{
        return Err("Invalid trace frame".into());
    }
    let side=u32::from(s.max_side);
    let scaled;
    let img=if image.width().max(image.height())>side{
        let factor=side as f64/image.width().max(image.height()) as f64;
        scaled=imageops::resize(image,(image.width() as f64*factor).round().max(1.) as u32,
            (image.height() as f64*factor).round().max(1.) as u32,imageops::FilterType::Triangle);
        &scaled
    }else{image};
    let (w,h)=img.dimensions();
    let (mut labels,palette)=classify(img,s);
    remove_small_islands(&mut labels,w,h,s.min_area);
    let mut result=TraceResult{parts:Vec::new(),width:w,height:h,contours:0,nodes:0};
    let mut budget=0usize;
    let point=|(x,y):Vertex|Point{x:frame.x+frame.w*x as f32/w as f32,
        y:frame.y+frame.h*y as f32/h as f32};
    for (index,color) in palette.iter().enumerate(){
        let edges=edges_for(&labels,w,h,index as u8+1,&mut budget)?;
        let loops=rings(&edges)?;
        let mut contours=Vec::new();
        for ring in loops{
            let rounded=s.mode!=TraceMode::Color&&s.round_corners>0;
            result.nodes+=ring.len()*if rounded{2}else{1};result.contours+=1;
            if result.nodes>MAX_NODES||result.contours>MAX_RINGS{
                return Err("Trace exceeds vector complexity budget; lower resolution or increase minimum area".into());
            }
            if rounded{
                let points:Vec<_>=ring.into_iter().map(point).collect();
                contours.push(rounded_ring(&points,s.round_corners));
            }else{
                let start=point(ring[0]);
                let segments=(0..ring.len()).map(|i|line(point(ring[i]),point(ring[(i+1)%ring.len()]))).collect();
                contours.push(Contour{start,segments,closed:true});
            }
        }
        if !contours.is_empty(){
            result.parts.push(TracePart{shape:Shape::CompoundPath{contours},
                style:Style{fill:format!("#{:02X}{:02X}{:02X}",color[0],color[1],color[2]),
                    stroke:"none".into(),stroke_width_mm:0.}});
        }
    }
    if result.parts.is_empty(){return Err("No foreground contours found; adjust threshold, color or area".into());}
    Ok(result)
}
pub fn trace_shape(shape:&Shape,s:TraceSettings)->Result<TraceResult,String>{
    s.validate()?;
    let Shape::Raster{data_base64,pixel_width,pixel_height,mime,..}=shape
        else{return Err("Select raster artwork".into());};
    if data_base64.len()>56*1024*1024{return Err("Raster data too large".into());}
    let bytes=STANDARD.decode(data_base64).map_err(|_|"Invalid raster data")?;
    if bytes.len()>40*1024*1024{return Err("Raster data too large".into());}
    let format=match mime.as_str(){"image/png"=>image::ImageFormat::Png,
        "image/jpeg"=>image::ImageFormat::Jpeg,_=>return Err("Unsupported raster MIME".into())};
    if image::guess_format(&bytes).map_err(|e|e.to_string())?!=format{return Err("Raster MIME mismatch".into());}
    let dims=ImageReader::with_format(Cursor::new(&bytes),format).into_dimensions().map_err(|e|e.to_string())?;
    if dims!=(*pixel_width,*pixel_height)||dims.0==0||dims.1==0||dims.0>16000||dims.1>16000
        ||u64::from(dims.0)*u64::from(dims.1)>MAX_SOURCE_PIXELS{
        return Err("Invalid source raster dimensions".into());
    }
    let image=image::load_from_memory(&bytes).map_err(|e|e.to_string())?.to_rgba8();
    trace_pixels(&image,shape.bounds(),s)
}
impl Document{
    /// Trace into new editable objects immediately above their source.
    /// The source is retained by default; replacing it is explicit. Clipped
    /// and linked/grouped objects must first be detached to avoid lost links.
    pub fn trace_raster(&mut self,id:u64,s:TraceSettings,replace:bool)->Result<(Vec<u64>,TraceResult),String>{
        let source=self.get(id).ok_or("Source not found")?;
        let layer=source.layer_id;
        if source.group_id.is_some()||!self.layer(layer).is_some_and(|l|l.visible&&!l.locked)
            ||self.clip_links.iter().any(|l|l.source_id==id||l.target_id==id){
            return Err("Trace requires ungrouped, unclipped artwork on an editable layer".into());
        }
        let result=trace_shape(&source.shape,s)?;
        let count=result.parts.len();
        if self.all_object_count().saturating_add(count)>100_000||
            self.next_id.checked_add(count as u64).is_none_or(|n|n==u64::MAX){
            return Err("Document object capacity exceeded".into());
        }
        let mut draft=self.clone();
        let position=draft.objects.iter().position(|o|o.id==id).ok_or("Source not found")?;
        let mut ids=Vec::new();
        for part in &result.parts{
            let new=draft.add_to_layer(part.shape.clone(),part.style.clone(),layer);
            if let Some(o)=draft.get_mut(new){o.name=format!("Trace {}",part.style.fill);}
            ids.push(new);
        }
        let tail=draft.objects.split_off(draft.objects.len()-count);
        draft.objects.splice(position+1..position+1,tail);
        if replace{draft.remove(id);}
        draft.validate()?;
        *self=draft;
        Ok((ids,result))
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use image::Rgba;
    fn frame()->Bounds{Bounds{x:10.,y:20.,w:50.,h:50.}}
    fn black()->Rgba<u8>{Rgba([0,0,0,255])}
    fn white_pixel()->Rgba<u8>{Rgba([255,255,255,255])}
    fn settings()->TraceSettings{TraceSettings{min_area:0,..Default::default()}}
    #[test] fn rectangle_has_four_native_segments(){
        let img=RgbaImage::from_pixel(5,5,black());
        let r=trace_pixels(&img,frame(),settings()).unwrap();
        assert_eq!((r.contours,r.nodes),(1,4));
        assert_eq!(r.parts[0].shape.bounds(),frame());
    }
    #[test] fn rounded_silhouette_contains_real_curved_segments(){
        let img=RgbaImage::from_pixel(5,5,black());
        let r=trace_pixels(&img,frame(),TraceSettings{round_corners:60,..settings()}).unwrap();
        assert_eq!((r.contours,r.nodes),(1,8));
        let Shape::CompoundPath{contours}=&r.parts[0].shape else{panic!()};
        let c=&contours[0];let s=&c.segments[0];
        let cross=(s.control1.x-c.start.x)*(s.end.y-c.start.y)
            -(s.control1.y-c.start.y)*(s.end.x-c.start.x);
        assert!(cross.abs()>0.01);
    }
    #[test] fn ring_preserves_hole_and_diagonal_islands(){
        let img=RgbaImage::from_fn(5,5,|x,y|if x==0||x==4||y==0||y==4{black()}else{white_pixel()});
        let r=trace_pixels(&img,frame(),settings()).unwrap();
        assert_eq!((r.contours,r.nodes),(2,8));
        let img=RgbaImage::from_fn(2,2,|x,y|if x==y{black()}else{white_pixel()});
        let r=trace_pixels(&img,frame(),settings()).unwrap();
        assert_eq!((r.contours,r.nodes),(2,8));
    }
    #[test] fn color_and_alpha_are_respected(){
        let img=RgbaImage::from_fn(3,1,|x,_|match x{0=>Rgba([255,0,0,255]),
            1=>Rgba([0,0,255,255]),_=>Rgba([0,255,0,0])});
        let r=trace_pixels(&img,frame(),TraceSettings{mode:TraceMode::Color,..settings()}).unwrap();
        assert_eq!(r.parts.len(),2);
        assert!(r.parts.iter().any(|p|p.style.fill=="#FF0000"));
        assert!(r.parts.iter().any(|p|p.style.fill=="#0000FF"));
    }
    #[test] fn removes_small_island_and_rejects_empty(){
        let img=RgbaImage::from_fn(4,4,|x,y|if x==0&&y==0{black()}else{white_pixel()});
        assert!(trace_pixels(&img,frame(),TraceSettings::default()).is_err());
        assert!(trace_pixels(&img,frame(),settings()).is_ok());
    }
    #[test] fn document_trace_is_atomic_and_keeps_source(){
        let img=RgbaImage::from_pixel(4,4,black());
        let mut out=Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut out,image::ImageFormat::Png).unwrap();
        let mut d=Document::default();
        let id=d.add(Shape::Raster{x:10.,y:20.,w:50.,h:50.,pixel_width:4,pixel_height:4,
            mime:"image/png".into(),data_base64:STANDARD.encode(out.into_inner()),edit:None},Style::default());
        d.layers[0].locked=true;
        let old=d.clone();
        assert!(d.trace_raster(id,settings(),false).is_err());assert_eq!(d,old);
        d.layers[0].locked=false;
        let (ids,_)=d.trace_raster(id,settings(),false).unwrap();
        assert_eq!(d.objects.len(),2);assert!(d.get(id).is_some());
        assert!(matches!(d.get(ids[0]).unwrap().shape,Shape::CompoundPath{..}));
        d.validate().unwrap();
        let (ids,_)=d.trace_raster(id,settings(),true).unwrap();
        assert!(d.get(id).is_none());assert!(d.get(ids[0]).is_some());
        d.validate().unwrap();
    }
}
