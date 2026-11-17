//! Generate a native, editable four-page design reference catalogue.
//! Every preset can be copied, recolored and edited with F10 afterwards.
//! This is useful as a visual picker and as a user-verifiable geometry QA
//! fixture, not as a raster preview or unrelated splash screen.
use super::Editor;
use opencorel_core::{Document,Shape,Style,Bounds};
use opencorel_core::smart_shapes::{self,Category,SmartShape,Options};

const PER_PAGE:usize=24;
const A4_WIDTH:f32=210.0;
const A4_HEIGHT:f32=297.0;
const COLUMNS:usize=4;
const CELL_W:f32=47.0;
const CELL_H:f32=42.0;

fn text_style()->Style{
    Style{fill:"#283446".into(),stroke:"none".into(),stroke_width_mm:0.0}
}
fn color_for(category:Category)->&'static str{
    match category{
        Category::Geometry=>"#1D5D78",
        Category::Stars=>"#A06624",
        Category::Arrows=>"#2D745E",
        Category::Symbols=>"#943E75",
        Category::Flowcharts=>"#3F5488",
        Category::Technical=>"#7A4557",
    }
}
fn wrap_label(s:&str,max_chars:usize)->String{
    let mut out=String::new();
    let mut line=String::new();
    for word in s.split_whitespace(){
        let count=word.chars().count();
        if line.chars().count()+usize::from(!line.is_empty())+count>max_chars
            && !line.is_empty(){
            if !out.is_empty(){out.push('\n');}
            out.push_str(&line);
            line.clear();
        }
        if !line.is_empty(){line.push(' ');}
        if count>max_chars{
            for (i,c) in word.chars().enumerate(){
                if i>0&&i%max_chars==0{
                    if !out.is_empty(){out.push('\n');}
                    out.push_str(&line);
                    line.clear();
                }
                line.push(c);
            }
        }else{line.push_str(word);}
    }
    if !line.is_empty(){
        if !out.is_empty(){out.push('\n');}
        out.push_str(&line);
    }
    out
}
fn build_catalog_on(doc:&mut Document)->Result<usize,String>{
    let catalog_pages=SmartShape::ALL.len().div_ceil(PER_PAGE);
    if doc.page_count().saturating_add(catalog_pages)>64{
        return Err("Недостаточно свободных страниц для библиотеки (нужны четыре)".into());
    }
    let mut first=None;
    for page_number in 0..catalog_pages{
        let index=doc.append_page(false)?;
        first.get_or_insert(index);
        doc.page.width_mm=A4_WIDTH;
        doc.page.height_mm=A4_HEIGHT;
        doc.rename_page(index,&format!("Каталог фигур {}/{}",page_number+1,catalog_pages))?;
        doc.add(Shape::Text{x:10.0,y:7.0,
            value:format!("OpenCorel · Фигуры {}/{}",page_number+1,catalog_pages),
            size_mm:5.5},text_style());
        doc.add(Shape::Text{x:10.0,y:17.0,
            value:"Нативные векторы · выделите и копируйте любые символы".into(),
            size_mm:3.2},text_style());
        let base=page_number*PER_PAGE;
        let count=(SmartShape::ALL.len()-base).min(PER_PAGE);
        for i in 0..count{
            let kind=SmartShape::ALL[base+i];
            let col=i%COLUMNS;
            let row=i/COLUMNS;
            let left=11.0+col as f32*CELL_W;
            let top=29.0+row as f32*CELL_H;
            let rect=Bounds{x:left+11.0,y:top+1.0,w:24.0,h:22.0};
            let shape=smart_shapes::create(kind,rect,Options::default())?;
            let color=color_for(kind.category());
            let style=if kind.is_open(){
                Style{fill:"none".into(),stroke:color.into(),stroke_width_mm:0.8}
            }else{
                Style{fill:color.into(),stroke:"none".into(),stroke_width_mm:0.0}
            };
            let id=doc.add(shape,style);
            if let Some(obj)=doc.get_mut(id){
                obj.name=kind.label().into();
            }
            doc.add(Shape::Text{x:left+1.0,y:top+26.0,
                value:wrap_label(kind.label(),23),size_mm:3.0},text_style());
        }
    }
    let page=first.ok_or("No catalog pages generated")?;
    doc.switch_page(page)?;
    doc.validate()?;
    Ok(catalog_pages)
}
impl Editor{
    /// Build the entire visual library atomically on separate A4 pages.
    /// Existing artwork stays untouched and the operation uses one Ctrl+Z.
    pub(super) fn create_smart_catalog(&mut self){
        let mut candidate=self.doc.clone();
        match build_catalog_on(&mut candidate){
            Ok(pages)=>{
                self.accept_page_change(candidate,
                    format!("Создан каталог из {} профессиональных фигур на {} листах A4. Копируйте и редактируйте объекты. Ctrl+Z отменяет всё.",
                        SmartShape::ALL.len(),pages));
            },
            Err(e)=>self.status=format!("Каталог фигур: {e}"),
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn generated_catalog_has_every_preset_in_four_undoable_pages(){
        let mut e=Editor::default();
        e.doc=Document::default();
        let id=e.doc.add(Shape::Rectangle{x:1.,y:2.,w:4.,h:5.},
            Style::default());
        let original=e.doc.clone();
        e.create_smart_catalog();
        assert_eq!(e.doc.page_count(),5);
        assert_eq!(e.doc.active_page_index,1);
        assert_eq!(e.undo.len(),1);
        let mut shape_count=0;
        for i in 1..5{
            let page=e.doc.page_document(i).unwrap();
            assert_eq!(page.page.width_mm,A4_WIDTH);
            assert_eq!(page.page.height_mm,A4_HEIGHT);
            shape_count+=page.objects.iter().filter(|o|
                !matches!(o.shape,Shape::Text{..})).count();
            page.validate().unwrap();
        }
        assert_eq!(shape_count,93);
        assert_eq!(e.doc.page_document(0).unwrap().objects[0].id,id);
        e.undo();
        assert_eq!(e.doc,original);
    }
    #[test]
    fn rejects_catalog_if_document_cannot_add_four_pages(){
        let mut doc=Document::default();
        for _ in 0..60{doc.append_page(false).unwrap();}
        assert_eq!(doc.page_count(),61);
        assert!(build_catalog_on(&mut doc).is_err());
    }
    #[test]
    fn labels_wrap_without_breaking_unicode_bytes(){
        let wrapped=wrap_label("Профессиональная стрелка двойная",12);
        assert!(wrapped.contains('\n'));
        assert_eq!(wrapped.replace('\n'," ").split_whitespace()
            .collect::<Vec<_>>().join(" "),
            "Профессиональная стрелка двойная");
    }
}
