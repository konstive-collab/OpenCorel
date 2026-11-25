//! Non-destructive raster adjustments. The original compressed source and
//! parameters remain editable; the existing raster payload is a derived PNG
//! consumed identically by canvas, SVG/PDF and raster proof exporters.
use crate::{Document,Shape};
use base64::{Engine,engine::general_purpose::STANDARD};
use image::{ImageFormat,ImageReader,RgbaImage,imageops};
use serde::{Deserialize,Serialize};
use std::io::{Cursor,Write};

pub const MAX_EDIT_PIXELS:u64=24_000_000;
pub const MAX_ENCODED_BYTES:usize=40*1024*1024;
pub const MAX_EDIT_DOCUMENT_BYTES:usize=80*1024*1024;

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(default)]
pub struct RasterSettings{
    /// Crop margins in thousandths of the original, unrotated image.
    pub left:u16,pub top:u16,pub right:u16,pub bottom:u16,
    pub turns:u8,
    pub flip_x:bool,pub flip_y:bool,
    pub brightness:i16,pub contrast:i16,pub saturation:i16,
    pub exposure:i16,
    pub gamma:u16,
    pub warmth:i16,pub tint:i16,
    pub black:u8,pub white:u8,
    pub grayscale:bool,pub sepia:bool,pub invert:bool,
    pub threshold:u8,pub posterize:u8,
    pub opacity:u8,
    pub key_enabled:bool,pub key_color:[u8;3],pub key_tolerance:u8,pub key_softness:u8,
    /// Zero preserves the cropped pixel resolution.
    pub max_side:u16,
}
impl Default for RasterSettings{
    fn default()->Self{
        Self{left:0,top:0,right:0,bottom:0,turns:0,flip_x:false,flip_y:false,
            brightness:0,contrast:0,saturation:0,exposure:0,gamma:100,
            warmth:0,tint:0,black:0,white:255,grayscale:false,sepia:false,
            invert:false,threshold:0,posterize:0,opacity:100,
            key_enabled:false,key_color:[255,255,255],key_tolerance:20,key_softness:15,
            max_side:0}
    }
}
impl RasterSettings{
    pub fn validate(&self)->Result<(),String>{
        if self.left>999||self.right>999||self.top>999||self.bottom>999||
            u32::from(self.left)+u32::from(self.right)>=1000||
            u32::from(self.top)+u32::from(self.bottom)>=1000||self.turns>3{
            return Err("Invalid crop margins or orientation".into());
        }
        if [self.brightness,self.contrast,self.saturation,self.warmth,self.tint]
            .iter().any(|v|!(-100..=100).contains(v))||
            !(-300..=300).contains(&self.exposure)||
            !(20..=500).contains(&self.gamma)||self.black>=self.white||
            self.opacity>100||self.key_tolerance>100||self.key_softness>100||
            (self.posterize!=0&&!(2..=32).contains(&self.posterize))||
            self.max_side>16000{
            return Err("Invalid raster adjustment range".into());
        }
        Ok(())
    }
    pub fn crop_pixels(&self,w:u32,h:u32)->Result<(u32,u32,u32,u32),String>{
        self.validate()?;
        if w==0||h==0{return Err("Empty source image".into());}
        let x=(u64::from(w)*u64::from(self.left)/1000) as u32;
        let y=(u64::from(h)*u64::from(self.top)/1000) as u32;
        let x2=w-(u64::from(w)*u64::from(self.right)/1000) as u32;
        let y2=h-(u64::from(h)*u64::from(self.bottom)/1000) as u32;
        if x2<=x||y2<=y{return Err("Crop is empty".into());}
        Ok((x,y,x2-x,y2-y))
    }
    pub fn output_size(&self,w:u32,h:u32)->Result<(u32,u32),String>{
        let (_,_,mut w,mut h)=self.crop_pixels(w,h)?;
        if self.turns%2==1{std::mem::swap(&mut w,&mut h);}
        let side=u32::from(self.max_side);
        if side>0&&w.max(h)>side{
            let scale=side as f64/w.max(h) as f64;
            w=((w as f64*scale).round() as u32).max(1);
            h=((h as f64*scale).round() as u32).max(1);
        }
        Ok((w,h))
    }
}

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RasterEdit{
    pub source_width:u32,pub source_height:u32,
    pub source_mime:String,pub source_base64:String,
    pub settings:RasterSettings,
}
impl RasterEdit{
    pub fn source_bytes(&self)->usize{self.source_base64.len()+self.source_mime.len()}
    pub fn validate(&self,w:u32,h:u32,mime:&str)->Result<(),String>{
        self.settings.validate()?;
        if mime!="image/png"||self.settings==RasterSettings::default(){
            return Err("Edited raster must contain a derived PNG and non-default settings".into());
        }
        let raw=source_bytes(&self.source_base64)?;
        let dims=checked_dimensions(&raw,&self.source_mime)?;
        if dims!=(self.source_width,self.source_height)||
            self.settings.output_size(dims.0,dims.1)?!=(w,h){
            return Err("Raster source or derived dimensions do not match edit parameters".into());
        }
        Ok(())
    }
}

fn source_bytes(data:&str)->Result<Vec<u8>,String>{
    if data.len()>56*1024*1024{return Err("Raster source exceeds encoded size limit".into());}
    let bytes=STANDARD.decode(data).map_err(|_|"Invalid raster base64")?;
    if bytes.is_empty()||bytes.len()>MAX_ENCODED_BYTES{return Err("Raster source exceeds 40 MiB".into());}
    Ok(bytes)
}
fn checked_dimensions(raw:&[u8],mime:&str)->Result<(u32,u32),String>{
    let expected=match mime{"image/png"=>ImageFormat::Png,"image/jpeg"=>ImageFormat::Jpeg,
        _=>return Err("Unsupported raster source MIME".into())};
    if image::guess_format(raw).map_err(|e|e.to_string())?!=expected{
        return Err("Raster source MIME mismatch".into());
    }
    let dims=ImageReader::with_format(Cursor::new(raw),expected)
        .into_dimensions().map_err(|e|e.to_string())?;
    if dims.0==0||dims.1==0||dims.0>16000||dims.1>16000||
        u64::from(dims.0)*u64::from(dims.1)>MAX_EDIT_PIXELS{
        return Err("Raster editing limit: 24 megapixels / 16000 pixels per side".into());
    }
    Ok(dims)
}
struct CappedWriter{bytes:Vec<u8>,limit:usize}
impl Write for CappedWriter{
    fn write(&mut self,data:&[u8])->std::io::Result<usize>{
        if self.bytes.len().saturating_add(data.len())>self.limit{
            return Err(std::io::Error::other("Derived PNG exceeds 40 MiB"));
        }
        self.bytes.extend_from_slice(data);Ok(data.len())
    }
    fn flush(&mut self)->std::io::Result<()>{Ok(())}
}
fn encode_png(rgba:RgbaImage)->Result<Vec<u8>,String>{
    use image::ImageEncoder;
    let mut output=CappedWriter{bytes:Vec::new(),limit:MAX_ENCODED_BYTES};
    image::codecs::png::PngEncoder::new(&mut output)
        .write_image(rgba.as_raw(),rgba.width(),rgba.height(),image::ExtendedColorType::Rgba8)
        .map_err(|e|format!("Cannot encode adjusted raster: {e}"))?;
    Ok(output.bytes)
}
fn luma(r:f32,g:f32,b:f32)->f32{0.2126*r+0.7152*g+0.0722*b}
fn channel(value:f32)->u8{(value.clamp(0.,1.)*255.).round() as u8}
pub fn process_pixels(mut input:RgbaImage,s:RasterSettings)->Result<RgbaImage,String>{
    let (w,h)=input.dimensions();
    if u64::from(w)*u64::from(h)>MAX_EDIT_PIXELS||w>16000||h>16000{
        return Err("Raster editing limit exceeded".into());
    }
    let (x,y,cw,ch)=s.crop_pixels(w,h)?;
    if (x,y,cw,ch)!=(0,0,w,h){
        input=imageops::crop_imm(&input,x,y,cw,ch).to_image();
    }
    input=match s.turns{1=>imageops::rotate90(&input),2=>imageops::rotate180(&input),
        3=>imageops::rotate270(&input),_=>input};
    if s.flip_x{imageops::flip_horizontal_in_place(&mut input);}
    if s.flip_y{imageops::flip_vertical_in_place(&mut input);}
    let (ow,oh)=s.output_size(w,h)?;
    if input.dimensions()!=(ow,oh){
        input=imageops::resize(&input,ow,oh,imageops::FilterType::Triangle);
    }
    let exp=2.0f32.powf(s.exposure as f32/100.);
    let gamma=100./s.gamma as f32;
    let c=1.+s.contrast as f32/100.;
    let sat=1.+s.saturation as f32/100.;
    let brightness=s.brightness as f32/100.;
    let warmth=s.warmth as f32/400.;
    let tint=s.tint as f32/400.;
    let black=s.black as f32/255.;
    let span=(s.white-s.black) as f32/255.;
    // A LUT handles the expensive power function once per input value.
    let lut:[f32;256]=std::array::from_fn(|i|{
        let v=((i as f32/255.-black)/span).clamp(0.,1.);
        ((v*exp).clamp(0.,1.).powf(gamma)-0.5)*c+0.5+brightness
    });
    for pixel in input.pixels_mut(){
        let [r0,g0,b0,a0]=pixel.0;
        let mut r=lut[r0 as usize]+warmth+tint*0.5;
        let mut g=lut[g0 as usize]-tint;
        let mut b=lut[b0 as usize]-warmth+tint*0.5;
        let gray=luma(r,g,b);
        r=gray+(r-gray)*sat;g=gray+(g-gray)*sat;b=gray+(b-gray)*sat;
        if s.grayscale{let v=luma(r,g,b);r=v;g=v;b=v;}
        if s.sepia{
            let (rr,gg,bb)=(r,g,b);
            r=0.393*rr+0.769*gg+0.189*bb;
            g=0.349*rr+0.686*gg+0.168*bb;
            b=0.272*rr+0.534*gg+0.131*bb;
        }
        if s.invert{r=1.-r;g=1.-g;b=1.-b;}
        if s.threshold!=0{
            let v=if luma(r,g,b)>=s.threshold as f32/255.{1.}else{0.};
            r=v;g=v;b=v;
        }
        if s.posterize>1{
            let steps=(s.posterize-1) as f32;
            let q=|v:f32|(v.clamp(0.,1.)*steps).round()/steps;
            r=q(r);g=q(g);b=q(b);
        }
        let mut alpha=a0 as f32/255.*s.opacity as f32/100.;
        if s.key_enabled{
            // Key on original RGB, not on the color-corrected result.
            let distance=[r0,g0,b0].into_iter().zip(s.key_color)
                .map(|(x,y)|(f32::from(x)-f32::from(y)).powi(2)).sum::<f32>()
                .sqrt()/(255.*3.0f32.sqrt());
            let threshold=s.key_tolerance as f32/100.;
            let softness=s.key_softness as f32/100.;
            let mask=if softness==0.{if distance<=threshold{0.}else{1.}}
                else{((distance-threshold)/softness).clamp(0.,1.)};
            alpha*=mask;
        }
        pixel.0=[channel(r),channel(g),channel(b),channel(alpha)];
    }
    Ok(input)
}
/// Apply parameters from the immutable original, never from a previous
/// correction. Physical frame is deliberately preserved; crop/rotate changes
/// the artwork inside the frame. Reset restores original encoded bytes.
pub fn adjusted_shape(shape:&Shape,settings:RasterSettings)->Result<Shape,String>{
    settings.validate()?;
    let Shape::Raster{x,y,w,h,pixel_width,pixel_height,mime,data_base64,edit}=shape
        else{return Err("Select a raster image".into());};
    let (sw,sh,sm,source)=if let Some(edit)=edit{
        (edit.source_width,edit.source_height,&edit.source_mime,&edit.source_base64)
    }else{(*pixel_width,*pixel_height,mime,data_base64)};
    if settings==RasterSettings::default(){
        return Ok(Shape::Raster{x:*x,y:*y,w:*w,h:*h,pixel_width:sw,pixel_height:sh,
            mime:sm.clone(),data_base64:source.clone(),edit:None});
    }
    let raw=source_bytes(source)?;
    if checked_dimensions(&raw,sm)?!=(sw,sh){return Err("Source image dimensions do not match metadata".into());}
    let decoded=image::load_from_memory(&raw).map_err(|e|format!("Raster decode failed: {e}"))?;
    let out=process_pixels(decoded.to_rgba8(),settings)?;
    let (pw,ph)=out.dimensions();
    let data=STANDARD.encode(encode_png(out)?);
    Ok(Shape::Raster{x:*x,y:*y,w:*w,h:*h,pixel_width:pw,pixel_height:ph,
        mime:"image/png".into(),data_base64:data,
        edit:Some(Box::new(RasterEdit{source_width:sw,source_height:sh,
            source_mime:sm.clone(),source_base64:source.clone(),settings}))})
}
pub fn raster_storage_bytes(shape:&Shape)->usize{
    match shape{
        Shape::Raster{data_base64,edit,..}=>data_base64.len().saturating_add(
            edit.as_ref().map_or(0,|e|e.source_bytes()+std::mem::size_of::<RasterEdit>())),
        _=>0,
    }
}
impl Document{
    /// Prepare an edit without mutating the document, allowing the GUI to
    /// checkpoint exactly once only after successful decoding and validation.
    pub fn prepare_raster_edit(&self,id:u64,settings:RasterSettings)->Result<Shape,String>{
        let obj=self.get(id).ok_or("Raster object not found")?;
        if !self.layer(obj.layer_id).is_some_and(|l|l.visible&&!l.locked){
            return Err("Raster is on a hidden or locked layer".into());
        }
        if obj.group_id.is_some(){return Err("Detach or ungroup the raster before editing".into());}
        let result=adjusted_shape(&obj.shape,settings)?;
        let size=self.estimated_size_bytes().saturating_sub(raster_storage_bytes(&obj.shape))
            .saturating_add(raster_storage_bytes(&result));
        if size>MAX_EDIT_DOCUMENT_BYTES && raster_storage_bytes(&result)>raster_storage_bytes(&obj.shape){
            return Err("Edit would exceed the 80 MiB document memory budget; reduce image resolution or split the document".into());
        }
        Ok(result)
    }
    pub fn apply_raster_edit(&mut self,id:u64,settings:RasterSettings)->Result<(),String>{
        let result=self.prepare_raster_edit(id,settings)?;
        self.get_mut(id).ok_or("Raster object not found")?.shape=result;
        Ok(())
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::Style;
    fn sample()->Shape{
        let data=RgbaImage::from_fn(4,2,|x,y|image::Rgba([x as u8*70,y as u8*200,20,255]));
        Shape::Raster{x:1.,y:2.,w:40.,h:20.,pixel_width:4,pixel_height:2,
            mime:"image/png".into(),data_base64:STANDARD.encode(encode_png(data).unwrap()),edit:None}
    }
    fn pixels(shape:&Shape)->RgbaImage{
        let Shape::Raster{data_base64,..}=shape else{panic!()};
        image::load_from_memory(&STANDARD.decode(data_base64).unwrap()).unwrap().to_rgba8()
    }
    #[test] fn crop_rotate_flip_has_exact_pixel_mapping(){
        let s=sample();let src=pixels(&s);
        let edited=adjusted_shape(&s,RasterSettings{left:250,turns:1,flip_x:true,..Default::default()}).unwrap();
        let actual=pixels(&edited);
        assert_eq!(actual.dimensions(),(2,3));
        assert_eq!(actual.get_pixel(0,0),src.get_pixel(1,0));
        assert_eq!(actual.get_pixel(1,2),src.get_pixel(3,1));
        assert_eq!(edited.bounds(),s.bounds());
    }
    #[test] fn repeated_edit_uses_original_and_reset_is_exact(){
        let source=sample();
        let first=adjusted_shape(&source,RasterSettings{brightness:30,..Default::default()}).unwrap();
        let params=RasterSettings{contrast:25,invert:true,..Default::default()};
        let next=adjusted_shape(&first,params).unwrap();
        assert_eq!(pixels(&next),pixels(&adjusted_shape(&source,params).unwrap()));
        assert_eq!(adjusted_shape(&next,RasterSettings::default()).unwrap(),source);
    }
    #[test] fn alpha_key_grayscale_and_opacity(){
        let mut img=RgbaImage::new(2,1);
        img.put_pixel(0,0,image::Rgba([255,255,255,255]));
        img.put_pixel(1,0,image::Rgba([255,0,0,128]));
        let out=process_pixels(img,RasterSettings{key_enabled:true,key_tolerance:0,key_softness:0,
            grayscale:true,opacity:50,..Default::default()}).unwrap();
        assert_eq!(out.get_pixel(0,0).0[3],0);
        let p=out.get_pixel(1,0).0;
        assert_eq!(p[0],p[1]);assert_eq!(p[1],p[2]);assert_eq!(p[3],64);
    }
    #[test] fn invalid_edit_and_locked_layer_are_atomic(){
        let mut d=Document::default();let id=d.add(sample(),Style::default());
        let before=d.clone();
        assert!(d.apply_raster_edit(id,RasterSettings{left:800,right:300,..Default::default()}).is_err());
        assert_eq!(d,before);
        d.layers[0].locked=true;let before=d.clone();
        assert!(d.apply_raster_edit(id,RasterSettings{invert:true,..Default::default()}).is_err());
        assert_eq!(d,before);
    }
    #[test] fn kdr_roundtrip_and_svg_use_derived_image(){
        let mut d=Document::default();let id=d.add(sample(),Style::default());
        d.apply_raster_edit(id,RasterSettings{invert:true,turns:1,..Default::default()}).unwrap();
        d.validate().unwrap();
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("photo.kdr");
        crate::kdr::save_kdr(&d,&path).unwrap();
        let reopened=crate::kdr::read_kdr(&path).unwrap();
        assert_eq!(reopened,d);
        let Shape::Raster{data_base64,edit,..}=&reopened.get(id).unwrap().shape else{panic!()};
        assert!(reopened.to_svg().contains(data_base64));
        assert!(!reopened.to_svg().contains(&edit.as_ref().unwrap().source_base64));
    }
    #[test] fn resampling_and_legacy_json(){
        let source=sample();
        let value=serde_json::to_value(&source).unwrap();
        assert!(value.get("edit").is_none());
        assert_eq!(serde_json::from_value::<Shape>(value).unwrap(),source);
        let out=adjusted_shape(&source,RasterSettings{max_side:2,..Default::default()}).unwrap();
        assert_eq!(pixels(&out).dimensions(),(2,1));
    }
}
