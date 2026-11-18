//! Editable procedural vector primitives. All output is ordinary native geometry;
//! there is no proprietary effect blob or extra file-format dependency.
use crate::{Bounds,Contour,CubicSegment,Point,Shape};
use std::f32::consts::{PI,TAU};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Preset{
    Polygon,Star,Burst,Gear,Flower,Cloud,Spiral,Wave,Zigzag,
    Arrow,DoubleArrow,Chevron,Cross,Diamond,Lightning,Heart,
    SpeechBubble,RoundedRectangle,Ring
}
impl Preset{
    pub const ALL:[Self;19]=[
        Self::Polygon,Self::Star,Self::Burst,Self::Gear,Self::Flower,
        Self::Cloud,Self::Spiral,Self::Wave,Self::Zigzag,Self::Arrow,
        Self::DoubleArrow,Self::Chevron,Self::Cross,Self::Diamond,
        Self::Lightning,Self::Heart,Self::SpeechBubble,
        Self::RoundedRectangle,Self::Ring
    ];
    pub fn label(self)->&'static str{
        match self{
            Self::Polygon=>"Правильный многоугольник",Self::Star=>"Звезда",
            Self::Burst=>"Лучевая вспышка",Self::Gear=>"Шестерня",
            Self::Flower=>"Цветок",Self::Cloud=>"Облако",
            Self::Spiral=>"Спираль",Self::Wave=>"Волна",
            Self::Zigzag=>"Зигзаг",Self::Arrow=>"Стрелка",
            Self::DoubleArrow=>"Двусторонняя стрелка",Self::Chevron=>"Шеврон",
            Self::Cross=>"Крест",Self::Diamond=>"Ромб",
            Self::Lightning=>"Молния",Self::Heart=>"Сердце",
            Self::SpeechBubble=>"Выноска",Self::RoundedRectangle=>"Скруглённый прямоугольник",
            Self::Ring=>"Кольцо с отверстием"
        }
    }
}
#[derive(Clone,Copy,Debug)]
pub struct Parameters{
    pub bounds:Bounds,
    pub detail:usize,
    /// Ratio between 0.10 and 0.90. Controls inner radii, corner radius or hole.
    pub ratio:f32,
    /// Number of turns for an open spiral (1–20).
    pub turns:f32,
}
fn vec_at(b:Bounds,u:f32,v:f32)->Point{
    Point{x:b.x+b.w*u,y:b.y+b.h*v}
}
fn polar(b:Bounds,angle:f32,r:f32)->Point{
    vec_at(b,0.5+0.5*r*angle.cos(),0.5+0.5*r*angle.sin())
}
fn polygon(b:Bounds,normalized:&[(f32,f32)],closed:bool)->Shape{
    Shape::Polyline{
        points:normalized.iter().map(|(u,v)|vec_at(b,*u,*v)).collect(),
        closed
    }
}
fn line(a:Point,b:Point)->CubicSegment{
    CubicSegment{control1:Point{x:a.x+(b.x-a.x)/3.0,y:a.y+(b.y-a.y)/3.0},
        control2:Point{x:a.x+2.0*(b.x-a.x)/3.0,y:a.y+2.0*(b.y-a.y)/3.0},
        end:b}
}
fn ellipse_contour(b:Bounds)->Contour{
    let k=0.552_284_8_f32;
    let (cx,cy)=(b.x+b.w/2.0,b.y+b.h/2.0);
    let (rx,ry)=(b.w/2.0,b.h/2.0);
    let start=Point{x:cx+rx,y:cy};
    Contour{start,closed:true,segments:vec![
        CubicSegment{control1:Point{x:cx+rx,y:cy+ry*k},control2:Point{x:cx+rx*k,y:cy+ry},end:Point{x:cx,y:cy+ry}},
        CubicSegment{control1:Point{x:cx-rx*k,y:cy+ry},control2:Point{x:cx-rx,y:cy+ry*k},end:Point{x:cx-rx,y:cy}},
        CubicSegment{control1:Point{x:cx-rx,y:cy-ry*k},control2:Point{x:cx-rx*k,y:cy-ry},end:Point{x:cx,y:cy-ry}},
        CubicSegment{control1:Point{x:cx+rx*k,y:cy-ry},control2:Point{x:cx+rx,y:cy-ry*k},end:start},
    ]}
}
pub fn generate(preset:Preset,params:Parameters)->Result<Shape,String>{
    let b=params.bounds;
    if ![b.x,b.y,b.w,b.h,params.ratio,params.turns].iter().all(|x|x.is_finite())
        || b.w<=0.1 || b.h<=0.1
        || b.x.abs()>100_000.0 || b.y.abs()>100_000.0
        || (b.x+b.w).abs()>100_000.0 || (b.y+b.h).abs()>100_000.0
        || !(3..=36).contains(&params.detail)
        || !(0.10..=0.90).contains(&params.ratio)
        || !(1.0..=20.0).contains(&params.turns){
        return Err("Invalid procedural shape parameters".into());
    }
    let n=params.detail;
    let r=params.ratio;
    let shape=match preset{
        Preset::Polygon=>Shape::Polyline{points:(0..n)
            .map(|i|polar(b,-PI/2.0+TAU*i as f32/n as f32,1.0)).collect(),closed:true},
        Preset::Star|Preset::Burst=>{
            let count=if preset==Preset::Burst{n*2}else{n};
            let mut points=Vec::with_capacity(count*2);
            for i in 0..count*2{
                let angle=-PI/2.0+TAU*i as f32/(count*2) as f32;
                let radius=if i%2==0{1.0}else if preset==Preset::Burst{r*0.5}else{r};
                points.push(polar(b,angle,radius));
            }
            Shape::Polyline{points,closed:true}
        }
        Preset::Gear=>{
            let mut points=Vec::with_capacity(n*4);
            for i in 0..n*4{
                let angle=-PI/2.0+TAU*i as f32/(n*4) as f32;
                let radius=if i%4==1||i%4==2{1.0}else{r};
                points.push(polar(b,angle,radius));
            }
            Shape::Polyline{points,closed:true}
        }
        Preset::Flower|Preset::Cloud=>{
            let count=(n*12).max(96);
            let points=(0..count).map(|i|{
                let angle=-PI/2.0+TAU*i as f32/count as f32;
                let lobes=if preset==Preset::Cloud{8.0}else{n as f32};
                let radius=if preset==Preset::Cloud{
                    0.77+0.18*(angle*lobes).cos()
                }else{
                    0.58+0.42*(angle*lobes).cos()
                };
                polar(b,angle,radius)
            }).collect();
            Shape::Polyline{points,closed:true}
        }
        Preset::Spiral=>{
            let count=(n*30).min(1000);
            let points=(0..=count).map(|i|{
                let t=i as f32/count as f32;
                polar(b,-PI/2.0+params.turns*TAU*t,t)
            }).collect();
            Shape::Polyline{points,closed:false}
        }
        Preset::Wave=>{
            let count=n*12;
            let points=(0..=count).map(|i|{
                let t=i as f32/count as f32;
                vec_at(b,t,0.5+0.42*(t*params.turns*TAU).sin())
            }).collect();
            Shape::Polyline{points,closed:false}
        }
        Preset::Zigzag=>{
            let points=(0..=n*2).map(|i|{
                vec_at(b,i as f32/(2*n) as f32,if i%2==0{0.16}else{0.84})
            }).collect();
            Shape::Polyline{points,closed:false}
        }
        Preset::Arrow=>polygon(b,&[(0.0,0.3),(0.62,0.3),(0.62,0.0),(1.0,0.5),
            (0.62,1.0),(0.62,0.7),(0.0,0.7)],true),
        Preset::DoubleArrow=>polygon(b,&[(0.0,0.5),(0.34,0.0),(0.34,0.3),
            (0.66,0.3),(0.66,0.0),(1.0,0.5),(0.66,1.0),(0.66,0.7),
            (0.34,0.7),(0.34,1.0)],true),
        Preset::Chevron=>polygon(b,&[(0.0,0.0),(0.56,0.0),(1.0,0.5),
            (0.56,1.0),(0.0,1.0),(0.44,0.5)],true),
        Preset::Cross=>polygon(b,&[(0.33,0.0),(0.67,0.0),(0.67,0.33),
            (1.0,0.33),(1.0,0.67),(0.67,0.67),(0.67,1.0),
            (0.33,1.0),(0.33,0.67),(0.0,0.67),(0.0,0.33),(0.33,0.33)],true),
        Preset::Diamond=>polygon(b,&[(0.5,0.0),(1.0,0.5),(0.5,1.0),(0.0,0.5)],true),
        Preset::Lightning=>polygon(b,&[(0.48,0.0),(0.17,0.55),(0.48,0.55),
            (0.32,1.0),(0.86,0.38),(0.58,0.38),(0.73,0.0)],true),
        Preset::SpeechBubble=>polygon(b,&[(0.08,0.06),(0.92,0.06),(0.96,0.16),
            (0.96,0.72),(0.86,0.79),(0.32,0.79),(0.16,0.98),
            (0.20,0.79),(0.08,0.79),(0.04,0.70),(0.04,0.16)],true),
        Preset::Heart=>{
            let start=vec_at(b,0.5,0.94);
            let segment=|a:(f32,f32),c:(f32,f32),d:(f32,f32)|CubicSegment{
                control1:vec_at(b,a.0,a.1),control2:vec_at(b,c.0,c.1),end:vec_at(b,d.0,d.1)};
            Shape::Bezier{start,closed:true,segments:vec![
                segment((0.30,0.68),(0.01,0.48),(0.10,0.27)),
                segment((0.17,0.02),(0.43,0.05),(0.50,0.25)),
                segment((0.57,0.05),(0.83,0.02),(0.90,0.27)),
                segment((0.99,0.48),(0.70,0.68),(0.50,0.94)),
            ]}
        }
        Preset::RoundedRectangle=>{
            let radius=(b.w.min(b.h)*r*0.5).max(0.01);
            let k=0.552_284_8_f32;
            let point=|x:f32,y:f32|Point{x,y};
            let (x,y,w,h)=(b.x,b.y,b.w,b.h);
            let start=point(x+radius,y);
            let p1=point(x+w-radius,y);
            let p2=point(x+w,y+radius);
            let p3=point(x+w,y+h-radius);
            let p4=point(x+w-radius,y+h);
            let p5=point(x+radius,y+h);
            let p6=point(x,y+h-radius);
            let p7=point(x,y+radius);
            let segments=vec![
                line(start,p1),
                CubicSegment{control1:point(p1.x+k*radius,p1.y),
                    control2:point(p2.x,p2.y-k*radius),end:p2},
                line(p2,p3),
                CubicSegment{control1:point(p3.x,p3.y+k*radius),
                    control2:point(p4.x+k*radius,p4.y),end:p4},
                line(p4,p5),
                CubicSegment{control1:point(p5.x-k*radius,p5.y),
                    control2:point(p6.x,p6.y+k*radius),end:p6},
                line(p6,p7),
                CubicSegment{control1:point(p7.x,p7.y-k*radius),
                    control2:point(start.x-k*radius,start.y),end:start}
            ];
            Shape::Bezier{start,segments,closed:true}
        }
        Preset::Ring=>{
            let inner=Bounds{x:b.x+b.w*(1.0-r)/2.0,
                y:b.y+b.h*(1.0-r)/2.0,w:b.w*r,h:b.h*r};
            Shape::CompoundPath{contours:vec![ellipse_contour(b),ellipse_contour(inner)]}
        }
    };
    Ok(shape)
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Document,Style};
    #[test] fn every_preset_is_serializable_and_editable(){
        for preset in Preset::ALL{
            let params=Parameters{bounds:Bounds{x:10.,y:20.,w:90.,h:60.},
                detail:12,ratio:0.5,turns:3.0};
            let shape=generate(preset,params).unwrap();
            let mut doc=Document::default();
            doc.add(shape,Style::default());
            doc.validate().unwrap();
            let restored:Document=serde_json::from_str(
                &serde_json::to_string(&doc).unwrap()).unwrap();
            assert_eq!(doc,restored,"preset {}",preset.label());
        }
    }
    #[test] fn ring_has_two_editable_contours(){
        let shape=generate(Preset::Ring,Parameters{
            bounds:Bounds{x:0.,y:0.,w:100.,h:100.},detail:8,ratio:0.5,turns:2.
        }).unwrap();
        let Shape::CompoundPath{contours}=shape else{panic!()};
        assert_eq!(contours.len(),2);
        assert!(contours.iter().all(|c|c.closed&&c.segments.len()==4));
    }
    #[test] fn invalid_parameters_never_produce_shapes(){
        let p=Parameters{bounds:Bounds{x:0.,y:0.,w:10.,h:10.},
            detail:8,ratio:0.5,turns:2.};
        assert!(generate(Preset::Star,Parameters{ratio:0.0,..p}).is_err());
        assert!(generate(Preset::Star,Parameters{detail:1000,..p}).is_err());
        assert!(generate(Preset::Spiral,Parameters{turns:f32::NAN,..p}).is_err());
    }
}
