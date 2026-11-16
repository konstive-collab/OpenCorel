//! Corel-style, independently implemented workspace layout.
//! Uses familiar panel positions and workflows but no proprietary assets.
use super::{color, productivity, text_edit_ops, Editor, Tool};
use eframe::egui::{self, Color32, RichText, Vec2};
use opencorel_core::{Bounds, Shape, Alignment, Distribution, TextAlign, valid_paint};
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum InspectorTab { Objects, Properties, Page }

const SWATCHES: [&str; 27] = [
    "#FFFFFF", "#E8E8E8", "#C0C0C0", "#808080", "#333333", "#000000",
    "#7A0018", "#E02020", "#FF8B00", "#FFD523", "#8AC440", "#16A05A",
    "#067E65", "#00B5CA", "#117CC1", "#1244A5", "#553DB6", "#9D42AA",
    "#ED52A9", "#F5B5B5", "#F8D5AB", "#FFF2B3", "#D1ECC3", "#B7E7EC",
    "#B9CDEC", "#E4C9F1", "#725440",
];

impl Editor {
    pub(super) fn change_tool(&mut self, next: Tool) {
        self.tool=next;
        if next!=Tool::Curve {
            self.curve_start=None;self.curve_draft=None;
            self.curve_outgoing=None;self.curve_drag_anchor=None;
        }
        self.drag_start=None;
        self.creation_start=None;
        self.resize_drag=None;self.node_drag=None;
        self.node_drag_checkpointed=false;
        if next!=Tool::Nodes{self.active_node=None;}
        if matches!(next,Tool::Text|Tool::Nodes){
            self.inspector_tab=InspectorTab::Properties;
            self.show_inspector=true;
        }
    }

    pub(super) fn new_document(&mut self) {
        self.checkpoint();
        self.doc=Default::default();
        self.active_node=None;self.node_drag=None;
        self.node_drag_checkpointed=false;
        self.fill_cache.entries.clear();
        self.vector_cache.clear();
        self.raster_cache.clear();
        self.compound_cache.clear();
        self.selected=None;
        self.multi_selected.clear();
        self.resize_drag=None;
        self.active_layer=1;
        self.pan=Vec2::ZERO;
        self.zoom=1.0;
        self.curve_start=None;
        self.curve_draft=None;
        self.curve_outgoing=None;
        self.curve_drag_anchor=None;
        self.file="untitled.kdr".into();
        self.status="Создан новый документ. Выберите инструмент слева или нажмите F1 для помощи.".into();
    }

    pub(super) fn top_workspace(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("main_menubar").exact_height(28.0).show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.label(RichText::new("◈ OpenCorel").strong().size(17.0)
                    .color(Color32::from_rgb(32,201,170)));
                ui.label(RichText::new(format!("v{}  •  EDITOR",env!("CARGO_PKG_VERSION").split('-').next().unwrap_or("dev")) ).small()
                    .color(Color32::from_rgb(129,144,162)));

                ui.separator();
                ui.menu_button("Файл",|ui|{
                    if ui.button("Новый лист A4").clicked(){self.blank_page(210.0,297.0);ui.close_menu();}
                    if ui.button("Новый лист A3").clicked(){self.blank_page(297.0,420.0);ui.close_menu();}
                    if ui.button("Новый баннер 3×2 м").clicked(){self.blank_page(3000.0,2000.0);ui.close_menu();}
                    if ui.button("Пример готового макета").clicked(){self.sample_design();ui.close_menu();}
                    ui.separator();
                    if ui.button("Новый документ       Ctrl+N").clicked(){self.new_document();ui.close_menu();}
                    if ui.button("Открыть...         Ctrl+O").clicked(){self.open_picker();ui.close_menu();}
                    if ui.button("Импорт SVG/SVGZ...").clicked(){self.import_svg_picker();ui.close_menu();}
                    if ui.button("Поместить изображение...").clicked(){self.place_bitmap_picker();ui.close_menu();}
                    if ui.button("Сохранить           Ctrl+S").clicked(){self.save();ui.close_menu();}
                    if ui.button("Сохранить как...  Ctrl+Shift+S").clicked(){self.save_as_picker();ui.close_menu();}
                    if ui.button("Экспорт SVG...").clicked(){self.export_dialog("svg");ui.close_menu();}
                    if ui.button("Экспорт SVGZ...").clicked(){self.export_dialog("svgz");ui.close_menu();}
                    if ui.button("Экспорт PDF (RGB)...").clicked(){self.export_dialog("pdf");ui.close_menu();}
                    ui.separator();
                    ui.horizontal(|ui|{
                        ui.label("DPI");
                        ui.add(egui::DragValue::new(&mut self.proof_dpi).range(36..=300));
                    });
                    if ui.button("Экспорт PNG...").clicked(){self.export_dialog("png");ui.close_menu();}
                    if ui.button("Экспорт JPEG...").clicked(){self.export_dialog("jpg");ui.close_menu();}
                    ui.separator();
                    if ui.button("Проверка макета...").clicked(){self.run_preflight();ui.close_menu();}
                });
                ui.menu_button("Правка",|ui|{
                    if ui.add_enabled(!self.undo.is_empty(),egui::Button::new("Отменить      Ctrl+Z")).clicked(){self.undo();ui.close_menu();}
                    if ui.add_enabled(!self.redo.is_empty(),egui::Button::new("Повторить     Ctrl+Y")).clicked(){self.redo();ui.close_menu();}
                    if ui.button("Дублировать     Ctrl+D").clicked(){self.duplicate_preserving_styles();ui.close_menu();}
                    ui.separator();
                    if ui.button("Копировать      Ctrl+C").clicked(){self.copy_selection();ui.close_menu();}
                    if ui.button("Вырезать        Ctrl+X").clicked(){self.cut_selection();ui.close_menu();}
                    if ui.button("Вставить        Ctrl+V").clicked(){self.paste_selection();ui.close_menu();}
                    ui.label("Буфер работает только внутри OpenCorel");
                    if ui.button("Выделить все    Ctrl+A").clicked(){self.select_all();ui.close_menu();}
                    if ui.button("Удалить       Del").clicked(){self.delete_selection();ui.close_menu();}
                });
                ui.menu_button("Вид",|ui|{
                    if ui.button("Показать всю страницу  F4").clicked(){self.fit_page();ui.close_menu();}
                    ui.checkbox(&mut self.show_grid,"Сетка");
                    ui.checkbox(&mut self.snap_grid,"Привязка к сетке");
                    ui.checkbox(&mut self.show_rulers,"Линейки");
                    ui.checkbox(&mut self.light_theme,"Светлая тема");
                });
                ui.menu_button("Макет",|ui|{
                    if ui.button("A0 — 841×1189 мм").clicked(){self.page_size(841.0,1189.0);ui.close_menu();}
                    if ui.button("A2 — 420×594 мм").clicked(){self.page_size(420.0,594.0);ui.close_menu();}
                    if ui.button("A4 — 210×297 мм").clicked(){self.page_size(210.0,297.0);ui.close_menu();}
                    if ui.button("A3 — 297×420 мм").clicked(){self.page_size(297.0,420.0);ui.close_menu();}
                    if ui.button("A1 — 594×841 мм").clicked(){self.page_size(594.0,841.0);ui.close_menu();}
                    if ui.button("Баннер 3×2 м").clicked(){self.page_size(3000.0,2000.0);ui.close_menu();}
                    if ui.button("Баннер 6×3 м (SVG)").clicked(){self.page_size(6000.0,3000.0);ui.close_menu();}
                });
                ui.menu_button("Контуры",|ui|{
                    ui.label(RichText::new("ЛОГИЧЕСКИЕ ОПЕРАЦИИ").strong());
                    ui.label("Выделите две фигуры с Shift.");
                    ui.label("Первая фигура — A, вторая — B.");
                    ui.separator();
                    let eligible=self.selection_ids().len()==2;
                    ui.add_enabled_ui(eligible,|ui|{
                        for (name,operation) in [
                            ("Объединить A ∪ B",opencorel_core::boolean_ops::BooleanOperation::Union),
                            ("Вычесть B из A",opencorel_core::boolean_ops::BooleanOperation::Subtract),
                            ("Пересечение A ∩ B",opencorel_core::boolean_ops::BooleanOperation::Intersect),
                            ("Исключить общую часть (XOR)",opencorel_core::boolean_ops::BooleanOperation::Xor),
                        ]{
                            if ui.button(name).clicked(){self.apply_boolean(operation);ui.close_menu();}
                        }
                    });
                    ui.separator();
                    ui.label("Результат: редактируемые контуры.");
                    ui.label("Ctrl+Z возвращает обе фигуры.");
                });
                ui.menu_button("Маски",|ui|{
                    ui.label("Обтравка без изменения исходной геометрии");
                    ui.label("Выберите сначала фигуру-маску, затем объект с Shift.");
                    let selection=self.selection_ids();
                    if ui.add_enabled(selection.len()==2,
                        egui::Button::new("Создать обтравку")).clicked(){
                        self.apply_clipping_selection();
                        ui.close_menu();
                    }
                    if ui.add_enabled(selection.iter().any(|id|
                        self.doc.clip_links.iter().any(|link|link.target_id==*id)),
                        egui::Button::new("Снять обтравку")).clicked(){
                        self.remove_clipping_selection();
                        ui.close_menu();
                    }
                    ui.small("Экспорт: SVG/PDF. Просмотр на холсте пока предварительный.");
                });
                ui.menu_button("Объект",|ui|{
                    ui.label(RichText::new("ГРУППЫ").strong());
                    if ui.button("Центрировать по листу").clicked(){
                        self.center_selection_on_page();ui.close_menu();
                    }
                    if ui.button("Группировать       Ctrl+G").clicked(){
                        self.group_selection();ui.close_menu();
                    }
                    if ui.button("Разгруппировать    Ctrl+Shift+G").clicked(){
                        self.ungroup_selection();ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("На передний план").clicked(){self.reorder_selection(true);ui.close_menu();}
                    if ui.button("На задний план").clicked(){self.reorder_selection(false);ui.close_menu();}
                    if ui.button("Дублировать").clicked(){self.duplicate_preserving_styles();ui.close_menu();}
                    ui.separator();
                    if ui.button("Повернуть вправо на 90°").clicked(){self.rotate_selection(opencorel_core::transform::QuarterTurn::Clockwise);ui.close_menu();}
                    if ui.button("Повернуть влево на 90°").clicked(){self.rotate_selection(opencorel_core::transform::QuarterTurn::CounterClockwise);ui.close_menu();}
                    if ui.button("Отразить по горизонтали").clicked(){self.mirror_selection(true);ui.close_menu();}
                    if ui.button("Отразить по вертикали").clicked(){self.mirror_selection(false);ui.close_menu();}
                    ui.menu_button("Поворот и наклон — точно",|ui|{
                        ui.label("Угол поворота, градусы:");
                        ui.add(egui::DragValue::new(&mut self.rotation_degrees)
                            .speed(0.5).range(-360.0..=360.0).suffix("°"));
                        ui.label("Наклон X / Y, градусы:");
                        ui.horizontal(|ui|{
                            ui.add(egui::DragValue::new(&mut self.skew_x_degrees)
                                .speed(0.5).range(-79.0..=79.0).suffix("°"));
                            ui.add(egui::DragValue::new(&mut self.skew_y_degrees)
                                .speed(0.5).range(-79.0..=79.0).suffix("°"));
                        });
                        ui.checkbox(&mut self.custom_pivot,"Задать центр преобразования (мм)");
                        if self.custom_pivot{
                            ui.horizontal(|ui|{
                                ui.label("X:");
                                ui.add(egui::DragValue::new(&mut self.pivot_x)
                                    .speed(0.5).range(-100_000.0..=100_000.0));
                                ui.label("Y:");
                                ui.add(egui::DragValue::new(&mut self.pivot_y)
                                    .speed(0.5).range(-100_000.0..=100_000.0));
                            });
                        }else{
                            ui.small("Центр — геометрический центр выделения.");
                        }
                        if ui.button("Применить поворот").clicked(){
                            self.rotate_arbitrary();ui.close_menu();
                        }
                        if ui.button("Применить наклон").clicked(){
                            self.skew_arbitrary();ui.close_menu();
                        }
                        ui.separator();
                        ui.label("Масштаб выделения, % X / Y:");
                        ui.horizontal(|ui|{
                            ui.add(egui::DragValue::new(&mut self.scale_x_percent)
                                .speed(1.0).range(1.0..=1000.0).suffix("%"));
                            ui.add(egui::DragValue::new(&mut self.scale_y_percent)
                                .speed(1.0).range(1.0..=1000.0).suffix("%"));
                        });
                        if ui.button("Масштабировать всё выделение").clicked(){
                            self.scale_selected();ui.close_menu();
                        }
                        ui.small("Ctrl+Z — отмена. Растровые изображения и текст требуют преобразования.");
                    });
                    if ui.button("Редактировать узлы      F10").clicked(){
                        self.change_tool(Tool::Nodes);ui.close_menu();
                    }
                    if ui.button("Удалить выбранный узел   Del").clicked(){
                        self.remove_active_node();ui.close_menu();
                    }
                    if ui.button("Преобразовать текст в кривые").clicked(){
                        self.text_to_curves();ui.close_menu();
                    }
                    ui.separator();
                    ui.label("Выравнивание объектов (Shift для выделения):");
                    for (label,mode) in [
                        ("По левому краю",Alignment::Left),
                        ("По центру горизонтально",Alignment::CenterX),
                        ("По правому краю",Alignment::Right),
                        ("По верхнему краю",Alignment::Top),
                        ("По центру вертикально",Alignment::CenterY),
                        ("По нижнему краю",Alignment::Bottom),
                    ]{
                        if ui.button(label).clicked(){self.align_selection(mode);ui.close_menu();}
                    }
                    ui.separator();
                    if ui.button("Распределить горизонтально").clicked(){
                        self.distribute_selection(Distribution::HorizontalCenters);ui.close_menu();
                    }
                    if ui.button("Распределить вертикально").clicked(){
                        self.distribute_selection(Distribution::VerticalCenters);ui.close_menu();
                    }
                });
                ui.menu_button("Оформление",|ui|{
                    ui.label(RichText::new("ЗАЛИВКА И ОБВОДКА").strong());
                    ui.horizontal(|ui|{
                        ui.label("Непрозрачность");
                        ui.add(egui::Slider::new(&mut self.appearance_opacity,0.0..=1.0)
                            .step_by(0.01).show_value(true));
                    });
                    ui.separator();
                    ui.label("Градиент:");
                    ui.horizontal(|ui|{
                        ui.selectable_value(&mut self.appearance_gradient_mode,0,"Нет");
                        ui.selectable_value(&mut self.appearance_gradient_mode,1,"Линейный");
                        ui.selectable_value(&mut self.appearance_gradient_mode,2,"Радиальный");
                    });
                    if self.appearance_gradient_mode!=0{
                        ui.horizontal(|ui|{
                            ui.label("Начальный #RGB");
                            ui.text_edit_singleline(&mut self.appearance_gradient_start);
                        });
                        ui.horizontal(|ui|{
                            ui.label("Конечный #RGB");
                            ui.text_edit_singleline(&mut self.appearance_gradient_end);
                        });
                        if self.appearance_gradient_mode==1{
                            ui.horizontal(|ui|{
                                ui.label("Угол:");
                                ui.add(egui::DragValue::new(&mut self.appearance_gradient_angle)
                                    .speed(1.0).range(-360.0..=360.0).suffix("°"));
                            });
                        }
                    }
                    ui.separator();
                    ui.horizontal(|ui|{
                        ui.label("Пунктир, мм (0 — без пунктира)");
                        ui.add(egui::DragValue::new(&mut self.appearance_dash_mm)
                            .speed(0.2).range(0.0..=100.0));
                    });
                    ui.label("Окончание линии:");
                    ui.horizontal(|ui|{
                        ui.selectable_value(&mut self.appearance_cap,0,"Прямое");
                        ui.selectable_value(&mut self.appearance_cap,1,"Круглое");
                        ui.selectable_value(&mut self.appearance_cap,2,"Квадратное");
                    });
                    ui.label("Соединение линии:");
                    ui.horizontal(|ui|{
                        ui.selectable_value(&mut self.appearance_join,0,"Острое");
                        ui.selectable_value(&mut self.appearance_join,1,"Круглое");
                        ui.selectable_value(&mut self.appearance_join,2,"Срез");
                    });
                    ui.separator();
                    if ui.button("Применить к выделению").clicked(){
                        self.apply_appearance_selection();ui.close_menu();
                    }
                    if ui.button("Сбросить оформление").clicked(){
                        self.clear_appearance_selection();ui.close_menu();
                    }
                    if ui.add_enabled(self.selection_ids().len()==2,
                        egui::Button::new("Пипетка: первый → второй")).clicked(){
                        self.copy_appearance_between_selected();ui.close_menu();
                    }
                    ui.small("Цвета градиента: #RRGGBB. Ctrl+Z — отмена.");
                });
                ui.menu_button("Окно",|ui|{
                    ui.checkbox(&mut self.show_inspector,"Показывать панель справа   F9");
                    ui.separator();
                    if ui.selectable_label(self.inspector_tab==InspectorTab::Objects,"Объекты и слои").clicked(){
                        self.inspector_tab=InspectorTab::Objects;self.show_inspector=true;ui.close_menu();
                    }
                    if ui.selectable_label(self.inspector_tab==InspectorTab::Properties,"Свойства").clicked(){
                        self.inspector_tab=InspectorTab::Properties;self.show_inspector=true;ui.close_menu();
                    }
                    if ui.selectable_label(self.inspector_tab==InspectorTab::Page,"Страница").clicked(){
                        self.inspector_tab=InspectorTab::Page;self.show_inspector=true;ui.close_menu();
                    }
                });
                ui.menu_button("Справка",|ui|{
                    if ui.button("Начало работы и горячие клавиши — F1").clicked(){
                        self.show_welcome=true;ui.close_menu();
                    }
                    if ui.button("Открыть учебный макет").clicked(){self.sample_design();ui.close_menu();}
                    ui.separator();
                    ui.label("OpenCorel — независимый векторный редактор (альфа)");
                    ui.label("CDR-экспорт, CMYK и PDF/X пока отсутствуют.");
                    ui.hyperlink_to("Исходники и задачи","https://github.com/konstive-collab/OpenCorel");
                });
            });
        });
        egui::TopBottomPanel::top("main_toolbar").exact_height(44.0).show(ctx,|ui|{
            ui.horizontal(|ui|{
                if ui.button("Новый").on_hover_text("Новый пустой документ, Ctrl+N").clicked(){self.new_document();}
                if ui.button("Открыть").on_hover_text("Открыть KDR, SVG, CDR и другие файлы, Ctrl+O").clicked(){self.open_picker();}
                if ui.button("Сохранить").on_hover_text("Сохранить .kdr, Ctrl+S").clicked(){self.save();}
                ui.separator();
                if ui.add_enabled(!self.undo.is_empty(),egui::Button::new("↶")).on_hover_text("Отменить Ctrl+Z").clicked(){self.undo();}
                if ui.add_enabled(!self.redo.is_empty(),egui::Button::new("↷")).on_hover_text("Повторить Ctrl+Y").clicked(){self.redo();}
                ui.separator();
                if ui.button("Фото / логотип").on_hover_text("Добавить PNG, JPEG, WebP и другие изображения").clicked(){self.place_bitmap_picker();}
                ui.menu_button("Экспорт",|ui|{
                    if ui.button("SVG — редактируемый обмен...").clicked(){self.export_dialog("svg");ui.close_menu();}
                    if ui.button("PDF — проверочный RGB...").clicked(){self.export_dialog("pdf");ui.close_menu();}
                    if ui.button("PNG — для согласования...").clicked(){self.export_dialog("png");ui.close_menu();}
                    if ui.button("JPEG — для согласования...").clicked(){self.export_dialog("jpg");ui.close_menu();}
                });
                ui.separator();
                if ui.small_button("−").on_hover_text("Уменьшить масштаб").clicked(){self.zoom=(self.zoom/1.25).max(0.2);}
                ui.label(format!("{:.0}%",self.zoom*100.0));
                if ui.small_button("+").on_hover_text("Увеличить масштаб").clicked(){self.zoom=(self.zoom*1.25).min(8.0);}
                if ui.button("Вписать").on_hover_text("Показать лист целиком, F4").clicked(){self.fit_page();}
                ui.separator();
                if ui.button("? Помощь").on_hover_text("Обучение и сочетания клавиш, F1").clicked(){self.show_welcome=true;}
            });
        });
        egui::TopBottomPanel::top("property_bar").exact_height(39.0).show(ctx,|ui|{
            ui.horizontal(|ui|{
                ui.label(RichText::new("Макет").strong());
                ui.label(self.doc.name.chars().take(28).collect::<String>())
                    .on_hover_text(&self.file);
                ui.separator();
                if let Some(bounds)=self.selection_bounds(){
                    ui.label(format!("Выделено: {}",self.selection_ids().len()));
                    ui.label(format!("X {:.1}  Y {:.1}  Ш {:.1}  В {:.1} мм",
                        bounds.x,bounds.y,bounds.w,bounds.h));
                    ui.separator();
                    if self.selection_ids().len()==2{
                        if ui.small_button("Создать маску").on_hover_text("Первый выбранный объект — контур; второй — содержимое").clicked(){
                            self.apply_clipping_selection();
                        }
                        ui.menu_button("Логические операции",|ui|{
                            if ui.button("Объединить").clicked(){
                                self.apply_boolean(opencorel_core::boolean_ops::BooleanOperation::Union);
                                ui.close_menu();
                            }
                            if ui.button("Вычесть второй").clicked(){
                                self.apply_boolean(opencorel_core::boolean_ops::BooleanOperation::Subtract);
                                ui.close_menu();
                            }
                            if ui.button("Пересечение").clicked(){
                                self.apply_boolean(opencorel_core::boolean_ops::BooleanOperation::Intersect);
                                ui.close_menu();
                            }
                            if ui.button("Исключить (XOR)").clicked(){
                                self.apply_boolean(opencorel_core::boolean_ops::BooleanOperation::Xor);
                                ui.close_menu();
                            }
                        });
                    }
                    if ui.small_button("Центр листа").on_hover_text("Поместить выбранные объекты по центру страницы").clicked(){
                        self.center_selection_on_page();
                    }
                    if ui.small_button("Копия").on_hover_text("Ctrl+D").clicked(){
                        self.duplicate_preserving_styles();
                    }
                    if ui.small_button("Удалить").on_hover_text("Delete").clicked(){
                        self.delete_selection();
                    }
                }else{
                    ui.label(format!("Лист {:.0} × {:.0} мм",
                        self.doc.page.width_mm,self.doc.page.height_mm));
                    ui.separator();
                    ui.label(format!("{}  ·  {}",
                        productivity::tool_label(self.tool),
                        match self.tool{
                            Tool::Select=>"V",
                            Tool::Nodes=>"F10",
                            Tool::Text=>"F8",
                            Tool::Curve=>"B",
                            Tool::Rect=>"F6",
                            Tool::Ellipse=>"F7",
                            Tool::Line=>"L",
                            Tool::Polygon=>"P",
                        }))
                        .on_hover_text(productivity::tool_hint(self.tool));
                }
            });
        });
    }

    /// Compact by default: a 64px tool rail frees space for the document.
    /// Full tool names and hints are available via tooltips and F1.
    pub(super) fn tool_sidebar(&mut self,ctx:&egui::Context){
        self.draw_tool_dock(ctx);
    }

    pub(super) fn color_palette(&mut self,ctx:&egui::Context){
        egui::SidePanel::right("color_palette").exact_width(55.0).resizable(false).show(ctx,|ui|{
            ui.vertical_centered(|ui|{
                ui.label(RichText::new("Цвет").small()).on_hover_text(
                    "Левый щелчок — заливка, правый — обводка");
                ui.label(RichText::new("ЛКМ/ПКМ").small()).on_hover_text(
                    "ЛКМ: цвет заливки. ПКМ: цвет контура.");
                for swatch in SWATCHES {
                    let response=ui.add_sized([28.0,20.0],
                        egui::Button::new("").fill(color(swatch)))
                        .on_hover_text(format!("{swatch}\nЛКМ: заливка\nПКМ: контур"));
                    if response.clicked(){self.apply_color(swatch,false);}
                    if response.clicked_by(egui::PointerButton::Secondary){self.apply_color(swatch,true);}
                }
                ui.label("×");
                if ui.small_button("Без зал.").on_hover_text("Убрать заливку").clicked(){self.apply_color("none",false);}
                if ui.small_button("Без конт.").on_hover_text("Убрать обводку").clicked(){self.apply_color("none",true);}
            });
        });
    }

    pub(super) fn inspector(&mut self,ctx:&egui::Context){
        egui::SidePanel::right("inspector_docker").default_width(272.0)
            .width_range(220.0..=450.0).resizable(true).show(ctx,|ui|{
            ui.label(RichText::new("ПАНЕЛЬ УПРАВЛЕНИЯ").strong().size(11.0)
                .color(Color32::from_rgb(35,183,159)));
            ui.horizontal(|ui|{
                ui.selectable_value(&mut self.inspector_tab,InspectorTab::Objects,"Объекты");
                ui.selectable_value(&mut self.inspector_tab,InspectorTab::Properties,"Свойства");
                ui.selectable_value(&mut self.inspector_tab,InspectorTab::Page,"Лист");
            });
            ui.separator();
            if self.inspector_tab==InspectorTab::Objects{
                // The virtualized object list owns its own scroll viewport.
                self.inspector_objects(ui);
            }else{
                // Long typography/path properties must remain reachable on
                // compact laptop displays rather than being clipped offscreen.
                egui::ScrollArea::vertical()
                    .id_salt("properties_docker")
                    .auto_shrink([false,false])
                    .show(ui,|ui|{
                        match self.inspector_tab{
                            InspectorTab::Properties=>self.inspector_properties(ui),
                            InspectorTab::Page=>self.inspector_page(ui),
                            InspectorTab::Objects=>{},
                        }
                    });
            }
        });
    }

    fn inspector_objects(&mut self,ui:&mut egui::Ui){
        ui.horizontal(|ui|{
            ui.heading("СЛОИ");
            if ui.small_button("+ Слой").on_hover_text("Создать новый слой").clicked(){
                self.checkpoint();
                let name=format!("Слой {}",self.doc.layers.len()+1);
                self.active_layer=self.doc.add_layer(name);
            }
        });
        let layers:Vec<_>=self.doc.layers.iter()
            .map(|layer|(layer.id,layer.name.clone(),layer.visible,layer.locked)).collect();
        for (id,name,mut visible,mut locked) in layers{
            ui.horizontal(|ui|{
                if ui.selectable_label(self.active_layer==id,
                    format!("{} {}",if visible{"●"}else{"○"},name)).clicked(){
                    self.active_layer=id;
                }
                if ui.checkbox(&mut visible,"").on_hover_text("Видимость").changed(){
                    self.checkpoint();
                    if let Some(layer)=self.doc.layers.iter_mut().find(|l|l.id==id){
                        layer.visible=visible;
                    }
                }
                if ui.checkbox(&mut locked,"").on_hover_text("Блокировка").changed(){
                    self.checkpoint();
                    if let Some(layer)=self.doc.layers.iter_mut().find(|l|l.id==id){
                        layer.locked=locked;
                    }
                }
            });
        }
        ui.separator();
        ui.horizontal(|ui|{
            ui.heading("ОБЪЕКТЫ");
            ui.label(RichText::new(format!("{}",self.doc.objects.len()))
                .color(Color32::from_rgb(33,187,161)));
        });
        ui.add(egui::TextEdit::singleline(&mut self.object_filter)
            .hint_text("Поиск объекта или названия..."));
        ui.horizontal(|ui|{
            if ui.add_enabled(self.selection_ids().len()>=2,
                egui::Button::new("Группа  Ctrl+G")).clicked(){
                self.group_selection();
            }
            if ui.button("Разгрупп.").on_hover_text("Ctrl+Shift+G").clicked(){
                self.ungroup_selection();
            }
        });
        let query=self.object_filter.trim().to_lowercase();
        let filtered:Option<Vec<usize>>=if query.is_empty(){None}else{
            Some(self.doc.objects.iter().enumerate().filter_map(|(index,obj)|{
                let label=if !obj.name.is_empty(){obj.name.to_lowercase()}else{
                    match &obj.shape{
                        Shape::Text{value,..}=>value.to_lowercase(),
                        Shape::Rectangle{..}=>"прямоугольник".into(),
                        Shape::Ellipse{..}=>"эллипс".into(),
                        Shape::Raster{..}=>"фото изображение".into(),
                        Shape::EmbeddedSvg{..}=>"вектор".into(),
                        Shape::CompoundPath{..}=>"контуры".into(),
                        Shape::Bezier{..}=>"кривая".into(),
                        Shape::Polyline{..}=>"линия".into(),
                    }
                };
                if label.contains(&query)||obj.id.to_string().contains(&query){
                    Some(index)
                }else{None}
            }).collect())
        };
        let count=filtered.as_ref().map_or(self.doc.objects.len(),Vec::len);
        ui.label(RichText::new(format!("Показано: {count}. Shift — множественное выделение."))
            .small().color(Color32::GRAY));
        egui::ScrollArea::vertical().id_salt("objects_list")
            .show_rows(ui,26.0,count,|ui,rows|{
                for row in rows{
                    let index=if let Some(ref filtered)=filtered{
                        filtered[count-1-row]
                    }else{count-1-row};
                    let obj=&self.doc.objects[index];
                    let id=obj.id;let layer_id=obj.layer_id;
                    let display=if obj.name.is_empty(){
                        match &obj.shape{
                            Shape::Rectangle{..}=>"▭ Прямоугольник".to_string(),
                            Shape::Ellipse{..}=>"◯ Эллипс".into(),
                            Shape::Text{value,..}=>format!("T {}",value.chars().take(20).collect::<String>()),
                            Shape::Bezier{..}=>"⌁ Безье".into(),
                            Shape::CompoundPath{contours}=>format!("✒ Контуры ({})",contours.len()),
                            Shape::Polyline{..}=>"╱ Линия".into(),
                            Shape::EmbeddedSvg{source_format,..}=>format!("▧ Вектор {source_format}"),
                            Shape::Raster{pixel_width,pixel_height,..}=>format!("▧ Фото {pixel_width}×{pixel_height}"),
                        }
                    }else{obj.name.clone()};
                    let name=if let Some(group)=obj.group_id{
                        format!("▣ {}  · G{}",display,group)
                    }else{display};
                    let visible=self.doc.layer(layer_id).map(|l|l.visible).unwrap_or(false);
                    let button=egui::Button::new(name)
                        .selected(self.is_selected(id))
                        .min_size(Vec2::new(ui.available_width(),23.0));
                    if ui.add_enabled(visible,button).clicked(){
                        let shift=ui.input(|i|i.modifiers.shift);
                        self.select_object(Some(id),shift);
                        self.active_layer=layer_id;
                        self.inspector_tab=InspectorTab::Properties;
                    }
                }
            });
    }

    fn inspector_properties(&mut self,ui:&mut egui::Ui){
        // Context tools live here, not in the permanent left rail. A user
        // can always find the active text and node controls even with the
        // minimal panel collapsed to just icons.
        if self.tool==Tool::Text{
            ui.heading("Новая надпись");
            ui.label("Введите текст, затем щёлкните на странице для размещения.");
            ui.add(egui::TextEdit::multiline(&mut self.text)
                .desired_rows(4).hint_text("Введите надпись..."));
            ui.label("F8 — текст. Esc — выделение. Шрифт можно изменить после создания.");
            return;
        }
        if self.tool==Tool::Nodes{
            ui.heading("Редактирование узлов");
            ui.label("Клик по кривой — выбор. Двойной клик по сегменту — новый узел.");
            if let Some((id,address))=self.active_node{
                ui.label(format!("Узел: {address:?}"));
                let location=self.doc.objects.iter().find(|o|o.id==id)
                    .and_then(|o|opencorel_core::node_edit::point_at(&o.shape,address));
                if let Some(p)=location{
                    let (mut x,mut y)=(p.x,p.y);
                    let mut changed=false;
                    ui.horizontal(|ui|{
                        ui.label("X, мм");
                        changed|=ui.add(egui::DragValue::new(&mut x)
                            .speed(0.25).range(-100_000.0..=100_000.0)).changed();
                    });
                    ui.horizontal(|ui|{
                        ui.label("Y, мм");
                        changed|=ui.add(egui::DragValue::new(&mut y)
                            .speed(0.25).range(-100_000.0..=100_000.0)).changed();
                    });
                    if changed{self.nudge_active_node(x-p.x,y-p.y);}
                    if ui.button("Удалить выбранный узел  Del").clicked(){
                        self.remove_active_node();
                    }
                    let has_pair=self.doc.objects.iter().find(|o|o.id==id)
                        .is_some_and(|o|
                            opencorel_core::node_edit::has_tangent_pair(&o.shape,address));
                    if has_pair{
                        use opencorel_core::node_edit::TangentOperation;
                        ui.separator();
                        let current=self.doc.objects.iter().find(|o|o.id==id)
                            .and_then(|o|opencorel_core::node_edit::mode_at(
                                &o.node_modes,address));
                        let mode_text=match current{
                            Some(TangentOperation::Corner)=>"Угловой — независимые рычаги",
                            Some(TangentOperation::Smooth)=>"Гладкий — сохраняет направление",
                            Some(TangentOperation::Symmetric)=>"Симметричный — равные рычаги",
                            None=>"Свободный — рычаги независимы",
                        };
                        ui.label(format!("Режим узла: {mode_text}"));
                        ui.horizontal_wrapped(|ui|{
                            if ui.button("Угол")
                                .on_hover_text("Сбросить оба рычага в узел").clicked(){
                                self.apply_tangent_operation(TangentOperation::Corner);
                            }
                            if ui.button("Гладкий")
                                .on_hover_text("Сделать касательные коллинеарными; длины сохранить")
                                .clicked(){
                                self.apply_tangent_operation(TangentOperation::Smooth);
                            }
                            if ui.button("Симметрия")
                                .on_hover_text("Выровнять направления и длины обоих рычагов")
                                .clicked(){
                                self.apply_tangent_operation(TangentOperation::Symmetric);
                            }
                            if ui.button("Свободно")
                                .on_hover_text("Отключить автоматическую связь рычагов без изменения формы")
                                .clicked(){
                                self.unlink_active_tangents();
                            }
                        });
                        ui.small("Гладкий и Симметрия автоматически связывают рычаги при перетаскивании. Режим сохраняется в .kdr.");
                    }
                }
            }else{
                ui.label("Выберите опорную точку для точного позиционирования.");
            }
            if self.selected.is_some_and(|id|self.node_edit_allowed(id)){
                ui.separator();
                ui.label("Операции с контуром");
                ui.horizontal_wrapped(|ui|{
                    if ui.button("Развернуть направление")
                        .on_hover_text("Поменять начало и конец пути, сохранив форму и типы узлов")
                        .clicked(){
                        self.reverse_selected_path();
                    }
                    if ui.button("Замкнуть")
                        .on_hover_text("Соединить начало и конец контура")
                        .clicked(){
                        self.set_selected_path_closed(true);
                    }
                    if ui.button("Разомкнуть")
                        .on_hover_text("Убрать замыкающий сегмент")
                        .clicked(){
                        self.set_selected_path_closed(false);
                    }
                });
            }
            ui.small("F10 — узлы. Стрелки — сдвиг. Ctrl+Z — отмена.");
            ui.separator();
        }
        let Some(id)=self.selected else{
            ui.heading("Свойства объекта");
            ui.label("Щёлкните по объекту инструментом «Выделение» или выберите его во вкладке «Объекты».");
            ui.label("Объект можно перетащить, а размер изменить ручками по углам.");
            ui.label("F10 — редактирование узлов кривой прямо на холсте.");
            if self.tool==Tool::Text{
                ui.label("Содержимое нового текстового объекта:");
                ui.text_edit_multiline(&mut self.text);
            }
            return;
        };
        let Some(index)=self.doc.objects.iter().position(|o|o.id==id) else{
            self.selected=None;return;
        };
        ui.collapsing("Название и группа",|ui|{
            let mut label=self.doc.objects[index].name.clone();
            ui.label("Название объекта в панели слоёв:");
            if ui.add(egui::TextEdit::singleline(&mut label)
                .hint_text("Например: логотип, заголовок, фон...")).changed(){
                if label.len()<=256 && !label.chars().any(|c|c.is_control()){
                    self.checkpoint();
                    self.doc.objects[index].name=label;
                }else{
                    self.status="Название: максимум 256 байт, без управляющих символов".into();
                }
            }
            if let Some(group)=self.doc.objects[index].group_id{
                ui.label(format!("Группа #{group}: {} элементов",
                    self.doc.group_members(id).len()));
                if ui.button("Разгруппировать    Ctrl+Shift+G").clicked(){
                    self.ungroup_selection();
                }
            }else if self.selection_ids().len()>=2{
                if ui.button("Сгруппировать выделенное    Ctrl+G").clicked(){
                    self.group_selection();
                }
            }
        });
        ui.collapsing("Точное положение и размер (мм)",|ui|{
            if let Some(bounds)=self.selection_bounds(){
                let (mut x,mut y)=(bounds.x,bounds.y);
                let mut pos_change=false;
                ui.horizontal(|ui|{
                    ui.label("X");
                    pos_change|=ui.add(egui::DragValue::new(&mut x)
                        .speed(0.5).range(-100_000.0..=100_000.0)).changed();
                    ui.label("Y");
                    pos_change|=ui.add(egui::DragValue::new(&mut y)
                        .speed(0.5).range(-100_000.0..=100_000.0)).changed();
                });
                if pos_change{self.set_selection_position(x,y);}
                if self.multi_selected.is_empty() &&
                    opencorel_core::transform::resizable(&self.doc.objects[index].shape){
                    let (mut w,mut h)=(bounds.w,bounds.h);
                    let mut size_change=false;
                    ui.horizontal(|ui|{
                        ui.label("Ш");
                        size_change|=ui.add(egui::DragValue::new(&mut w)
                            .speed(1.0).range(0.1..=100_000.0)).changed();
                        ui.label("В");
                        size_change|=ui.add(egui::DragValue::new(&mut h)
                            .speed(1.0).range(0.1..=100_000.0)).changed();
                    });
                    if size_change{self.resize_single(w,h);}
                }else{
                    ui.label(RichText::new("Размер группы меняется через отдельные элементы.")
                        .small().color(Color32::GRAY));
                }
            }
        });
        ui.separator();
        // Images may contain tens of megabytes of Base64. Never clone the
        // whole object per GUI frame just to show a few numeric fields.
        let asset=match &self.doc.objects[index].shape{
            Shape::Raster{x,y,w,h,pixel_width,pixel_height,..}=>
                Some((true,*x,*y,*w,*h,*pixel_width,*pixel_height,String::new())),
            Shape::EmbeddedSvg{x,y,w,h,source_format,..}=>
                Some((false,*x,*y,*w,*h,0,0,source_format.clone())),
            _=>None
        };
        if let Some((raster,mut x,mut y,mut width,mut height,pw,ph,source))=asset{
            let allowed=self.doc.layer(self.doc.objects[index].layer_id)
                .map(|l|l.visible&&!l.locked).unwrap_or(false);
            ui.heading(format!("Объект #{id}"));
            if self.multi_selected.len()>0{
                ui.label(format!("Выбрано {} объектов",self.selection_ids().len()));
            }
            if !allowed{ui.colored_label(Color32::RED,"Слой заблокирован");}
            let mut changed=false;
            ui.add_enabled_ui(allowed,|ui|{
                if raster{
                    ui.label(format!("Фотография {} × {} пикселей",pw,ph));
                    if width>0.0 {ui.label(format!("Фактическое разрешение: {:.0} DPI",
                        pw as f32*25.4/width));}
                    ui.checkbox(&mut self.keep_bitmap_ratio,"Сохранять пропорции");
                }else{
                    ui.label(format!("Векторный макет: {source}"));
                    ui.label("Контуры внутри импортированного объекта пока не редактируются");
                }
                for (name,num) in [("X мм",&mut x),("Y мм",&mut y)]{
                    ui.horizontal(|ui|{
                        ui.label(name);
                        changed|=ui.add(egui::DragValue::new(num).speed(1.0)).changed();
                    });
                }
                ui.horizontal(|ui|{
                    ui.label("Ширина, мм");
                    if ui.add(egui::DragValue::new(&mut width).speed(1.0)).changed(){
                        width=width.max(0.1);
                        if raster && self.keep_bitmap_ratio{
                            height=width*ph as f32/pw as f32;
                        }
                        changed=true;
                    }
                });
                ui.horizontal(|ui|{
                    ui.label("Высота, мм");
                    if ui.add(egui::DragValue::new(&mut height).speed(1.0)).changed(){
                        height=height.max(0.1);
                        if raster && self.keep_bitmap_ratio{
                            width=height*pw as f32/ph as f32;
                        }
                        changed=true;
                    }
                });
            });
            if changed{
                self.checkpoint();
                match &mut self.doc.objects[index].shape{
                    Shape::Raster{x:xx,y:yy,w:ww,h:hh,..} |
                    Shape::EmbeddedSvg{x:xx,y:yy,w:ww,h:hh,..}=>{
                        *xx=x;*yy=y;*ww=width;*hh=height;
                    }
                    _=>{}
                }
            }
            ui.separator();
            ui.add_enabled_ui(allowed,|ui|{
                ui.horizontal(|ui|{
                    if ui.button("Дублировать").clicked(){self.duplicate_selection();}
                    if ui.button("Удалить").clicked(){self.delete_selection();}
                });
                ui.horizontal(|ui|{
                    if ui.small_button("На передний план").clicked(){self.reorder_selection(true);}
                    if ui.small_button("На задний план").clicked(){self.reorder_selection(false);}
                });
            });
            return;
        }
        if matches!(self.doc.objects[index].shape,Shape::Text{..}){
            if ui.button("Преобразовать в кривые").on_hover_text(
                "Создаёт настоящий составной контур, отверстия букв сохраняются. Ctrl+Z — отмена."
            ).clicked(){
                self.text_to_curves();
                return;
            }
        }
        // A long outlined headline can contain thousands of path segments.
        // Clone only the visible inspector subset, never the full vector object
        // on every GUI frame.
        if let Shape::CompoundPath{contours}=&self.doc.objects[index].shape {
            let total=contours.len();
            let segments=contours.iter().map(|c|c.segments.len()).sum::<usize>();
            let mut sample:Vec<opencorel_core::Contour>=contours.iter()
                .take(24).map(|c|opencorel_core::Contour{
                    start:c.start,closed:c.closed,
                    segments:c.segments.iter().take(20).copied().collect()
                }).collect();
            let mut style=self.doc.objects[index].style.clone();
            let allowed=self.doc.layer(self.doc.objects[index].layer_id)
                .map(|layer|layer.visible&&!layer.locked).unwrap_or(false);
            ui.heading(format!("Составной векторный объект #{id}"));
            ui.label(format!("Контуров: {total}, сегментов: {segments}"));
            ui.label("Отверстия букв сохраняются (even-odd).");
            if !allowed {ui.colored_label(Color32::RED,"Слой заблокирован");}
            let mut changed=false;
            ui.add_enabled_ui(allowed,|ui|{
                ui.collapsing("Изменить координаты контуров",|ui|{
                    ui.label("Показаны первые 24 контура и до 20 сегментов каждого.");
                    for (i,c) in sample.iter_mut().enumerate(){
                        ui.collapsing(format!("Контур {}",i+1),|ui|{
                            changed|=ui.add(egui::DragValue::new(&mut c.start.x)
                                .prefix("Начало X: ")).changed();
                            changed|=ui.add(egui::DragValue::new(&mut c.start.y)
                                .prefix("Начало Y: ")).changed();
                            for (j,segment) in c.segments.iter_mut().enumerate(){
                                ui.collapsing(format!("Сегмент {}",j+1),|ui|{
                                    for (name,p) in [
                                        ("Рычаг 1",&mut segment.control1),
                                        ("Рычаг 2",&mut segment.control2),
                                        ("Узел",&mut segment.end)
                                    ]{
                                        ui.label(name);
                                        changed|=ui.add(egui::DragValue::new(&mut p.x)
                                            .prefix("X: ")).changed();
                                        changed|=ui.add(egui::DragValue::new(&mut p.y)
                                            .prefix("Y: ")).changed();
                                    }
                                });
                            }
                        });
                    }
                });
                ui.separator();
                ui.label("Заливка:");
                changed|=ui.text_edit_singleline(&mut style.fill).changed();
                ui.label("Обводка:");
                changed|=ui.text_edit_singleline(&mut style.stroke).changed();
                changed|=ui.add(egui::DragValue::new(&mut style.stroke_width_mm)
                    .speed(0.1).range(0.0..=1000.0).prefix("Контур, мм: ")).changed();
            });
            if changed{
                if valid_paint(&style.fill)&&valid_paint(&style.stroke){
                    self.checkpoint();
                    if let Shape::CompoundPath{contours}=&mut self.doc.objects[index].shape{
                        for (original,edited) in contours.iter_mut().zip(sample.iter()){
                            original.start=edited.start;
                            for (dst,src) in original.segments.iter_mut()
                                .zip(edited.segments.iter()) {*dst=*src;}
                        }
                    }
                    self.doc.objects[index].style=style;
                    self.compound_cache.clear();
                }else {
                    self.status="Цвет задаётся в виде #RRGGBB или none".into();
                }
            }
            ui.separator();
            ui.horizontal(|ui|{
                if ui.button("Дублировать").clicked(){self.duplicate_selection();}
                if ui.button("Удалить").clicked(){self.delete_selection();}
            });
            return;
        }
        let mut object=self.doc.objects[index].clone();
        let editing_allowed=self.doc.layer(object.layer_id).map(|l|l.visible&&!l.locked).unwrap_or(false);
        ui.heading(format!("Объект #{id}"));
        if !self.multi_selected.is_empty(){
            ui.label(format!("Выделено {} объектов. Свойства основного объекта:",self.selection_ids().len()));
        }
        if !editing_allowed {ui.colored_label(Color32::RED,"Слой скрыт или заблокирован");}
        ui.add_enabled_ui(editing_allowed,|ui|{
            let mut changed=false;
            match &mut object.shape{
                Shape::Rectangle{x,y,w,h}|Shape::Ellipse{x,y,w,h}=>{
                    for (label,value) in [("X мм",x),("Y мм",y),("Ширина мм",w),("Высота мм",h)]{
                        ui.horizontal(|ui|{
                            ui.label(label);
                            changed|=ui.add(egui::DragValue::new(value).speed(1.0)).changed();
                        });
                    }
                    *w=w.max(0.1);
                    *h=h.max(0.1);
                }
                Shape::Text{x,y,value,size_mm}=>{
                    ui.label("Текст:");
                    changed|=ui.text_edit_multiline(value).changed();
                    ui.horizontal(|ui|{
                        if ui.small_button("АБВ → верхний регистр")
                            .on_hover_text("Unicode-преобразование, включая кириллицу")
                            .clicked(){
                            let converted=value.to_uppercase();
                            if *value!=converted{*value=converted;changed=true;}
                        }
                        if ui.small_button("абв → нижний регистр").clicked(){
                            let converted=value.to_lowercase();
                            if *value!=converted{*value=converted;changed=true;}
                        }
                    });
                    ui.label(format!("Символов: {} · Строк: {}",
                        value.chars().count(),value.lines().count().max(1)));
                    ui.collapsing("Найти и заменить в надписи",|ui|{
                        ui.add(egui::TextEdit::singleline(&mut self.text_find)
                            .hint_text("Искать точное совпадение"));
                        ui.add(egui::TextEdit::singleline(&mut self.text_replacement)
                            .hint_text("Заменить на..."));
                        let count=if self.text_find.is_empty(){0}else{
                            value.matches(self.text_find.as_str()).count()
                        };
                        ui.label(format!("Совпадений: {count} · Поиск с учётом регистра"));
                        if ui.add_enabled(count>0,egui::Button::new("Заменить все")).clicked(){
                            match text_edit_ops::replace_all_literal(
                                value,&self.text_find,&self.text_replacement){
                                Ok((updated,replaced))=>{
                                    if updated!=*value{
                                        *value=updated;
                                        changed=true;
                                        self.status=format!(
                                            "Заменено совпадений в тексте: {replaced}");
                                    }
                                },
                                Err(reason)=>{self.status=format!("Замена отклонена: {reason}");}
                            }
                        }
                    });
                    for (label,num) in [("X мм",x),("Y мм",y),("Размер мм",size_mm)]{
                        ui.horizontal(|ui|{
                            ui.label(label);
                            changed|=ui.add(egui::DragValue::new(num).speed(0.5)).changed();
                        });
                    }
                    *size_mm=size_mm.clamp(0.1,10_000.0);
                }
                Shape::Raster{x,y,w,h,pixel_width,pixel_height,..}=>{
                    ui.label(format!("Растровое изображение: {} × {} пикселей",pixel_width,pixel_height));
                    let effective_dpi=if *w>0.0{*pixel_width as f32*25.4 / *w}else{0.0};
                    ui.label(format!("Разрешение по ширине: {:.0} DPI",effective_dpi));
                    ui.checkbox(&mut self.keep_bitmap_ratio,"Сохранять пропорции");
                    for (label,num) in [("X мм",x),("Y мм",y)]{
                        ui.horizontal(|ui|{
                            ui.label(label);
                            changed|=ui.add(egui::DragValue::new(num).speed(1.0)).changed();
                        });
                    }
                    ui.horizontal(|ui|{
                        ui.label("Ширина мм");
                        let edit=ui.add(egui::DragValue::new(w).speed(1.0));
                        if edit.changed(){
                            *w=w.max(0.1);
                            if self.keep_bitmap_ratio {
                                *h=*w*(*pixel_height as f32)/(*pixel_width as f32);
                            }
                            changed=true;
                        }
                    });
                    ui.horizontal(|ui|{
                        ui.label("Высота мм");
                        let edit=ui.add(egui::DragValue::new(h).speed(1.0));
                        if edit.changed(){
                            *h=h.max(0.1);
                            if self.keep_bitmap_ratio {
                                *w=*h*(*pixel_width as f32)/(*pixel_height as f32);
                            }
                            changed=true;
                        }
                    });
                }
                Shape::EmbeddedSvg{x,y,w,h,source_format,..}=>{
                    ui.colored_label(Color32::from_rgb(187,105,28),
                        format!("{source_format}: единый векторный объект"));
                    ui.label("Внутренние контуры пока не редактируются.");
                    for (label,num) in [("X мм",x),("Y мм",y),("Ширина",w),("Высота",h)]{
                        ui.horizontal(|ui|{
                            ui.label(label);
                            changed|=ui.add(egui::DragValue::new(num).speed(1.0)).changed();
                        });
                    }
                    *w=w.max(1.0);*h=h.max(1.0);
                }
                Shape::Polyline{points,closed}=>{
                    ui.label(format!("Узлов: {}",points.len()));
                    changed|=ui.checkbox(closed,"Замкнуто").changed();
                }
                Shape::CompoundPath{contours}=>{
                    let segment_count:usize=contours.iter().map(|c|c.segments.len()).sum();
                    ui.label(format!("Составной контур: {} частей, {} кубических сегментов",
                        contours.len(),segment_count));
                    ui.label("Внутренние отверстия сохраняются по правилу even-odd.");
                    ui.label("Координаты ограничены первыми 24 контурами для быстродействия.");
                    for (i,contour) in contours.iter_mut().take(24).enumerate(){
                        ui.collapsing(format!("Контур {}",i+1),|ui|{
                            changed|=ui.add(egui::DragValue::new(&mut contour.start.x)
                                .prefix("X: ")).changed();
                            changed|=ui.add(egui::DragValue::new(&mut contour.start.y)
                                .prefix("Y: ")).changed();
                            for (j,seg) in contour.segments.iter_mut().take(20).enumerate(){
                                ui.collapsing(format!("Сегмент {}",j+1),|ui|{
                                    for (label,p) in [
                                        ("Рычаг 1",&mut seg.control1),
                                        ("Рычаг 2",&mut seg.control2),
                                        ("Узел",&mut seg.end)]{
                                        ui.label(label);
                                        changed|=ui.add(egui::DragValue::new(&mut p.x)
                                            .prefix("X: ")).changed();
                                        changed|=ui.add(egui::DragValue::new(&mut p.y)
                                            .prefix("Y: ")).changed();
                                    }
                                });
                            }
                        });
                    }
                }
                Shape::Bezier{start,segments,closed}=>{
                    ui.label(format!("Сегментов: {}",segments.len()));
                    changed|=ui.checkbox(closed,"Замкнуто").changed();
                    changed|=ui.add(egui::DragValue::new(&mut start.x).prefix("X: ")).changed();
                    changed|=ui.add(egui::DragValue::new(&mut start.y).prefix("Y: ")).changed();
                    for (i,seg) in segments.iter_mut().enumerate(){
                        ui.collapsing(format!("Узел {}",i+1),|ui|{
                            for (label,p) in [("Контроль 1",&mut seg.control1),
                                ("Контроль 2",&mut seg.control2),("Конец",&mut seg.end)]{
                                ui.label(label);
                                changed|=ui.add(egui::DragValue::new(&mut p.x).prefix("X: ")).changed();
                                changed|=ui.add(egui::DragValue::new(&mut p.y).prefix("Y: ")).changed();
                            }
                        });
                    }
                }
            }
            if matches!(object.shape,Shape::Text{..}){
                ui.separator();
                ui.label(RichText::new("Типографика").strong());
                ui.label("Название шрифта (системного):");
                changed|=ui.text_edit_singleline(&mut object.text_format.family).changed();
                ui.collapsing("Выбрать из установленных шрифтов",|ui|{
                    ui.add(egui::TextEdit::singleline(&mut self.font_search)
                        .hint_text("Поиск по названию шрифта..."));
                    let query=self.font_search.trim().to_lowercase();
                    let fonts=self.text_outliner.font_families();
                    let matches:Vec<_>=fonts.into_iter().filter(|name|
                        name.to_lowercase().contains(&query)).collect();
                    if matches.is_empty(){
                        ui.label(if query.is_empty() {
                            "Системные шрифты не обнаружены"
                        } else {
                            "По запросу шрифты не найдены"
                        });
                    }else{
                        ui.label(format!("Найдено семейств: {}",matches.len()));
                        egui::ScrollArea::vertical().max_height(180.0)
                            .id_salt("font_family_list")
                            .show_rows(ui,24.0,matches.len(),|ui,rows|{
                                for index in rows{
                                    let family=&matches[index];
                                    changed|=ui.selectable_value(
                                        &mut object.text_format.family,
                                        family.clone(),family
                                    ).changed();
                                }
                            });
                    }
                });
                if ui.button("Шрифт по умолчанию").clicked(){
                    object.text_format.family="sans-serif".into();changed=true;
                }
                if !self.text_outliner.font_available(&object.text_format.family){
                    ui.colored_label(Color32::from_rgb(231,156,69),
                        "Шрифт не найден в системе: возможна подмена при экспорте. Выберите установленный шрифт.");
                }
                ui.horizontal(|ui|{
                    ui.label("Начертание:");
                    egui::ComboBox::from_id_salt("text_weight")
                        .selected_text(format!("{}",object.text_format.weight))
                        .show_ui(ui,|ui|{
                            for (label,weight) in [
                                ("Тонкий",100),("Светлый",300),
                                ("Обычный",400),("Средний",500),
                                ("Полужирный",600),("Жирный",700),
                                ("Очень жирный",800),("Чёрный",900),
                            ]{
                                changed|=ui.selectable_value(&mut object.text_format.weight,
                                    weight,label).changed();
                            }
                        });
                    changed|=ui.checkbox(&mut object.text_format.italic,"Курсив").changed();
                });
                ui.label("Выравнивание строк:");
                ui.horizontal(|ui|{
                    for (label,align) in [
                        ("Слева",TextAlign::Left),("Центр",TextAlign::Center),
                        ("Справа",TextAlign::Right)
                    ]{
                        changed|=ui.selectable_value(&mut object.text_format.alignment,
                            align,label).changed();
                    }
                });
                ui.horizontal(|ui|{
                    ui.label("Межбуквенный интервал, мм");
                    changed|=ui.add(egui::DragValue::new(
                        &mut object.text_format.letter_spacing_mm)
                        .speed(0.05).range(-100.0..=100.0)).changed();
                });
                ui.horizontal(|ui|{
                    ui.label("Межстрочный множитель");
                    changed|=ui.add(egui::DragValue::new(
                        &mut object.text_format.line_height)
                        .speed(0.05).range(0.7..=4.0)).changed();
                });
                ui.label(RichText::new("На холсте — упрощённый предпросмотр. Шрифты и печать сверяйте по PDF.")
                    .small().color(Color32::GRAY));
            }
            ui.separator();
            ui.label("Заливка: #RRGGBB или none");
            changed|=ui.text_edit_singleline(&mut object.style.fill).changed();
            ui.label("Контур: #RRGGBB или none");
            changed|=ui.text_edit_singleline(&mut object.style.stroke).changed();
            ui.label("Толщина обводки (мм)");
            changed|=ui.add(egui::DragValue::new(&mut object.style.stroke_width_mm)
                .speed(0.1).range(0.0..=1000.0)).changed();
            if changed{
                if valid_paint(&object.style.fill)&&valid_paint(&object.style.stroke)
                    && object.text_format.validate().is_ok(){
                    self.checkpoint();
                    self.doc.objects[index]=object.clone();
                    self.compound_cache.clear();
                }else{
                    self.status=if !valid_paint(&object.style.fill) ||
                        !valid_paint(&object.style.stroke) {
                        "Цвет задаётся в виде #RRGGBB или none".into()
                    }else{
                        format!("Недопустимые параметры текста: {}",
                            object.text_format.validate().unwrap_err())
                    };
                }
            }
            ui.separator();
            ui.horizontal(|ui|{
                if ui.button("Дублировать").clicked(){self.duplicate_selection();}
                if ui.button("Удалить").clicked(){self.delete_selection();}
            });
            ui.horizontal(|ui|{
                if ui.small_button("На передний план").clicked(){self.reorder_selection(true);}
                if ui.small_button("На задний план").clicked(){self.reorder_selection(false);}
            });
        });
    }

    fn inspector_page(&mut self,ui:&mut egui::Ui){
        ui.heading("Параметры листа");
        ui.label("Наименование документа");
        let mut name=self.doc.name.clone();
        if ui.text_edit_singleline(&mut name).changed(){
            self.checkpoint();self.doc.name=name;
        }
        let (mut w,mut h)=(self.doc.page.width_mm,self.doc.page.height_mm);
        ui.label("Размер в миллиметрах:");
        let width=ui.add(egui::DragValue::new(&mut w).speed(1.0).range(1.0..=100_000.0)).changed();
        let height=ui.add(egui::DragValue::new(&mut h).speed(1.0).range(1.0..=100_000.0)).changed();
        if width || height{self.page_size(w,h);}
        ui.horizontal_wrapped(|ui|{
            if ui.small_button("A0").clicked(){self.page_size(841.0,1189.0);}
            if ui.small_button("A2").clicked(){self.page_size(420.0,594.0);}
            if ui.small_button("A4").clicked(){self.page_size(210.0,297.0);}
            if ui.small_button("A3").clicked(){self.page_size(297.0,420.0);}
            if ui.small_button("A1").clicked(){self.page_size(594.0,841.0);}
            if ui.small_button("3×2 м").clicked(){self.page_size(3000.0,2000.0);}
        });
        ui.separator();
        ui.checkbox(&mut self.show_grid,"Показать сетку");
        ui.checkbox(&mut self.snap_grid,"Привязка к сетке");
        ui.horizontal(|ui|{
            ui.label("Шаг сетки, мм");
            ui.add(egui::DragValue::new(&mut self.grid_mm)
                .speed(0.5).range(0.5..=1000.0));
        });
        ui.checkbox(&mut self.show_rulers,"Показать линейки");
        ui.checkbox(&mut self.show_safe_area,"Показать безопасную область");
        ui.horizontal(|ui|{
            ui.label("Отступ от края, мм");
            ui.add(egui::DragValue::new(&mut self.safe_margin_mm)
                .speed(0.5).range(0.0..=100.0));
        });
        ui.label("Оранжевая рамка — только экранная подсказка, в файл не выводится.");
        ui.separator();
        ui.heading("Предпечатная проверка");
        ui.horizontal(|ui|{
            ui.label("Порог DPI для фото");
            ui.add(egui::DragValue::new(&mut self.minimum_image_dpi)
                .speed(5.0).range(30.0..=600.0));
        });
        if ui.button("Проверить макет").clicked(){self.run_preflight();}
        ui.label("Один редактируемый лист. Многостраничность — следующий этап.");
    }

    pub(super) fn workspace_footer(&mut self,ctx:&egui::Context){
        egui::TopBottomPanel::bottom("statusbar").exact_height(26.0).show(ctx,|ui|{
            ui.horizontal(|ui|{
                ui.label(RichText::new(&self.status).color(
                    if self.status.starts_with("Ошибка"){Color32::from_rgb(242,115,115)}
                    else{Color32::from_rgb(171,202,206)}));
                ui.separator();
                let pos=if let Some(p)=self.cursor_mm{
                    format!("X: {:.1} мм   Y: {:.1} мм",p.x,p.y)
                }else{"X: —   Y: —".into()};
                ui.label(pos);
                ui.separator();
                ui.label(format!("Объектов: {}",self.doc.objects.len()));
                if !self.multi_selected.is_empty(){
                    ui.separator();
                    ui.label(format!("Выделено: {}",self.selection_ids().len()));
                }
            });
        });
        egui::TopBottomPanel::bottom("page_navigator").exact_height(29.0).show(ctx,|ui|{
            ui.horizontal(|ui|{
                ui.label(RichText::new("Страница 1 из 1").strong())
                    .on_hover_text("Многостраничные проекты пока не реализованы.");
                ui.separator();
                ui.label(RichText::new("ЦВЕТА").strong().small());
                for swatch in SWATCHES.iter().take(14){
                    let res=ui.add_sized([16.0,16.0],egui::Button::new("").fill(color(swatch)))
                        .on_hover_text(format!("{swatch} — заливка"));
                    if res.clicked(){self.apply_color(swatch,false);}
                }
            });
        });
    }

    pub(super) fn page_size(&mut self,w:f32,h:f32){
        if !w.is_finite()||!h.is_finite()||w<1.0||h<1.0||w>100_000.0||h>100_000.0{return;}
        if self.doc.page.width_mm==w && self.doc.page.height_mm==h{return;}
        self.checkpoint();
        self.doc.page.width_mm=w;
        self.doc.page.height_mm=h;
    }

    pub(super) fn fit_page(&mut self){
        self.zoom=1.0;
        self.pan=Vec2::ZERO;
    }

    fn apply_color(&mut self,value:&str,is_stroke:bool){
        if !valid_paint(value){return;}
        let ids:HashSet<u64>=self.selection_ids().into_iter().collect();
        if ids.is_empty(){self.status="Сначала выберите объект".into();return;}
        let editable:HashSet<u64>=self.doc.layers.iter()
            .filter(|l|l.visible&&!l.locked).map(|l|l.id).collect();
        if self.doc.objects.iter().filter(|o|ids.contains(&o.id))
            .any(|o|!editable.contains(&o.layer_id)){
            self.status="Цвет не изменён: часть выделения находится в закрытом слое".into();
            return;
        }
        if !self.doc.objects.iter().any(|o|ids.contains(&o.id)){return;}
        self.checkpoint();
        for obj in self.doc.objects.iter_mut(){
            if ids.contains(&obj.id)&&editable.contains(&obj.layer_id){
                if is_stroke{
                    obj.style.stroke=value.into();
                    if value!="none" && obj.style.stroke_width_mm==0.0{
                        obj.style.stroke_width_mm=0.5;
                    }
                }else{obj.style.fill=value.into();}
            }
        }
    }
    pub(super) fn delete_selection(&mut self){
        let ids:HashSet<u64>=self.selection_ids().into_iter().collect();
        if ids.is_empty(){return;}
        let editable:HashSet<u64>=self.doc.layers.iter()
            .filter(|l|l.visible&&!l.locked).map(|l|l.id).collect();
        if self.doc.objects.iter().any(|o|ids.contains(&o.id)&&
            !editable.contains(&o.layer_id)){
            self.status="Удаление отменено: выделение включает заблокированные слои".into();
            return;
        }
        if !self.doc.objects.iter().any(|o|ids.contains(&o.id)){return;}
        self.checkpoint();
        self.doc.objects.retain(|o|!ids.contains(&o.id));
        self.doc.clip_links.retain(|clip|
            !ids.contains(&clip.source_id)&&!ids.contains(&clip.target_id));
        self.doc.appearances.retain(|id,_|!ids.contains(id));
        self.selected=None;self.multi_selected.clear();
        self.status="Выделенные объекты удалены".into();
    }
    pub(super) fn duplicate_selection(&mut self){
        // Kept for existing keyboard/UI call sites; preserves text formatting.
        self.duplicate_preserving_styles();
    }
    pub(super) fn reorder_selection(&mut self,front:bool){
        let ids:HashSet<u64>=self.selection_ids().into_iter().collect();
        if ids.is_empty(){return;}
        let editable:HashSet<u64>=self.doc.layers.iter()
            .filter(|l|l.visible&&!l.locked).map(|l|l.id).collect();
        if self.doc.objects.iter().any(|o|ids.contains(&o.id)&&
            !editable.contains(&o.layer_id)){
            self.status="Порядок не изменён: слой заблокирован".into();return;
        }
        if !self.doc.objects.iter().any(|o|ids.contains(&o.id)){return;}
        self.checkpoint();
        let mut selected=Vec::new();
        let mut rest=Vec::new();
        for obj in self.doc.objects.drain(..){
            if ids.contains(&obj.id){selected.push(obj);}
            else{rest.push(obj);}
        }
        self.doc.objects=if front{
            rest.extend(selected);rest
        }else{
            selected.extend(rest);selected
        };
        self.status=if front{
            "Выделенные объекты перемещены на передний план".into()
        }else{
            "Выделенные объекты перемещены на задний план".into()
        };
    }
}
