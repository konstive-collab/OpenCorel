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
/// Production-friendly editable presets. Colors remain normal RGB hex; they
/// are not device-profile CMYK or ICC-managed spot colors.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum GradientPreset{
    Sunrise, Ocean, Forest, Sunset, Steel, Candy, Monochrome
}
impl GradientPreset{
    pub const ALL:[Self;7]=[
        Self::Sunrise,Self::Ocean,Self::Forest,Self::Sunset,
        Self::Steel,Self::Candy,Self::Monochrome
    ];
    pub fn label(self)->&'static str{
        match self{
            Self::Sunrise=>"Рассвет", Self::Ocean=>"Океан",
            Self::Forest=>"Лес",Self::Sunset=>"Закат",
            Self::Steel=>"Сталь",Self::Candy=>"Неон",
            Self::Monochrome=>"Ч/Б"
        }
    }
    pub fn create(self)->Gradient{
        let (kind,angle,colors):(GradientKind,f32,&[&str])=match self{
            Self::Sunrise=>(GradientKind::Linear,25.,&["#FEF6A8","#FC8A43","#D22A7B"]),
            Self::Ocean=>(GradientKind::Linear,-30.,&["#061D54","#1678B8","#8CE4DA"]),
            Self::Forest=>(GradientKind::Linear,75.,&["#0A3529","#35A36F","#C6E9A4"]),
            Self::Sunset=>(GradientKind::Radial,0.,&["#FFE78E","#EA7358","#572D91"]),
            Self::Steel=>(GradientKind::Linear,90.,&["#1B263B","#ADB9C9","#536378"]),
            Self::Candy=>(GradientKind::Linear,-55.,&["#732BEA","#FA4EAD","#65E4F9"]),
            Self::Monochrome=>(GradientKind::Linear,0.,&["#111111","#EEEEEE"])
        };
        let max=(colors.len()-1) as f32;
        Gradient{kind,angle_degrees:angle,
            stops:colors.iter().enumerate().map(|(i,color)|
                GradientStop{offset:i as f32/max,color:(*color).to_owned()}
            ).collect()}
    }
}
fn rgb_hex(color:&str)->Result<[u8;3],String>{
    if !crate::valid_color(color){return Err("Gradient requires #RRGGBB color".into());}
    let parse=|offset|u8::from_str_radix(&color[offset..offset+2],16)
        .map_err(|e|e.to_string());
    Ok([parse(1)?,parse(3)?,parse(5)?])
}
impl Gradient{
    /// Sample linear RGB channel interpolation at a normalized position.
    /// This matches the SVG stop colors used for screen export previews.
    pub fn color_at(&self,position:f32)->Result<String,String>{
        self.validate()?;
        if !position.is_finite(){return Err("Non-finite gradient sample".into());}
        let t=position.clamp(0.,1.);
        let first=&self.stops[0];
        if t<=first.offset{return Ok(first.color.clone());}
        for pair in self.stops.windows(2){
            let (a,b)=(&pair[0],&pair[1]);
            if t<=b.offset{
                if (b.offset-a.offset).abs()<f32::EPSILON{return Ok(b.color.clone());}
                let ratio=((t-a.offset)/(b.offset-a.offset)).clamp(0.,1.);
                let rgb1=rgb_hex(&a.color)?;
                let rgb2=rgb_hex(&b.color)?;
                let color=|index:usize|{
                    (rgb1[index] as f32+(rgb2[index] as f32-rgb1[index] as f32)*ratio)
                        .round().clamp(0.,255.) as u8
                };
                return Ok(format!("#{:02X}{:02X}{:02X}",
                    color(0),color(1),color(2)));
            }
        }
        Ok(self.stops.last().expect("Validated gradient has 2 stops").color.clone())
    }
    pub fn insert_stop(&mut self,position:f32)->Result<usize,String>{
        self.validate()?;
        if self.stops.len()>=32{return Err("Maximum 32 gradient stops".into());}
        if !position.is_finite()||!(0.0..=1.0).contains(&position){
            return Err("Stop position must be 0–100%".into());
        }
        let color=self.color_at(position)?;
        let index=self.stops.partition_point(|s|s.offset<=position);
        self.stops.insert(index,GradientStop{offset:position,color});
        Ok(index)
    }
    pub fn remove_stop(&mut self,index:usize)->Result<(),String>{
        if self.stops.len()<=2{return Err("Gradient needs at least 2 stops".into());}
        if index>=self.stops.len(){return Err("Unknown gradient stop".into());}
        self.stops.remove(index);
        Ok(())
    }
    pub fn set_stop(&mut self,index:usize,position:f32,color:&str)->Result<usize,String>{
        self.validate()?;
        if index>=self.stops.len(){return Err("Unknown gradient stop".into());}
        if !position.is_finite()||!(0.0..=1.0).contains(&position)
            ||!crate::valid_color(color){
            return Err("Invalid gradient stop position or hex color".into());
        }
        let mut edited=self.clone();
        edited.stops.remove(index);
        let destination=edited.stops.partition_point(|s|s.offset<=position);
        edited.stops.insert(destination,GradientStop{
            offset:position,color:color.into()
        });
        edited.validate()?;
        *self=edited;
        Ok(destination)
    }
}
#[cfg(test)]
mod extended_tests{
    use super::*;
    #[test] fn seven_presets_are_valid_three_stop_gradients(){
        for preset in GradientPreset::ALL{
            let gradient=preset.create();
            gradient.validate().unwrap();
            assert!(gradient.svg_definition(99).contains("appearance-99"));
        }
    }
    #[test] fn insert_and_reorder_gradient_stops_preserve_interpolation(){
        let mut gradient=GradientPreset::Monochrome.create();
        let index=gradient.insert_stop(0.5).unwrap();
        assert_eq!(index,1);
        assert_eq!(gradient.stops[1].color,"#808080");
        let new_index=gradient.set_stop(1,0.8,"#FFFFFF").unwrap();
        assert_eq!(new_index,1);
        gradient.validate().unwrap();
        gradient.remove_stop(1).unwrap();
        assert_eq!(gradient.stops.len(),2);
        assert!(gradient.remove_stop(0).is_err());
    }
    #[test] fn invalid_color_does_not_mutate_existing_stops(){
        let mut gradient=GradientPreset::Sunrise.create();
        let before=gradient.clone();
        assert!(gradient.set_stop(1,0.6,"red").is_err());
        assert_eq!(gradient,before);
        assert!(gradient.insert_stop(f32::NAN).is_err());
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
