//! Exact, non-boolean editing of compound cubic paths.
//! Combine uses even-odd fill: overlaps toggle parity rather than weld.
//! Individual cubic traces and their handles are never flattened.
use crate::{Contour,Shape};

const MAX_CONTOURS:usize=10_000;
const MAX_SEGMENTS:usize=60_000;

fn contours_of(shape:&Shape)->Result<Vec<Contour>,String>{
    match shape{
        Shape::Bezier{start,segments,closed:true} if !segments.is_empty()=>
            Ok(vec![Contour{start:*start,segments:segments.clone(),closed:true}]),
        Shape::CompoundPath{contours} if !contours.is_empty()
            && contours.iter().all(|c|c.closed && !c.segments.is_empty())=>
            Ok(contours.clone()),
        _=>Err("Only closed Bézier or fully closed compound paths can be combined".into()),
    }
}

/// Combine two or more closed cubic objects into one editable compound path.
/// This is NOT a Boolean union; even-odd fill toggles where contours overlap.
pub fn combine_exact(shapes:&[&Shape])->Result<Shape,String>{
    if shapes.len()<2 || shapes.len()>MAX_CONTOURS{
        return Err("Select between 2 and 10,000 closed vector paths".into());
    }
    let mut contours=Vec::new();
    let mut segments_count=0usize;
    for shape in shapes{
        let parts=contours_of(shape)?;
        segments_count=segments_count.checked_add(
            parts.iter().map(|c|c.segments.len()).sum::<usize>()
        ).ok_or("Segment count overflow")?;
        if contours.len().checked_add(parts.len()).is_none_or(|n|n>MAX_CONTOURS)
            || segments_count>MAX_SEGMENTS{
            return Err("Compound path exceeds geometry limits".into());
        }
        for c in &parts{
            let valid=|p:crate::Point|p.x.is_finite() && p.y.is_finite()
                && p.x.abs()<=100_000.0 && p.y.abs()<=100_000.0;
            if !valid(c.start)||c.segments.iter().any(|s|
                !valid(s.control1)||!valid(s.control2)||!valid(s.end)){
                return Err("Compound geometry contains invalid coordinates".into());
            }
        }
        contours.extend(parts);
    }
    Ok(Shape::CompoundPath{contours})
}

/// Extract all contours as standalone closed cubic objects, preserving the
/// exact original geometry. The caller owns IDs, styling and history.
pub fn break_apart_exact(shape:&Shape)->Result<Vec<Shape>,String>{
    let Shape::CompoundPath{contours}=shape else{
        return Err("Select an editable compound path".into());
    };
    if contours.len()<2 || contours.len()>MAX_CONTOURS{
        return Err("Compound path must have at least two closed contours".into());
    }
    let total:usize=contours.iter().map(|c|c.segments.len()).sum();
    if total>MAX_SEGMENTS || contours.iter().any(|c|!c.closed||c.segments.is_empty()){
        return Err("Unsupported open or oversized compound path".into());
    }
    let valid=|p:crate::Point|p.x.is_finite() && p.y.is_finite()
        && p.x.abs()<=100_000.0 && p.y.abs()<=100_000.0;
    if contours.iter().any(|c|!valid(c.start)||c.segments.iter().any(|s|
        !valid(s.control1)||!valid(s.control2)||!valid(s.end))){
        return Err("Compound geometry contains invalid coordinates".into());
    }
    Ok(contours.iter().map(|c|Shape::Bezier{
        start:c.start,segments:c.segments.clone(),closed:true
    }).collect())
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Point,CubicSegment};
    fn p(x:f32,y:f32)->Point{Point{x,y}}
    fn shape(x:f32)->Shape{
        Shape::Bezier{start:p(x,0.),segments:vec![
            CubicSegment{control1:p(x+2.,0.),control2:p(x+8.,0.),end:p(x+10.,0.)},
            CubicSegment{control1:p(x+10.,2.),control2:p(x+10.,8.),end:p(x+10.,10.)},
            CubicSegment{control1:p(x+8.,10.),control2:p(x+2.,10.),end:p(x,10.)},
        ],closed:true}
    }
    #[test]
    fn exact_combine_and_break_preserve_all_cubic_handles(){
        let a=shape(0.);
        let b=shape(5.);
        let result=combine_exact(&[&a,&b]).unwrap();
        let parts=break_apart_exact(&result).unwrap();
        assert_eq!(parts,vec![a,b]);
    }
    #[test]
    fn nested_compound_roundtrip(){
        let a=shape(0.);let b=shape(4.);let c=shape(10.);
        let ab=combine_exact(&[&a,&b]).unwrap();
        let abc=combine_exact(&[&ab,&c]).unwrap();
        assert_eq!(break_apart_exact(&abc).unwrap(),vec![a,b,c]);
    }
    #[test]
    fn invalid_and_open_paths_are_rejected(){
        let a=shape(0.);
        assert!(combine_exact(&[&a]).is_err());
        let mut open=shape(10.);
        if let Shape::Bezier{closed,..}=&mut open{*closed=false;}
        assert!(combine_exact(&[&a,&open]).is_err());
        assert!(break_apart_exact(&a).is_err());
        let mut invalid=shape(5.);
        if let Shape::Bezier{start,..}=&mut invalid{start.x=f32::NAN;}
        assert!(combine_exact(&[&a,&invalid]).is_err());
        let bad=Shape::CompoundPath{contours:vec![Contour{
            start:p(0.,0.),segments:vec![CubicSegment{
                control1:p(1.,0.),control2:p(2.,0.),end:p(3.,0.)}],closed:false
        },Contour{start:p(4.,0.),segments:vec![CubicSegment{
                control1:p(5.,0.),control2:p(6.,0.),end:p(7.,0.)}],closed:true
        }]};
        assert!(break_apart_exact(&bad).is_err());
    }
}
