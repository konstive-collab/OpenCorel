//! Persistent nonprinting guides. Coordinates are stored in millimetres and
//! deliberately independent of the paper size so guides can exist in bleed
//! and the user may resize the page without losing their layout references.
use crate::Point;
use serde::{Deserialize,Serialize};

pub const MAX_GUIDES:usize=256;
const MAX_MM:f32=100_000.0;

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum GuideAxis{Horizontal,Vertical}

#[derive(Clone,Copy,Debug,PartialEq,Serialize,Deserialize)]
pub struct Guide{
    pub axis:GuideAxis,
    pub position_mm:f32,
}
impl Guide{
    pub fn new(axis:GuideAxis,position_mm:f32)->Result<Self,String>{
        if !position_mm.is_finite() || position_mm.abs()>MAX_MM{
            return Err("Guide position must be finite and within ±100000 mm".into());
        }
        Ok(Self{axis,position_mm})
    }
}
pub fn validate(guides:&[Guide])->Result<(),String>{
    if guides.len()>MAX_GUIDES{return Err("Too many guides (limit 256)".into());}
    if guides.iter().any(|g|
        !g.position_mm.is_finite()||g.position_mm.abs()>MAX_MM){
        return Err("Invalid guide position".into());
    }
    Ok(())
}
/// A guide snaps only its own axis; perpendicular guides can snap both axes
/// at their intersection. Tolerance is measured in document mm and must be
/// computed from pixels by the caller.
pub fn snap(p:Point,guides:&[Guide],tolerance_mm:f32)->Point{
    if !p.x.is_finite()||!p.y.is_finite()||
        !tolerance_mm.is_finite()||tolerance_mm<=0.0{
        return p;
    }
    let mut closest_x=tolerance_mm.min(500.);
    let mut closest_y=tolerance_mm.min(500.);
    let mut result=p;
    for g in guides.iter().take(MAX_GUIDES){
        match g.axis{
            GuideAxis::Vertical=>{
                let delta=(p.x-g.position_mm).abs();
                if delta<=closest_x && g.position_mm.is_finite(){
                    result.x=g.position_mm;closest_x=delta;
                }
            },
            GuideAxis::Horizontal=>{
                let delta=(p.y-g.position_mm).abs();
                if delta<=closest_y && g.position_mm.is_finite(){
                    result.y=g.position_mm;closest_y=delta;
                }
            },
        }
    }
    result
}
/// Return exact guide coordinates only for axes within tolerance; use this
/// in the UI to draw lightweight crosshair/guide snap indicators.
pub fn snapped_axes(p:Point,guides:&[Guide],tolerance_mm:f32)->(bool,bool){
    let target=snap(p,guides,tolerance_mm);
    let x=guides.iter().take(MAX_GUIDES).any(|g|
        g.axis==GuideAxis::Vertical && g.position_mm==target.x &&
        (p.x-target.x).abs()<=tolerance_mm && tolerance_mm>0.0);
    let y=guides.iter().take(MAX_GUIDES).any(|g|
        g.axis==GuideAxis::Horizontal && g.position_mm==target.y &&
        (p.y-target.y).abs()<=tolerance_mm && tolerance_mm>0.0);
    (x,y)
}

#[cfg(test)]
mod tests{
    use super::*;
    fn p(x:f32,y:f32)->Point{Point{x,y}}
    #[test]
    fn one_guide_moves_only_matching_axis(){
        let gs=[Guide::new(GuideAxis::Vertical,10.).unwrap()];
        assert_eq!(snap(p(10.5,8.),&gs,1.),p(10.,8.));
        assert_eq!(snap(p(12.,8.),&gs,1.),p(12.,8.));
        assert_eq!(snapped_axes(p(10.5,8.),&gs,1.),(true,false));
    }
    #[test]
    fn intersection_snap_is_nearest_on_each_axis(){
        let gs=[
            Guide::new(GuideAxis::Vertical,10.).unwrap(),
            Guide::new(GuideAxis::Vertical,11.).unwrap(),
            Guide::new(GuideAxis::Horizontal,20.).unwrap(),
        ];
        assert_eq!(snap(p(10.8,20.4),&gs,1.),p(11.,20.));
        assert_eq!(snapped_axes(p(10.8,20.4),&gs,1.),(true,true));
    }
    #[test]
    fn invalid_guide_is_rejected(){
        assert!(Guide::new(GuideAxis::Horizontal,f32::NAN).is_err());
        assert!(Guide::new(GuideAxis::Horizontal,100_001.).is_err());
        assert!(Guide::new(GuideAxis::Vertical,-90_000.).is_ok());
        assert!(validate(&vec![Guide{axis:GuideAxis::Vertical,
            position_mm:5.0};MAX_GUIDES+1]).is_err());
    }
    #[test]
    fn invalid_cursor_and_tolerance_do_not_snap(){
        let gs=[Guide::new(GuideAxis::Vertical,10.).unwrap()];
        assert_eq!(snap(p(10.5,8.),&gs,-1.),p(10.5,8.));
        assert_eq!(snap(p(10.5,8.),&gs,f32::INFINITY),p(10.5,8.));
        assert_eq!(snap(p(f32::NAN,8.),&gs,1.).y,8.);
    }
}
