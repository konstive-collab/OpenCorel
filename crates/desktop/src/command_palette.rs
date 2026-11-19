//! Discoverable keyboard command palette. All actions delegate to the real
//! editor engine rather than injecting text or relying on OS automation.
use super::{Editor,Tool};
use super::workspace::InspectorTab;
use eframe::egui::{self,RichText,Color32,Key};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) enum Command{
    New,Open,Save,SaveAs,ImportImage,ExportSvg,ExportPdf,ExportPng,ExportDock,
    NewPage,DuplicatePage,DeletePage,PageSettings,GuideSettings,
    Select,Nodes,Curve,Text,Rect,Ellipse,Line,Polygon,
    SelectAll,Duplicate,Group,Ungroup,AlignCenterX,AlignCenterY,
    MoveUp,MoveDown,Objects,Properties,ZoomFit,ToggleGrid,
    ToggleGuides,ToggleTheme,Preflight,Help,
}
const COMMANDS:&[(Command,&str,&str,&str)]=&[
    (Command::New,"Новый документ","Файл","Ctrl+N"),
    (Command::Open,"Открыть проект","Файл","Ctrl+O"),
    (Command::Save,"Сохранить KDR","Файл","Ctrl+S"),
    (Command::SaveAs,"Сохранить как","Файл","Ctrl+Shift+S"),
    (Command::ImportImage,"Поместить изображение","Файл",""),
    (Command::ExportSvg,"Экспорт SVG","Экспорт",""),
    (Command::ExportPdf,"Экспорт PDF (RGB)","Экспорт",""),
    (Command::ExportPng,"Экспорт PNG","Экспорт",""),
    (Command::ExportDock,"Центр экспорта и PDF-книга","Экспорт",""),
    (Command::NewPage,"Новая страница","Страницы",""),
    (Command::DuplicatePage,"Дублировать страницу","Страницы",""),
    (Command::DeletePage,"Удалить страницу","Страницы",""),
    (Command::PageSettings,"Параметры листа","Страницы",""),
    (Command::GuideSettings,"Направляющие и привязки","Страницы",""),
    (Command::Select,"Инструмент: Выделение","Инструменты","V"),
    (Command::Nodes,"Инструмент: Редактирование узлов","Инструменты","F10"),
    (Command::Curve,"Инструмент: Кривая Безье","Инструменты","B"),
    (Command::Text,"Инструмент: Текст","Инструменты","F8"),
    (Command::Rect,"Инструмент: Прямоугольник","Инструменты","F6"),
    (Command::Ellipse,"Инструмент: Эллипс","Инструменты","F7"),
    (Command::Line,"Инструмент: Линия","Инструменты","L"),
    (Command::Polygon,"Инструмент: Многоугольник","Инструменты","P"),
    (Command::SelectAll,"Выделить все объекты","Объекты","Ctrl+A"),
    (Command::Duplicate,"Дублировать выделение","Объекты","Ctrl+D"),
    (Command::Group,"Группировать","Объекты","Ctrl+G"),
    (Command::Ungroup,"Разгруппировать","Объекты","Ctrl+Shift+G"),
    (Command::AlignCenterX,"Выровнять по центру X","Объекты",""),
    (Command::AlignCenterY,"Выровнять по центру Y","Объекты",""),
    (Command::MoveUp,"Поднять на уровень","Объекты",""),
    (Command::MoveDown,"Опустить на уровень","Объекты",""),
    (Command::Objects,"Панель объектов и слоёв","Окно",""),
    (Command::Properties,"Панель свойств","Окно",""),
    (Command::ZoomFit,"Показать весь лист","Вид","F4"),
    (Command::ToggleGrid,"Включить / выключить сетку","Вид",""),
    (Command::ToggleGuides,"Включить / выключить направляющие","Вид",""),
    (Command::ToggleTheme,"Светлая / тёмная тема","Вид",""),
    (Command::Preflight,"Проверить макет","Производство",""),
    (Command::Help,"Инструкция и горячие клавиши","Справка","F1"),
];
impl Editor{
    pub(super) fn execute_command(&mut self,command:Command){
        match command{
            Command::New=>self.new_document(),
            Command::Open=>self.open_picker(),
            Command::Save=>self.save(),
            Command::SaveAs=>self.save_as_picker(),
            Command::ImportImage=>self.place_bitmap_picker(),
            Command::ExportSvg=>self.export_dialog("svg"),
            Command::ExportPdf=>self.export_dialog("pdf"),
            Command::ExportPng=>self.export_dialog("png"),
            Command::ExportDock=>{
                self.inspector_tab=InspectorTab::Export;self.show_inspector=true;
            }
            Command::NewPage=>self.append_page(self.doc.page.width_mm,self.doc.page.height_mm),
            Command::DuplicatePage=>self.copy_page(),
            Command::DeletePage=>self.remove_page(),
            Command::PageSettings|Command::GuideSettings=>{
                self.inspector_tab=InspectorTab::Page;self.show_inspector=true;
            }
            Command::Select=>self.change_tool(Tool::Select),
            Command::Nodes=>self.change_tool(Tool::Nodes),
            Command::Curve=>self.change_tool(Tool::Curve),
            Command::Text=>self.change_tool(Tool::Text),
            Command::Rect=>self.change_tool(Tool::Rect),
            Command::Ellipse=>self.change_tool(Tool::Ellipse),
            Command::Line=>self.change_tool(Tool::Line),
            Command::Polygon=>self.change_tool(Tool::Polygon),
            Command::SelectAll=>self.select_all(),
            Command::Duplicate=>self.duplicate_preserving_styles(),
            Command::Group=>self.group_selection(),
            Command::Ungroup=>self.ungroup_selection(),
            Command::AlignCenterX=>self.align_selection(opencorel_core::Alignment::CenterX),
            Command::AlignCenterY=>self.align_selection(opencorel_core::Alignment::CenterY),
            Command::MoveUp=>self.z_step(true),
            Command::MoveDown=>self.z_step(false),
            Command::Objects=>{
                self.inspector_tab=InspectorTab::Objects;self.show_inspector=true;
            }
            Command::Properties=>{
                self.inspector_tab=InspectorTab::Properties;self.show_inspector=true;
            }
            Command::ZoomFit=>self.fit_page(),
            Command::ToggleGrid=>self.show_grid=!self.show_grid,
            Command::ToggleGuides=>self.show_guides=!self.show_guides,
            Command::ToggleTheme=>self.light_theme=!self.light_theme,
            Command::Preflight=>self.run_preflight(),
            Command::Help=>self.show_welcome=true,
        }
    }
    pub(super) fn command_palette(&mut self,ctx:&egui::Context){
        if !self.show_command_palette{return;}
        let mut open=true;
        let mut chosen=None;
        let mut dismiss=false;
        egui::Window::new("Поиск команд  ·  Ctrl+K")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(490.0)
            .anchor(egui::Align2::CENTER_TOP,[0.0,70.0])
            .show(ctx,|ui|{
                let edit=ui.add(egui::TextEdit::singleline(&mut self.command_query)
                    .hint_text("Инструмент, экспорт, страница, выравнивание..."));
                if self.command_focus{
                    edit.request_focus();
                    self.command_focus=false;
                }
                if ui.input(|i|i.key_pressed(Key::Escape)){dismiss=true;}
                ui.separator();
                let query=self.command_query.trim().to_lowercase();
                let entries:Vec<_>=COMMANDS.iter()
                    .filter(|(_,title,category,short)|{
                        query.is_empty()||format!("{title} {category} {short}")
                            .to_lowercase().contains(&query)
                    }).collect();
                if entries.is_empty(){
                    ui.label("Совпадений нет. Попробуйте другое название инструмента.");
                }
                egui::ScrollArea::vertical().max_height(350.).show(ui,|ui|{
                    for (index,entry) in entries.iter().enumerate(){
                        let (action,name,category,shortcut)=**entry;
                        ui.horizontal(|ui|{
                            let trigger=ui.selectable_label(false,
                                RichText::new(name).strong());
                            ui.label(RichText::new(category).small().color(Color32::GRAY));
                            if !shortcut.is_empty(){
                                ui.label(RichText::new(shortcut).small()
                                    .color(Color32::from_rgb(29,174,155)));
                            }
                            if trigger.clicked()||index==0&&ui.input(|i|
                                i.key_pressed(Key::Enter)){
                                chosen=Some(action);
                            }
                        });
                    }
                });
            });
        self.show_command_palette=open && !dismiss;
        if let Some(command)=chosen{
            self.show_command_palette=false;
            self.command_query.clear();
            self.execute_command(command);
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn palette_executes_real_tool_and_page_actions(){
        let mut editor=Editor::default();
        editor.execute_command(Command::Curve);
        assert!(editor.tool==Tool::Curve);
        editor.execute_command(Command::NewPage);
        assert_eq!(editor.doc.pages().len(),2);
        editor.execute_command(Command::DuplicatePage);
        assert_eq!(editor.doc.pages().len(),3);
        editor.execute_command(Command::ToggleGuides);
        assert!(!editor.show_guides);
        editor.undo();
        editor.doc.validate().unwrap();
    }
}
