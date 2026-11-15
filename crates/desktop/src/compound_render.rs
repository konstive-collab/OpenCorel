//! Tessellate true closed compound paths with even-odd filling.
//! Retains counters/holes of characters like O, B and Р.
use crate::View;
use eframe::egui::{self,Color32};
use lyon_tessellation::{FillTessellator,FillOptions,math::{Point as LyonPoint,point},
    geometry_builder::{BuffersBuilder,VertexBuffers}};
use opencorel_core::{Contour,Point};
use std::{collections::HashMap,hash::{Hash,Hasher},collections::hash_map::DefaultHasher};

#[derive(Default)]
pub(super) struct CompoundCache{entries:HashMap<u64,Geometry>}
struct Geometry{
    signature:u64,
    vertices:Vec<LyonPoint>,
    triangles:Vec<u32>,
}
fn signature(contours:&[Contour])->u64{
    let mut h=DefaultHasher::new();
    contours.len().hash(&mut h);
    for c in contours{
        c.start.x.to_bits().hash(&mut h);
        c.start.y.to_bits().hash(&mut h);
        c.closed.hash(&mut h);
        c.segments.len().hash(&mut h);
        for s in &c.segments{
            for p in [s.control1,s.control2,s.end]{
                p.x.to_bits().hash(&mut h);
                p.y.to_bits().hash(&mut h);
            }
        }
    }
    h.finish()
}
fn tessellate(contours:&[Contour])->Result<Geometry,String>{
    let mut buffer:VertexBuffers<LyonPoint,u32>=VertexBuffers::new();
    let mut tess=FillTessellator::new();
    let options=FillOptions::default().with_tolerance(0.15);
    {
        let mut geometry=BuffersBuilder::new(&mut buffer,
            |vertex:lyon_tessellation::FillVertex<'_>|vertex.position());
        let mut builder=tess.builder(&options,&mut geometry);
        for contour in contours{
            if contour.segments.is_empty(){continue;}
            builder.begin(point(contour.start.x,contour.start.y));
            for segment in &contour.segments{
                builder.cubic_bezier_to(
                    point(segment.control1.x,segment.control1.y),
                    point(segment.control2.x,segment.control2.y),
                    point(segment.end.x,segment.end.y));
            }
            builder.end(contour.closed);
        }
        builder.build().map_err(|e|format!("Compound path tessellation failed: {e:?}"))?;
    }
    if buffer.vertices.len()>1_000_000 || buffer.indices.len()>3_000_000{
        return Err("Too many outline vertices".into());
    }
    Ok(Geometry{signature:signature(contours),
        vertices:buffer.vertices,triangles:buffer.indices})
}
impl CompoundCache {
    pub fn clear(&mut self){self.entries.clear();}
    pub fn paint(&mut self,p:&egui::Painter,id:u64,contours:&[Contour],v:View,
        fill:Color32,stroke:egui::Stroke){
        if fill.a()>0 {
            let hash=signature(contours);
            if self.entries.get(&id).map(|geo|geo.signature!=hash).unwrap_or(true){
                if self.entries.len()>=16{
                    if let Some(k)=self.entries.keys().copied().next(){self.entries.remove(&k);}
                }
                match tessellate(contours){
                    Ok(geometry)=>{self.entries.insert(id,geometry);},
                    Err(_)=>{self.entries.remove(&id);}
                }
            }
            if let Some(geometry)=self.entries.get(&id){
                let mut mesh=egui::Mesh::default();
                mesh.reserve_vertices(geometry.vertices.len());
                mesh.reserve_triangles(geometry.triangles.len()/3);
                for vertex in &geometry.vertices{
                    mesh.colored_vertex(v.screen(Point{x:vertex.x,y:vertex.y}),fill);
                }
                for tri in geometry.triangles.chunks_exact(3){
                    mesh.add_triangle(tri[0],tri[1],tri[2]);
                }
                p.add(egui::Shape::mesh(mesh));
            }
        }
        if stroke.width>0.0 && stroke.color.a()>0{
            for contour in contours{
                let mut points=vec![v.screen(contour.start)];
                let mut previous=contour.start;
                for seg in &contour.segments{
                    let steps=10usize;
                    for i in 1..=steps{
                        let t=i as f32/steps as f32;
                        let a=1.0-t;
                        points.push(v.screen(Point{
                            x:a*a*a*previous.x+3.0*a*a*t*seg.control1.x+
                                3.0*a*t*t*seg.control2.x+t*t*t*seg.end.x,
                            y:a*a*a*previous.y+3.0*a*a*t*seg.control1.y+
                                3.0*a*t*t*seg.control2.y+t*t*t*seg.end.y,
                        }));
                    }
                    previous=seg.end;
                }
                if contour.closed{points.push(v.screen(contour.start));}
                if points.len()>1{p.add(egui::Shape::line(points,stroke));}
            }
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::CubicSegment;
    fn square(x:f32,y:f32,size:f32)->Contour{
        let a=Point{x,y};
        let b=Point{x:x+size,y};
        let c=Point{x:x+size,y:y+size};
        let d=Point{x,y:y+size};
        Contour{start:a,closed:true,segments:vec![
            CubicSegment{control1:a,control2:b,end:b},
            CubicSegment{control1:b,control2:c,end:c},
            CubicSegment{control1:c,control2:d,end:d},
            CubicSegment{control1:d,control2:a,end:a}
        ]}
    }
    #[test] fn hole_area_is_not_filled(){
        let geo=tessellate(&[square(0.0,0.0,100.0),square(25.0,25.0,50.0)]).unwrap();
        let mut area=0.0;
        for t in geo.triangles.chunks_exact(3){
            let a=geo.vertices[t[0] as usize];
            let b=geo.vertices[t[1] as usize];
            let c=geo.vertices[t[2] as usize];
            area+=((b.x-a.x)*(c.y-a.y)-(b.y-a.y)*(c.x-a.x)).abs()*0.5;
        }
        assert!((area-7500.0).abs()<15.0,"fill area {area}");
    }
}
