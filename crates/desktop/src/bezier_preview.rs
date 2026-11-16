//! Bounded adaptive preview of cubic Bézier paths.
//! A path's geometry is tessellated only when its points or zoom change;
//! panning and unchanged repaint frames reuse the sampled document points.
use opencorel_core::{CubicSegment,Point};
use std::{collections::{HashMap,hash_map::DefaultHasher},hash::{Hash,Hasher}};

const MAX_CACHED_PATHS:usize=32;
const MAX_SAMPLES:usize=20_000;

#[derive(Default)]
pub(super) struct BezierPreviewCache{entries:HashMap<u64,Entry>}
struct Entry{signature:u64,points:Vec<Point>}

fn signature(start:Point,segments:&[CubicSegment],scale:f32)->u64{
    let mut hash=DefaultHasher::new();
    scale.to_bits().hash(&mut hash);
    for p in [start]{
        p.x.to_bits().hash(&mut hash);
        p.y.to_bits().hash(&mut hash);
    }
    segments.len().hash(&mut hash);
    for s in segments{
        for p in [s.control1,s.control2,s.end]{
            p.x.to_bits().hash(&mut hash);
            p.y.to_bits().hash(&mut hash);
        }
    }
    hash.finish()
}

fn cubic(a:Point,b:Point,c:Point,d:Point,t:f32)->Point{
    let u=1.0-t;
    Point{
        x:u*u*u*a.x+3.0*u*u*t*b.x+3.0*u*t*t*c.x+t*t*t*d.x,
        y:u*u*u*a.y+3.0*u*u*t*b.y+3.0*u*t*t*c.y+t*t*t*d.y,
    }
}
fn sample(start:Point,segments:&[CubicSegment],scale:f32)->Vec<Point>{
    let mut points=Vec::with_capacity(1+segments.len().min(MAX_SAMPLES-1));
    points.push(start);
    if segments.len()>=MAX_SAMPLES{
        // A pathological or imported curve must never allocate unbounded
        // vertex buffers on each repaint. Retain its final endpoint.
        let stride=segments.len().div_ceil(MAX_SAMPLES-1);
        for (i,s) in segments.iter().enumerate(){
            if i%stride==0 || i+1==segments.len(){points.push(s.end);}
        }
        return points;
    }
    let per_segment=((MAX_SAMPLES-1)/segments.len().max(1)).clamp(1,48);
    let mut previous=start;
    for seg in segments{
        let approx=(seg.end.x-previous.x).hypot(seg.end.y-previous.y)
            +(seg.control1.x-previous.x).hypot(seg.control1.y-previous.y)
            +(seg.control2.x-seg.end.x).hypot(seg.control2.y-seg.end.y);
        let scaled=(approx*scale.max(0.001)).max(0.0);
        let steps=((scaled/10.0).ceil() as usize).clamp(1,per_segment);
        for i in 1..=steps{
            points.push(cubic(previous,seg.control1,seg.control2,seg.end,
                i as f32/steps as f32));
        }
        previous=seg.end;
    }
    points
}
impl BezierPreviewCache{
    pub fn clear(&mut self){self.entries.clear();}
    pub fn points(&mut self,id:u64,start:Point,segments:&[CubicSegment],
        scale:f32)->&[Point]{
        let hash=signature(start,segments,scale);
        if self.entries.get(&id).is_none_or(|entry|entry.signature!=hash){
            if self.entries.len()>=MAX_CACHED_PATHS && !self.entries.contains_key(&id){
                if let Some(old_id)=self.entries.keys().copied().next(){
                    self.entries.remove(&old_id);
                }
            }
            self.entries.insert(id,Entry{signature:hash,
                points:sample(start,segments,scale)});
        }
        &self.entries.get(&id).expect("cached path").points
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    fn segment(end:f32)->CubicSegment{
        CubicSegment{
            control1:Point{x:end-2.0,y:0.0},
            control2:Point{x:end-1.0,y:10.0},
            end:Point{x:end,y:0.0},
        }
    }
    #[test]
    fn identical_path_reuses_the_same_allocation(){
        let mut cache=BezierPreviewCache::default();
        let start=Point{x:0.0,y:0.0};
        let segments=vec![segment(15.0)];
        let first=cache.points(3,start,&segments,1.0).as_ptr();
        let second=cache.points(3,start,&segments,1.0).as_ptr();
        assert_eq!(first,second);
        assert_eq!(cache.entries.len(),1);
    }
    #[test]
    fn changes_to_handles_or_zoom_invalidate_cached_sampling(){
        let start=Point{x:0.0,y:0.0};
        let mut segments=vec![segment(20.0)];
        let first=signature(start,&segments,1.0);
        segments[0].control1.y=5.0;
        assert_ne!(first,signature(start,&segments,1.0));
        assert_ne!(signature(start,&segments,1.0),
            signature(start,&segments,2.0));
    }
    #[test]
    fn large_path_does_not_exceed_preview_budget(){
        let start=Point{x:0.0,y:0.0};
        let many=vec![segment(5.0);50_000];
        let preview=sample(start,&many,1.0);
        assert!(preview.len()<=MAX_SAMPLES+1);
        assert_eq!(preview.first().copied(),Some(start));
        assert_eq!(preview.last().copied(),Some(many.last().unwrap().end));
    }
    #[test]
    fn cache_limits_distinct_paths(){
        let mut cache=BezierPreviewCache::default();
        for id in 0..100 {
            let _=cache.points(id,Point{x:0.0,y:0.0},&[segment(id as f32)],1.0);
        }
        assert!(cache.entries.len()<=MAX_CACHED_PATHS);
    }
}
