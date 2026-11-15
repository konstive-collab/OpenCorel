//! Optional professional object appearance, independent of legacy Style fields.
//! Saved in KDR as an object-ID keyed extension; old KDR v1 projects remain readable.
use serde::{Deserialize,Serialize};

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum GradientKind{Linear,Radial}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize,Default)]
#[serde(rename_all="snake_case")]
pub enum StrokeCap{#[default] Butt,Round,Square}
impl StrokeCap {
    fn as_svg(self)->&'static str{
        match self{Self::Butt=>"butt",Self::Round=>"round",Self::Square=>"square"}
    }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize,Default)]
#[serde(rename_all="snake_case")]
pub enum StrokeJoin{#[default] Miter,Round,Bevel}
impl StrokeJoin {
    fn as_svg(self)->&'static str{
        match self{Self::Miter=>"miter",Self::Round=>"round",Self::Bevel=>"bevel"}
    }
}
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
pub struct GradientStop{pub offset:f32,pub color:String}
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
pub struct Gradient{
    pub kind:GradientKind,
    pub angle_degrees:f32,
    pub stops:Vec<GradientStop>,
}
impl Gradient{
    pub fn validate(&self)->Result<(),String>{
        if !self.angle_degrees.is_finite()||self.angle_degrees.abs()>360.0 {
            return Err("Invalid gradient angle".into());
        }
        if self.stops.len()<2||self.stops.len()>32 {
            return Err("Gradient needs 2–32 stops".into());
        }
        let mut last=-1.0_f32;
        for stop in &self.stops{
            if !stop.offset.is_finite()||!(0.0..=1.0).contains(&stop.offset)
                ||stop.offset<last||!crate::valid_color(&stop.color) {
                return Err("Invalid gradient stop or ordering".into());
            }
            last=stop.offset;
        }
        Ok(())
    }
    pub fn svg_definition(&self,id:u64)->String{
        let mut out=match self.kind{
            GradientKind::Linear=>{
                let (sin,cos)=self.angle_degrees.to_radians().sin_cos();
                format!("<linearGradient id=\"appearance-{id}\" x1=\"{:.4}%\" y1=\"{:.4}%\" x2=\"{:.4}%\" y2=\"{:.4}%\">",
                    50.0-50.0*cos,50.0-50.0*sin,50.0+50.0*cos,50.0+50.0*sin)
            }
            GradientKind::Radial=>format!(
                "<radialGradient id=\"appearance-{id}\" cx=\"50%\" cy=\"50%\" r=\"50%\">"),
        };
        for stop in &self.stops{
            out.push_str(&format!("<stop offset=\"{:.5}%\" stop-color=\"{}\"/>",
                stop.offset*100.0,stop.color));
        }
        out.push_str(match self.kind{
            GradientKind::Linear=>"</linearGradient>",
            GradientKind::Radial=>"</radialGradient>",
        });
        out
    }
}
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(default)]
pub struct Appearance{
    pub opacity:f32,
    pub gradient:Option<Gradient>,
    pub dash_array:Vec<f32>,
    pub cap:StrokeCap,
    pub join:StrokeJoin,
}
impl Default for Appearance{
    fn default()->Self{
        Self{opacity:1.0,gradient:None,dash_array:Vec::new(),
            cap:StrokeCap::Butt,join:StrokeJoin::Miter}
    }
}
impl Appearance{
    pub fn validate(&self)->Result<(),String>{
        if !self.opacity.is_finite()||!(0.0..=1.0).contains(&self.opacity){
            return Err("Opacity must be in the 0..1 range".into());
        }
        if let Some(g)=&self.gradient{g.validate()?;}
        if self.dash_array.len()>16||
            self.dash_array.iter().any(|v|!v.is_finite()||!(0.01..=1000.0).contains(v)){
            return Err("Invalid stroke dash array".into());
        }
        Ok(())
    }
    pub fn is_default(&self)->bool{self==&Self::default()}
    pub fn svg_attributes(&self)->String{
        let mut out=format!(" opacity=\"{}\" stroke-linecap=\"{}\" stroke-linejoin=\"{}\"",
            self.opacity,self.cap.as_svg(),self.join.as_svg());
        if !self.dash_array.is_empty(){
            let values=self.dash_array.iter().map(|n|n.to_string()).collect::<Vec<_>>().join(",");
            out.push_str(&format!(" stroke-dasharray=\"{values}\""));
        }
        out
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn gradient_serialization_and_validation(){
        let g=Gradient{kind:GradientKind::Linear,angle_degrees:45.0,stops:vec![
            GradientStop{offset:0.0,color:"#000000".into()},
            GradientStop{offset:1.0,color:"#FFFFFF".into()},
        ]};
        g.validate().unwrap();
        assert!(g.svg_definition(9).contains("appearance-9"));
        let reloaded:Gradient=serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
        assert_eq!(reloaded,g);
    }
    #[test] fn malformed_gradient_is_rejected(){
        let mut g=Gradient{kind:GradientKind::Radial,angle_degrees:0.0,stops:vec![
            GradientStop{offset:1.0,color:"#000000".into()},
            GradientStop{offset:0.0,color:"#FFFFFF".into()},
        ]};
        assert!(g.validate().is_err());
        g.stops[1].offset=1.0;
        g.stops[1].color="<svg>".into();
        assert!(g.validate().is_err());
    }
}
