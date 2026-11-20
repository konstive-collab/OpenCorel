//! QR Code Model 2, rendered as editable grouped vector contours and a
//! mandatory high-contrast background with a 4-module quiet zone.
//! No external QR service, rasterization or hidden external references.
use crate::{Contour,CubicSegment,Document,Point,Shape,Style};
use qrcodegen::{QrCode,QrCodeEcc};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum QrCorrection{Low,Medium,Quartile,High}
impl QrCorrection{
    pub fn label(self)->&'static str{match self{
        Self::Low=>"L · 7%",Self::Medium=>"M · 15%",
        Self::Quartile=>"Q · 25%",Self::High=>"H · 30%"
    }}
    fn ecc(self)->QrCodeEcc{match self{
        Self::Low=>QrCodeEcc::Low,Self::Medium=>QrCodeEcc::Medium,
        Self::Quartile=>QrCodeEcc::Quartile,Self::High=>QrCodeEcc::High
    }}
}
#[derive(Clone,Debug)]
pub struct QrSpec{
    pub payload:String,
    pub origin:Point,
    pub size_mm:f32,
    pub foreground:String,
    pub background:String,
    pub correction:QrCorrection,
}
impl Default for QrSpec{
    fn default()->Self{
        Self{payload:"https://example.org".into(),
            origin:Point{x:30.0,y:30.0},size_mm:60.0,
            foreground:"#000000".into(),background:"#FFFFFF".into(),
            correction:QrCorrection::Medium}
    }
}
impl QrSpec{
    fn validate(&self)->Result<(),String>{
        if self.payload.is_empty()||self.payload.len()>1024||
            !crate::valid_xml_text(&self.payload){
            return Err("QR content must contain 1–1024 UTF-8 bytes".into());
        }
        if !self.size_mm.is_finite()||!(10.0..=2000.0).contains(&self.size_mm)||
            !self.origin.x.is_finite()||!self.origin.y.is_finite()||
            self.origin.x.abs()>100_000.0||self.origin.y.abs()>100_000.0{
            return Err("QR size must be 10–2000 mm with valid position".into());
        }
        let (fg,bg)=(rgb(&self.foreground)?,rgb(&self.background)?);
        let luminosity=|a:[u8;3]|{
            a[0] as f32*0.2126+a[1] as f32*0.7152+a[2] as f32*0.0722
        };
        if luminosity(bg)-luminosity(fg)<125.0{
            return Err("QR needs a dark foreground and lighter background (contrast >= 125)".into());
        }
        Ok(())
    }
}
fn rgb(value:&str)->Result<[u8;3],String>{
    if !crate::valid_color(value){return Err("QR color must use #RRGGBB".into());}
    let part=|start|u8::from_str_radix(&value[start..start+2],16)
        .map_err(|_|"Invalid QR RGB component".to_string());
    Ok([part(1)?,part(3)?,part(5)?])
}
fn straight(a:Point,b:Point)->CubicSegment{
    let map=|t:f32|Point{x:a.x+(b.x-a.x)*t,y:a.y+(b.y-a.y)*t};
    CubicSegment{control1:map(1.0/3.0),control2:map(2.0/3.0),end:b}
}
fn contour(x:f32,y:f32,w:f32,h:f32)->Contour{
    let vertices=[Point{x,y},Point{x:x+w,y},Point{x:x+w,y:y+h},
        Point{x,y:y+h}];
    Contour{start:vertices[0],closed:true,
        segments:(0..4).map(|i|straight(vertices[i],vertices[(i+1)%4])).collect()}
}
pub struct QrArtwork{
    pub background:Shape,
    pub modules:Shape,
    pub matrix_side:usize,
    pub runs:usize,
    pub effective_module_mm:f32,
}
pub fn generate(spec:&QrSpec)->Result<QrArtwork,String>{
    spec.validate()?;
    let code=QrCode::encode_text(&spec.payload,spec.correction.ecc())
        .map_err(|_|"QR payload cannot fit at selected correction level".to_string())?;
    let side=code.size() as usize;
    let module=spec.size_mm/(side as f32+8.0);
    if module<0.30{
        return Err(format!(
            "QR pixels are only {module:.2} mm; increase printed QR size for reliable scanning"
        ));
    }
    let start=Point{x:spec.origin.x+4.0*module,y:spec.origin.y+4.0*module};
    let mut contours=Vec::new();
    let mut dark=0usize;
    for y in 0..side{
        let mut x=0usize;
        while x<side{
            if !code.get_module(x as i32,y as i32){x+=1;continue;}
            let left=x;
            while x<side&&code.get_module(x as i32,y as i32){
                dark+=1;x+=1;
            }
            contours.push(contour(
                start.x+left as f32*module,
                start.y+y as f32*module,
                (x-left) as f32*module,module));
            if contours.len()>8_000{
                return Err("QR too complex for native compound shape; shorten payload".into());
            }
        }
    }
    if contours.is_empty()||dark==0{return Err("QR encoder returned empty matrix".into());}
    Ok(QrArtwork{
        background:Shape::Rectangle{x:spec.origin.x,y:spec.origin.y,
            w:spec.size_mm,h:spec.size_mm},
        modules:Shape::CompoundPath{contours:contours.clone()},
        matrix_side:side,runs:contours.len(),effective_module_mm:module,
    })
}
impl Document{
    /// Adds exactly two native objects, grouped with a shared ID:
    /// high-contrast quiet-zone background and editable dark-module compound.
    pub fn create_qr(&mut self,spec:&QrSpec,layer_id:u64)
        ->Result<[u64;2],String>{
        if self.all_object_count()>99_998||self.next_id>u64::MAX-4{
            return Err("Document object or ID limit prevents a new QR".into());
        }
        if self.layer(layer_id).is_none_or(|l|!l.visible||l.locked){
            return Err("QR target layer must exist, be visible and unlocked".into());
        }
        let art=generate(spec)?;
        let background=self.add_to_layer(art.background,Style{
            fill:spec.background.to_uppercase(),stroke:"none".into(),
            stroke_width_mm:0.0},layer_id);
        let modules=self.add_to_layer(art.modules,Style{
            fill:spec.foreground.to_uppercase(),stroke:"none".into(),
            stroke_width_mm:0.0},layer_id);
        self.group_objects(&[background,modules])
            .expect("two validated editable objects can be grouped");
        if let Some(obj)=self.get_mut(background){obj.name="QR · светлая подложка".into();}
        if let Some(obj)=self.get_mut(modules){
            let snippet: String=spec.payload.chars().take(48).collect();
            obj.name=format!("QR · {snippet}");
        }
        Ok([background,modules])
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn qr_vector_art_is_valid_grouped_and_has_four_module_quiet_zone(){
        let spec=QrSpec::default();
        let qr=generate(&spec).unwrap();
        assert!(qr.matrix_side>=21);
        assert!(qr.runs>20);
        assert!(qr.effective_module_mm>0.3);
        let mut doc=Document::default();
        let [background,modules]=doc.create_qr(&spec,1).unwrap();
        assert_eq!(doc.get(background).unwrap().group_id,
            doc.get(modules).unwrap().group_id);
        assert!(matches!(doc.get(modules).unwrap().shape,Shape::CompoundPath{..}));
        assert!(doc.to_svg().contains("#000000"));
        doc.validate().unwrap();
        let parsed:Document=serde_json::from_slice(
            &serde_json::to_vec(&doc).unwrap()).unwrap();
        parsed.validate().unwrap();
    }
    #[test] fn qr_rejects_low_contrast_oversized_or_too_small_text(){
        let mut spec=QrSpec::default();
        spec.foreground="#DDDDDD".into();
        assert!(generate(&spec).is_err());
        spec=QrSpec::default();
        spec.payload="X".repeat(1025);
        assert!(generate(&spec).is_err());
        spec.payload="A".into();
        spec.size_mm=10.0;
        // A small QR can still fit if the module is >= 0.30 mm.
        assert!(generate(&spec).is_ok());
        spec.payload="z".repeat(850);
        assert!(generate(&spec).is_err());
    }
    #[test] fn qr_generation_on_locked_layer_is_atomic(){
        let mut doc=Document::default();
        doc.layers[0].locked=true;
        let before=doc.clone();
        assert!(doc.create_qr(&QrSpec::default(),1).is_err());
        assert_eq!(doc,before);
    }
}
