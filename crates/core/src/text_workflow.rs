//! Unicode-aware text search, atomic document-wide replacement and casing.
//! Works across all editable KDR pages while preserving object IDs, typography,
//! fill, groups and layer assignments. Binary/raster artwork remains untouched.
use crate::{Document,Object,Shape};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum TextCase{Upper,Lower,Title,Sentence}
impl TextCase{
    pub fn label(self)->&'static str{match self{
        Self::Upper=>"ПРОПИСНЫЕ", Self::Lower=>"строчные",
        Self::Title=>"Каждое Слово",Self::Sentence=>"Начало предложений"
    }}
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct TextHit{
    pub page_id:u64,
    pub object_id:u64,
    pub occurrences:usize,
    pub preview:String,
}
fn validate_query(query:&str)->Result<(),String>{
    if query.is_empty()||query.len()>256||!crate::valid_xml_text(query){
        return Err("Search text must contain 1–256 valid UTF-8 bytes".into());
    }
    Ok(())
}
fn validate_replacement(value:&str)->Result<(),String>{
    if value.len()>1024||!crate::valid_xml_text(value){
        return Err("Replacement must contain at most 1024 valid UTF-8 bytes".into());
    }
    Ok(())
}
/// Unicode-case-aware matching without unsafe byte offsets. For case-insensitive
/// replacement, compare character windows so that returned spans always align
/// with UTF-8 scalar boundaries (including non-ASCII Cyrillic characters).
fn edit_string(value:&str,query:&str,replacement:&str,case_sensitive:bool)
    ->(String,usize){
    if case_sensitive{
        let count=value.matches(query).count();
        if count==0{return (value.to_owned(),0);}
        return (value.replace(query,replacement),count);
    }
    let query_lower=query.to_lowercase();
    let chars=query.chars().count();
    let mut out=String::with_capacity(value.len());
    let mut remaining=value;
    let mut hits=0;
    while !remaining.is_empty(){
        // Avoid repeated expensive allocation for short remaining tails.
        let candidate:String=remaining.chars().take(chars).collect();
        if candidate.chars().count()==chars
            &&candidate.to_lowercase()==query_lower{
            out.push_str(replacement);
            remaining=&remaining[candidate.len()..];
            hits+=1;
        }else{
            let first=remaining.chars().next().expect("Nonempty text has a char");
            out.push(first);
            remaining=&remaining[first.len_utf8()..];
        }
    }
    (out,hits)
}
fn make_case(value:&str,mode:TextCase)->String{
    match mode{
        TextCase::Upper=>value.to_uppercase(),
        TextCase::Lower=>value.to_lowercase(),
        TextCase::Title=>{
            let mut word_start=true;
            let mut out=String::with_capacity(value.len());
            for c in value.chars(){
                if c.is_alphabetic(){
                    if word_start{out.extend(c.to_uppercase());}
                    else{out.extend(c.to_lowercase());}
                    word_start=false;
                }else{
                    out.push(c);
                    if !c.is_numeric(){word_start=true;}
                }
            }
            out
        }
        TextCase::Sentence=>{
            let mut sentence_start=true;
            let mut out=String::with_capacity(value.len());
            for c in value.chars(){
                if sentence_start&&c.is_alphabetic(){
                    out.extend(c.to_uppercase());sentence_start=false;
                }else{out.push(c);}
                if matches!(c,'.'|'!'|'?'|'\n'){sentence_start=true;}
            }
            out
        }
    }
}
impl Document{
    /// Find text on the active page or throughout all 64 page states.
    /// Does not copy, switch or mutate any page. Limits visual results.
    pub fn find_text(&self,query:&str,case_sensitive:bool,all_pages:bool)
        ->Result<Vec<TextHit>,String>{
        validate_query(query)?;
        let mut result=Vec::new();
        let mut scan=|id:u64,objects:&[Object]|{
            for object in objects{
                let Shape::Text{value,..}=&object.shape else{continue;};
                let (_,hits)=edit_string(value,query,"",case_sensitive);
                if hits>0{
                    result.push(TextHit{page_id:id,object_id:object.id,
                        occurrences:hits,
                        preview:value.chars().take(120).collect()});
                    if result.len()==5000{break;}
                }
            }
        };
        scan(self.active_page_id,&self.objects);
        if all_pages&&result.len()<5000{
            for (id,page) in &self.inactive_pages{
                scan(*id,&page.objects);
                if result.len()==5000{break;}
            }
        }
        // Keep the same page order displayed in the Page navigator.
        let order=self.pages();
        result.sort_by_key(|hit|order.iter().position(|p|p.id==hit.page_id)
            .unwrap_or(usize::MAX));
        Ok(result)
    }
    /// Atomically replace text across one or all pages. A locked/hidden hit,
    /// invalid value or overlong resulting text aborts the whole operation.
    pub fn replace_text(&mut self,query:&str,replacement:&str,
        case_sensitive:bool,all_pages:bool)->Result<usize,String>{
        validate_query(query)?;
        validate_replacement(replacement)?;
        let mut planned=Vec::<(u64,u64,String)>::new();
        let mut total=0usize;
        let mut scan=|id:u64,objects:&[Object],layers:&[crate::Layer]|
            ->Result<(),String>{
            for object in objects{
                let Shape::Text{value,..}=&object.shape else{continue;};
                let (next,count)=edit_string(value,query,replacement,case_sensitive);
                if count==0{continue;}
                if layers.iter().find(|l|l.id==object.layer_id)
                    .is_none_or(|l|!l.visible||l.locked){
                    return Err(format!("Text object #{} belongs to a hidden or locked layer",
                        object.id));
                }
                if next.len()>1_000_000||!crate::valid_xml_text(&next){
                    return Err("Replacement produces invalid or oversized text".into());
                }
                total=total.checked_add(count).ok_or("Too many replacements")?;
                if total>100_000{return Err("Maximum 100,000 replacements".into());}
                planned.push((id,object.id,next));
            }
            Ok(())
        };
        scan(self.active_page_id,&self.objects,&self.layers)?;
        if all_pages{
            for (id,page) in &self.inactive_pages{
                scan(*id,&page.objects,&page.layers)?;
            }
        }
        // Every affected object was prevalidated; apply only after the whole
        // scan succeeds. Existing object IDs and appearances stay unchanged.
        for (page_id,object_id,next) in planned{
            let objects=if page_id==self.active_page_id{
                &mut self.objects
            }else{
                &mut self.inactive_pages.get_mut(&page_id)
                    .ok_or("Missing inactive page")?.objects
            };
            let obj=objects.iter_mut().find(|o|o.id==object_id)
                .ok_or("Text object disappeared before replacement")?;
            if let Shape::Text{value,..}=&mut obj.shape{*value=next;}
        }
        Ok(total)
    }
    /// Change case on selected editable text objects. Ordinary vectors and
    /// bitmaps in the selection are intentionally ignored.
    pub fn transform_text_case(&mut self,ids:&[u64],mode:TextCase)
        ->Result<usize,String>{
        let chosen:std::collections::HashSet<_>=ids.iter().copied().collect();
        let mut edits=Vec::new();
        for obj in &self.objects{
            if !chosen.contains(&obj.id){continue;}
            let Shape::Text{value,..}=&obj.shape else{continue;};
            if self.layer(obj.layer_id).is_none_or(|l|!l.visible||l.locked){
                return Err("Selected text is on a hidden or locked layer".into());
            }
            let next=make_case(value,mode);
            if next.len()>1_000_000||!crate::valid_xml_text(&next){
                return Err("Case transformation exceeds text limits".into());
            }
            if next!=*value{edits.push((obj.id,next));}
        }
        let count=edits.len();
        for (id,next) in edits{
            if let Some(obj)=self.get_mut(id){
                if let Shape::Text{value,..}=&mut obj.shape{*value=next;}
            }
        }
        Ok(count)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Page,Shape,Style};
    #[test] fn unicode_search_and_replace_keep_object_style(){
        let mut doc=Document::default();
        let a=doc.add(Shape::Text{x:1.,y:2.,size_mm:7.,
            value:"Привет Мир! привет мир".into()},Style::default());
        let before=doc.get(a).unwrap().style.clone();
        let hits=doc.find_text("ПРИВЕТ",false,false).unwrap();
        assert_eq!(hits.len(),1);
        assert_eq!(hits[0].occurrences,2);
        assert_eq!(doc.replace_text("привет","Здравствуйте",false,false).unwrap(),2);
        let Shape::Text{value,..}=&doc.get(a).unwrap().shape else{panic!()};
        assert_eq!(value,"Здравствуйте Мир! Здравствуйте мир");
        assert_eq!(doc.get(a).unwrap().style,before);
        doc.validate().unwrap();
    }
    #[test] fn publication_search_and_replace_preserve_active_page(){
        let mut doc=Document::default();
        doc.add(Shape::Text{x:1.,y:2.,size_mm:6.,value:"cat".into()},
            Style::default());
        let second=doc.add_page(Page{width_mm:210.,height_mm:297.}).unwrap();
        doc.add(Shape::Text{x:2.,y:3.,size_mm:6.,value:"CAT".into()},
            Style::default());
        let active=doc.active_page_id;
        assert_eq!(doc.find_text("cat",false,true).unwrap().len(),2);
        assert_eq!(doc.replace_text("CAT","dog",false,true).unwrap(),2);
        assert_eq!(doc.active_page_id,active);
        doc.switch_page(1).unwrap();
        let Shape::Text{value,..}=&doc.objects[0].shape else{panic!()};
        assert_eq!(value,"dog");
        doc.switch_page(second).unwrap();
        doc.validate().unwrap();
    }
    #[test] fn locked_page_hit_aborts_all_replacements(){
        let mut doc=Document::default();
        let first=doc.add(Shape::Text{x:1.,y:2.,size_mm:6.,value:"old".into()},
            Style::default());
        doc.add_page(Page{width_mm:210.,height_mm:297.}).unwrap();
        doc.add(Shape::Text{x:1.,y:2.,size_mm:6.,value:"old".into()},
            Style::default());
        doc.layers[0].locked=true;
        let before=doc.clone();
        assert!(doc.replace_text("old","new",true,true).is_err());
        assert_eq!(doc,before);
        assert!(doc.get(first).is_none());
    }
    #[test] fn title_and_sentence_preserve_unicode_and_punctuation(){
        assert_eq!(make_case("сЛОво  кОТ",TextCase::Title),"Слово  Кот");
        assert_eq!(make_case("привет. мир!\nновый",TextCase::Sentence),
            "Привет. Мир!\nНовый");
    }
}
