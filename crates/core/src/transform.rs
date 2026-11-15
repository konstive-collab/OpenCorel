//! Safe, millimetre-accurate transformations for editable shapes.
//! Unsupported transformations return errors rather than corrupting artwork.
use crate::{Bounds,Contour,Document,Object,Point,Shape};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum QuarterTurn { Clockwise, CounterClockwise }

pub fn resizable(shape:&Shape)->bool{
    !matches!(shape,Shape::Text{..})
}
pub fn resize_shape(shape:&mut Shape,target:Bounds)->Result<(),String>{
    if !resizable(shape){return Err("Размер текста изменяйте через кегль".into());}
    if ![target.x,target.y,target.w,target.h].iter().all(|v|v.is_finite()) ||
        target.w<0.1 || target.h<0.1 || target.w>100_000.0 || target.h>100_000.0 {
        return Err("Размер должен быть больше 0.1 мм и не превышать 100 м".into());
    }
    match shape{
        Shape::Rectangle{x,y,w,h}|Shape::Ellipse{x,y,w,h}|
        Shape::EmbeddedSvg{x,y,w,h,..}|Shape::Raster{x,y,w,h,..}=>{
            *x=target.x;*y=target.y;*w=target.w;*h=target.h;
        }
        Shape::Polyline{points,..}=>{
            let old=shape_bounds_polyline(points);
            let (sx,sy)=scales(old,target)?;
            for pt in points{map_resize(pt,old,target,sx,sy);}
        }
        Shape::Bezier{start,segments,..}=>{
            let old=Shape::Bezier{start:*start,segments:segments.clone(),closed:false}.bounds();
            let (sx,sy)=scales(old,target)?;
            map_resize(start,old,target,sx,sy);
            for seg in segments {
                for pt in [&mut seg.control1,&mut seg.control2,&mut seg.end]{
                    map_resize(pt,old,target,sx,sy);
                }
            }
        }
        Shape::CompoundPath{contours}=>{
            let old=compound_bounds(contours);
            let (sx,sy)=scales(old,target)?;
            for contour in contours{
                map_resize(&mut contour.start,old,target,sx,sy);
                for seg in &mut contour.segments{
                    for p in [&mut seg.control1,&mut seg.control2,&mut seg.end]{
                        map_resize(p,old,target,sx,sy);
                    }
                }
            }
        }
        Shape::Text{..}=>unreachable!(),
    }
    Ok(())
}
fn scales(old:Bounds,target:Bounds)->Result<(f32,f32),String>{
    if old.w.abs()<1e-5 || old.h.abs()<1e-5{
        return Err("Нельзя масштабировать вырожденный контур".into());
    }
    Ok((target.w/old.w,target.h/old.h))
}
fn map_resize(p:&mut Point,old:Bounds,target:Bounds,sx:f32,sy:f32){
    p.x=target.x+(p.x-old.x)*sx;
    p.y=target.y+(p.y-old.y)*sy;
}
fn shape_bounds_polyline(points:&[Point])->Bounds{
    let mut min=Point{x:f32::INFINITY,y:f32::INFINITY};
    let mut max=Point{x:f32::NEG_INFINITY,y:f32::NEG_INFINITY};
    for p in points {min.x=min.x.min(p.x);min.y=min.y.min(p.y);
        max.x=max.x.max(p.x);max.y=max.y.max(p.y);}
    Bounds{x:min.x,y:min.y,w:max.x-min.x,h:max.y-min.y}
}
fn compound_bounds(contours:&[Contour])->Bounds{
    let mut min=Point{x:f32::INFINITY,y:f32::INFINITY};
    let mut max=Point{x:f32::NEG_INFINITY,y:f32::NEG_INFINITY};
    for c in contours{
        for p in std::iter::once(&c.start).chain(c.segments.iter().flat_map(|s|
            [&s.control1,&s.control2,&s.end])){
            min.x=min.x.min(p.x);min.y=min.y.min(p.y);
            max.x=max.x.max(p.x);max.y=max.y.max(p.y);
        }
    }
    Bounds{x:min.x,y:min.y,w:max.x-min.x,h:max.y-min.y}
}
fn for_all_points<F:FnMut(&mut Point)>(shape:&mut Shape,mut modify:F){
    match shape{
        Shape::Polyline{points,..}=>for p in points{modify(p);},
        Shape::Bezier{start,segments,..}=>{
            modify(start);
            for s in segments{for p in [&mut s.control1,&mut s.control2,&mut s.end]{modify(p);}}
        }
        Shape::CompoundPath{contours}=>for c in contours{
            modify(&mut c.start);
            for s in &mut c.segments{
                for p in [&mut s.control1,&mut s.control2,&mut s.end]{modify(p);}
            }
        },
        _=>{}
    }
}
pub fn rotate_90(shape:&mut Shape,center:Point,turn:QuarterTurn)->Result<(),String>{
    if matches!(shape,Shape::Text{..}|Shape::Raster{..}|Shape::EmbeddedSvg{..}){
        return Err("Поворот этой фигуры пока не поддерживается; текст сначала преобразуйте в кривые".into());
    }
    if !center.x.is_finite()||!center.y.is_finite(){
        return Err("Не удалось определить центр поворота".into());
    }
    let rotate=|p:&mut Point|{
        let (dx,dy)=(p.x-center.x,p.y-center.y);
        match turn {
            QuarterTurn::Clockwise=>{p.x=center.x-dy;p.y=center.y+dx;},
            QuarterTurn::CounterClockwise=>{p.x=center.x+dy;p.y=center.y-dx;},
        }
    };
    match shape{
        Shape::Rectangle{x,y,w,h}|Shape::Ellipse{x,y,w,h}=>{
            let mut a=Point{x:*x,y:*y};let mut b=Point{x:*x+*w,y:*y+*h};
            rotate(&mut a);rotate(&mut b);
            *x=a.x.min(b.x);*y=a.y.min(b.y);
            *w=(a.x-b.x).abs();*h=(a.y-b.y).abs();
        },
        _=>for_all_points(shape,rotate),
    }
    Ok(())
}
pub fn flip_shape(shape:&mut Shape,center:Point,horizontal:bool)->Result<(),String>{
    if matches!(shape,Shape::Text{..}|Shape::Raster{..}|Shape::EmbeddedSvg{..}){
        return Err("Зеркальное отражение доступно для редактируемой геометрии".into());
    }
    if !center.x.is_finite()||!center.y.is_finite(){
        return Err("Не удалось определить центр зеркального отражения".into());
    }
    let reflect=|p:&mut Point|{
        if horizontal {p.x=2.0*center.x-p.x;}
        else {p.y=2.0*center.y-p.y;}
    };
    match shape{
        Shape::Rectangle{x,y,w,h}|Shape::Ellipse{x,y,w,h}=>{
            if horizontal{*x=2.0*center.x-*x-*w;}
            else{*y=2.0*center.y-*y-*h;}
        }
        _=>for_all_points(shape,reflect),
    }
    Ok(())
}
impl Document{
    /// Copies an object into the destination layer, retaining its text format.
    /// Every pasted object receives a fresh ID; the source remains unchanged.
    pub fn copy_object(&mut self,original:&Object,layer_id:u64,dx:f32,dy:f32)->u64{
        let mut shape=original.shape.clone();
        shape.translate(dx,dy);
        let id=self.add_to_layer(shape,original.style.clone(),layer_id);
        self.get_mut(id).unwrap().text_format=original.text_format.clone();
        id
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{CubicSegment,Style,TextFormat};
    #[test] fn resize_rect_is_exact(){
        let mut r=Shape::Rectangle{x:10.,y:20.,w:30.,h:40.};
        resize_shape(&mut r,Bounds{x:5.,y:6.,w:60.,h:80.}).unwrap();
        assert_eq!(r.bounds(),Bounds{x:5.,y:6.,w:60.,h:80.});
    }
    #[test] fn rotate_rectangle_twice_swaps_dimensions(){
        let mut r=Shape::Rectangle{x:0.,y:0.,w:60.,h:20.};
        rotate_90(&mut r,Point{x:30.,y:10.},QuarterTurn::Clockwise).unwrap();
        assert_eq!(r.bounds(),Bounds{x:20.,y:-20.,w:20.,h:60.});
        rotate_90(&mut r,Point{x:30.,y:10.},QuarterTurn::CounterClockwise).unwrap();
        assert_eq!(r.bounds(),Bounds{x:0.,y:0.,w:60.,h:20.});
    }
    #[test] fn bezier_flip_preserves_all_nodes(){
        let mut path=Shape::Bezier{start:Point{x:1.,y:0.},
            segments:vec![CubicSegment{
                control1:Point{x:2.,y:0.},
                control2:Point{x:3.,y:1.},
                end:Point{x:4.,y:0.}}],closed:false};
        flip_shape(&mut path,Point{x:2.5,y:0.},true).unwrap();
        if let Shape::Bezier{start,segments,..}=path{
            assert_eq!(start.x,4.0);
            assert_eq!(segments[0].end.x,1.0);
        }else{panic!("expected Bezier");}
    }
    #[test] fn cannot_silently_resize_text_or_rotate_photo(){
        let mut t=Shape::Text{x:0.,y:0.,value:"Hello".into(),size_mm:12.};
        assert!(resize_shape(&mut t,Bounds{x:0.,y:0.,w:20.,h:30.}).is_err());
        let mut image=Shape::Raster{x:0.,y:0.,w:20.,h:20.,
            pixel_width:1,pixel_height:1,mime:"image/png".into(),data_base64:"".into()};
        assert!(rotate_90(&mut image,Point::default(),QuarterTurn::Clockwise).is_err());
    }
    #[test] fn copy_retains_fonts_and_style(){
        let mut doc=Document::default();
        let id=doc.add(Shape::Text{x:5.,y:6.,value:"Коди".into(),size_mm:12.},Style::default());
        doc.get_mut(id).unwrap().text_format=TextFormat{family:"Arial".into(),weight:700,..TextFormat::default()};
        let from=doc.objects[0].clone();
        let copied=doc.copy_object(&from,1,10.,10.);
        assert_ne!(id,copied);
        assert_eq!(doc.objects[1].text_format.family,"Arial");
        assert_eq!(doc.objects[1].shape.bounds().x,15.0);
        doc.validate().unwrap();
    }
}