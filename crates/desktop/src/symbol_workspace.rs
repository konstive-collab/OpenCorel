//! Local symbol-library panel and transactional editor commands.
use super::*;
use eframe::egui::{self,Color32,RichText};

impl Editor{
    pub(super) fn load_symbol_library(&mut self){
        self.symbol_library_path=opencorel_core::SymbolLibrary::default_path();
        if !self.symbol_library_path.exists(){
            self.symbol_library=opencorel_core::SymbolLibrary::default();
            self.symbol_library_error=None;
            return;
        }
        match opencorel_core::SymbolLibrary::read(&self.symbol_library_path){
            Ok(library)=>{
                self.symbol_library=library;
                self.symbol_library_error=None;
            }
            Err(error)=>{
                // Never overwrite a corrupt library automatically. The user
                // can still work with the current document and recover the file.
                self.symbol_library=opencorel_core::SymbolLibrary::default();
                self.symbol_library_error=Some(error);
            }
        }
    }
    fn persist_symbol_library(&mut self,library:&opencorel_core::SymbolLibrary)
        ->Result<(),String>{
        if self.symbol_library_error.is_some(){
            return Err("The local OCLIB failed validation. Import a valid backup before replacing it.".into());
        }
        library.save_atomic(&self.symbol_library_path)?;
        self.symbol_library_error=None;
        Ok(())
    }
    fn create_symbol_from_selection(&mut self){
        let ids=self.selection_ids();
        if ids.is_empty(){
            self.status="Сначала выделите один или несколько объектов".into();
            return;
        }
        let mut document=self.doc.clone();
        let definition=match document.create_symbol(&ids,
            &self.symbol_new_name,&self.symbol_new_category){
            Ok(value)=>value,
            Err(error)=>{self.status=format!("Символ не создан: {error}");return;}
        };
        let mut library=self.symbol_library.clone();
        if let Err(error)=library.upsert(definition.clone()){
            self.status=format!("Символ создан в проекте, но библиотека не обновлена: {error}");
            return;
        }
        if let Err(error)=self.persist_symbol_library(&library){
            self.symbol_library_error=Some(error.clone());
            self.status=format!("Не удалось сохранить библиотеку: {error}");
            return;
        }
        self.checkpoint();
        self.doc=document;
        self.symbol_library=library;
        self.selected_symbol_key=Some(definition.library_key.clone());
        self.status=format!("Символ «{}» создан · {} объектов. Исходная группа связана с определением.",
            definition.name,definition.objects.len());
    }
    fn insert_selected_symbol(&mut self){
        let Some(key)=self.selected_symbol_key.clone() else{
            self.status="Выберите символ в библиотеке".into();return;
        };
        let Some(asset)=self.symbol_library.symbols.iter()
            .find(|symbol|symbol.library_key==key).cloned()
            .or_else(||self.doc.symbols.iter()
                .find(|symbol|symbol.library_key==key).cloned()) else{
            self.status="Символ больше не найден в проекте или библиотеке".into();return;
        };
        let mut document=self.doc.clone();
        let id=match document.import_symbol_definition(&asset){
            Ok(value)=>value,
            Err(error)=>{self.status=format!("Импорт символа не выполнен: {error}");return;}
        };
        let bounds=document.symbol_definition(id).map(|s|s.bounds())
            .unwrap_or(Bounds{x:0.,y:0.,w:0.,h:0.});
        let x=((document.page.width_mm-bounds.w)*0.5).max(0.0);
        let y=((document.page.height_mm-bounds.h)*0.5).max(0.0);
        let ids=match document.insert_symbol(id,self.active_layer,x,y){
            Ok(value)=>value,
            Err(error)=>{self.status=format!("Вставка символа не выполнена: {error}");return;}
        };
        self.checkpoint();
        self.doc=document;
        self.selected=ids.first().copied();
        self.multi_selected=ids.iter().copied().skip(1).collect();
        self.fill_cache.entries.clear();
        self.vector_cache.clear();
        self.raster_cache.clear();
        self.compound_cache.clear();
        self.status=format!("Вставлен связанный экземпляр «{}» · {} объектов",
            asset.name,ids.len());
    }
    fn update_selected_symbol(&mut self){
        let Some(id)=self.selected else{
            self.status="Выберите связанный экземпляр на странице".into();return;
        };
        let Some(group_id)=self.doc.get(id).and_then(|object|object.group_id) else{
            self.status="Выбранный объект не является экземпляром символа".into();return;
        };
        if self.doc.symbol_instance_for_group(group_id).is_none(){
            self.status="Группа не связана с библиотечным символом".into();return;
        }
        let mut document=self.doc.clone();
        let definition=match document.update_symbol_from_group(group_id){
            Ok(value)=>value,
            Err(error)=>{self.status=format!("Обновление не выполнено: {error}");return;}
        };
        let mut library=self.symbol_library.clone();
        if let Err(error)=library.upsert(definition.clone()){
            self.status=format!("Определение обновлено в проекте, библиотека не обновлена: {error}");
            return;
        }
        if let Err(error)=self.persist_symbol_library(&library){
            self.symbol_library_error=Some(error.clone());
            self.status=format!("Не удалось сохранить библиотеку: {error}");
            return;
        }
        self.checkpoint();
        self.doc=document;
        self.symbol_library=library;
        self.selected_symbol_key=Some(definition.library_key.clone());
        self.fill_cache.entries.clear();
        self.vector_cache.clear();
        self.raster_cache.clear();
        self.compound_cache.clear();
        self.status=format!("«{}» обновлён. Связанные экземпляры синхронизированы.",
            definition.name);
    }
    fn detach_selected_symbol(&mut self){
        let Some(id)=self.selected else{return;};
        let Some(group_id)=self.doc.get(id).and_then(|object|object.group_id) else{
            self.status="Выбранный объект не входит в группу".into();return;
        };
        let mut document=self.doc.clone();
        match document.detach_symbol_instance(group_id){
            Ok(count)=>{
                self.checkpoint();
                self.doc=document;
                self.status=format!("Связь с символом разорвана · {count} объектов сохранены как обычная группа");
            }
            Err(error)=>self.status=format!("Не удалось отвязать экземпляр: {error}"),
        }
    }
    fn export_symbol_library_picker(&mut self){
        if self.symbol_library_error.is_some(){
            self.status="Сначала восстановите локальную библиотеку из корректного OCLIB-файла".into();
            return;
        }
        if let Some(path)=rfd::FileDialog::new()
            .add_filter("OpenCorel Symbol Library",&["oclib"])
            .set_file_name("symbols.oclib").save_file(){
            match self.symbol_library.save_atomic(&path){
                Ok(())=>self.status=format!("Библиотека экспортирована: {}",path.display()),
                Err(error)=>self.status=format!("Экспорт библиотеки не выполнен: {error}"),
            }
        }
    }
    fn import_symbol_library_picker(&mut self){
        let Some(path)=rfd::FileDialog::new()
            .add_filter("OpenCorel Symbol Library",&["oclib"]).pick_file() else{return;};
        let imported=match opencorel_core::SymbolLibrary::read(&path){
            Ok(value)=>value,
            Err(error)=>{self.status=format!("Не удалось прочитать библиотеку: {error}");return;}
        };
        // Importing a valid backup is the explicit recovery action for a
        // corrupt local file; in that case start from the backup, not the
        // empty in-memory fallback, and replace the broken file atomically.
        let replacing_corrupt=self.symbol_library_error.is_some();
        let mut library=if replacing_corrupt{
            opencorel_core::SymbolLibrary::default()
        }else{self.symbol_library.clone()};
        let mut count=0usize;
        for symbol in imported.symbols{
            if let Err(error)=library.upsert(symbol){
                self.status=format!("Импорт остановлен: {error}");return;
            }
            count+=1;
        }
        match library.save_atomic(&self.symbol_library_path){
            Ok(())=>{
                self.symbol_library=library;
                self.symbol_library_error=None;
                self.status=format!("Импортировано или обновлено символов: {count}");
            }
            Err(error)=>{
                self.symbol_library_error=Some(error.clone());
                self.status=format!("Не удалось сохранить локальную библиотеку: {error}");
            }
        }
    }
    fn save_selected_document_symbol_to_library(&mut self){
        let Some(key)=self.selected_symbol_key.clone() else{return;};
        let Some(definition)=self.doc.symbols.iter()
            .find(|symbol|symbol.library_key==key).cloned() else{
            self.status="Определение символа не найдено в текущем проекте".into();return;
        };
        let mut library=self.symbol_library.clone();
        if let Err(error)=library.upsert(definition.clone()){
            self.status=format!("Не удалось добавить символ в библиотеку: {error}");return;
        }
        if let Err(error)=self.persist_symbol_library(&library){
            self.symbol_library_error=Some(error.clone());
            self.status=format!("Не удалось сохранить библиотеку: {error}");return;
        }
        self.symbol_library=library;
        self.status=format!("«{}» добавлен в локальную библиотеку",definition.name);
    }
    fn remove_selected_library_symbol(&mut self){
        let Some(key)=self.selected_symbol_key.clone() else{return;};
        let mut library=self.symbol_library.clone();
        if !library.remove(&key){
            self.status="Символ не найден в библиотеке".into();return;
        }
        if let Err(error)=self.persist_symbol_library(&library){
            self.symbol_library_error=Some(error.clone());
            self.status=format!("Не удалось обновить библиотеку: {error}");
            return;
        }
        self.symbol_library=library;
        self.selected_symbol_key=None;
        self.status="Символ удалён из локальной библиотеки. Уже вставленные экземпляры в проектах не затронуты.".into();
    }
    pub(super) fn symbol_library_docker(&mut self,ui:&mut egui::Ui){
        ui.heading("БИБЛИОТЕКА СИМВОЛОВ");
        ui.label(RichText::new("Связанные векторные компоненты").color(Color32::from_rgb(35,183,159)));
        ui.label("Создавайте элементы один раз, вставляйте экземпляры и обновляйте их из исходной группы.");
        if let Some(error)=&self.symbol_library_error{
            ui.colored_label(Color32::from_rgb(232,126,92),
                format!("Библиотека не загружена: {error}"));
            ui.label("Файл не перезаписывался. Проверьте или восстановите его вручную.");
        }
        ui.separator();
        ui.label("Новый символ из выделения");
        ui.add(egui::TextEdit::singleline(&mut self.symbol_new_name)
            .hint_text("Название символа"));
        ui.horizontal(|ui|{
            ui.label("Категория");
            ui.add(egui::TextEdit::singleline(&mut self.symbol_new_category)
                .desired_width(120.).hint_text("Общее"));
        });
        if ui.add_enabled(!self.selection_ids().is_empty(),
            egui::Button::new("＋ Создать из выделения")).clicked(){
            self.create_symbol_from_selection();
        }
        ui.separator();
        ui.horizontal(|ui|{
            ui.heading("РЕСУРСЫ");
            ui.label(RichText::new(format!("{}",self.symbol_library.symbols.len()))
                .color(Color32::from_rgb(35,183,159)));
        });
        ui.add(egui::TextEdit::singleline(&mut self.symbol_search)
            .hint_text("Поиск символа или категории..."));
        ui.horizontal_wrapped(|ui|{
            if ui.small_button("Импорт OCLIB...").clicked(){self.import_symbol_library_picker();}
            if ui.small_button("Экспорт OCLIB...").clicked(){self.export_symbol_library_picker();}
        });
        let mut categories:Vec<String>=self.symbol_library.symbols.iter()
            .map(|item|item.category.clone()).filter(|item|!item.is_empty()).collect();
        categories.sort();categories.dedup();
        egui::ComboBox::from_id_salt("symbol_category_filter")
            .selected_text(if self.symbol_category_filter.is_empty(){
                "Все категории"
            }else{self.symbol_category_filter.as_str()})
            .show_ui(ui,|ui|{
                ui.selectable_value(&mut self.symbol_category_filter,String::new(),"Все категории");
                for category in categories{
                    ui.selectable_value(&mut self.symbol_category_filter,category.clone(),category);
                }
            });
        let query=self.symbol_search.trim().to_lowercase();
        let category=self.symbol_category_filter.clone();
        let filtered:Vec<_>=self.symbol_library.symbols.iter().filter(|item|{
            (category.is_empty()||item.category==category)&&
            (query.is_empty()||item.name.to_lowercase().contains(&query)||
                item.category.to_lowercase().contains(&query))
        }).map(|item|(item.library_key.clone(),item.name.clone(),
            item.category.clone(),item.objects.len())).collect();
        ui.label(RichText::new(format!("Найдено: {}",filtered.len())).small().color(Color32::GRAY));
        egui::ScrollArea::vertical().id_salt("symbol_library_items")
            .max_height(220.).show(ui,|ui|{
                if filtered.is_empty(){
                    ui.weak("В библиотеке пока нет подходящих символов.");
                }
                for (key,name,category,count) in filtered{
                    let selected=self.selected_symbol_key.as_deref()==Some(key.as_str());
                    let label=if category.is_empty(){
                        format!("◇ {name} · {count} объектов")
                    }else{format!("◇ {name} · {category} · {count}")};
                    if ui.selectable_label(selected,label).clicked(){
                        self.selected_symbol_key=Some(key);
                    }
                }
            });
        let local_keys:HashSet<String>=self.symbol_library.symbols.iter()
            .map(|symbol|symbol.library_key.clone()).collect();
        let document_symbols:Vec<_>=self.doc.symbols.iter().filter(|symbol|
            !local_keys.contains(&symbol.library_key)&&
            (category.is_empty()||symbol.category==category)&&
            (query.is_empty()||symbol.name.to_lowercase().contains(&query)||
                symbol.category.to_lowercase().contains(&query)))
            .map(|symbol|(symbol.library_key.clone(),symbol.name.clone(),
                symbol.category.clone(),symbol.objects.len())).collect();
        if !document_symbols.is_empty(){
            ui.separator();
            ui.label(RichText::new("СИМВОЛЫ ТЕКУЩЕГО ДОКУМЕНТА")
                .small().color(Color32::from_rgb(35,183,159)));
            for (key,name,category,count) in document_symbols{
                let selected=self.selected_symbol_key.as_deref()==Some(key.as_str());
                let label=if category.is_empty(){
                    format!("◇ {name} · {count} объектов · в проекте")
                }else{format!("◇ {name} · {category} · в проекте")};
                if ui.selectable_label(selected,label).clicked(){
                    self.selected_symbol_key=Some(key);
                }
            }
        }
        let selected_key=self.selected_symbol_key.as_ref();
        let selected_asset=selected_key.and_then(|key|
            self.symbol_library.symbols.iter()
                .find(|item|&item.library_key==key)).cloned()
            .or_else(||selected_key.and_then(|key|self.doc.symbols.iter()
                .find(|item|&item.library_key==key)).cloned());
        let selected_is_local=selected_key.is_some_and(|key|
            self.symbol_library.symbols.iter().any(|item|&item.library_key==key));
        if let Some(asset)=selected_asset{
            ui.separator();
            ui.label(RichText::new(asset.name.as_str()).strong());
            if !asset.category.is_empty(){ui.small(format!("Категория: {}",asset.category));}
            ui.small(format!("Объектов: {}",asset.objects.len()));
            ui.horizontal_wrapped(|ui|{
                if ui.button("Вставить в центр").clicked(){self.insert_selected_symbol();}
                if selected_is_local{
                    if ui.button("Удалить из библиотеки").clicked(){
                        self.remove_selected_library_symbol();
                    }
                }else if ui.button("Сохранить в библиотеку").clicked(){
                    self.save_selected_document_symbol_to_library();
                }
            });
        }
        let linked_group=self.selected.and_then(|id|self.doc.get(id))
            .and_then(|object|object.group_id)
            .filter(|group|self.doc.symbol_instance_for_group(*group).is_some());
        if linked_group.is_some(){
            ui.separator();
            ui.label("Связанный экземпляр на странице");
            if ui.button("Обновить определение из выделенной группы").clicked(){
                self.update_selected_symbol();
            }
            if ui.button("Разорвать связь, оставить группу").clicked(){
                self.detach_selected_symbol();
            }
        }
        ui.separator();
        ui.small(format!("Локальный файл: {}",self.symbol_library_path.display()));
        ui.small("KDR хранит определения внутри проекта; OCLIB — общая библиотека пользователя.");
    }
}
