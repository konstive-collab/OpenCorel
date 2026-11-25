//! Non-destructive checks before proofing and printing. Not print certification.
use crate::{Document, Shape};
use std::collections::HashSet;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Severity { Error, Warning, Notice }
#[derive(Clone,Debug,PartialEq)]
pub struct Issue {
    pub severity:Severity,
    pub code:&'static str,
    pub object_id:Option<u64>,
    pub message:String,
}
#[derive(Clone,Copy,Debug)]
pub struct PreflightOptions {
    pub minimum_image_dpi:f32,
    pub safe_margin_mm:f32,
}
impl Default for PreflightOptions {
    fn default()->Self {Self{minimum_image_dpi:150.0,safe_margin_mm:5.0}}
}
/// Review the whole publication instead of silently checking only the
/// active page. Each result carries the corresponding stable page ID.
pub fn check_book(doc:&Document,opts:PreflightOptions)->Vec<(u64,Issue)>{
    if let Err(error)=doc.validate(){
        return vec![(doc.active_page_id,Issue{severity:Severity::Error,
            code:"INVALID_DOCUMENT",object_id:None,message:error})];
    }
    let mut found=Vec::new();
    for page in doc.pages(){
        let Ok(sheet)=doc.page_document(page.id) else{continue;};
        for issue in check(&sheet,opts){
            found.push((page.id,issue));
            if found.len()>=5000{
                found.push((page.id,Issue{severity:Severity::Warning,
                    code:"ISSUE_LIMIT",object_id:None,
                    message:"Достигнут лимит 5000 замечаний на документ".into()}));
                return found;
            }
        }
    }
    found
}

pub fn check(doc:&Document,opts:PreflightOptions)->Vec<Issue> {
    let mut issues=Vec::new();
    if let Err(e)=doc.validate(){
        issues.push(Issue{severity:Severity::Error,code:"INVALID_DOCUMENT",
            object_id:None,message:e});
        return issues;
    }
    if doc.objects.is_empty(){
        issues.push(Issue{severity:Severity::Warning,code:"EMPTY_PAGE",object_id:None,
            message:"Документ не содержит объектов".into()});
    }
    if doc.page.width_mm>5000.0 || doc.page.height_mm>5000.0 {
        issues.push(Issue{severity:Severity::Warning,code:"PDF_SIZE",
            object_id:None,
            message:"Предпросмотр PDF ограничен 5000 мм на сторону; используйте SVG или масштаб".into()});
    }
    let min_dpi=if opts.minimum_image_dpi.is_finite()&&opts.minimum_image_dpi>0.0{
        opts.minimum_image_dpi
    }else{150.0};
    let margin=if opts.safe_margin_mm.is_finite(){opts.safe_margin_mm.clamp(0.0,100.0)}
        else{5.0};
    let mut has_text=false;
    let mut has_embedded_vector=false;
    let visible:HashSet<u64>=doc.layers.iter().filter(|l|l.visible)
        .map(|l|l.id).collect();
    for obj in &doc.objects {
        if !visible.contains(&obj.layer_id) {continue;}
        let b=obj.bounds();
        if b.x<0.0||b.y<0.0||
            b.x+b.w>doc.page.width_mm||b.y+b.h>doc.page.height_mm {
            issues.push(Issue{severity:Severity::Warning,code:"OUTSIDE_PAGE",
                object_id:Some(obj.id),
                message:"Объект выходит за границу листа; проверьте обрезку и вылеты".into()});
        }else if (b.x<margin||b.y<margin||
            b.x+b.w>doc.page.width_mm-margin||
            b.y+b.h>doc.page.height_mm-margin)
            && !matches!(obj.shape,Shape::Rectangle{..}|Shape::Ellipse{..}) {
            issues.push(Issue{severity:Severity::Notice,code:"NEAR_EDGE",
                object_id:Some(obj.id),
                message:format!("Объект ближе {:.1} мм к краю листа",margin)});
        }
        match &obj.shape {
            Shape::Text{value,size_mm,..}=>{
                has_text=true;
                if value.trim().is_empty(){
                    issues.push(Issue{severity:Severity::Warning,code:"EMPTY_TEXT",
                        object_id:Some(obj.id),message:"Пустой текстовый объект".into()});
                }
                if *size_mm<1.0 {
                    issues.push(Issue{severity:Severity::Notice,code:"SMALL_TEXT",
                        object_id:Some(obj.id),
                        message:"Размер текста меньше 1 мм — проверьте читаемость".into()});
                }
            }
            Shape::Raster{w,h,pixel_width,pixel_height,..}=>{
                let xdpi=*pixel_width as f32*25.4/ *w;
                let ydpi=*pixel_height as f32*25.4/ *h;
                let dpi=xdpi.min(ydpi);
                if dpi<min_dpi{
                    issues.push(Issue{severity:Severity::Warning,code:"IMAGE_DPI",
                        object_id:Some(obj.id),
                        message:format!("Эффективное разрешение {:.0} DPI, ниже {:.0} DPI. Проверьте качество на печати.",dpi,min_dpi)});
                }
            }
            Shape::EmbeddedSvg{..}=>{has_embedded_vector=true;}
            _=>{}
        }
        if issues.len()>2000 {
            issues.push(Issue{severity:Severity::Warning,code:"ISSUE_LIMIT",
                object_id:None,
                message:"Показаны первые 2000 замечаний".into()});
            break;
        }
    }
    if has_text {
        issues.push(Issue{severity:Severity::Notice,code:"FONTS",
            object_id:None,
            message:"Шрифты не встроены в KDR: проверьте совпадение начертаний на другом компьютере".into()});
    }
    if has_embedded_vector {
        issues.push(Issue{severity:Severity::Notice,code:"PLACED_VECTOR",
            object_id:None,
            message:"Конвертированные векторные файлы могут содержать заменённые шрифты и неточные цвета".into()});
    }
    issues.push(Issue{severity:Severity::Notice,code:"COLOR",
        object_id:None,
        message:"Экспорт выполняется в RGB; ICC/CMYK/PDF/X пока не сертифицированы".into()});
    issues
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Shape,Style};
    #[test]
    fn warn_on_low_resolution_and_clipped_artwork(){
        let mut doc=Document::default();
        doc.page.width_mm=100.0;doc.page.height_mm=100.0;
        doc.add(Shape::Raster{x:80.0,y:80.0,w:100.0,h:100.0,
            pixel_width:100,pixel_height:100,mime:"image/png".into(),
            data_base64:base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                b"\x89PNG\r\n\x1a\nfake"),edit:None},Style::default());
        let issues=check(&doc,PreflightOptions::default());
        assert!(issues.iter().any(|i|i.code=="IMAGE_DPI"));
        assert!(issues.iter().any(|i|i.code=="OUTSIDE_PAGE"));
    }
    #[test]
    fn book_preflight_reports_issues_from_every_page(){
        let mut doc=Document::default();
        doc.page.width_mm=100.0;doc.page.height_mm=100.0;
        doc.add(Shape::Rectangle{x:101.,y:5.,w:20.,h:10.},Style::default());
        let second=doc.add_page(crate::Page{width_mm:100.,height_mm:100.}).unwrap();
        doc.add(Shape::Text{x:1.,y:1.,value:String::new(),size_mm:5.},Style::default());
        let found=check_book(&doc,PreflightOptions::default());
        assert!(found.iter().any(|(id,issue)|*id==1&&issue.code=="OUTSIDE_PAGE"));
        assert!(found.iter().any(|(id,issue)|*id==second&&issue.code=="EMPTY_TEXT"));
    }
    #[test]
    fn empty_document_is_not_marked_print_ready(){
        let doc=Document::default();
        let issues=check(&doc,PreflightOptions::default());
        assert!(issues.iter().any(|i|i.code=="EMPTY_PAGE"));
        assert!(issues.iter().any(|i|i.code=="COLOR"));
    }
    #[test]
    fn no_false_warnings_for_high_dpi_image(){
        let mut doc=Document::default();
        doc.add(Shape::Raster{x:30.0,y:30.0,w:10.0,h:10.0,
            pixel_width:300,pixel_height:300,mime:"image/png".into(),
            data_base64:base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                b"\x89PNG\r\n\x1a\nfake"),edit:None},Style::default());
        let issues=check(&doc,PreflightOptions::default());
        assert!(!issues.iter().any(|i|i.code=="IMAGE_DPI"));
    }
}
