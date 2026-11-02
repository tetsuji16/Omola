//! Window chrome: menu bar, ribbon (tabs + context toolbar), status bar, messages window.

use crate::app::{GraphicsLayer, OmolaApp, PathDialog, RibbonTab};
use crate::backend::LogLevel;
use egui::{Color32, RichText};

impl OmolaApp {
    pub fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New model").clicked() {
                    self.document = crate::model::Document::new_untitled(
                        "model Unnamed\nequation\nend Unnamed;\n",
                    );
                    self.selected_class = None;
                    self.refresh_outline();
                    self.tab = RibbonTab::Text;
                }
                if ui.button("Open…").clicked() {
                    self.path_dialog = Some(PathDialog::Open(String::new()));
                }
                if ui.button("Save   Ctrl+S").clicked() {
                    self.save();
                }
                if ui.button("Save as…").clicked() {
                    self.path_dialog = Some(PathDialog::SaveAs(String::new()));
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Simulation", |ui| {
                if ui.button("Setup…").clicked() {
                    self.show_setup = true;
                }
                if ui.button("Check").clicked() {
                    self.check();
                }
                if ui.button("Simulate   F9").clicked() {
                    self.simulate();
                }
            });
            ui.menu_button("Plot", |ui| {
                if ui.button("Erase curves").clicked() {
                    self.plotted.clear();
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
            for tab in RibbonTab::ALL {
                let selected = self.tab == tab;
                let text = RichText::new(tab.label()).strong();
                if ui.selectable_label(selected, text).clicked() {
                    self.tab = tab;
                }
            }
        });
        ui.separator();
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
                    ui.add_enabled(false, egui::Button::new("Connect mode"))
                        .on_disabled_hover_text("Diagram editing: issue G07");
                    self.check_button(ui);
                }
                RibbonTab::Documentation => {
                    ui.label("Documentation layer (read-only preview)");
                }
                RibbonTab::Text => {
                    self.check_button(ui);
                    if ui.button("Save").clicked() {
                        self.save();
                    }
                }
                RibbonTab::Simulation => {
                    if ui.button("Setup…").clicked() {
                        self.show_setup = true;
                    }
                    self.check_button(ui);
                    ui.add_enabled(false, egui::Button::new("Translate"))
                        .on_disabled_hover_text("Separate translate step: issue G05");
                    let simulate = ui.add_enabled(!self.busy, egui::Button::new("Simulate"));
                    if simulate.on_hover_text("F9").clicked() {
                        self.simulate();
                    }
                    ui.separator();
                    if ui.button("Erase curves").clicked() {
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
                    if ui.button("Locate OMC").clicked() {
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
                if ui
                    .add_enabled(!self.busy, egui::Button::new("Simulate"))
                    .clicked()
                {
                    self.simulate();
                }
            }
        });
    }

    fn check_button(&mut self, ui: &mut egui::Ui) {
        if ui
            .add_enabled(!self.busy, egui::Button::new("Check"))
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
                Some(v) => ui.label(format!("Backend: {v}")),
                None => ui.colored_label(Color32::from_rgb(200, 120, 0), "Backend: OMC not found"),
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
            if ui.small_button("Clear").clicked() {
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
                        LogLevel::Warning => Color32::from_rgb(200, 140, 0),
                        LogLevel::Error => Color32::from_rgb(220, 60, 60),
                    };
                    ui.label(RichText::new(message).monospace().color(color));
                }
            });
    }
}
