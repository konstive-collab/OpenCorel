//! Corel-style, independently implemented workspace layout.
//! Uses familiar panel positions and workflows but no proprietary assets.
use super::{color, productivity, text_edit_ops, Editor, Tool};
use eframe::egui::{self, Color32, RichText, Vec2};
use opencorel_core::{Bounds, Shape, Alignment, Distribution, TextAlign, valid_paint};
use opencorel_core::object_studio::StackOrder;
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
        self.guide_drag=None;
        if next!=Tool::Freehand{
            self.freehand_samples.clear();self.freehand_overflow=false;
        }
        if next!=Tool::Curve {
            self.curve_start=None;self.curve_draft=None;
            self.curve_resumed=false;
            self.curve_outgoing=None;self.curve_drag_anchor=None;
        }
        self.drag_start=None;
        self.selection_drag_checkpointed=false;
        self.marquee_drag=None;
        self.creation_start=None;
        self.resize_drag=None;self.node_drag=None;
        self.resize_drag_base=None;self.resize_drag_checkpointed=false;
        self.node_drag_checkpointed=false;
        if next!=Tool::Nodes{self.active_node=None;}
        if matches!(next,Tool::Text|Tool::Nodes|Tool::Freehand|Tool::Smart){
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
        self.resize_drag=None;self.resize_drag_base=None;
        self.resize_drag_checkpointed=false;
        self.drag_start=None;self.selection_drag_checkpointed=false;
        self.marquee_drag=None;self.creation_start=None;
        self.active_layer=1;
        self.pan=Vec2::ZERO;
        self.zoom=1.0;
        self.curve_start=None;
        self.curve_draft=None;
        self.curve_resumed=false;
        self.curve_outgoing=None;
        self.curve_drag_anchor=None;
        self.freehand_samples.clear();self.freehand_overflow=false;
        self.guide_drag=None;
        self.guide_numeric_session=None;
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
                    ui.menu_button("Создать из шаблона",|ui|{
                        ui.small("Готовые редактируемые макеты — без растровой подложки");
                        for kind in super::starter_templates::StarterTemplate::ALL{
                            if ui.button(kind.title()).clicked(){
                                self.create_starter_template(kind);
                                ui.close_menu();
                            }
                        }
                    });
                    ui.separator();
                    if ui.button("Новый документ       Ctrl+N").clicked(){self.new_document();ui.close_menu();}
                    if ui.button("Открыть...         Ctrl+O").clicked(){self.open_picker();ui.close_menu();}
                    if ui.button("Импорт SVG/SVGZ...").clicked(){self.import_svg_picker();ui.close_menu();}
                    if ui.button("Поместить изображение...").clicked(){self.place_bitmap_picker();ui.close_menu();}
                    if ui.button("Сохранить           Ctrl+S").clicked(){self.save();ui.close_menu();}
                    if ui.button("Сохранить как...  Ctrl+Shift+S").clicked(){self.save_as_picker();ui.close_menu();}
                    if ui.button("Экспорт SVG...").clicked(){self.export_dialog("svg");ui.close_menu();}
                    if ui.button("Экспорт SVGZ...").clicked(){self.export_dialog("svgz");ui.close_menu();}
                    if ui.button("Экспорт PDF (все страницы, RGB)...").clicked(){self.export_dialog("pdf");ui.close_menu();}
                    ui.menu_button("Экспортировать все страницы...",|ui|{
                        ui.small("В отдельную папку, по одному файлу на страницу");
                        if ui.button("Все страницы → SVG").clicked(){
                            self.export_all_pages_dialog("svg");ui.close_menu();
                        }
                        if ui.button("Все страницы → PDF (RGB)").clicked(){
                            self.export_all_pages_dialog("pdf");ui.close_menu();
                        }
                    });
                    if ui.button("Печатный SVG с вылетами и метками реза...").clicked(){
                        self.export_print_sheet_dialog();ui.close_menu();
                    }
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
                    if ui.button("Дублировать     Ctrl+D").clicked(){self.object_duplicate_selection();ui.close_menu();}
                    ui.menu_button("Массив копий / сетка...",|ui|{
                        ui.label("Размножить выделенные объекты по строкам и столбцам");
                        ui.horizontal(|ui|{
                            ui.label("Столбцы");
                            ui.add(egui::DragValue::new(&mut self.array_columns).range(1..=50));
                            ui.label("Строки");
                            ui.add(egui::DragValue::new(&mut self.array_rows).range(1..=50));
                        });
                        ui.horizontal(|ui|{
                            ui.label("Зазор X, мм");
                            ui.add(egui::DragValue::new(&mut self.array_gap_x_mm)
                                .range(-9999.0..=10000.0).speed(0.5));
                        });
                        ui.horizontal(|ui|{
                            ui.label("Зазор Y, мм");
                            ui.add(egui::DragValue::new(&mut self.array_gap_y_mm)
                                .range(-9999.0..=10000.0).speed(0.5));
                        });
                        ui.small("Первая ячейка — исходные объекты; не более 2000 новых объектов.");
                        if ui.add_enabled(!self.selection_ids().is_empty(),
                            egui::Button::new("Создать сетку")).clicked(){
                            self.repeat_selection_as_grid();
                            ui.close_menu();
                        }
                    });
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
                    ui.checkbox(&mut self.show_guides,"Показать направляющие");
                    ui.checkbox(&mut self.snap_guides,"Привязка к направляющим");
                    ui.checkbox(&mut self.snap_nodes,"Привязка к узлам объектов");
                    if self.snap_nodes{
                        ui.horizontal(|ui|{
                            ui.label("Допуск, пикс.");
                            ui.add(egui::DragValue::new(&mut self.snap_tolerance_px)
                                .speed(0.2).range(2.0..=30.0));
                        });
                    }
                    ui.checkbox(&mut self.show_rulers,"Линейки");
                    ui.checkbox(&mut self.light_theme,"Светлая тема");
                });
                ui.menu_button("Текст",|ui|{
                    ui.label(RichText::new("ТЕКСТОВАЯ МАСТЕРСКА").strong());
                    ui.small("23 массовые операции и 7 стилевых пресетов");
                    ui.horizontal(|ui|{
                        ui.label("Перенос");
                        ui.add(egui::DragValue::new(&mut self.text_studio_wrap_columns)
                            .range(8..=160).suffix(" зн."));
                    });
                    ui.menu_button("Регистр и слова",|ui|{
                        for action in super::text_studio::TextAction::ALL.iter().take(6){
                            if ui.button(action.label()).clicked(){
                                self.run_text_action(*action);ui.close_menu();
                            }
                        }
                    });
                    ui.menu_button("Абзацы и списки",|ui|{
                        for action in super::text_studio::TextAction::ALL.iter().skip(6){
                            if ui.button(action.label()).clicked(){
                                self.run_text_action(*action);ui.close_menu();
                            }
                        }
                    });
                    ui.menu_button("Текст по кривой...",|ui|{
                        ui.set_min_width(315.0);
                        self.text_path_controls(ui);
                    });
                    ui.separator();
                    ui.menu_button("Стили типографики",|ui|{
                        for preset in [
                            super::text_studio::TextPreset::Display,
                            super::text_studio::TextPreset::Heading,
                            super::text_studio::TextPreset::Subheading,
                            super::text_studio::TextPreset::Body,
                            super::text_studio::TextPreset::Caption,
                            super::text_studio::TextPreset::Technical,
                            super::text_studio::TextPreset::Quote,
                        ]{
                            if ui.button(preset.label()).clicked(){
                                self.run_text_preset(preset);ui.close_menu();
                            }
                        }
                    });
                });
                ui.menu_button("Композиции",|ui|{
                    ui.set_min_width(275.0);
                    self.pattern_studio_panel(ui);
                });
                ui.menu_button("Эффекты",|ui|{
                    ui.set_min_width(285.0);
                    self.vector_effects_panel(ui);
                });
                ui.menu_button("Цвет",|ui|{
                    ui.set_min_width(330.0);
                    egui::ScrollArea::vertical().max_height(590.0).show(ui,|ui|{
                        self.color_studio_panel(ui);
                    });
                });
                ui.menu_button("Макет",|ui|{
                    ui.menu_button("Страницы документа",|ui|{
                        ui.label(format!("Страниц: {} / максимум 64",self.doc.page_count()));
                        for index in 0..self.doc.page_count(){
                            let name=self.doc.page_title(index).unwrap_or("Страница").to_string();
                            if ui.selectable_label(index==self.doc.active_page_index,
                                format!("{} — {}",index+1,name)).clicked(){
                                self.navigate_page(index);ui.close_menu();
                            }
                        }
                        ui.separator();
                        if ui.button("+ Пустая страница").clicked(){
                            self.create_page(false);ui.close_menu();
                        }
                        if ui.button("Дублировать текущую страницу").clicked(){
                            self.create_page(true);ui.close_menu();
                        }
                        if ui.add_enabled(self.doc.page_count()>1,
                            egui::Button::new("Удалить текущую страницу")).clicked(){
                            self.remove_current_page();ui.close_menu();
                        }
                    });
                    ui.separator();
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
                    ui.label(RichText::new("СОСТАВНЫЕ КОНТУРЫ (EVEN-ODD)").strong());
                    ui.small("Без сваривания: кривые сохраняют исходную геометрию, а пересечения меняют правило заливки.");
                    let selected_count=self.selection_ids().len();
                    if ui.add_enabled(selected_count>=2,
                        egui::Button::new("Объединить контуры (Combine)")).clicked(){
                        self.combine_selected_contours();ui.close_menu();
                    }
                    let one_compound=if selected_count==1{
                        self.selected.and_then(|id|self.doc.objects.iter()
                            .find(|o|o.id==id)).is_some_and(|o|
                                matches!(o.shape,opencorel_core::Shape::CompoundPath{..}))
                    }else{false};
                    if ui.add_enabled(one_compound,
                        egui::Button::new("Разделить контуры (Break Apart)")).clicked(){
                        self.break_apart_selected_contour();ui.close_menu();
                    }
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
                    ui.menu_button("Библиотека фигур — 93",|ui|{
                        ui.set_min_width(320.0);
                        self.smart_shape_picker(ui);
                    });
                    ui.separator();
                    ui.label(RichText::new("ГРУППЫ").strong());
                    if ui.button("Центрировать по листу").clicked(){
                        self.center_selection_on_page();ui.close_menu();
                    }
                    ui.menu_button("Выравнивание и распределение",|ui|{
                        use opencorel_core::{Alignment,Distribution};
                        let count=self.selection_ids().len();
                        ui.label("Выравнивание по общему габариту выделения");
                        ui.add_enabled_ui(count>=2,|ui|{
                            for (title,mode) in [
                                ("По левому краю",Alignment::Left),
                                ("По центру X",Alignment::CenterX),
                                ("По правому краю",Alignment::Right),
                                ("По верхнему краю",Alignment::Top),
                                ("По центру Y",Alignment::CenterY),
                                ("По нижнему краю",Alignment::Bottom),
                            ]{
                                if ui.button(title).clicked(){
                                    self.align_selection_precise(mode);ui.close_menu();
                                }
                            }
                        });
                        ui.separator();
                        ui.label("Равные интервалы между центрами объектов");
                        ui.add_enabled_ui(count>=3,|ui|{
                            if ui.button("По горизонтали").clicked(){
                                self.distribute_selection_precise(Distribution::HorizontalCenters);
                                ui.close_menu();
                            }
                            if ui.button("По вертикали").clicked(){
                                self.distribute_selection_precise(Distribution::VerticalCenters);
                                ui.close_menu();
                            }
                        });
                        ui.small("Ctrl+Z отменяет выравнивание одним действием.");
                    });
                    if ui.button("Группировать       Ctrl+G").clicked(){
                        self.group_selection();ui.close_menu();
                    }
                    if ui.button("Разгруппировать    Ctrl+Shift+G").clicked(){
                        self.ungroup_selection();ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("На передний план").clicked(){self.reorder_selection(true);ui.close_menu();}
                    if ui.button("На задний план").clicked(){self.reorder_selection(false);ui.close_menu();}
                    if ui.button("Дублировать").clicked(){self.object_duplicate_selection();ui.close_menu();}
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
                        self.object_duplicate_selection();
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
                            Tool::Freehand=>"H",
                            Tool::Rect=>"F6",
                            Tool::Ellipse=>"F7",
                            Tool::Line=>"L",
                            Tool::Polygon=>"P",
                            Tool::Smart=>"S", 
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
        use opencorel_core::object_studio::StackOrder;
        #[derive(Clone)]
        struct Row{
            group_header:bool,id:u64,group:Option<u64>,label:String,
            visible:bool,locked:bool,depth:usize,
        }
        ui.horizontal(|ui|{
            ui.heading("СЛОИ");
            ui.label(RichText::new(format!("{}",self.doc.layers.len()))
                .small().color(Color32::GRAY));
            if ui.small_button("+ Слой").on_hover_text("Создать новый слой").clicked(){
                self.checkpoint();
                let name=format!("Слой {}",self.doc.layers.len()+1);
                self.active_layer=self.doc.add_layer(name);
            }
        });
        let layers:Vec<_>=self.doc.layers.iter().enumerate().map(|(index,l)|
            (index,l.id,l.name.clone(),l.visible,l.locked,
                self.doc.objects.iter().filter(|o|o.layer_id==l.id).count()))
            .collect();
        for (index,id,name,mut visible,mut locked,count) in layers{
            ui.horizontal(|ui|{
                if ui.selectable_label(self.active_layer==id,
                    format!("{}  {}",if self.active_layer==id{"▸"}else{" "},name))
                    .on_hover_text(format!("Активный слой · {count} объектов")).clicked(){
                    self.active_layer=id;
                }
                if ui.small_button(if visible{"◉"}else{"○"})
                    .on_hover_text(if visible{"Скрыть слой"}else{"Показать слой"}).clicked(){
                    self.checkpoint();
                    if let Some(layer)=self.doc.layers.iter_mut().find(|l|l.id==id){
                        layer.visible=!visible;
                    }
                    if visible{self.selected=None;self.multi_selected.clear();}
                }
                if ui.small_button(if locked{"🔒"}else{"🔓"})
                    .on_hover_text(if locked{"Разблокировать слой"}else{"Заблокировать слой"}).clicked(){
                    self.checkpoint();
                    if let Some(layer)=self.doc.layers.iter_mut().find(|l|l.id==id){
                        layer.locked=!locked;
                    }
                }
                if ui.small_button("✎").on_hover_text("Переименовать слой").clicked(){
                    self.layer_rename_id=Some(id);
                    self.layer_rename_draft=name.clone();
                }
                if ui.add_enabled(index>0,egui::Button::new("↑"))
                    .on_hover_text("Поднять слой в списке").clicked(){
                    self.object_move_layer(id,index-1);
                }
                if ui.add_enabled(index+1<self.doc.layers.len(),
                    egui::Button::new("↓")).on_hover_text("Опустить слой в списке").clicked(){
                    self.object_move_layer(id,index+1);
                }
                if ui.add_enabled(self.doc.layers.len()>1&&id!=self.active_layer,
                    egui::Button::new("×")).on_hover_text("Удалить слой и перенести его объекты на активный слой").clicked(){
                    self.object_remove_layer(id);
                }
            });
        }
        if let Some(id)=self.layer_rename_id{
            ui.group(|ui|{
                ui.label("Новое название слоя");
                ui.text_edit_singleline(&mut self.layer_rename_draft);
                ui.horizontal(|ui|{
                    if ui.button("Сохранить").clicked(){
                        let draft=self.layer_rename_draft.clone();
                        self.object_rename_layer(id,&draft);
                        self.layer_rename_id=None;
                        self.layer_rename_draft.clear();
                    }
                    if ui.button("Отмена").clicked(){
                        self.layer_rename_id=None;
                        self.layer_rename_draft.clear();
                    }
                });
            });
        }
        ui.separator();
        ui.horizontal(|ui|{
            ui.heading("ОБЪЕКТЫ");
            ui.label(RichText::new(format!("{}",self.doc.objects.len()))
                .color(Color32::from_rgb(33,187,161)));
            ui.label(RichText::new(format!("выбрано {}",self.selection_ids().len()))
                .small().color(Color32::GRAY));
        });
        ui.add(egui::TextEdit::singleline(&mut self.object_filter)
            .hint_text("Поиск: название, тип, слой, ID..."));
        let has_selection=!self.selection_ids().is_empty();
        ui.horizontal_wrapped(|ui|{
            if ui.add_enabled(has_selection,egui::Button::new("Копия"))
                .on_hover_text("Дублировать выделенные объекты с сохранением групп, эффектов и масок")
                .clicked(){self.object_duplicate_selection();}
            if ui.add_enabled(has_selection,egui::Button::new("Удалить"))
                .on_hover_text("Удалить выделенные объекты").clicked(){
                self.object_delete_selection();
            }
            if ui.add_enabled(has_selection,egui::Button::new("Вперёд"))
                .on_hover_text("Поднять на один уровень").clicked(){
                self.object_reorder_selection(StackOrder::RaiseOne);
            }
            if ui.add_enabled(has_selection,egui::Button::new("Назад"))
                .on_hover_text("Опустить на один уровень").clicked(){
                self.object_reorder_selection(StackOrder::LowerOne);
            }
        });
        ui.horizontal_wrapped(|ui|{
            if ui.add_enabled(has_selection,egui::Button::new("На передний план"))
                .clicked(){self.object_reorder_selection(StackOrder::Front);}
            if ui.add_enabled(has_selection,egui::Button::new("На задний план"))
                .clicked(){self.object_reorder_selection(StackOrder::Back);}
            if ui.add_enabled(has_selection&&self.doc.layer(self.active_layer)
                .is_some_and(|l|l.visible&&!l.locked),
                egui::Button::new("На активный слой"))
                .on_hover_text("Переместить выделенные объекты на выбранный слой")
                .clicked(){self.object_move_to_active_layer();}
        });
        ui.horizontal_wrapped(|ui|{
            if ui.add_enabled(self.selected.is_some(),
                egui::Button::new("Выбрать тот же тип")).clicked(){
                self.object_select_same_type();
            }
            if ui.add_enabled(self.selection_ids().len()>=2,
                egui::Button::new("Группировать  Ctrl+G")).clicked(){
                self.group_selection();
            }
            if ui.add_enabled(has_selection,egui::Button::new("Разгруппировать"))
                .clicked(){self.ungroup_selection();}
        });
        let query=self.object_filter.trim().to_lowercase();
        let mut rows=Vec::<Row>::new();
        let mut emitted=HashSet::<u64>::new();
        for obj in self.doc.objects.iter().rev(){
            let group=obj.group_id;
            let label=if !obj.name.is_empty(){obj.name.clone()}else{
                match &obj.shape{
                    Shape::Rectangle{..}=>"Прямоугольник".into(),
                    Shape::Ellipse{..}=>"Эллипс".into(),
                    Shape::Text{value,..}=>format!("Текст: {}",value.chars().take(24).collect::<String>()),
                    Shape::Bezier{..}=>"Кривая Безье".into(),
                    Shape::CompoundPath{contours}=>format!("Составной контур ({})",contours.len()),
                    Shape::Polyline{..}=>"Ломаная".into(),
                    Shape::EmbeddedSvg{source_format,..}=>format!("Вектор SVG ({source_format})"),
                    Shape::Raster{pixel_width,pixel_height,..}=>format!("Изображение {pixel_width}×{pixel_height}"),
                }
            };
            let matches=|o:&opencorel_core::Object|{
                let title=if o.name.is_empty(){match &o.shape{
                    Shape::Rectangle{..}=>"прямоугольник",
                    Shape::Ellipse{..}=>"эллипс",
                    Shape::Text{..}=>"текст",
                    Shape::Bezier{..}=>"безье",
                    Shape::CompoundPath{..}=>"контур",
                    Shape::Polyline{..}=>"ломаная",
                    Shape::EmbeddedSvg{..}=>"svg вектор",
                    Shape::Raster{..}=>"изображение",
                }}else{o.name.as_str()};
                let layer=self.doc.layer(o.layer_id).map(|l|l.name.as_str()).unwrap_or("");
                query.is_empty()||title.to_lowercase().contains(&query)||
                    o.id.to_string().contains(&query)||layer.to_lowercase().contains(&query)
            };
            if let Some(group_id)=group{
                if !emitted.insert(group_id){continue;}
                let members:Vec<_>=self.doc.objects.iter().rev()
                    .filter(|o|o.group_id==Some(group_id)).collect();
                let group_matches=query.is_empty()||group_id.to_string().contains(&query)||
                    members.iter().any(|member|matches(member));
                if !group_matches{continue;}
                let first=members.first().map(|o|o.id).unwrap_or(obj.id);
                rows.push(Row{group_header:true,id:first,group:Some(group_id),
                    label:format!("Группа {} · {} объектов",group_id,members.len()),
                    visible:members.iter().all(|o|o.visible),
                    locked:members.iter().all(|o|o.locked),depth:0});
                for member in members{
                    if !query.is_empty()&&!group_id.to_string().contains(&query)&&!matches(member){
                        continue;
                    }
                    let text=if member.name.is_empty(){match &member.shape{
                        Shape::Rectangle{..}=>"Прямоугольник".into(),
                        Shape::Ellipse{..}=>"Эллипс".into(),
                        Shape::Text{value,..}=>format!("Текст: {}",value.chars().take(24).collect::<String>()),
                        Shape::Bezier{..}=>"Кривая Безье".into(),
                        Shape::CompoundPath{contours}=>format!("Контур ({})",contours.len()),
                        Shape::Polyline{..}=>"Ломаная".into(),
                        Shape::EmbeddedSvg{source_format,..}=>format!("Вектор {source_format}"),
                        Shape::Raster{pixel_width,pixel_height,..}=>format!("Фото {pixel_width}×{pixel_height}"),
                    }}else{member.name.clone()};
                    rows.push(Row{group_header:false,id:member.id,group,
                        label:text,visible:member.visible,locked:member.locked,depth:1});
                }
            }else if matches(obj){
                rows.push(Row{group_header:false,id:obj.id,group:None,
                    label,visible:obj.visible,locked:obj.locked,depth:0});
            }
        }
        ui.label(RichText::new(format!("Показано строк: {} · Shift — множественный выбор · верхний объект рисуется последним",rows.len()))
            .small().color(Color32::GRAY));
        egui::ScrollArea::vertical().id_salt("objects_tree_studio")
            .auto_shrink([false,false]).show_rows(ui,27.0,rows.len(),|ui,range|{
                for row_index in range{
                    let row=&rows[row_index];
                    ui.horizontal(|ui|{
                        if row.group_header{
                            ui.add_space(2.0);
                            if ui.selectable_label(
                                self.selection_ids().iter().any(|id|
                                    self.doc.objects.iter().find(|o|o.id==*id)
                                        .is_some_and(|o|o.group_id==row.group)),
                                &row.label).clicked(){
                                self.select_object(Some(row.id),false);
                                self.inspector_tab=InspectorTab::Properties;
                            }
                        }else{
                            ui.add_space(if row.depth>0{16.0}else{0.0});
                            if ui.small_button(if row.visible{"◉"}else{"○"})
                                .on_hover_text(if row.visible{"Скрыть объект"}else{"Показать объект"})
                                .clicked(){self.object_visibility(row.id,!row.visible);}
                            if ui.small_button(if row.locked{"🔒"}else{"🔓"})
                                .on_hover_text(if row.locked{"Разблокировать объект"}else{"Заблокировать объект"})
                                .clicked(){self.object_lock(row.id,!row.locked);}
                            let layer_id=self.doc.objects.iter().find(|o|o.id==row.id)
                                .map(|o|o.layer_id).unwrap_or(self.active_layer);
                            if ui.add_enabled(row.visible&&self.doc.layer(layer_id)
                                .is_some_and(|l|l.visible),
                                egui::Button::new(&row.label)
                                    .selected(self.is_selected(row.id))
                                    .min_size(Vec2::new(ui.available_width(),23.0)))
                                .on_hover_text(format!("Объект #{} · слой {}",row.id,layer_id))
                                .clicked(){
                                let shift=ui.input(|i|i.modifiers.shift);
                                self.select_object(Some(row.id),shift);
                                self.active_layer=layer_id;
                                self.inspector_tab=InspectorTab::Properties;
                            }
                        }
                    });
                }
            });
    }

    fn inspector_properties(&mut self,ui:&mut egui::Ui){
        if self.tool==Tool::Smart{
            ui.heading("Профессиональные фигуры");
            self.smart_shape_picker(ui);
            return;
        }
        if self.tool==Tool::Freehand{
            ui.heading("Свободное рисование");
            ui.label("Удерживайте кнопку мыши и ведите линию. После отпускания появится редактируемая кривая Безье.");
            ui.separator();
            ui.label("Точность линии, пикс.");
            ui.add(egui::Slider::new(&mut self.freehand_precision_px,0.5..=12.0)
                .step_by(0.5).suffix(" px"));
            ui.label("Плавность изгибов");
            ui.add(egui::Slider::new(&mut self.freehand_smoothness,0.0..=1.0)
                .step_by(0.05));
            ui.checkbox(&mut self.freehand_closed,"Замкнутый контур");
            ui.small("Для замкнутого контура плавность сохраняется и на стыке начала с концом. Заливку можно назначить после рисования.");
            ui.small("Меньший допуск — больше узлов, лучше повторяет движение мыши. Больший допуск — проще контур.");
            ui.small("Штрихи до 4096 отсчётов; одна линия = одна запись Ctrl+Z. Клавиша H — этот инструмент.");
            return;
        }
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
        if matches!(self.tool,Tool::Nodes|Tool::Select) &&
            self.selected.is_some() && self.multi_selected.is_empty(){
            let can_offset=self.doc.objects.iter()
                .find(|o|Some(o.id)==self.selected)
                .is_some_and(|o|matches!(&o.shape,
                    opencorel_core::Shape::Polyline{..}|
                    opencorel_core::Shape::Bezier{..}|
                    opencorel_core::Shape::Rectangle{..}|
                    opencorel_core::Shape::Ellipse{..}));
            if can_offset{
                ui.heading("Параллельный контур");
                ui.horizontal(|ui|{
                    ui.label("Смещение");
                    ui.add(egui::DragValue::new(&mut self.offset_distance_mm)
                        .range(-1000.0..=1000.0).speed(0.2)
                        .fixed_decimals(2).suffix(" мм"));
                });
                if ui.button("Создать смещённую копию")
                    .on_hover_text("Создаёт новый редактируемый контур, исходный объект сохраняется")
                    .clicked(){
                    self.offset_selected_path();
                }
                ui.small("Знак задаёт сторону относительно направления контура. Безье/эллипс — приближённая ломаная; острые углы ограничиваются.");
                ui.separator();
            }
        }
        if self.tool==Tool::Nodes{
            ui.heading("Редактирование узлов");
            ui.label("Клик по кривой — выбор. Двойной клик по сегменту — новый узел.");
            if let Some(id)=self.selected{
                if self.multi_selected.is_empty() && self.node_edit_allowed(id){
                    let editable_kind=self.doc.objects.iter().find(|o|o.id==id)
                        .map(|o|match &o.shape{
                            opencorel_core::Shape::Polyline{..}=>1u8,
                            opencorel_core::Shape::Bezier{..}=>2u8,
                            _=>0u8,
                        }).unwrap_or(0);
                    if editable_kind!=0{
                        ui.separator();
                        if editable_kind==1 &&
                            ui.button("Ломаную → Безье (точно)")
                                .on_hover_text("Без сглаживания: каждый прямой сегмент остаётся геометрически прямым")
                                .clicked(){
                            self.convert_selected_polyline_exact();
                        }
                        ui.label("Упрощение и сглаживание");
                        ui.horizontal(|ui|{
                            ui.label("Допуск, мм");
                            ui.add(egui::DragValue::new(&mut self.refit_tolerance_mm)
                                .speed(0.1).range(0.0..=100.0).fixed_decimals(2));
                        });
                        ui.label("Плавность");
                        ui.add(egui::Slider::new(&mut self.freehand_smoothness,0.0..=1.0)
                            .step_by(0.05));
                        let clicked=if editable_kind==1{
                            ui.button("Ломаную → плавная кривая")
                                .on_hover_text("Преобразовать прямые сегменты в редактируемую кривую (в том числе замкнутую)")
                                .clicked()
                        }else{
                            ui.button("Упростить число узлов кривой")
                                .on_hover_text("Повторная аппроксимация меняет геометрию и сбрасывает прежние типы узлов")
                                .clicked()
                        };
                        if clicked{self.refit_selected_path(editable_kind==1);}
                        ui.small("Повторная аппроксимация изменяет геометрию. Ctrl+Z восстанавливает исходные узлы и их типы.");
                    }
                }
            }
            if self.selection_ids().len()==2{
                use opencorel_core::path_topology::JoinMode;
                ui.separator();
                ui.label("Соединить две открытые кривые или ломаные");
                ui.small("V → Shift+щелчок по второй кривой → F10");
                ui.horizontal(|ui|{
                    ui.label("Допуск сшивки, мм");
                    ui.add(egui::DragValue::new(&mut self.join_tolerance_mm)
                        .speed(0.1).range(0.0..=1000.0).fixed_decimals(2));
                });
                ui.horizontal_wrapped(|ui|{
                    if ui.button("Сшить концы")
                        .on_hover_text("Совместить ближайшие конечные узлы с заданным допуском")
                        .clicked(){
                        self.join_selected_beziers(JoinMode::Snap{
                            tolerance_mm:self.join_tolerance_mm});
                    }
                    if ui.button("Сшить плавно")
                        .on_hover_text("Только для кривых Безье: выровнять касательные и сохранить гладкость")
                        .clicked(){
                        self.join_selected_beziers(JoinMode::SmoothSnap{
                            tolerance_mm:self.join_tolerance_mm});
                    }
                    if ui.button("Соединить мостиком")
                        .on_hover_text("Прямой сегмент между концами без смещения исходных точек")
                        .clicked(){
                        self.join_selected_beziers(JoinMode::Bridge);
                    }
                });
            }
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
                    let can_split=self.doc.objects.iter().find(|o|o.id==id)
                        .is_some_and(|o|match (&o.shape,address){
                            (opencorel_core::Shape::Bezier{segments,closed:false,..},
                             opencorel_core::node_edit::NodeAddress::BezierEnd(i))=>
                                i.checked_add(1).is_some_and(|n|n<segments.len()),
                            (opencorel_core::Shape::Polyline{points,closed:false},
                             opencorel_core::node_edit::NodeAddress::Polyline(i))=>
                                i>0 && i.checked_add(1).is_some_and(|n|n<points.len()),
                            _=>false,
                        });
                    if can_split && ui.button("Разрезать контур в этом узле")
                        .on_hover_text("Создаёт два отдельных открытых контура с общей конечной точкой")
                        .clicked(){
                        self.split_selected_bezier();
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
                let resumable=self.selected
                    .and_then(|id|self.doc.objects.iter().find(|o|o.id==id))
                    .is_some_and(|obj|matches!(&obj.shape,
                        opencorel_core::Shape::Bezier{segments,closed:false,..}
                            if !segments.is_empty() && segments.len()<10_000));
                if resumable && ui.button("Продолжить кривую инструментом B")
                    .on_hover_text("Добавить новые сегменты к существующей кривой без создания второго объекта")
                    .clicked(){
                    self.continue_selected_bezier();
                }
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
        if self.selection_is_text_plus_guide(){
            ui.collapsing("Текст по кривой — профессиональный инструмент",|ui|{
                self.text_path_controls(ui);
            });
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
        // Text Studio is a batch action: it operates on the selected set,
        // not only the single object currently shown in this inspector.
        let has_text_selection=self.selection_ids().iter().any(|selected_id|
            self.doc.objects.iter().any(|o|
                o.id==*selected_id&&matches!(o.shape,Shape::Text{..})));
        if has_text_selection{
            ui.collapsing("Мастерская текста — 30 инструментов",|ui|{
                self.text_studio_panel(ui);
            });
        }
        if !self.selection_ids().is_empty(){
            ui.collapsing("Композиции — 7 массивов и розеток",|ui|{
                self.pattern_studio_panel(ui);
            });
            ui.collapsing("Векторные эффекты — 9 инструментов",|ui|{
                self.vector_effects_panel(ui);
            });
            ui.collapsing("Студия цвета — 16 гармоний",|ui|{
                self.color_studio_panel(ui);
            });
        }
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
        ui.heading("Страницы документа");
        ui.label(format!("Страница {} из {}",
            self.doc.active_page_index+1,self.doc.page_count()));
        let entries:Vec<_>=(0..self.doc.page_count()).map(|index|{
            let label=self.doc.page_title(index).unwrap_or("Страница").to_owned();
            let dims=self.doc.page_dimensions(index).unwrap_or((0.,0.));
            (index,label,dims)
        }).collect();
        egui::ScrollArea::vertical()
            .id_salt("pages_list")
            .max_height(190.0).show(ui,|ui|{
                for (index,label,(w,h)) in entries{
                    let title=format!("{} — {}  ({w:.0}×{h:.0} мм)",
                        index+1,label);
                    if ui.selectable_label(self.doc.active_page_index==index,title).clicked(){
                        self.navigate_page(index);
                    }
                }
            });
        ui.horizontal_wrapped(|ui|{
            if ui.button("+ Страница").clicked(){self.create_page(false);}
            if ui.button("Дублировать").clicked(){self.create_page(true);}
            if ui.add_enabled(self.doc.page_count()>1,
                egui::Button::new("Удалить")).clicked(){self.remove_current_page();}
        });
        ui.horizontal_wrapped(|ui|{
            if ui.add_enabled(self.doc.active_page_index>0,
                egui::Button::new("← Ранее")).clicked(){self.shift_current_page(false);}
            if ui.add_enabled(self.doc.active_page_index+1<self.doc.page_count(),
                egui::Button::new("Позднее →")).clicked(){self.shift_current_page(true);}
        });
        if !self.page_rename_mode{
            ui.horizontal(|ui|{
                ui.label(format!("Название: {}",self.doc.active_page_title));
                if ui.small_button("Изменить").clicked(){
                    self.page_name_edit=self.doc.active_page_title.clone();
                    self.page_rename_mode=true;
                }
            });
        }else{
            ui.label("Новое имя страницы");
            ui.text_edit_singleline(&mut self.page_name_edit);
            ui.horizontal(|ui|{
                if ui.button("Применить").clicked(){
                    let next=self.page_name_edit.clone();
                    self.rename_current_page(&next);
                }
                if ui.button("Отмена").clicked(){self.page_rename_mode=false;}
            });
        }
        ui.horizontal(|ui|{
            if ui.small_button("Все SVG").on_hover_text("Экспорт всех страниц в отдельные SVG-файлы").clicked(){
                self.export_all_pages_dialog("svg");
            }
            if ui.small_button("Все PDF").on_hover_text("Экспорт по одному RGB PDF на страницу").clicked(){
                self.export_all_pages_dialog("pdf");
            }
        });
        ui.small("Страницы хранят собственные объекты, слои, направляющие и размеры.");
        ui.collapsing("Заменить фирменный цвет во всех страницах",|ui|{
            self.color_studio_panel(ui);
        });
        ui.small("Экспорт PDF из меню «Файл» объединяет все страницы в один векторный документ.");
        ui.separator();
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
        ui.checkbox(&mut self.show_guides,"Показать направляющие");
        ui.checkbox(&mut self.snap_guides,"Привязка к направляющим");
        ui.small("Тяните из линеек для создания направляющих; Alt+перетаскивание — перемещение.");
        ui.horizontal_wrapped(|ui|{
            if ui.button("+ Вертикальная").clicked(){
                self.add_guide(opencorel_core::guides::GuideAxis::Vertical,
                    self.doc.page.width_mm/2.0);
            }
            if ui.button("+ Горизонтальная").clicked(){
                self.add_guide(opencorel_core::guides::GuideAxis::Horizontal,
                    self.doc.page.height_mm/2.0);
            }
            if ui.button("Удалить все").clicked(){self.clear_guides();}
        });
        ui.label(format!("Направляющих: {}",self.doc.guides.len()));
        let mut guide_update=None;
        let mut guide_remove=None;
        let mut guide_edit_finished=false;
        for (i,guide) in self.doc.guides.iter().enumerate(){
            ui.horizontal(|ui|{
                let axis=match guide.axis{
                    opencorel_core::guides::GuideAxis::Horizontal=>"Г",
                    opencorel_core::guides::GuideAxis::Vertical=>"В",
                };
                ui.label(format!("{axis} #{}",i+1));
                let mut coordinate=guide.position_mm;
                let response=ui.add(egui::DragValue::new(&mut coordinate)
                    .speed(0.2).range(-100_000.0..=100_000.0)
                    .fixed_decimals(2).suffix(" мм"));
                if response.changed(){guide_update=Some((i,coordinate));}
                if response.drag_stopped() || response.lost_focus(){
                    guide_edit_finished=true;
                }
                if ui.small_button("×").clicked(){guide_remove=Some(i);}
            });
        }
        if let Some(i)=guide_remove{self.remove_guide(i);}
        else if let Some((i,p))=guide_update{self.edit_guide_numeric(i,p);}
        if guide_edit_finished{self.guide_numeric_session=None;}
        ui.checkbox(&mut self.show_safe_area,"Показать безопасную область");
        ui.horizontal(|ui|{
            ui.label("Отступ от края, мм");
            ui.add(egui::DragValue::new(&mut self.safe_margin_mm)
                .speed(0.5).range(0.0..=100.0));
        });
        ui.label("Оранжевая рамка — только экранная подсказка, в файл не выводится.");
        ui.separator();
        ui.heading("Печатный лист SVG");
        ui.horizontal(|ui|{
            ui.label("Вылет, мм");
            ui.add(egui::DragValue::new(&mut self.print_bleed_mm)
                .speed(0.25).range(0.0..=25.0));
        });
        ui.checkbox(&mut self.print_crop_marks,"Печатать метки реза");
        if ui.button("Экспортировать печатный SVG...").clicked(){
            self.export_print_sheet_dialog();
        }
        ui.small("Вылеты показывают только рисунок, уже выходящий за границы листа; края автоматически не растягиваются.");
        ui.small("Отдельный файл RGB SVG, без PDF/X и CMYK. Исходный .kdr не меняется.");
        ui.separator();
        ui.heading("Предпечатная проверка");
        ui.horizontal(|ui|{
            ui.label("Порог DPI для фото");
            ui.add(egui::DragValue::new(&mut self.minimum_image_dpi)
                .speed(5.0).range(30.0..=600.0));
        });
        if ui.button("Проверить макет").clicked(){self.run_preflight();}
        ui.label("Многостраничные проекты сохраняются в KDR. SVG/PDF в обычном экспорте содержат только активную страницу.");
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
                if ui.add_enabled(self.doc.active_page_index>0,
                    egui::Button::new("◀")).on_hover_text("Предыдущая страница").clicked(){
                    self.navigate_page(self.doc.active_page_index-1);
                }
                ui.menu_button(format!("Страница {} из {}",
                    self.doc.active_page_index+1,self.doc.page_count()),|ui|{
                    for i in 0..self.doc.page_count(){
                        let name=self.doc.page_title(i).unwrap_or("Страница").to_owned();
                        if ui.selectable_label(i==self.doc.active_page_index,
                            format!("{} — {}",i+1,name)).clicked(){
                            self.navigate_page(i);ui.close_menu();
                        }
                    }
                });
                if ui.add_enabled(self.doc.active_page_index+1<self.doc.page_count(),
                    egui::Button::new("▶")).on_hover_text("Следующая страница").clicked(){
                    self.navigate_page(self.doc.active_page_index+1);
                }
                if ui.small_button("+").on_hover_text("Добавить пустую страницу").clicked(){
                    self.create_page(false);
                }
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
            .any(|o|!o.visible||o.locked||!editable.contains(&o.layer_id)){
            self.status="Цвет не изменён: часть выделения находится в закрытом слое".into();
            return;
        }
        if !self.doc.objects.iter().any(|o|ids.contains(&o.id)){return;}
        self.checkpoint();
        for obj in self.doc.objects.iter_mut(){
            if ids.contains(&obj.id)&&obj.visible&&!obj.locked&&editable.contains(&obj.layer_id){
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
        self.object_delete_selection();
    }
    pub(super) fn duplicate_selection(&mut self){
        self.object_duplicate_selection();
    }
    pub(super) fn reorder_selection(&mut self,front:bool){
        self.object_reorder_selection(if front{StackOrder::Front}else{StackOrder::Back});
    }
}
