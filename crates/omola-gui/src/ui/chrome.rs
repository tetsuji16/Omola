//! Window chrome: menu bar, ribbon (tabs + context toolbar), status bar, messages window.
//!
//! Styling stays close to Win32/WinUI conventions: a flat menu bar, a borderless
//! toolbar whose buttons only gain a frame on hover, and a single accent-coloured
//! primary action.

use crate::app::{GraphicsLayer, OmolaApp, PathDialog, RibbonTab};
use crate::backend::LogLevel;
use crate::theme;
use egui::{Button, CornerRadius, RichText, Stroke};

impl OmolaApp {
    pub fn menu_bar(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.x = 2.0;
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui
                    .add(Button::new("New model").shortcut_text("Ctrl+N"))
                    .clicked()
                {
                    self.new_model();
                    ui.close();
                }
                if ui
                    .add(Button::new("Open…").shortcut_text("Ctrl+O"))
                    .clicked()
                {
                    self.path_dialog = Some(PathDialog::Open(String::new()));
                    ui.close();
                }
                if ui
                    .add(Button::new("Save").shortcut_text("Ctrl+S"))
                    .clicked()
                {
                    self.save();
                    ui.close();
                }
                if ui.button("Save as…").clicked() {
                    self.path_dialog = Some(PathDialog::SaveAs(String::new()));
                    ui.close();
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("View", |ui| {
                for tab in RibbonTab::ALL {
                    if ui.selectable_label(self.tab == tab, tab.label()).clicked() {
                        self.tab = tab;
                        ui.close();
                    }
                }
            });
            ui.menu_button("Simulation", |ui| {
                if ui.button("Setup…").clicked() {
                    self.show_setup = true;
                    ui.close();
                }
                if ui.button("Check").clicked() {
                    self.check();
                    ui.close();
                }
                if ui
                    .add(Button::new("Simulate").shortcut_text("F9"))
                    .clicked()
                {
                    self.simulate();
                    ui.close();
                }
            });
            ui.menu_button("Plot", |ui| {
                if ui.button("Erase curves").clicked() {
                    self.plotted.clear();
                    ui.close();
                }
            });
            ui.menu_button("Tools", |ui| {
                if ui.button("Locate OMC").clicked() {
                    self.worker.submit(crate::backend::Job::Doctor);
                    ui.close();
                }
            });
            ui.menu_button("Help", |ui| {
                ui.label("Omola — open-source Modelica workbench (OMC backend).");
                ui.label("Not affiliated with Dassault Systèmes.");
            });
        });
    }

    pub fn ribbon(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for tab in RibbonTab::ALL {
                if tab_button(ui, self.tab == tab, tab.label()).clicked() {
                    self.tab = tab;
                }
            }
        });
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            match self.tab {
                RibbonTab::Graphics => {
                    ui.selectable_value(
                        &mut self.graphics_layer,
                        GraphicsLayer::Diagram,
                        "Diagram",
                    );
                    ui.selectable_value(&mut self.graphics_layer, GraphicsLayer::Icon, "Icon");
                    ui.separator();
                    ui.add_enabled(false, Button::new("Connect mode"))
                        .on_disabled_hover_text("Diagram editing: issue G07");
                    self.check_button(ui);
                }
                RibbonTab::Documentation => {
                    ui.label(RichText::new("Documentation layer (read-only preview)").weak());
                }
                RibbonTab::Text => {
                    self.check_button(ui);
                    if toolbar_button(ui, "Save").clicked() {
                        self.save();
                    }
                }
                RibbonTab::Simulation => {
                    if toolbar_button(ui, "Setup…").clicked() {
                        self.show_setup = true;
                    }
                    self.check_button(ui);
                    ui.add_enabled(false, Button::new("Translate"))
                        .on_disabled_hover_text("Separate translate step: issue G05");
                    if primary_button(ui, !self.busy, "Simulate")
                        .on_hover_text("F9")
                        .clicked()
                    {
                        self.simulate();
                    }
                    ui.separator();
                    if toolbar_button(ui, "Erase curves").clicked() {
                        self.plotted.clear();
                    }
                    ui.label("Stop time:");
                    ui.add(
                        egui::DragValue::new(&mut self.setup.stop_time)
                            .range(0.0..=f64::MAX)
                            .speed(0.1)
                            .suffix(" s"),
                    );
                }
                RibbonTab::Tools => {
                    if toolbar_button(ui, "Locate OMC").clicked() {
                        self.worker.submit(crate::backend::Job::Doctor);
                    }
                }
            }
            if self.tab != RibbonTab::Simulation {
                ui.separator();
                ui.label("Model:");
                let classes: Vec<String> = self
                    .classes
                    .iter()
                    .map(|c| c.qualified_name.clone())
                    .collect();
                egui::ComboBox::from_id_salt("class_select")
                    .selected_text(self.selected_class.clone().unwrap_or_else(|| "—".into()))
                    .show_ui(ui, |ui| {
                        for name in classes {
                            ui.selectable_value(&mut self.selected_class, Some(name.clone()), name);
                        }
                    });
                if primary_button(ui, !self.busy, "Simulate").clicked() {
                    self.simulate();
                }
            }
        });
    }

    fn check_button(&mut self, ui: &mut egui::Ui) {
        if ui
            .add_enabled(!self.busy, Button::new("Check").frame_when_inactive(false))
            .clicked()
        {
            self.check();
        }
    }

    pub fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.busy {
                ui.spinner();
                ui.label("Running OMC…");
            } else {
                ui.label("Ready");
            }
            ui.separator();
            match &self.omc_version {
                Some(v) => ui.label(RichText::new(format!("Backend: {v}")).weak()),
                None => ui.colored_label(
                    ui.visuals().error_fg_color,
                    RichText::new("Backend: OMC not found"),
                ),
            };
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Mode switch in the bottom-right corner, as in classic Dymola layouts.
                if ui
                    .selectable_label(self.tab == RibbonTab::Simulation, "Simulation")
                    .clicked()
                {
                    self.tab = RibbonTab::Simulation;
                }
                if ui
                    .selectable_label(self.tab.is_modeling(), "Modeling")
                    .clicked()
                {
                    self.tab = RibbonTab::Text;
                }
            });
        });
    }

    pub fn messages_window(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("Messages");
            if ui
                .add(Button::new("Clear").small().frame_when_inactive(false))
                .clicked()
            {
                self.log.clear();
            }
        });
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for (level, message) in &self.log {
                    let color = match level {
                        LogLevel::Info => ui.visuals().text_color(),
                        LogLevel::Warning => ui.visuals().warn_fg_color,
                        LogLevel::Error => ui.visuals().error_fg_color,
                    };
                    ui.label(RichText::new(message).monospace().color(color));
                }
            });
    }
}

/// Flat, Windows-style toolbar button: no frame until hovered.
fn toolbar_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(Button::new(text).frame_when_inactive(false))
}

/// Filled accent button for the primary action (Simulate).
fn primary_button(ui: &mut egui::Ui, enabled: bool, text: &str) -> egui::Response {
    let accent = theme::accent(ui);
    ui.add_enabled(
        enabled,
        Button::new(RichText::new(text).color(theme::on_accent(ui)).strong())
            .fill(accent)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(4)),
    )
}

/// Menu-style tab: flat label, subtle hover fill and an accent underline when selected.
fn tab_button(ui: &mut egui::Ui, selected: bool, label: &str) -> egui::Response {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let color = if selected {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, color);
    let size = egui::vec2(galley.size().x + 22.0, 28.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        if selected {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(4), ui.visuals().faint_bg_color);
        } else if response.hovered() {
            ui.painter().rect_filled(
                rect,
                CornerRadius::same(4),
                ui.visuals().widgets.hovered.weak_bg_fill,
            );
        }
        ui.painter()
            .galley(rect.center() - galley.size() * 0.5, galley, color);
        if selected {
            let y = rect.bottom() - 1.0;
            let underline = egui::Rect::from_min_max(
                egui::pos2(rect.left() + 9.0, y - 2.0),
                egui::pos2(rect.right() - 9.0, y),
            );
            ui.painter()
                .rect_filled(underline, CornerRadius::same(1), theme::accent(ui));
        }
    }

    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
