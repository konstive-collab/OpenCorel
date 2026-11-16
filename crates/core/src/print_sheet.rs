//! Print-sheet SVG export with real outer bleed area, optional crop marks,
//! and an independent printable trim boundary. Original document is untouched.
//! This exporter intentionally does not promise CMYK or PDF/X.
use crate::Document;
use std::{io,path::Path};

#[derive(Clone,Copy,Debug,PartialEq)]
pub struct PrintSheetOptions{
    /// Additional visible artwork area outside the trim rectangle.
    pub bleed_mm:f32,
    /// Adds black crop marks outside the bleed area.
    pub crop_marks:bool,
}
impl Default for PrintSheetOptions{
    fn default()->Self{Self{bleed_mm:3.0,crop_marks:true}}
}
impl PrintSheetOptions{
    pub fn validate(self)->Result<(),String>{
        if !self.bleed_mm.is_finite() || !(0.0..=25.0).contains(&self.bleed_mm){
            return Err("Bleed must be from 0 to 25 mm".into());
        }
        Ok(())
    }
}

/// Renders a larger SVG artwork sheet with the original page placed at the
/// actual trim coordinates. Elements outside the document page become visible
/// in the bleed band if they were drawn there; no artificial edge stretching.
/// Crop marks begin outside the bleed, so they never cut into printed art.
pub fn to_print_svg(doc:&Document,options:PrintSheetOptions)->Result<String,String>{
    doc.validate()?;
    options.validate()?;
    let mark_len=if options.crop_marks{5.0}else{0.0};
    let mark_gap=if options.crop_marks{2.0}else{0.0};
    let margin=options.bleed_mm+mark_len+mark_gap;
    let outer_w=doc.page.width_mm+margin*2.0;
    let outer_h=doc.page.height_mm+margin*2.0;
    if outer_w>100_000.0||outer_h>100_000.0 ||
        !outer_w.is_finite()||!outer_h.is_finite(){
        return Err("Print sheet exceeds permitted dimensions".into());
    }
    let original=doc.to_svg();
    let header=original.find('>').ok_or("SVG root missing")?;
    let end=original.rfind("</svg>").ok_or("SVG closing tag missing")?;
    if end<=header{return Err("SVG structure invalid".into());}
    let body=&original[header+1..end];
    let trim_x=margin;
    let trim_y=margin;
    let right=trim_x+doc.page.width_mm;
    let bottom=trim_y+doc.page.height_mm;
    let mut output=format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{outer_w}mm\" height=\"{outer_h}mm\" viewBox=\"0 0 {outer_w} {outer_h}\">\n"
    );
    // The artwork is NOT clipped at the trim edge. Any geometry extending
    // outside the document remains present in the allowed bleed zone.
    // Restrict only to the printable bleed rectangle to keep crop marks clear.
    let bleed_x=trim_x-options.bleed_mm;
    let bleed_y=trim_y-options.bleed_mm;
    let bleed_w=doc.page.width_mm+options.bleed_mm*2.0;
    let bleed_h=doc.page.height_mm+options.bleed_mm*2.0;
    output.push_str(&format!(
        "<defs><clipPath id=\"oc-print-artwork-window\" clipPathUnits=\"userSpaceOnUse\"><rect x=\"{bleed_x}\" y=\"{bleed_y}\" width=\"{bleed_w}\" height=\"{bleed_h}\"/></clipPath></defs>\n"
    ));
    output.push_str(&format!(
        "<g clip-path=\"url(#oc-print-artwork-window)\"><g transform=\"translate({trim_x} {trim_y})\">{body}</g></g>\n"
    ));
    if options.crop_marks{
        let left_start=bleed_x-mark_gap;
        let left_end=left_start-mark_len;
        let right_start=right+options.bleed_mm+mark_gap;
        let right_end=right_start+mark_len;
        let top_start=bleed_y-mark_gap;
        let top_end=top_start-mark_len;
        let bottom_start=bottom+options.bleed_mm+mark_gap;
        let bottom_end=bottom_start+mark_len;
        output.push_str("<g id=\"oc-crop-marks\" fill=\"none\" stroke=\"#000000\" stroke-width=\"0.2\">\n");
        for y in [trim_y,bottom]{
            output.push_str(&format!(
                "<path d=\"M {left_end} {y} H {left_start} M {right_start} {y} H {right_end}\"/>\n"
            ));
        }
        for x in [trim_x,right]{
            output.push_str(&format!(
                "<path d=\"M {x} {top_end} V {top_start} M {x} {bottom_start} V {bottom_end}\"/>\n"
            ));
        }
        output.push_str("</g>\n");
    }
    output.push_str("</svg>\n");
    Ok(output)
}

/// Atomic write: existing output is never replaced if validation or rendering fails.
pub fn export_print_svg(doc:&Document,path:impl AsRef<Path>,
    options:PrintSheetOptions)->io::Result<()>{
    let markup=to_print_svg(doc,options).map_err(io::Error::other)?;
    crate::atomic_write(path.as_ref(),markup.as_bytes())
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Shape,Style};
    fn fixture()->Document{
        let mut d=Document::default();
        d.name="Print test".into();
        d.page.width_mm=90.;d.page.height_mm=50.;
        d.add(Shape::Rectangle{x:-2.,y:-2.,w:94.,h:54.},Style::default());
        d
    }
    #[test]
    fn expanded_svg_preserves_art_and_places_crop_marks_outside_bleed(){
        let d=fixture();
        let original=d.clone();
        let result=to_print_svg(&d,PrintSheetOptions::default()).unwrap();
        assert!(result.contains("width=\"110mm\" height=\"70mm\""));
        assert!(result.contains("translate(10 10)"));
        assert!(result.contains("id=\"oc-crop-marks\""));
        assert!(result.contains("M 0 10 H 5"));
        assert!(result.contains("M 105 10 H 110"));
        assert!(result.contains("oc-print-artwork-window"));
        assert_eq!(d,original);
    }
    #[test]
    fn trim_only_sheet_has_no_crop_marks_or_extra_margins(){
        let d=fixture();
        let result=to_print_svg(&d,PrintSheetOptions{
            bleed_mm:0.,crop_marks:false}).unwrap();
        assert!(result.contains("width=\"90mm\" height=\"50mm\""));
        assert!(!result.contains("id=\"oc-crop-marks\""));
    }
    #[test]
    fn invalid_bleed_is_rejected_without_changes(){
        let d=fixture();
        for value in [f32::NAN,f32::INFINITY,-1.,25.01]{
            assert!(to_print_svg(&d,PrintSheetOptions{
                bleed_mm:value,crop_marks:true}).is_err());
        }
    }
    #[test]
    fn output_escapes_title_using_existing_svg_serializer(){
        let mut d=fixture();
        d.name="A & B".into();
        let svg=to_print_svg(&d,PrintSheetOptions::default()).unwrap();
        assert!(svg.contains("<title>A &amp; B</title>"));
    }
}
