//! Title bar (quick access toolbar), ribbon tabs and the ribbon bodies.

use super::widgets::{
    big_button, column, combo_frame, field_frame, fixed_group, group, icon_button, paint_icon,
    place, small_button,
};
use crate::app::{DocView, Layer, OmolaApp, PathDialog, RibbonTab};
use crate::theme;
use egui::{Align2, Color32, FontId, RichText, Sense, Stroke, Vec2};
use egui_material_icons::icons::*;

const BLUE: Color32 = Color32::from_rgb(0x1F, 0x6F, 0xC8);
const DARK: Color32 = Color32::from_rgb(0x44, 0x44, 0x44);
const GREEN: Color32 = Color32::from_rgb(0x2E, 0x9E, 0x45);
const ORANGE: Color32 = Color32::from_rgb(0xE0, 0x8A, 0x00);
const RED: Color32 = Color32::from_rgb(0xC6, 0x28, 0x28);

impl OmolaApp {
    pub fn title_bar(&mut self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        // Drag the window by its title bar (custom title bar only).
        let drag = ui.interact(
            rect,
            ui.scope_id().with("title_drag"),
            Sense::click_and_drag(),
        );
        if super::custom_title_bar() {
            if drag.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if drag.double_clicked() {
                let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
            }
        }
        let plot_x = (self.tab == RibbonTab::Simulation)
            .then(|| {
                ui.ctx()
                    .data(|d| d.get_temp::<f32>(egui::Id::NULL.with("plot_tab_x")))
            })
            .flatten();
        if let Some(x) = plot_x {
            // Contextual "Plot" tab header above "Plot Options".
            let r = egui::Rect::from_min_size(
                egui::pos2(x, rect.top()),
                Vec2::new(96.0, rect.height()),
            );
            ui.painter().rect_filled(r, 0.0, theme::CONTEXT_TAB);
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                "Plot",
                FontId::proportional(12.0),
                Color32::WHITE,
            );
        }
        let right = plot_x.unwrap_or(rect.right() - 160.0);
        ui.painter().text(
            egui::pos2((rect.left() + 170.0 + right) / 2.0, rect.center().y),
            Align2::CENTER_CENTER,
            self.window_title(),
            FontId::proportional(12.0),
            theme::TEXT,
        );
        ui.horizontal_centered(|ui| {
            ui.add_space(1.0);
            ui.spacing_mut().item_spacing.x = 2.5;
            if icon_button(ui, ICON_DRAFT, DARK, 16.0, true, false)
                .on_hover_text("New model")
                .clicked()
            {
                self.new_model();
            }
            if icon_button(ui, ICON_FOLDER_OPEN, ORANGE, 16.0, true, false)
                .on_hover_text("Open...")
                .clicked()
            {
                self.request_open_dialog();
            }
            let can_save = self.active_doc().is_some();
            if icon_button(ui, ICON_SAVE, BLUE, 16.0, can_save, false)
                .on_hover_text("Save")
                .clicked()
            {
                self.save();
            }
            icon_button(ui, ICON_UNDO, DARK, 16.0, false, false);
            icon_button(ui, ICON_REDO, DARK, 16.0, false, false);
            if icon_button(ui, ICON_PLAY_CIRCLE, DARK, 17.0, !self.busy, false)
                .on_hover_text("Simulate (F9)")
                .clicked()
            {
                self.simulate();
            }
            if icon_button(ui, ICON_CANCEL, DARK, 17.0, self.busy, false)
                .on_hover_text("Stop")
                .clicked()
            {
                self.worker.cancel();
            }
            icon_button(ui, ICON_ARROW_DROP_DOWN, DARK, 14.0, false, false);
            if super::custom_title_bar() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if window_button(ui, ICON_CLOSE, true).clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                    if window_button(
                        ui,
                        if max {
                            ICON_FILTER_NONE
                        } else {
                            ICON_CROP_SQUARE
                        },
                        false,
                    )
                    .clicked()
                    {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                    }
                    if window_button(ui, ICON_REMOVE, false).clicked() {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                });
            }
        });
    }

    pub fn ribbon_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            // File: blue menu button.
            let (rect, file) = ui.allocate_exact_size(Vec2::new(54.0, 26.0), Sense::click());
            ui.painter().rect_filled(rect, 0.0, theme::FILE_BLUE);
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                "File",
                FontId::proportional(12.0),
                Color32::WHITE,
            );
            egui::Popup::menu(&file).show(|ui| self.file_menu(ui));
            for tab in RibbonTab::ALL {
                if ribbon_tab(ui, tab.label(), self.tab == tab).clicked() {
                    self.select_tab(tab);
                }
            }
            if self.tab == RibbonTab::Simulation {
                let r = ribbon_tab(ui, "Plot Options", false);
                ui.ctx()
                    .data_mut(|d| d.insert_temp(egui::Id::NULL.with("plot_tab_x"), r.rect.left()));
                r.on_hover_text("Plot options: use the Plot group of the Simulation tab");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(2.0);
                for icon in [ICON_CLOSE, ICON_FILTER_NONE, ICON_REMOVE] {
                    icon_button(ui, icon, DARK, 10.0, false, false);
                }
                ui.add_space(4.0);
                icon_button(ui, ICON_VERTICAL_ALIGN_TOP, DARK, 14.0, false, false)
                    .on_disabled_hover_text("Collapse the ribbon");
                ui.add_space(4.0);
                ui.menu_button(
                    RichText::new(format!("{} Windows  \u{25BE}", ICON_FILTER_NONE.codepoint))
                        .size(12.0),
                    |ui| {
                        if ui.button("Package Browser").clicked() {
                            self.docks.package_browser = !self.docks.package_browser;
                        }
                        if ui.button("Component Browser").clicked() {
                            self.docks.component_browser = !self.docks.component_browser;
                        }
                        if ui.button("Variable Browser").clicked() {
                            self.docks.variable_browser = !self.docks.variable_browser;
                        }
                        if ui.button("Commands / Logs").clicked() {
                            self.docks.bottom = !self.docks.bottom;
                        }
                    },
                );
            });
        });
    }

    pub fn not_implemented(&mut self, what: &str) {
        self.info(
            crate::backend::LogTab::Simulation,
            format!("{what}: not implemented yet in Omola."),
        );
        self.log_tab = crate::backend::LogTab::Simulation;
    }

    pub fn select_tab(&mut self, tab: RibbonTab) {
        self.tab = tab;
        if tab == RibbonTab::Text {
            if let Some(active) = self.active.clone() {
                self.request_text(&active);
            }
        }
    }

    fn file_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(200.0);
        if ui.button("New Model").clicked() {
            self.new_model();
        }
        if ui.button("Open...").clicked() {
            self.request_open_dialog();
        }
        if ui
            .add_enabled(
                self.active_doc().is_some(),
                egui::Button::new("Save   Ctrl+S"),
            )
            .clicked()
        {
            self.save();
        }
        if ui
            .add_enabled(self.active_doc().is_some(), egui::Button::new("Save As..."))
            .clicked()
        {
            self.request_save_as_dialog();
        }
        // Recent files submenu.
        if !self.recent_files.is_empty() {
            ui.menu_button("Recent files", |ui| {
                let count = self.recent_files.len();
                for i in 0..count {
                    let path = self.recent_files[i].clone();
                    let exists = path.is_file();
                    let label = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.display().to_string());
                    let full = path.display().to_string();
                    if ui
                        .add_enabled(exists, egui::Button::new(&label))
                        .on_hover_text(&full)
                        .clicked()
                    {
                        ui.close();
                        self.open_file(path);
                    }
                }
            });
        }
        ui.separator();
        ui.menu_button("Libraries", |ui| {
            for spec in crate::library::default_libraries() {
                if ui.button(format!("Reload {}", spec.name)).clicked() {
                    self.load_library(spec);
                }
            }
        });
        ui.separator();
        if ui.button("Exit").clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    pub fn ribbon(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::horizontal()
            .id_salt("ribbon_scroll")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    match self.tab {
                        RibbonTab::Graphics => self.graphics_ribbon(ui),
                        RibbonTab::Documentation => self.documentation_ribbon(ui),
                        RibbonTab::Text => self.text_ribbon(ui),
                        RibbonTab::Simulation => self.simulation_ribbon(ui),
                        RibbonTab::Tools => self.tools_ribbon(ui),
                    }
                });
            });
    }

    fn model_group(&mut self, ui: &mut egui::Ui, large: bool) {
        let back = self.can_go_back();
        let forward = self.can_go_forward();
        group(ui, "Model", |ui| {
            if large {
                let recent = big_button(
                    ui,
                    ICON_HISTORY,
                    DARK,
                    "Recent\n▾",
                    !self.history.is_empty(),
                    false,
                );
                self.recent_popup(&recent);
                if big_button(ui, ICON_ARROW_BACK, BLUE, "Back", back, false).clicked() {
                    self.go_back();
                }
                if big_button(ui, ICON_ARROW_FORWARD, BLUE, "Forward", forward, false).clicked() {
                    self.go_forward();
                }
            } else {
                column(ui, |ui| {
                    let recent = small_button(
                        ui,
                        ICON_HISTORY,
                        DARK,
                        "Recent",
                        !self.history.is_empty(),
                        false,
                    );
                    self.recent_popup(&recent);
                    if small_button(ui, ICON_ARROW_BACK, BLUE, "Back", back, false).clicked() {
                        self.go_back();
                    }
                    if small_button(ui, ICON_ARROW_FORWARD, BLUE, "Forward", forward, false)
                        .clicked()
                    {
                        self.go_forward();
                    }
                });
            }
        });
    }

    fn recent_popup(&mut self, response: &egui::Response) {
        let mut chosen = None;
        egui::Popup::menu(response).show(|ui| {
            let mut seen = std::collections::BTreeSet::new();
            for class in self.history.iter().rev() {
                if seen.insert(class.clone()) && ui.button(class).clicked() {
                    chosen = Some(class.clone());
                }
            }
        });
        if let Some(class) = chosen {
            self.open_class(&class);
        }
    }

    fn graphics_ribbon(&mut self, ui: &mut egui::Ui) {
        self.model_group(ui, false);
        group(ui, "Layer", |ui| {
            column(ui, |ui| {
                if small_button(
                    ui,
                    ICON_WIDGETS,
                    DARK,
                    "Icon",
                    true,
                    self.graphics_layer == Layer::Icon,
                )
                .clicked()
                {
                    self.set_layer(Layer::Icon);
                }
                if small_button(
                    ui,
                    ICON_SCHEMA,
                    BLUE,
                    "Diagram",
                    true,
                    self.graphics_layer == Layer::Diagram,
                )
                .clicked()
                {
                    self.set_layer(Layer::Diagram);
                }
                ui.add_enabled(
                    false,
                    egui::Button::new("100%  ▾").min_size(Vec2::new(70.0, 18.0)),
                );
            });
            column(ui, |ui| {
                icon_button(ui, ICON_ZOOM_OUT_MAP, DARK, 16.0, false, false);
            });
        });
        group(ui, "History", |ui| {
            column(ui, |ui| {
                small_button(ui, ICON_UNDO, DARK, "Undo", false, false);
                small_button(ui, ICON_REDO, DARK, "Redo", false, false);
            });
        });
        let delete_enabled = self
            .active
            .as_deref()
            .is_some_and(|c| self.is_user_class(c))
            && self.selected_component.is_some();
        if clipboard_group(ui, delete_enabled) {
            self.delete_selected_component();
        }
        group(ui, "Draw", |ui| {
            column(ui, |ui| {
                small_button(ui, ICON_HORIZONTAL_RULE, BLUE, "Line", false, false);
                small_button(ui, ICON_RECTANGLE, BLUE, "Rectangle", false, false);
                small_button(ui, ICON_CIRCLE, BLUE, "Ellipse", false, false);
            });
            column(ui, |ui| {
                small_button(ui, ICON_PENTAGON, BLUE, "Polygon", false, false);
                small_button(ui, ICON_TITLE, BLUE, "Text", false, false);
                small_button(ui, ICON_PHOTO, BLUE, "Bitmap", false, false);
            });
            column(ui, |ui| {
                small_button(ui, ICON_BORDER_COLOR, BLUE, "▾", false, false);
                small_button(ui, ICON_FORMAT_COLOR_FILL, BLUE, "▾", false, false);
                small_button(ui, ICON_FORMAT_COLOR_TEXT, BLUE, "▾", false, false);
            });
        });
        group(ui, "Tools", |ui| {
            column(ui, |ui| {
                small_button(ui, ICON_CALL_SPLIT, DARK, "Split Model", false, false);
                if small_button(ui, ICON_CHECK_CIRCLE, GREEN, "Check  ▾", !self.busy, false)
                    .clicked()
                {
                    self.check();
                }
                small_button(ui, ICON_GRID_ON, BLUE, "", false, true);
            });
        });
        group(ui, "Find", |ui| {
            column(ui, |ui| {
                if small_button(ui, ICON_SEARCH, DARK, "Find", true, false).clicked() {
                    self.find_open = true;
                }
                small_button(
                    ui,
                    ICON_MANAGE_SEARCH,
                    DARK,
                    "Find Connection",
                    false,
                    false,
                );
                small_button(ui, ICON_FILTER_ALT, DARK, "Diagram Filter", false, false);
            });
        });
        group(ui, "", |ui| {
            if big_button(ui, ICON_DATA_TABLE, BLUE, "Variables\n▾", false, false).clicked() {}
        });
    }

    fn documentation_ribbon(&mut self, ui: &mut egui::Ui) {
        let read_only = self.active.as_ref().is_none_or(|c| !self.is_user_class(c));
        self.model_group(ui, true);
        group(ui, "Layer", |ui| {
            if big_button(
                ui,
                ICON_INFO,
                BLUE,
                "Formatted",
                true,
                self.doc_view == DocView::Formatted,
            )
            .clicked()
            {
                self.doc_view = DocView::Formatted;
            }
            big_button(ui, ICON_EDIT_DOCUMENT, BLUE, "Info\nEditor", false, false)
                .on_disabled_hover_text("WYSIWYG documentation editing is not implemented yet");
            if big_button(
                ui,
                ICON_CODE,
                BLUE,
                "Info\nSource",
                true,
                self.doc_view == DocView::InfoSource,
            )
            .clicked()
            {
                self.doc_view = DocView::InfoSource;
            }
            column(ui, |ui| {
                small_button(ui, ICON_EDIT_NOTE, BLUE, "Revision Editor", false, false);
                if small_button(
                    ui,
                    ICON_SOURCE_NOTES,
                    BLUE,
                    "Revision Source",
                    true,
                    self.doc_view == DocView::RevisionSource,
                )
                .clicked()
                {
                    self.doc_view = DocView::RevisionSource;
                }
                small_button(ui, ICON_DATA_OBJECT, DARK, "Meta Data", false, false);
            });
        });
        group(ui, "Language", |ui| {
            column(ui, |ui| {
                small_button(ui, ICON_CODE, BLUE, "HTML", true, true);
                small_button(ui, ICON_MARKDOWN, DARK, "Markdown", !read_only, false);
            });
        });
        clipboard_group(ui, false);
        group(ui, "Find", |ui| {
            if big_button(ui, ICON_SEARCH, DARK, "Find", true, self.find_open).clicked() {
                self.find_open = !self.find_open;
            }
            big_button(ui, ICON_FIND_REPLACE, DARK, "Replace", false, false);
            big_button(ui, ICON_ARROW_RIGHT_ALT, DARK, "Go\nTo", false, false);
        });
        group(ui, "Font", |ui| {
            column(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_enabled(
                        false,
                        egui::Button::new("Normal              ▾").min_size(Vec2::new(116.0, 18.0)),
                    );
                    small_button(ui, ICON_FORMAT_BOLD, DARK, "", false, false);
                    small_button(ui, ICON_FORMAT_ITALIC, DARK, "", false, false);
                });
                ui.horizontal(|ui| {
                    ui.add_enabled(
                        false,
                        egui::Button::new("(Default)           ▾").min_size(Vec2::new(116.0, 18.0)),
                    );
                    small_button(ui, ICON_FORMAT_UNDERLINED, DARK, "", false, false);
                    small_button(ui, ICON_SUPERSCRIPT, DARK, "", false, false);
                });
                ui.horizontal(|ui| {
                    ui.add_enabled(
                        false,
                        egui::Button::new("(Auto)              ▾").min_size(Vec2::new(116.0, 18.0)),
                    );
                    small_button(ui, ICON_FORMAT_COLOR_TEXT, DARK, "", false, false);
                    small_button(ui, ICON_SUBSCRIPT, DARK, "", false, false);
                });
            });
        });
        group(ui, "Paragraph", |ui| {
            column(ui, |ui| {
                ui.horizontal(|ui| {
                    small_button(ui, ICON_FORMAT_ALIGN_LEFT, DARK, "", false, false);
                    small_button(ui, ICON_FORMAT_ALIGN_CENTER, DARK, "", false, false);
                    small_button(ui, ICON_FORMAT_ALIGN_RIGHT, DARK, "", false, false);
                });
                small_button(
                    ui,
                    ICON_FORMAT_LIST_BULLETED,
                    DARK,
                    "Bullet List",
                    false,
                    false,
                );
                small_button(
                    ui,
                    ICON_FORMAT_LIST_NUMBERED,
                    DARK,
                    "Numbered List",
                    false,
                    false,
                );
            });
        });
        group(ui, "Tools", |ui| {
            column(ui, |ui| {
                small_button(ui, ICON_LINK, DARK, "Link", false, false);
                small_button(ui, ICON_IMAGE, DARK, "Image", false, false);
                small_button(ui, ICON_FUNCTIONS, DARK, "Equation", false, false);
            });
            column(ui, |ui| {
                small_button(ui, ICON_TABLE, DARK, "Table  ▾", false, false);
            });
        });
    }

    fn text_ribbon(&mut self, ui: &mut egui::Ui) {
        let editable = self.active.as_ref().is_some_and(|c| self.is_user_class(c));
        self.model_group(ui, false);
        group(ui, "History", |ui| {
            column(ui, |ui| {
                small_button(ui, ICON_UNDO, DARK, "Undo", false, false);
                small_button(ui, ICON_REDO, DARK, "Redo", false, false);
            });
        });
        clipboard_group(ui, false);
        group(ui, "Find", |ui| {
            column(ui, |ui| {
                if small_button(ui, ICON_SEARCH, DARK, "Find", true, false).clicked() {
                    self.find_open = true;
                }
                small_button(ui, ICON_FIND_REPLACE, DARK, "Replace", false, false);
                small_button(ui, ICON_ARROW_RIGHT_ALT, DARK, "Go To", false, false);
            });
        });
        group(ui, "Tools", |ui| {
            if big_button(ui, ICON_CHECK_CIRCLE, GREEN, "Check", !self.busy, false).clicked() {
                self.check();
            }
            if big_button(ui, ICON_SAVE, BLUE, "Save", editable, false).clicked() {
                self.save();
            }
        });
    }

    /// Simulation ribbon. Coordinates (x, y, width, height) are offsets from each group's
    /// top-left corner and follow the measured reference layout.
    fn simulation_ribbon(&mut self, ui: &mut egui::Ui) {
        fixed_group(ui, "Scripting", 113.0, |ui, o| {
            let run_script = place(ui, o, (8.0, 6.0, 100.0, 22.0), |ui| {
                small_button(
                    ui,
                    ICON_PLAY_CIRCLE,
                    BLUE,
                    "Run Script  \u{25BE}",
                    true,
                    false,
                )
                .on_hover_text("Run an OpenModelica script (.mos)")
            });
            if run_script.clicked() {
                self.path_dialog = Some(PathDialog::RunScript(String::new()));
            }
            place(ui, o, (8.0, 31.0, 100.0, 22.0), |ui| {
                small_button(
                    ui,
                    ICON_DESCRIPTION,
                    BLUE,
                    "New Script  \u{25BE}",
                    false,
                    false,
                )
            });
            let commands = place(ui, o, (8.0, 56.0, 100.0, 22.0), |ui| {
                small_button(ui, ICON_TERMINAL, BLUE, "Commands  \u{25BE}", true, false)
            });
            if commands.clicked() {
                self.docks.bottom = true;
                self.bottom_pane = crate::app::BottomPane::Commands;
            }
        });
        fixed_group(ui, "Simulation", 382.0, |ui, o| {
            let translate = place(ui, o, (6.0, 6.0, 90.0, 22.0), |ui| {
                small_button(ui, ICON_BUILD, BLUE, "Translate  \u{25BE}", true, false)
                    .on_hover_text("Check the model (a separate translation step is issue G05)")
            });
            if translate.clicked() {
                self.check();
            }
            let simulate = place(ui, o, (6.0, 31.0, 90.0, 22.0), |ui| {
                small_button(ui, ICON_PLAY_CIRCLE, BLUE, "Simulate", !self.busy, false)
                    .on_hover_text("Simulate (F9)")
            });
            if simulate.clicked() {
                self.simulate();
            }
            let stop = place(ui, o, (6.0, 56.0, 90.0, 22.0), |ui| {
                small_button(ui, ICON_CANCEL, DARK, "Stop", self.busy, false).on_hover_text("Stop")
            });
            if stop.clicked() {
                self.worker.cancel();
            }
            let sweep = place(ui, o, (97.0, 6.0, 125.0, 22.0), |ui| {
                small_button(
                    ui,
                    ICON_STACKED_LINE_CHART,
                    DARK,
                    "Sweep Parameters",
                    true,
                    false,
                )
            });
            if sweep.clicked() {
                self.not_implemented("Sweep Parameters");
            }
            let setup = place(ui, o, (222.0, 6.0, 60.0, 22.0), |ui| {
                small_button(ui, ICON_TIMER, DARK, "Setup", true, false)
            });
            if setup.clicked() {
                self.show_setup = true;
            }
            let field =
                egui::Rect::from_min_size(o + Vec2::new(159.0, 30.0), Vec2::new(62.0, 23.0));
            field_frame(ui, field);
            let buffer_id = ui.scope_id().with("stop_time_text");
            let mut text = ui
                .ctx()
                .data(|d| d.get_temp::<String>(buffer_id))
                .unwrap_or_else(|| crate::ui::browsers_format(self.setup.stop_time));
            let edit = place(ui, o, (163.0, 30.0, 56.0, 23.0), |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut text)
                        .frame(egui::Frame::NONE)
                        .desired_width(54.0),
                )
                .on_hover_text("Stop time [s]")
            });
            if edit.has_focus() {
                if let Ok(v) = text.trim().parse::<f64>() {
                    if v.is_finite() && v >= 0.0 {
                        self.setup.stop_time = v;
                    }
                }
                ui.ctx().data_mut(|d| d.insert_temp(buffer_id, text));
            } else {
                ui.ctx().data_mut(|d| d.remove::<String>(buffer_id));
            }
            place(ui, o, (98.0, 56.0, 40.0, 22.0), |ui| ui.label("Alg:"));
            let combo =
                egui::Rect::from_min_size(o + Vec2::new(159.0, 56.0), Vec2::new(62.0, 23.0));
            combo_frame(ui, combo);
            place(ui, o, (163.0, 56.0, 58.0, 23.0), |ui| {
                ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::NONE;
                ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                egui::ComboBox::from_id_salt("ribbon_alg")
                    .width(52.0)
                    .selected_text(display_method(&self.setup.method))
                    .show_ui(ui, |ui| {
                        for m in ["dassl", "ida", "cvode", "euler", "rungekutta"] {
                            ui.selectable_value(
                                &mut self.setup.method,
                                m.to_owned(),
                                display_method(m),
                            );
                        }
                    })
            });
            for (y, label) in [
                (6.0, "Continue  \u{25BE}"),
                (31.0, "Linearize"),
                (56.0, "Save in Model"),
            ] {
                place(ui, o, (292.0, y, 88.0, 22.0), |ui| {
                    ui.add_enabled(false, egui::Button::new(label).frame(false))
                });
            }
        });
        fixed_group(ui, "Plot", 268.0, |ui, o| {
            let new_diagram = place(ui, o, (6.0, 6.0, 120.0, 22.0), |ui| {
                small_button(ui, ICON_ADD_CHART, BLUE, "New diagram", true, false)
                    .on_hover_text("Stack another diagram under this one; the time axis is shared")
            });
            if new_diagram.clicked() {
                self.new_diagram();
            }
            let remove = place(ui, o, (6.0, 31.0, 120.0, 22.0), |ui| {
                small_button(
                    ui,
                    ICON_DELETE,
                    DARK,
                    "Remove diagram",
                    self.diagrams.len() > 1,
                    false,
                )
                .on_disabled_hover_text("The plot window always keeps one diagram")
            });
            if remove.clicked() {
                self.remove_active_diagram();
            }
            let erase = place(ui, o, (6.0, 56.0, 120.0, 22.0), |ui| {
                small_button(
                    ui,
                    ICON_REMOVE,
                    DARK,
                    "Erase curves",
                    !self.active_signals().is_empty(),
                    false,
                )
                .on_hover_text("Remove every curve of the active diagram")
            });
            if erase.clicked() {
                self.erase_curves();
            }
            let cursor = place(ui, o, (132.0, 6.0, 120.0, 22.0), |ui| {
                small_button(ui, ICON_HEIGHT, DARK, "Cursor", true, self.cursor_enabled)
                    .on_hover_text("Show the value of every curve at the mouse position")
            });
            if cursor.clicked() {
                self.cursor_enabled = !self.cursor_enabled;
            }
            let export = place(ui, o, (132.0, 31.0, 120.0, 22.0), |ui| {
                small_button(ui, ICON_DOWNLOAD, BLUE, "Export CSV", true, false)
                    .on_hover_text("Save the curves of the active diagram as CSV")
            });
            if export.clicked() {
                self.request_export_csv();
            }
        });
        fixed_group(ui, "Tools", 95.0, |ui, o| {
            let icons = [
                (7.0, 6.0, ICON_UPLOAD_FILE, BLUE, "Load Result: issue G14"),
                (47.0, 6.0, ICON_VIEW_IN_AR, GREEN, "Animation"),
                (72.0, 6.0, ICON_WARNING, ORANGE, "Messages"),
                (47.0, 31.0, ICON_MONITORING, DARK, "Visualize 3D"),
                (7.0, 56.0, ICON_TABLE_CHART, BLUE, "New Table"),
                (47.0, 56.0, ICON_DEPLOYED_CODE, BLUE, "3D View"),
            ];
            for (x, y, icon, color, tip) in icons {
                let r = place(ui, o, (x, y, 22.0, 22.0), |ui| {
                    icon_button(ui, icon, color, 17.0, true, false).on_hover_text(tip)
                });
                if r.clicked() {
                    self.not_implemented(tip);
                }
            }
            let new_plot = place(ui, o, (7.0, 31.0, 22.0, 22.0), |ui| {
                icon_button(ui, ICON_SHOW_CHART, BLUE, 17.0, true, false)
                    .on_hover_text("New diagram")
            });
            if new_plot.clicked() {
                self.new_diagram();
            }
            place(ui, o, (31.0, 31.0, 12.0, 22.0), |ui| {
                icon_button(ui, ICON_ARROW_DROP_DOWN, DARK, 10.0, true, false)
            });
        });
        fixed_group(ui, "Animation Control", 294.0, |ui, o| {
            let icons = [
                (8.0, 6.0, ICON_PLAY_ARROW),
                (33.0, 6.0, ICON_FAST_REWIND),
                (58.0, 6.0, ICON_SKIP_NEXT),
                (8.0, 31.0, ICON_PAUSE),
                (33.0, 31.0, ICON_FAST_FORWARD),
                (8.0, 56.0, ICON_SKIP_PREVIOUS),
                (33.0, 56.0, ICON_FIRST_PAGE),
            ];
            for (x, y, icon) in icons {
                let r = place(ui, o, (x, y, 22.0, 22.0), |ui| {
                    icon_button(ui, icon, BLUE, 17.0, true, false)
                });
                if r.clicked() {
                    self.not_implemented("Animation");
                }
            }
            place(ui, o, (85.0, 4.0, 60.0, 18.0), |ui| ui.label("Time:"));
            field_frame(
                ui,
                egui::Rect::from_min_size(o + Vec2::new(88.0, 23.0), Vec2::new(127.0, 21.0)),
            );
            place(ui, o, (92.0, 23.0, 121.0, 21.0), |ui| ui.label("0"));
            place(ui, o, (224.0, 23.0, 12.0, 21.0), |ui| ui.label("s"));
            let track = egui::Rect::from_min_size(o + Vec2::new(85.0, 52.0), Vec2::new(153.0, 4.0));
            ui.painter().rect(
                track,
                0.0,
                Color32::from_rgb(0xE7, 0xEA, 0xEA),
                egui::Stroke::new(1.0, Color32::from_rgb(0xD6, 0xD6, 0xD6)),
                egui::StrokeKind::Inside,
            );
            let thumb = egui::Rect::from_min_size(o + Vec2::new(85.0, 45.0), Vec2::new(8.0, 18.0));
            ui.painter().rect_filled(thumb, 0.0, theme::FILE_BLUE);
            place(ui, o, (240.0, 4.0, 50.0, 18.0), |ui| ui.label("Speed:"));
            field_frame(
                ui,
                egui::Rect::from_min_size(o + Vec2::new(239.0, 23.0), Vec2::new(47.0, 21.0)),
            );
            place(ui, o, (243.0, 23.0, 43.0, 21.0), |ui| {
                ui.label("   1    \u{2304}")
            });
        });
    }

    fn tools_ribbon(&mut self, ui: &mut egui::Ui) {
        group(ui, "Backend", |ui| {
            if big_button(ui, ICON_SETTINGS, DARK, "Locate\nOMC", true, false).clicked() {
                self.worker.submit(crate::backend::Job::Doctor);
                self.log_tab = crate::backend::LogTab::Version;
            }
            if big_button(
                ui,
                ICON_LIBRARY_BOOKS,
                BLUE,
                "Reload\nLibraries",
                true,
                false,
            )
            .clicked()
            {
                for spec in crate::library::default_libraries() {
                    self.load_library(spec);
                }
            }
            if big_button(ui, ICON_DOWNLOAD, BLUE, "Install\nLibraries", !self.busy, false)
                .on_hover_text(
                    "Install Modelica Standard Library 4.1.0 and ModelicaReference with the OpenModelica package manager",
                )
                .clicked()
            {
                self.worker.submit(crate::backend::Job::InstallLibraries);
                self.log_tab = crate::backend::LogTab::Version;
                self.bottom_pane = crate::app::BottomPane::Logs;
                self.docks.bottom = true;
            }
        });
        group(ui, "About", |ui| {
            column(ui, |ui| {
                ui.label(RichText::new("Omola - open-source Modelica workbench").strong());
                ui.label("Computation by OpenModelica (omc).");
                ui.label(
                    RichText::new("Not affiliated with Dassault Systemes.").color(theme::TEXT_WEAK),
                );
            });
        });
    }
}

fn clipboard_group(ui: &mut egui::Ui, delete_enabled: bool) -> bool {
    let mut delete = false;
    group(ui, "Clipboard", |ui| {
        big_button(ui, ICON_CONTENT_CUT, DARK, "Cut", false, false);
        big_button(ui, ICON_CONTENT_COPY, DARK, "Copy", false, false);
        big_button(ui, ICON_CONTENT_PASTE, DARK, "Paste", false, false);
        column(ui, |ui| {
            delete = small_button(ui, ICON_CLOSE, RED, "Delete", delete_enabled, false).clicked();
            small_button(ui, ICON_FILE_COPY, DARK, "Duplicate", false, false);
            small_button(ui, ICON_SELECT_ALL, DARK, "Select All", false, false);
        });
    });
    delete
}

fn display_method(m: &str) -> String {
    let mut c = m.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

fn ribbon_tab(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let font = FontId::proportional(12.0);
    let w = ui.ctx().fonts_mut(|f| {
        f.layout_no_wrap(label.into(), font.clone(), theme::TEXT)
            .size()
            .x
    }) + 30.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect(
            rect,
            0.0,
            theme::BG,
            Stroke::new(1.0, theme::BORDER),
            egui::StrokeKind::Inside,
        );
        // Merge the selected tab into the ribbon body.
        painter.hline(
            rect.x_range().shrink(1.0),
            rect.bottom(),
            Stroke::new(2.0, theme::BG),
        );
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, theme::HOVER_BG);
    }
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        font,
        theme::TEXT,
    );
    response
}

fn window_button(
    ui: &mut egui::Ui,
    icon: egui_material_icons::MaterialIcon,
    close: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(46.0, 30.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(
            rect,
            0.0,
            if close {
                Color32::from_rgb(0xE8, 0x11, 0x23)
            } else {
                Color32::from_rgb(0xE5, 0xE5, 0xE5)
            },
        );
    }
    let fg = if close && response.hovered() {
        Color32::WHITE
    } else {
        theme::TEXT
    };
    paint_icon(ui, rect.center(), icon, 16.0, fg);
    response
}
