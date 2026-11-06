//! Bottom window (Logs / Commands) and the bottom bar (browser toggles, model tabs,
//! status message, model layer toolbar).

use super::widgets::{flat_tab, icon_button, paint_icon};
use crate::app::{BottomPane, Layer, OmolaApp, RibbonTab};
use crate::backend::{LogLevel, LogTab};
use crate::text_editor::{parse_omc_locations, SourceLocation};
use crate::theme;
use egui::{Align2, Color32, FontId, RichText, Sense, Stroke, Vec2};
use egui_material_icons::icons::*;

impl OmolaApp {
    pub fn bottom_window(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_top(|ui| {
            // Left strip: close, float, and the vertical window name.
            let strip_h = ui.available_height();
            ui.vertical(|ui| {
                ui.set_width(18.0);
                ui.spacing_mut().item_spacing.y = 0.0;
                if icon_button(ui, ICON_CLOSE, theme::TEXT, 12.0, true, false).clicked() {
                    self.docks.bottom = false;
                }
                icon_button(ui, ICON_FILTER_NONE, theme::TEXT_WEAK, 11.0, false, false);
                let (r, _) = ui.allocate_exact_size(
                    Vec2::new(18.0, (strip_h - 40.0).max(20.0)),
                    Sense::hover(),
                );
                let name = match self.bottom_pane {
                    BottomPane::Logs => "Logs",
                    BottomPane::Commands => "Commands",
                };
                let galley = ui.painter().layout_no_wrap(
                    name.into(),
                    FontId::proportional(11.5),
                    theme::TEXT,
                );
                let pos = egui::pos2(r.center().x - galley.size().y / 2.0, r.bottom() - 2.0);
                ui.painter().add(
                    egui::epaint::TextShape::new(pos, galley, theme::TEXT)
                        .with_angle(-std::f32::consts::FRAC_PI_2),
                );
            });
            ui.vertical(|ui| match self.bottom_pane {
                BottomPane::Logs => self.logs_pane(ui),
                BottomPane::Commands => self.commands_pane(ui),
            });
        });
    }

    fn logs_pane(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(4.0);
                for tab in LogTab::ALL {
                    let warn = self
                        .logs
                        .iter()
                        .any(|e| e.tab == tab && e.level != LogLevel::Info);
                    if log_tab(ui, tab.label(), self.log_tab == tab, warn).clicked() {
                        self.log_tab = tab;
                    }
                }
            });
            egui::Frame::new()
                .fill(Color32::WHITE)
                .stroke(Stroke::new(1.0, Color32::from_rgb(0x82, 0x87, 0x90)))
                .inner_margin(egui::Margin {
                    left: 4,
                    right: 2,
                    top: 3,
                    bottom: 2,
                })
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_height(ui.available_height());
                    egui::ScrollArea::vertical()
                        .id_salt(("logs", self.log_tab as u8))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            // A click is handled after the loop so that `self.logs` is no
                            // longer borrowed when the document is opened.
                            let mut jump: Option<SourceLocation> = None;
                            for entry in self.logs.iter().filter(|e| e.tab == self.log_tab) {
                                let color = match entry.level {
                                    LogLevel::Info => theme::TEXT,
                                    LogLevel::Warning => Color32::from_rgb(0xA0, 0x60, 0x00),
                                    LogLevel::Error => Color32::from_rgb(0xC6, 0x28, 0x28),
                                };
                                let text = RichText::new(&entry.text)
                                    .monospace()
                                    .size(12.0)
                                    .color(color);
                                let locations = parse_omc_locations(&entry.text);
                                if locations.is_empty() {
                                    ui.label(text);
                                } else {
                                    // The message names a source line: clicking it shows
                                    // that line in the Text layer (G25).
                                    let response = ui
                                        .add(egui::Label::new(text).sense(Sense::click()))
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .on_hover_text("Click to show this line in the Text layer");
                                    if response.clicked() {
                                        jump = locations.into_iter().next();
                                    }
                                }
                                ui.add_space(12.0);
                            }
                            if let Some(location) = jump {
                                self.jump_to_location(&location);
                            }
                        });
                });
        });
    }

    fn commands_pane(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.command_input)
                    .font(egui::TextStyle::Monospace)
                    .hint_text("OpenModelica scripting command, e.g. getVersion()")
                    .desired_width(f32::INFINITY),
            );
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.run_command();
                r.request_focus();
            }
            egui::Frame::new()
                .fill(Color32::WHITE)
                .stroke(Stroke::new(1.0, Color32::from_rgb(0x82, 0x87, 0x90)))
                .inner_margin(egui::Margin::symmetric(4, 2))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_height(ui.available_height());
                    egui::ScrollArea::vertical()
                        .id_salt("commands")
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            for line in &self.command_history {
                                ui.label(RichText::new(line).monospace().size(12.0));
                            }
                        });
                });
        });
    }

    pub fn bottom_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            if self.tab == RibbonTab::Simulation {
                let strip = self.model_tabs.len() as f32 * 112.0 + 48.0;
                ui.add_space((ui.available_width() - 145.0 - strip).max(0.0));
                self.bottom_model_tabs(ui);
            } else {
                let simulation_tabs = [
                    ("Libraries", None),
                    ("Package Browser", Some(self.docks.package_browser)),
                    ("Component Browser", Some(self.docks.component_browser)),
                    ("Diagram Filter", None),
                    ("Variable Browser", Some(self.docks.variable_browser)),
                    ("Steady State", None),
                    ("Sweep Parameters", None),
                    (
                        "Commands",
                        Some(self.docks.bottom && self.bottom_pane == BottomPane::Commands),
                    ),
                    (
                        "Logs",
                        Some(self.docks.bottom && self.bottom_pane == BottomPane::Logs),
                    ),
                ];
                for (label, state) in simulation_tabs {
                    let r = flat_tab(ui, label, state == Some(true));
                    if state.is_none() {
                        r.on_hover_text("Not implemented yet");
                        continue;
                    }
                    if r.clicked() {
                        match label {
                            "Package Browser" => self.docks.package_browser ^= true,
                            "Component Browser" => self.docks.component_browser ^= true,
                            "Variable Browser" => self.docks.variable_browser ^= true,
                            "Commands" => self.toggle_bottom(BottomPane::Commands),
                            _ => self.toggle_bottom(BottomPane::Logs),
                        }
                    }
                }
                if self.busy {
                    ui.add_space(12.0);
                    ui.spinner();
                    ui.label(RichText::new(" Running OpenModelica...").size(12.0));
                    if ui.small_button("Stop").clicked() {
                        self.worker.cancel();
                    }
                } else if self.omc_version.is_none() {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("OpenModelica (omc) not found")
                            .size(12.0)
                            .color(Color32::from_rgb(0xC6, 0x28, 0x28)),
                    );
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(22.0);
                ui.spacing_mut().item_spacing.x = 3.0;
                let layer = self.layer();
                let sim = self.tab == RibbonTab::Simulation;
                icon_button(ui, ICON_ACCOUNT_TREE, theme::ACCENT, 15.0, false, false)
                    .on_disabled_hover_text("Used classes");
                if icon_button(
                    ui,
                    ICON_SUBJECT,
                    theme::ACCENT,
                    15.0,
                    true,
                    !sim && layer == Layer::Text,
                )
                .on_hover_text("Modelica Text")
                .clicked()
                {
                    self.set_layer(Layer::Text);
                }
                if icon_button(
                    ui,
                    ICON_INFO_I,
                    theme::ACCENT,
                    15.0,
                    true,
                    !sim && layer == Layer::Documentation,
                )
                .on_hover_text("Documentation")
                .clicked()
                {
                    self.set_layer(Layer::Documentation);
                }
                if icon_button(
                    ui,
                    ICON_SCHEMA,
                    theme::ACCENT,
                    15.0,
                    true,
                    !sim && layer == Layer::Diagram,
                )
                .on_hover_text("Diagram")
                .clicked()
                {
                    self.set_layer(Layer::Diagram);
                }
                if icon_button(
                    ui,
                    ICON_WIDGETS,
                    theme::ACCENT,
                    15.0,
                    true,
                    !sim && layer == Layer::Icon,
                )
                .on_hover_text("Icon")
                .clicked()
                {
                    self.set_layer(Layer::Icon);
                }
            });
        });
    }

    fn toggle_bottom(&mut self, pane: BottomPane) {
        if self.docks.bottom && self.bottom_pane == pane {
            self.docks.bottom = false;
        } else {
            self.docks.bottom = true;
            self.bottom_pane = pane;
        }
    }

    /// Model tabs in the bottom bar (simulation view): `Name x` ... `+ v`.
    fn bottom_model_tabs(&mut self, ui: &mut egui::Ui) {
        let mut activate = None;
        let mut close = None;
        for class in self.model_tabs.clone() {
            let selected = self.active.as_deref() == Some(class.as_str());
            let label = crate::library::short_name(&class);
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(112.0, 25.0), Sense::click());
            if selected {
                ui.painter().rect_filled(rect, 0.0, Color32::WHITE);
            }
            ui.painter().text(
                egui::pos2(rect.left() + 25.0, rect.center().y),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(12.0),
                theme::TEXT,
            );
            let x = egui::pos2(rect.left() + 92.0, rect.center().y);
            paint_icon(
                ui,
                x,
                ICON_CLOSE,
                14.0,
                if selected {
                    theme::TEXT
                } else {
                    theme::TEXT_WEAK
                },
            );
            if resp.clicked() {
                if resp
                    .interact_pointer_pos()
                    .is_some_and(|p| (p - x).length() < 9.0)
                {
                    close = Some(class.clone());
                } else {
                    activate = Some(class.clone());
                }
            }
        }
        ui.add_space(8.0);
        if icon_button(ui, ICON_ADD, theme::TEXT, 14.0, true, false)
            .on_hover_text("New model")
            .clicked()
        {
            self.new_model();
        }
        icon_button(ui, ICON_ARROW_DROP_DOWN, theme::ACCENT, 16.0, true, false);
        if let Some(c) = close {
            self.close_tab(&c);
        }
        if let Some(c) = activate {
            self.open_class(&c);
        }
    }
}

fn log_tab(ui: &mut egui::Ui, label: &str, selected: bool, warn: bool) -> egui::Response {
    let font = FontId::proportional(12.0);
    let galley = ui
        .painter()
        .layout_no_wrap(label.into(), font.clone(), theme::TEXT);
    let icon_w = if warn { 20.0 } else { 0.0 };
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(galley.size().x + 34.0 + icon_w, 22.0),
        Sense::click(),
    );
    let painter = ui.painter();
    if selected {
        painter.rect(
            rect,
            0.0,
            Color32::WHITE,
            Stroke::new(1.0, Color32::from_rgb(0xD0, 0xD0, 0xD0)),
            egui::StrokeKind::Inside,
        );
    } else {
        painter.vline(
            rect.right() - 0.5,
            rect.y_range().shrink(3.0),
            Stroke::new(1.0, Color32::from_rgb(0xD0, 0xD0, 0xD0)),
        );
        if response.hovered() {
            painter.rect_filled(rect, 0.0, theme::HOVER_BG);
        }
    }
    let x = rect.left() + 17.0;
    if warn {
        super::widgets::paint_icon(
            ui,
            egui::pos2(x + 7.0, rect.center().y),
            ICON_WARNING,
            15.0,
            Color32::from_rgb(0xE8, 0xA8, 0x00),
        );
    }
    ui.painter().text(
        egui::pos2(x + icon_w, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        theme::TEXT,
    );
    response
}
