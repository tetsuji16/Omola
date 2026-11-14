//! Title bar (quick access toolbar), ribbon tabs and the ribbon bodies.

use super::widgets::{
    big_button, column, combo_frame, field_frame, fixed_group, group, icon_button, paint_icon,
    place, small_button,
};
use crate::app::{DocView, EditorCommand, Layer, Loadable, OmolaApp, RibbonTab};
use crate::theme;
use egui::{Align2, Color32, FontId, RichText, Sense, Stroke, Vec2};
use egui_material_icons::icons::*;

const BLUE: Color32 = Color32::from_rgb(0x1F, 0x6F, 0xC8);
const DARK: Color32 = Color32::from_rgb(0x44, 0x44, 0x44);
const GREEN: Color32 = Color32::from_rgb(0x2E, 0x9E, 0x45);
const ORANGE: Color32 = Color32::from_rgb(0xE0, 0x8A, 0x00);
const RED: Color32 = Color32::from_rgb(0xC6, 0x28, 0x28);
/// Shown on the Graphics ribbon's drawing buttons, which stay disabled because the
/// graphics editor is not implemented (the Diagram layer is edited through OMC).
const DRAWING_NOT_AVAILABLE: &str =
    "Not available: drawing graphics primitives is not implemented yet";

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
        let quick_access_right = ui
            .horizontal_centered(|ui| {
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
                if icon_button(ui, ICON_UNDO, DARK, 16.0, self.can_undo(), false)
                    .on_hover_text("Undo")
                    .clicked()
                {
                    self.undo();
                }
                if icon_button(ui, ICON_REDO, DARK, 16.0, self.can_redo(), false)
                    .on_hover_text("Redo")
                    .clicked()
                {
                    self.redo();
                }
                if icon_button(ui, ICON_PLAY_CIRCLE, DARK, 17.0, !self.busy, false)
                    .on_hover_text("Simulate (F9)")
                    .clicked()
                {
                    self.simulate();
                }
                let stop = icon_button(ui, ICON_CANCEL, DARK, 17.0, self.busy, false)
                    .on_hover_text("Stop");
                if stop.clicked() {
                    self.worker.cancel();
                }
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
                stop.rect.right()
            })
            .inner;
        self.paint_window_title(ui, rect, quick_access_right, right);
    }

    /// Paint the window title between the quick-access icons and the contextual
    /// tab area. The title is centred while it fits and otherwise left-aligned
    /// and elided so that it can never run under the icons.
    fn paint_window_title(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        quick_access_right: f32,
        right: f32,
    ) {
        let area = egui::Rect::from_min_max(
            egui::pos2(quick_access_right + 8.0, rect.top()),
            egui::pos2(right - 8.0, rect.bottom()),
        );
        if area.width() <= 0.0 {
            return;
        }
        let title = self.window_title();
        let font = FontId::proportional(12.0);
        let text_width = ui.ctx().fonts_mut(|fonts| {
            fonts
                .layout_no_wrap(title.clone(), font.clone(), theme::TEXT)
                .size()
                .x
        });
        let layout = title_layout(area, text_width);
        let mut job = egui::text::LayoutJob::single_section(
            title,
            egui::text::TextFormat::simple(font, theme::TEXT),
        );
        job.wrap.max_width = area.width();
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        job.wrap.overflow_character = layout.truncated.then_some('\u{2026}');
        let galley = ui.painter().layout_job(job);
        ui.painter_at(area).galley(
            egui::pos2(layout.x, area.center().y - galley.size().y / 2.0),
            galley,
            theme::TEXT,
        );
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
                let r = ribbon_tab(ui, "Plot Options", self.plot_options);
                ui.ctx()
                    .data_mut(|d| d.insert_temp(egui::Id::NULL.with("plot_tab_x"), r.rect.left()));
                if r.clicked() {
                    self.plot_options = true;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(2.0);
                let collapse_tip = if self.ribbon_collapsed {
                    "Expand the ribbon"
                } else {
                    "Collapse the ribbon"
                };
                if icon_button(ui, ICON_VERTICAL_ALIGN_TOP, DARK, 14.0, true, false)
                    .on_hover_text(collapse_tip)
                    .clicked()
                {
                    self.ribbon_collapsed = !self.ribbon_collapsed;
                }
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

    pub fn select_tab(&mut self, tab: RibbonTab) {
        self.tab = tab;
        self.plot_options = false;
        self.ribbon_collapsed = false;
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
            .add_enabled(
                self.active_doc().is_some(),
                egui::Button::new("Save As...   Ctrl+Shift+S"),
            )
            .clicked()
        {
            self.request_save_as_dialog();
        }
        if ui
            .add_enabled(self.can_save_all(), egui::Button::new("Save All"))
            .clicked()
        {
            self.save_all();
        }
        if ui
            .add_enabled(self.active.is_some(), egui::Button::new("Close   Ctrl+W"))
            .clicked()
        {
            self.close_active();
        }
        if ui.button("Load Result...").clicked() {
            self.request_load_result_dialog();
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
        if self.ribbon_collapsed {
            return;
        }
        egui::ScrollArea::horizontal()
            .id_salt("ribbon_scroll")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if self.plot_options && self.tab == RibbonTab::Simulation {
                        self.plot_options_ribbon(ui);
                    } else {
                        match self.tab {
                            RibbonTab::Graphics => self.graphics_ribbon(ui),
                            RibbonTab::Documentation => self.documentation_ribbon(ui),
                            RibbonTab::Text => self.text_ribbon(ui),
                            RibbonTab::Simulation => self.simulation_ribbon(ui),
                            RibbonTab::Tools => self.tools_ribbon(ui),
                        }
                    }
                });
            });
    }

    fn plot_options_ribbon(&mut self, ui: &mut egui::Ui) {
        fixed_group(ui, "View", 158.0, |ui, o| {
            if place(ui, o, (6.0, 8.0, 142.0, 24.0), |ui| {
                small_button(ui, ICON_ZOOM_OUT_MAP, DARK, "Rescale", true, false)
            })
            .clicked()
            {
                let i = self
                    .active_diagram
                    .min(self.diagrams.len().saturating_sub(1));
                self.diagrams[i].rescale();
                clear_plot_range_inputs(ui.ctx(), i);
            }
            if place(ui, o, (6.0, 40.0, 142.0, 24.0), |ui| {
                small_button(ui, ICON_ZOOM_OUT_MAP, DARK, "Rescale All", true, false)
            })
            .clicked()
            {
                for diagram in &mut self.diagrams {
                    diagram.rescale();
                }
                for i in 0..self.diagrams.len() {
                    clear_plot_range_inputs(ui.ctx(), i);
                }
            }
        });
        fixed_group(ui, "Diagram", 158.0, |ui, o| {
            if place(ui, o, (6.0, 8.0, 142.0, 24.0), |ui| {
                small_button(ui, ICON_ADD_CHART, BLUE, "New diagram", true, false)
            })
            .clicked()
            {
                self.new_diagram();
            }
            if place(ui, o, (6.0, 40.0, 142.0, 24.0), |ui| {
                small_button(
                    ui,
                    ICON_DELETE,
                    DARK,
                    "Remove diagram",
                    self.diagrams.len() > 1,
                    false,
                )
            })
            .clicked()
            {
                self.remove_active_diagram();
            }
            if place(ui, o, (6.0, 72.0, 142.0, 24.0), |ui| {
                small_button(
                    ui,
                    ICON_REMOVE,
                    DARK,
                    "Erase curves",
                    !self.active_signals().is_empty(),
                    false,
                )
            })
            .clicked()
            {
                self.erase_curves();
            }
        });
        fixed_group(ui, "Display", 220.0, |ui, o| {
            let index = self
                .active_diagram
                .min(self.diagrams.len().saturating_sub(1));
            place(ui, o, (6.0, 8.0, 95.0, 23.0), |ui| {
                ui.checkbox(&mut self.diagrams[index].style.grid, "Grid")
            });
            place(ui, o, (6.0, 36.0, 205.0, 24.0), |ui| {
                egui::ComboBox::from_id_salt("plot_legend_position")
                    .selected_text(match self.diagrams[index].style.legend {
                        crate::plot::LegendPos::Top => "Legend: Top",
                        crate::plot::LegendPos::Bottom => "Legend: Bottom",
                        crate::plot::LegendPos::Hidden => "Legend: Hidden",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.diagrams[index].style.legend,
                            crate::plot::LegendPos::Top,
                            "Top",
                        );
                        ui.selectable_value(
                            &mut self.diagrams[index].style.legend,
                            crate::plot::LegendPos::Bottom,
                            "Bottom",
                        );
                        ui.selectable_value(
                            &mut self.diagrams[index].style.legend,
                            crate::plot::LegendPos::Hidden,
                            "Hidden",
                        );
                    })
            });
            place(ui, o, (6.0, 66.0, 205.0, 24.0), |ui| {
                egui::ComboBox::from_id_salt("plot_line_width")
                    .selected_text(format!(
                        "Line width: {}",
                        self.diagrams[index].style.line_width
                    ))
                    .show_ui(ui, |ui| {
                        for width in [1.0, 1.5, 2.0, 3.0] {
                            ui.selectable_value(
                                &mut self.diagrams[index].style.line_width,
                                width,
                                width.to_string(),
                            );
                        }
                    })
            });
        });
        fixed_group(ui, "Range", 256.0, |ui, o| {
            let index = self
                .active_diagram
                .min(self.diagrams.len().saturating_sub(1));
            for (i, label) in ["Time from", "Time to", "Y min", "Y max"]
                .into_iter()
                .enumerate()
            {
                let row = i as f32;
                place(ui, o, (6.0, 5.0 + row * 23.0, 68.0, 21.0), |ui| {
                    ui.label(label)
                });
                let id = egui::Id::NULL.with(("plot_range", index, i));
                let mut value = ui
                    .ctx()
                    .data(|d| d.get_temp::<String>(id))
                    .unwrap_or_default();
                let invalid_id = egui::Id::NULL.with(("plot_range_invalid", index, i / 2));
                let invalid = ui
                    .ctx()
                    .data(|d| d.get_temp::<bool>(invalid_id))
                    .unwrap_or(false);
                let response = place(ui, o, (78.0, 5.0 + row * 23.0, 100.0, 21.0), |ui| {
                    ui.add(egui::TextEdit::singleline(&mut value).desired_width(96.0))
                });
                if response.changed() {
                    ui.ctx().data_mut(|d| d.insert_temp(id, value.clone()));
                }
                if invalid {
                    response
                        .clone()
                        .on_hover_text("Enter finite increasing bounds, or leave both empty.");
                    ui.painter().text(
                        egui::pos2(response.rect.right() + 8.0, response.rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        "!",
                        FontId::proportional(14.0),
                        RED,
                    );
                }
                if response.lost_focus() {
                    let a = ui
                        .ctx()
                        .data(|d| {
                            d.get_temp::<String>(egui::Id::NULL.with((
                                "plot_range",
                                index,
                                (i / 2) * 2,
                            )))
                        })
                        .unwrap_or_default();
                    let b = ui
                        .ctx()
                        .data(|d| {
                            d.get_temp::<String>(egui::Id::NULL.with((
                                "plot_range",
                                index,
                                (i / 2) * 2 + 1,
                            )))
                        })
                        .unwrap_or_default();
                    let range = parse_optional_range(&a, &b);
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(invalid_id, range.is_err()));
                    let index = self
                        .active_diagram
                        .min(self.diagrams.len().saturating_sub(1));
                    if let Ok(bounds) = range {
                        if i < 2 {
                            self.diagrams[index].style.x_view = bounds;
                        } else {
                            self.diagrams[index].y_view = bounds;
                        }
                    }
                }
            }
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
                if big_button(ui, ICON_ARROW_BACK, BLUE, "Back", back, false)
                    .on_disabled_hover_text("No earlier class in this session")
                    .clicked()
                {
                    self.go_back();
                }
                if big_button(ui, ICON_ARROW_FORWARD, BLUE, "Forward", forward, false)
                    .on_disabled_hover_text("No later class in this session")
                    .clicked()
                {
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
                    if small_button(ui, ICON_ARROW_BACK, BLUE, "Back", back, false)
                        .on_disabled_hover_text("No earlier class in this session")
                        .clicked()
                    {
                        self.go_back();
                    }
                    if small_button(ui, ICON_ARROW_FORWARD, BLUE, "Forward", forward, false)
                        .on_disabled_hover_text("No later class in this session")
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
            let diagram = self.graphics_layer == Layer::Diagram;
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
                ui.add_enabled_ui(diagram, |ui| {
                    let label = format!("{:.0}%  ▾", self.diagram_zoom * 100.0);
                    ui.menu_button(RichText::new(label).size(12.0), |ui| {
                        if ui.button("50%").clicked() {
                            self.set_diagram_zoom(0.5);
                        }
                        if ui.button("100%").clicked() {
                            self.reset_diagram_view();
                        }
                        if ui.button("200%").clicked() {
                            self.set_diagram_zoom(2.0);
                        }
                        if ui.button("Fit to window").clicked() {
                            self.reset_diagram_view();
                        }
                    });
                });
            });
            column(ui, |ui| {
                if icon_button(ui, ICON_ZOOM_OUT_MAP, DARK, 16.0, diagram, false)
                    .on_hover_text("Fit diagram to window")
                    .on_disabled_hover_text("Zoom works in the Diagram layer")
                    .clicked()
                {
                    self.reset_diagram_view();
                }
            });
        });
        group(ui, "History", |ui| {
            column(ui, |ui| {
                if small_button(ui, ICON_UNDO, DARK, "Undo", self.can_undo(), false).clicked() {
                    self.undo();
                }
                if small_button(ui, ICON_REDO, DARK, "Redo", self.can_redo(), false).clicked() {
                    self.redo();
                }
            });
        });
        let delete_enabled = self
            .active
            .as_deref()
            .is_some_and(|c| self.is_user_class(c))
            && self.selected_component.is_some();
        if clipboard_group(ui, false, false, delete_enabled).1 {
            self.delete_selected_component();
        }
        group(ui, "Draw", |ui| {
            column(ui, |ui| {
                for (icon, label) in [
                    (ICON_HORIZONTAL_RULE, "Line"),
                    (ICON_RECTANGLE, "Rectangle"),
                    (ICON_CIRCLE, "Ellipse"),
                ] {
                    small_button(ui, icon, BLUE, label, false, false)
                        .on_disabled_hover_text(DRAWING_NOT_AVAILABLE);
                }
            });
            column(ui, |ui| {
                for (icon, label) in [
                    (ICON_PENTAGON, "Polygon"),
                    (ICON_TITLE, "Text"),
                    (ICON_PHOTO, "Bitmap"),
                ] {
                    small_button(ui, icon, BLUE, label, false, false)
                        .on_disabled_hover_text(DRAWING_NOT_AVAILABLE);
                }
            });
            column(ui, |ui| {
                for icon in [
                    ICON_BORDER_COLOR,
                    ICON_FORMAT_COLOR_FILL,
                    ICON_FORMAT_COLOR_TEXT,
                ] {
                    small_button(ui, icon, BLUE, "\u{25BE}", false, false)
                        .on_disabled_hover_text(DRAWING_NOT_AVAILABLE);
                }
            });
        });
        group(ui, "Tools", |ui| {
            column(ui, |ui| {
                small_button(ui, ICON_CALL_SPLIT, DARK, "Split Model", false, false)
                    .on_disabled_hover_text("Not available: splitting a class is not implemented");
                if small_button(ui, ICON_CHECK_CIRCLE, GREEN, "Check  ▾", !self.busy, false)
                    .clicked()
                {
                    self.check();
                }
                small_button(ui, ICON_GRID_ON, BLUE, "", false, true)
                    .on_disabled_hover_text("Not available: the grid is always drawn");
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
                )
                .on_disabled_hover_text("Not available: searching connections is not implemented");
                small_button(ui, ICON_FILTER_ALT, DARK, "Diagram Filter", false, false)
                    .on_disabled_hover_text("Not available: the diagram filter is not implemented");
            });
        });
    }

    fn documentation_ribbon(&mut self, ui: &mut egui::Ui) {
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
            if big_button(
                ui,
                ICON_SOURCE_NOTES,
                BLUE,
                "Revision\nSource",
                true,
                self.doc_view == DocView::RevisionSource,
            )
            .clicked()
            {
                self.doc_view = DocView::RevisionSource;
            }
        });
        group(ui, "Find", |ui| {
            if big_button(ui, ICON_SEARCH, DARK, "Find", true, self.find_open).clicked() {
                self.find_open = !self.find_open;
            }
        });
        group(ui, "Editing", |ui| {
            column(ui, |ui| {
                ui.label(RichText::new("Documentation is read-only").strong());
                ui.label(
                    RichText::new("Edit the class in the Text tab to change the annotation.")
                        .color(theme::TEXT_WEAK),
                );
            });
        });
    }

    fn text_ribbon(&mut self, ui: &mut egui::Ui) {
        let editable = self.active.as_ref().is_some_and(|c| self.is_user_class(c));
        let copy_enabled = self.active.as_deref().is_some_and(|class| {
            self.doc_of(class).is_some()
                || matches!(self.texts.get(class), Some(Loadable::Ready(_)))
        });
        self.model_group(ui, false);
        group(ui, "History", |ui| {
            column(ui, |ui| {
                if small_button(ui, ICON_UNDO, DARK, "Undo", self.can_undo(), false).clicked() {
                    self.undo();
                }
                if small_button(ui, ICON_REDO, DARK, "Redo", self.can_redo(), false).clicked() {
                    self.redo();
                }
            });
        });
        let (command, _) = clipboard_group(ui, copy_enabled, editable, false);
        if let Some(command) = command {
            self.pending_editor_event = Some(command);
        }
        group(ui, "Find", |ui| {
            column(ui, |ui| {
                if small_button(ui, ICON_SEARCH, DARK, "Find", true, false).clicked() {
                    self.find_open = true;
                }
                if small_button(ui, ICON_FIND_REPLACE, DARK, "Replace", editable, false)
                    .on_hover_text("Replace (Ctrl+H)")
                    .on_disabled_hover_text("Replace works in a user model")
                    .clicked()
                {
                    self.open_replace_dialog();
                }
                if small_button(ui, ICON_ARROW_RIGHT_ALT, DARK, "Go To", editable, false)
                    .on_hover_text("Go to line (Ctrl+G)")
                    .on_disabled_hover_text("Go To works in a user model")
                    .clicked()
                {
                    self.open_goto_line_dialog();
                }
            });
        });
        group(ui, "Tools", |ui| {
            if big_button(ui, ICON_CHECK_CIRCLE, GREEN, "Check", !self.busy, false).clicked() {
                self.check();
            }
            if big_button(ui, ICON_SAVE, BLUE, "Save", editable, false)
                .on_disabled_hover_text("Open a user model to save it")
                .clicked()
            {
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
                self.request_run_script_dialog();
            }
            let commands = place(ui, o, (8.0, 31.0, 100.0, 22.0), |ui| {
                small_button(ui, ICON_TERMINAL, BLUE, "Commands  \u{25BE}", true, false)
            });
            if commands.clicked() {
                self.docks.bottom = true;
                self.bottom_pane = crate::app::BottomPane::Commands;
            }
        });
        fixed_group(ui, "Simulation", 316.0, |ui, o| {
            let translate = place(ui, o, (6.0, 6.0, 90.0, 22.0), |ui| {
                small_button(ui, ICON_BUILD, BLUE, "Translate", !self.busy, false)
                    .on_hover_text("Translate (build the model without simulating)")
            });
            if translate.clicked() {
                self.translate();
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
                    !self.busy,
                    false,
                )
                .on_hover_text("Simulate once per value of one parameter")
            });
            if sweep.clicked() {
                self.open_sweep_dialog();
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
                    if v.is_finite() && v >= 0.0 && v != self.setup.stop_time {
                        self.setup.stop_time = v;
                        // A typed stop time is a manual edit: the `experiment` annotation
                        // of the next opened class must not overwrite it.
                        self.setup_edited_by_user = true;
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
                        for m in omola_core::SIMULATION_METHODS {
                            if ui
                                .selectable_value(
                                    &mut self.setup.method,
                                    (*m).to_owned(),
                                    display_method(m),
                                )
                                .changed()
                            {
                                self.setup_edited_by_user = true;
                            }
                        }
                    })
            });
            let store = place(ui, o, (222.0, 31.0, 88.0, 22.0), |ui| {
                ui.add_enabled(
                    self.can_store_setup(),
                    egui::Button::new("Save in Model").frame(false),
                )
                .on_hover_text("Store the setup in the experiment annotation of the model")
                .on_disabled_hover_text("Open a user model to store the setup")
            });
            if store.clicked() {
                self.save_setup_in_model();
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
            let load_result = place(ui, o, (7.0, 6.0, 22.0, 22.0), |ui| {
                icon_button(ui, ICON_UPLOAD_FILE, BLUE, 17.0, true, false)
                    .on_hover_text("Load Result")
            });
            if load_result.clicked() {
                self.request_load_result_dialog();
            }
            let messages = place(ui, o, (47.0, 6.0, 22.0, 22.0), |ui| {
                icon_button(ui, ICON_WARNING, ORANGE, 17.0, true, false).on_hover_text("Messages")
            });
            if messages.clicked() {
                self.bottom_pane = crate::app::BottomPane::Logs;
                self.docks.bottom = true;
            }
            let new_plot = place(ui, o, (7.0, 31.0, 22.0, 22.0), |ui| {
                icon_button(ui, ICON_SHOW_CHART, BLUE, 17.0, true, false)
                    .on_hover_text("New diagram")
            });
            if new_plot.clicked() {
                self.new_diagram();
            }
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

fn clear_plot_range_inputs(ctx: &egui::Context, index: usize) {
    ctx.data_mut(|data| {
        for i in 0_usize..4 {
            data.remove::<String>(egui::Id::NULL.with(("plot_range", index, i)));
        }
        for i in 0_usize..2 {
            data.remove::<bool>(egui::Id::NULL.with(("plot_range_invalid", index, i)));
        }
    });
}

fn parse_optional_range(min: &str, max: &str) -> Result<Option<(f64, f64)>, &'static str> {
    if min.trim().is_empty() && max.trim().is_empty() {
        return Ok(None);
    }
    let min = min.trim().parse::<f64>().map_err(|_| "invalid minimum")?;
    let max = max.trim().parse::<f64>().map_err(|_| "invalid maximum")?;
    if !min.is_finite() || !max.is_finite() || min >= max {
        return Err("range bounds must be finite and increasing");
    }
    Ok(Some((min, max)))
}

/// Where the window title is painted and whether it had to be elided.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TitleLayout {
    /// Left edge of the painted title.
    x: f32,
    /// True when the title is wider than the available area and must be elided.
    truncated: bool,
}

/// Place a title of `text_width` inside `area`: centre it while it fits, else
/// start at the left edge so the elided end stays clear of the right side.
fn title_layout(area: egui::Rect, text_width: f32) -> TitleLayout {
    if text_width <= area.width() {
        TitleLayout {
            x: area.center().x - text_width / 2.0,
            truncated: false,
        }
    } else {
        TitleLayout {
            x: area.left(),
            truncated: true,
        }
    }
}

/// The Clipboard group. `copy_enabled` lights Copy (any shown text), `editable` lights
/// Cut and Paste (a user model). Returns the clipboard button pressed, if any, and whether
/// Delete was pressed (G45).
fn clipboard_group(
    ui: &mut egui::Ui,
    copy_enabled: bool,
    editable: bool,
    delete_enabled: bool,
) -> (Option<EditorCommand>, bool) {
    let mut command = None;
    let mut delete = false;
    group(ui, "Clipboard", |ui| {
        if big_button(ui, ICON_CONTENT_CUT, DARK, "Cut", editable, false)
            .on_disabled_hover_text("Cut works in a user model")
            .clicked()
        {
            command = Some(EditorCommand::Cut);
        }
        if big_button(ui, ICON_CONTENT_COPY, DARK, "Copy", copy_enabled, false)
            .on_disabled_hover_text("Open the Text tab of a class first")
            .clicked()
        {
            command = Some(EditorCommand::Copy);
        }
        if big_button(ui, ICON_CONTENT_PASTE, DARK, "Paste", editable, false)
            .on_disabled_hover_text("Paste works in a user model")
            .clicked()
        {
            command = Some(EditorCommand::Paste);
        }
        column(ui, |ui| {
            delete = small_button(ui, ICON_CLOSE, RED, "Delete", delete_enabled, false)
                .on_disabled_hover_text("Select a component of a user model first")
                .clicked();
            small_button(ui, ICON_FILE_COPY, DARK, "Duplicate", false, false)
                .on_disabled_hover_text(
                    "Not available: duplicating a component is not implemented",
                );
            small_button(ui, ICON_SELECT_ALL, DARK, "Select All", false, false)
                .on_disabled_hover_text(
                    "Not available: selecting every component is not implemented",
                );
        });
    });
    (command, delete)
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

#[cfg(test)]
mod range_tests {
    use super::{clear_plot_range_inputs, parse_optional_range};

    #[test]
    fn rescale_clears_only_the_active_diagram_inputs() {
        let ctx = egui::Context::default();
        let key =
            |diagram: usize, field: usize| egui::Id::NULL.with(("plot_range", diagram, field));
        ctx.data_mut(|data| {
            data.insert_temp(key(0, 0), "0.4".to_owned());
            data.insert_temp(key(1, 0), "0.2".to_owned());
        });
        clear_plot_range_inputs(&ctx, 0);
        ctx.data(|data| {
            assert_eq!(data.get_temp::<String>(key(0, 0)), None);
            assert_eq!(data.get_temp::<String>(key(1, 0)).as_deref(), Some("0.2"));
        });
    }

    #[test]
    fn range_input_accepts_numbers_clear_and_rejects_invalid_pairs() {
        assert_eq!(parse_optional_range("0.5", "0.8"), Ok(Some((0.5, 0.8))));
        assert_eq!(parse_optional_range("", ""), Ok(None));
        assert!(parse_optional_range("a", "0.8").is_err());
        assert!(parse_optional_range("1", "1").is_err());
        assert!(parse_optional_range("NaN", "2").is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::title_layout;
    use egui::{pos2, Rect, Vec2};

    fn area(width: f32) -> Rect {
        Rect::from_min_size(pos2(100.0, 0.0), Vec2::new(width, 30.0))
    }

    #[test]
    fn title_is_centred_when_it_fits() {
        let layout = title_layout(area(100.0), 40.0);
        assert!(!layout.truncated);
        assert!((layout.x - 130.0).abs() < 0.01);
    }

    #[test]
    fn title_uses_the_whole_area_when_it_exactly_fits() {
        let layout = title_layout(area(100.0), 100.0);
        assert!(!layout.truncated);
        assert!((layout.x - 100.0).abs() < 0.01);
    }

    #[test]
    fn long_title_starts_at_the_left_and_is_truncated() {
        let layout = title_layout(area(100.0), 140.0);
        assert!(layout.truncated);
        assert!((layout.x - 100.0).abs() < 0.01);
    }

    /// G52: a button that only logs "not implemented" tells the user nothing.
    /// The source is checked directly because the ribbon cannot be enumerated
    /// headless. The needle is split so that this test does not match itself.
    #[test]
    fn ribbon_has_no_not_implemented_buttons() {
        let needle = concat!("not_", "implemented(");
        assert!(
            !include_str!("chrome.rs").contains(needle),
            "the ribbon must not keep buttons that only log `{needle}`; remove the button \
             or wire it up"
        );
    }
}
