//! Unicode-aware, batch-safe editable-text production tools.
//! Every operation creates ordinary editable OpenCorel text, not raster art.
//! Text actions use one undo checkpoint and validate the whole document first.
use super::Editor;
use eframe::egui::{self,RichText};
use opencorel_core::{Document,Shape,TextAlign};
use std::collections::HashSet;

const MAX_BYTES:usize=256*1024;
const MAX_EDIT_OBJECTS:usize=2500;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) enum TextAction{
    Upper,Lower,Title,Sentence,InvertCase,CapitalizeLines,
    TrimText,TrimLines,CollapseSpaces,RemoveEmptyLines,
    SortLines,SortLinesInsensitive,UniqueLines,ReverseLines,
    NumberLines,BulletLines,DashLines,QuoteLines,
    JoinLines,IndentLines,OutdentLines,TabsToSpaces,HardWrap,
}
impl TextAction{
    pub const ALL:[Self;23]=[
        Self::Upper,Self::Lower,Self::Title,Self::Sentence,
        Self::InvertCase,Self::CapitalizeLines,Self::TrimText,
        Self::TrimLines,Self::CollapseSpaces,Self::RemoveEmptyLines,
        Self::SortLines,Self::SortLinesInsensitive,Self::UniqueLines,
        Self::ReverseLines,Self::NumberLines,Self::BulletLines,
        Self::DashLines,Self::QuoteLines,Self::JoinLines,
        Self::IndentLines,Self::OutdentLines,Self::TabsToSpaces,
        Self::HardWrap,
    ];
    pub fn label(self)->&'static str{
        match self{
            Self::Upper=>"В ВЕРХНИЙ РЕГИСТР",Self::Lower=>"в нижний регистр",
            Self::Title=>"Каждое Слово С Заглавной",
            Self::Sentence=>"Предложения с заглавной",
            Self::InvertCase=>"Инвертировать регистр",
            Self::CapitalizeLines=>"Заглавная буква каждой строки",
            Self::TrimText=>"Убрать пробелы по краям текста",
            Self::TrimLines=>"Обрезать пробелы строк",
            Self::CollapseSpaces=>"Один пробел между словами",
            Self::RemoveEmptyLines=>"Удалить пустые строки",
            Self::SortLines=>"Сортировать строки A→Я",
            Self::SortLinesInsensitive=>"Сортировать без учёта регистра",
            Self::UniqueLines=>"Убрать дубли строк",
            Self::ReverseLines=>"Обратить порядок строк",
            Self::NumberLines=>"Нумеровать непустые строки",
            Self::BulletLines=>"Маркеры •",
            Self::DashLines=>"Список с тире",
            Self::QuoteLines=>"Цитировать строки >",
            Self::JoinLines=>"Соединить строки в абзац",
            Self::IndentLines=>"Увеличить отступ строк",
            Self::OutdentLines=>"Уменьшить отступ строк",
            Self::TabsToSpaces=>"Заменить табуляцию пробелами",
            Self::HardWrap=>"Переносить по числу символов",
        }
    }
    pub fn help(self)->&'static str{
        match self{
            Self::HardWrap=>"Перенос по количеству Unicode-символов, не по реальной ширине шрифта",
            Self::Title=>"Первую букву каждого слова делает прописной, остальные — строчными",
            Self::Sentence=>"Предложения после . ! ? и перевода строки начинаютcя с заглавной",
            Self::UniqueLines=>"Оставляет первое вхождение, сравнение строк точное",
            Self::NumberLines=>"Не нумерует пустые строки; начинает счёт с 1",
            Self::JoinLines=>"Заменяет внутренние переводы строк на одиночные пробелы",
            _=>"Применяется ко всем выделенным текстовым объектам как одно действие Ctrl+Z",
        }
    }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) enum TextPreset{
    Display,Heading,Subheading,Body,Caption,Technical,Quote,
}
impl TextPreset{
    const ALL:[Self;7]=[Self::Display,Self::Heading,Self::Subheading,
        Self::Body,Self::Caption,Self::Technical,Self::Quote];
    fn label(self)->&'static str{
        match self{
            Self::Display=>"Крупный заголовок",
            Self::Heading=>"Заголовок",
            Self::Subheading=>"Подзаголовок",
            Self::Body=>"Основной текст",
            Self::Caption=>"Подпись",
            Self::Technical=>"Техническая метка",
            Self::Quote=>"Цитата",
        }
    }
    fn specs(self)->(f32,u16,bool,f32,f32,TextAlign){
        match self{
            Self::Display=>(22.,700,false,0.2,1.10,TextAlign::Left),
            Self::Heading=>(15.,700,false,0.1,1.12,TextAlign::Left),
            Self::Subheading=>(9.,600,false,0.05,1.18,TextAlign::Left),
            Self::Body=>(5.,400,false,0.0,1.4,TextAlign::Left),
            Self::Caption=>(3.5,400,false,0.05,1.2,TextAlign::Left),
            Self::Technical=>(3.,600,false,0.2,1.1,TextAlign::Left),
            Self::Quote=>(7.,400,true,0.0,1.35,TextAlign::Left),
        }
    }
}

/// Only ASCII horizontal spaces are collapsed; Unicode words remain intact
/// and hard paragraph boundaries are preserved.
fn collapse_spaces(value:&str)->String{
    let mut result=String::with_capacity(value.len());
    for (line_index,line) in value.split('\n').enumerate(){
        if line_index>0{result.push('\n');}
        let mut previous_space=false;
        for ch in line.chars(){
            if matches!(ch,' '| '\t'){
                if !previous_space{result.push(' ');}
                previous_space=true;
            }else{
                result.push(ch);previous_space=false;
            }
        }
    }
    result
}
fn title_case(value:&str)->String{
    let mut result=String::with_capacity(value.len());
    let mut boundary=true;
    for ch in value.chars(){
        if ch.is_alphanumeric(){
            if boundary{result.extend(ch.to_uppercase());}
            else{result.extend(ch.to_lowercase());}
            boundary=false;
        }else{result.push(ch);boundary=true;}
    }
    result
}
fn sentence_case(value:&str)->String{
    let mut result=String::with_capacity(value.len());
    let mut boundary=true;
    for ch in value.chars(){
        if ch.is_alphabetic(){
            if boundary{result.extend(ch.to_uppercase());boundary=false;}
            else{result.extend(ch.to_lowercase());}
        }else{
            result.push(ch);
            if matches!(ch,'.'|'?'|'!'|'\n'){boundary=true;}
        }
    }
    result
}
fn capitalize_lines(value:&str)->String{
    value.split('\n').map(|line|{
        let mut started=false;
        line.chars().flat_map(|ch|{
            if !started&&ch.is_alphabetic(){
                started=true;ch.to_uppercase().collect::<Vec<_>>()
            }else{vec![ch]}
        }).collect::<String>()
    }).collect::<Vec<_>>().join("\n")
}
fn reverse_case(value:&str)->String{
    value.chars().flat_map(|c|{
        if c.is_lowercase(){c.to_uppercase().collect::<Vec<_>>()}
        else if c.is_uppercase(){c.to_lowercase().collect::<Vec<_>>()}
        else{vec![c]}
    }).collect()
}
fn wrap_line(text:&str,max_chars:usize)->String{
    let mut result=String::new();
    let mut position=0usize;
    for word in text.split_whitespace(){
        let mut part=word;
        while !part.is_empty(){
            let length=part.chars().count();
            if position>0 && position+1+length<=max_chars{
                result.push(' ');result.push_str(part);
                position+=1+length;break;
            }
            if position>0{
                result.push('\n');position=0;
                continue;
            }
            if length<=max_chars{
                result.push_str(part);position=length;break;
            }
            let end=part.char_indices().nth(max_chars)
                .map_or(part.len(),|(i,_)|i);
            result.push_str(&part[..end]);
            result.push('\n');
            part=&part[end..];
        }
    }
    result
}
fn apply_action(value:&str,action:TextAction,wrap_columns:usize,
    indent_spaces:usize)->Result<String,String>{
    if value.len()>MAX_BYTES{return Err("Текст превышает предел пакетной обработки".into());}
    if !(8..=160).contains(&wrap_columns) || !(1..=12).contains(&indent_spaces){
        return Err("Неверные параметры типографики".into());
    }
    let result=match action{
        TextAction::Upper=>value.to_uppercase(),
        TextAction::Lower=>value.to_lowercase(),
        TextAction::Title=>title_case(value),
        TextAction::Sentence=>sentence_case(value),
        TextAction::InvertCase=>reverse_case(value),
        TextAction::CapitalizeLines=>capitalize_lines(value),
        TextAction::TrimText=>value.trim().into(),
        TextAction::TrimLines=>value.lines().map(str::trim)
            .collect::<Vec<_>>().join("\n"),
        TextAction::CollapseSpaces=>collapse_spaces(value),
        TextAction::RemoveEmptyLines=>value.lines()
            .filter(|l|!l.trim().is_empty()).collect::<Vec<_>>().join("\n"),
        TextAction::SortLines=>{
            let mut lines=value.split('\n').collect::<Vec<_>>();
            lines.sort_unstable();lines.join("\n")
        }
        TextAction::SortLinesInsensitive=>{
            let mut lines=value.split('\n').collect::<Vec<_>>();
            lines.sort_by_cached_key(|l|l.to_lowercase());lines.join("\n")
        }
        TextAction::UniqueLines=>{
            let mut seen=HashSet::new();
            value.split('\n').filter(|line|seen.insert(*line))
                .collect::<Vec<_>>().join("\n")
        }
        TextAction::ReverseLines=>value.split('\n').rev()
            .collect::<Vec<_>>().join("\n"),
        TextAction::NumberLines=>{
            let mut n=0usize;
            value.split('\n').map(|line|{
                if line.trim().is_empty(){return String::new();}
                n+=1;format!("{n}. {line}")
            }).collect::<Vec<_>>().join("\n")
        }
        TextAction::BulletLines=>value.split('\n')
            .map(|line|if line.trim().is_empty(){String::new()}
                else{format!("• {line}")})
            .collect::<Vec<_>>().join("\n"),
        TextAction::DashLines=>value.split('\n')
            .map(|line|if line.trim().is_empty(){String::new()}
                else{format!("— {line}")})
            .collect::<Vec<_>>().join("\n"),
        TextAction::QuoteLines=>value.split('\n')
            .map(|line|format!("> {line}")).collect::<Vec<_>>().join("\n"),
        TextAction::JoinLines=>collapse_spaces(&value.split('\n')
            .map(str::trim).filter(|line|!line.is_empty())
            .collect::<Vec<_>>().join(" ")),
        TextAction::IndentLines=>{
            let prefix=" ".repeat(indent_spaces);
            value.split('\n').map(|line|format!("{prefix}{line}"))
                .collect::<Vec<_>>().join("\n")
        }
        TextAction::OutdentLines=>{
            value.split('\n').map(|line|{
                let spaces=line.as_bytes().iter().take(indent_spaces)
                    .take_while(|b|**b==b' ').count();
                &line[spaces..]
            }).collect::<Vec<_>>().join("\n")
        }
        TextAction::TabsToSpaces=>value.replace('\t',&" ".repeat(indent_spaces)),
        TextAction::HardWrap=>value.split('\n').map(|line|
            wrap_line(line,wrap_columns)).collect::<Vec<_>>().join("\n"),
    };
    if result.len()>MAX_BYTES || result.chars().any(|c|matches!(c,'\0'|'\u{0001}'..='\u{0008}'|'\u{000B}'..='\u{001F}')){
        return Err("Результат слишком велик или содержит недопустимые символы".into());
    }
    Ok(result)
}
/// Transform whole selection as one operation. Locked layers fail closed.
fn text_ids(doc:&Document,ids:&[u64])->Result<Vec<u64>,String>{
    let mut unique=HashSet::new();
    if ids.len()>MAX_EDIT_OBJECTS{return Err("Слишком большое выделение".into());}
    let mut result=Vec::new();
    for &id in ids{
        if !unique.insert(id){continue;}
        let o=doc.objects.iter().find(|o|o.id==id)
            .ok_or("Текстовый объект не найден")?;
        if !doc.layer(o.layer_id).is_some_and(|l|l.visible&&!l.locked){
            return Err("Выделение включает скрытый или заблокированный слой".into());
        }
        if matches!(o.shape,Shape::Text{..}){result.push(id);}
    }
    if result.is_empty(){return Err("Выделите хотя бы один текстовый объект".into());}
    Ok(result)
}
impl Editor{
    fn apply_text_preview(&mut self,preview:Document,what:&str,count:usize){
        if preview==self.doc{
            self.status="Текст не изменился".into();return;
        }
        if let Err(e)=preview.validate(){
            self.status=format!("Текстовая операция отклонена: {e}");return;
        }
        self.checkpoint();
        self.doc=preview;
        self.text_preview.clear();
        self.fill_cache.entries.clear();self.vector_cache.clear();
        self.bezier_preview.clear();self.compound_cache.clear();
        self.raster_cache.clear();
        self.status=format!("{what}: изменено текстовых объектов {count}. Ctrl+Z — отмена.");
    }
    pub(super) fn run_text_action(&mut self,action:TextAction){
        let ids=match text_ids(&self.doc,&self.selection_ids()){
            Ok(ids)=>ids,Err(e)=>{self.status=e;return;}
        };
        let mut preview=self.doc.clone();
        for id in &ids{
            let obj=preview.get_mut(*id).expect("validated text ID");
            if let Shape::Text{value,..}=&mut obj.shape{
                match apply_action(value,action,self.text_studio_wrap_columns as usize,
                    self.text_studio_indent_spaces as usize){
                    Ok(next)=>*value=next,
                    Err(e)=>{self.status=format!("Ошибка текста #{id}: {e}");return;}
                }
            }
        }
        self.apply_text_preview(preview,action.label(),ids.len());
    }
    pub(super) fn run_text_preset(&mut self,preset:TextPreset){
        let ids=match text_ids(&self.doc,&self.selection_ids()){
            Ok(ids)=>ids,Err(e)=>{self.status=e;return;}
        };
        let mut preview=self.doc.clone();
        let (size,weight,italic,letter_spacing,line_height,alignment)=preset.specs();
        for id in &ids{
            let obj=preview.get_mut(*id).expect("validated text ID");
            if let Shape::Text{size_mm,..}=&mut obj.shape{
                *size_mm=size;
                let fmt=&mut obj.text_format;
                fmt.weight=weight;fmt.italic=italic;
                fmt.letter_spacing_mm=letter_spacing;
                fmt.line_height=line_height;fmt.alignment=alignment;
            }
        }
        self.apply_text_preview(preview,preset.label(),ids.len());
    }
    pub(super) fn text_studio_panel(&mut self,ui:&mut egui::Ui){
        let total=self.selection_ids().iter().filter(|id|
            self.doc.objects.iter().any(|o|o.id==**id&&matches!(o.shape,Shape::Text{..}))
        ).count();
        ui.label(RichText::new(format!("{total} текстовых объектов в выделении")).small());
        ui.horizontal(|ui|{
            ui.label("Перенос:");
            ui.add(egui::DragValue::new(&mut self.text_studio_wrap_columns)
                .range(8..=160).suffix(" зн."));
            ui.label("Отступ:");
            ui.add(egui::DragValue::new(&mut self.text_studio_indent_spaces)
                .range(1..=12).suffix(" пр."));
        });
        ui.collapsing("Регистр и написание (6)",|ui|{
            for op in TextAction::ALL.iter().take(6){
                if ui.button(op.label()).on_hover_text(op.help()).clicked(){
                    self.run_text_action(*op);
                }
            }
        });
        ui.collapsing("Пробелы, списки и строки (17)",|ui|{
            for op in TextAction::ALL.iter().skip(6){
                if ui.button(op.label()).on_hover_text(op.help()).clicked(){
                    self.run_text_action(*op);
                }
            }
        });
        ui.collapsing("Стили заголовков и основного текста (7)",|ui|{
            for preset in TextPreset::ALL{
                if ui.button(preset.label()).clicked(){self.run_text_preset(preset);}
            }
        });
        ui.small("Операции применяются ко всем выделенным надписям, не превращают их в контуры и отменяются одним Ctrl+Z.");
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use opencorel_core::{Shape,Style};
    #[test]
    fn all_text_operations_keep_valid_utf8_and_respect_length_limit(){
        for action in TextAction::ALL{
            let text="  ПрИвЕт мир 🌍  \nстрока\n\nстрока\t  конец ";
            let transformed=apply_action(text,action,20,4).unwrap();
            assert!(transformed.len()<=MAX_BYTES);
            assert!(transformed.is_char_boundary(transformed.len()));
        }
    }
    #[test]
    fn word_wrap_preserves_cyrillic_content_and_breaks_long_words(){
        assert_eq!(wrap_line("Привет мир дела",8),"Привет\nмир дела");
        assert_eq!(wrap_line("абвгдежзийкл",8),"абвгдежз\nийкл");
        assert_eq!(wrap_line("ёжик",8),"ёжик");
    }
    #[test]
    fn whitespace_sort_case_and_lists(){
        assert_eq!(title_case("тЕСтовый тЕКСТ"),"Тестовый Текст");
        assert_eq!(sentence_case("ПРИВЕТ. мИР! аБВ?"),"Привет. Мир! Абв?");
        assert_eq!(collapse_spaces("А  Б\nВ\t  Г"),"А Б\nВ Г");
        assert_eq!(apply_action("да\n\nнет",TextAction::NumberLines,30,2).unwrap(),
            "1. да\n\n2. нет");
        assert_eq!(apply_action("ж\na\nж",TextAction::UniqueLines,30,2).unwrap(),
            "ж\na");
    }
    #[test]
    fn batch_selection_is_atomic_undoable_and_respects_locked_layers(){
        let mut editor=Editor::default();
        editor.doc=Document::default();
        let first=editor.doc.add(Shape::Text{
            x:0.,y:0.,value:"привет".into(),size_mm:5.},Style::default());
        let second=editor.doc.add(Shape::Text{
            x:0.,y:20.,value:"мир".into(),size_mm:5.},Style::default());
        editor.selected=Some(first);editor.multi_selected=vec![second];
        let before=editor.doc.clone();
        editor.run_text_action(TextAction::Upper);
        assert_eq!(editor.undo.len(),1);
        assert!(editor.doc.objects.iter().all(|o|match &o.shape{
            Shape::Text{value,..}=>value==&value.to_uppercase(),_=>true
        }));
        editor.undo();
        assert_eq!(editor.doc,before);
        editor.doc.layers[0].locked=true;
        let blocked=editor.doc.clone();
        editor.selected=Some(first);editor.multi_selected=vec![second];
        editor.run_text_preset(TextPreset::Display);
        assert_eq!(editor.doc,blocked);
    }
    #[test]
    fn seven_presets_are_valid_and_preserve_editable_text(){
        for preset in TextPreset::ALL{
            let mut editor=Editor::default();
            editor.doc=Document::default();
            let id=editor.doc.add(Shape::Text{
                x:1.,y:2.,value:"Sample".into(),size_mm:5.},Style::default());
            editor.selected=Some(id);
            editor.run_text_preset(preset);
            editor.doc.validate().unwrap();
            assert!(matches!(editor.doc.objects[0].shape,Shape::Text{..}));
        }
    }
    #[test]
    fn invalid_wrap_parameters_do_not_modify_document(){
        let mut e=Editor::default();e.doc=Document::default();
        let id=e.doc.add(Shape::Text{x:0.,y:0.,
            value:"word".into(),size_mm:4.},Style::default());
        e.selected=Some(id);
        e.text_studio_wrap_columns=0;
        let before=e.doc.clone();
        e.run_text_action(TextAction::HardWrap);
        assert_eq!(e.doc,before);
        assert!(e.undo.is_empty());
    }
}
