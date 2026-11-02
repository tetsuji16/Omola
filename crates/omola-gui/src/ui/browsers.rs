//! Left-side browsers: package + component browser (modeling), variable browser (simulation).

use crate::app::{OmolaApp, PlotSignal};
use egui::RichText;

impl OmolaApp {
    pub fn modeling_browsers(&mut self, ui: &mut egui::Ui) {
        let half = (ui.available_height() * 0.55).max(120.0);
        ui.strong("Package Browser");
        egui::ScrollArea::vertical()
            .id_salt("package_browser")
            .max_height(half)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.label(RichText::new(self.document.title()).italics());
                for entry in self.classes.clone() {
                    ui.horizontal(|ui| {
                        ui.add_space(12.0 * (entry.depth as f32 + 1.0));
                        let icon = class_icon(&entry.kind);
                        let selected =
                            self.selected_class.as_deref() == Some(entry.qualified_name.as_str());
                        let short = entry
                            .qualified_name
                            .rsplit('.')
                            .next()
                            .unwrap_or(&entry.qualified_name)
                            .to_owned();
                        let response = ui.selectable_label(selected, format!("{icon} {short}"));
                        let response = match &entry.description {
                            Some(d) => response.on_hover_text(format!(
                                "{} {}\n{d}",
                                entry.kind, entry.qualified_name
                            )),
                            None => response
                                .on_hover_text(format!("{} {}", entry.kind, entry.qualified_name)),
                        };
                        if response.clicked() {
                            self.selected_class = Some(entry.qualified_name.clone());
                        }
                    });
                }
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Modelica Standard Library: not loaded (issue G04)")
                        .small()
                        .weak(),
                );
            });
        ui.separator();
        ui.strong("Component Browser");
        ui.label(
            RichText::new("Component hierarchy of the selected class: issue G06")
                .small()
                .weak(),
        );
    }

    pub fn variable_browser(&mut self, ui: &mut egui::Ui) {
        ui.strong("Variable Browser");
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.variable_filter);
        });
        ui.horizontal(|ui| {
            ui.label("Keep results:");
            ui.add(egui::DragValue::new(&mut self.keep_results).range(1..=20));
        });
        ui.separator();
        if self.runs.is_empty() {
            ui.label(RichText::new("No results yet. Press Simulate (F9).").weak());
            return;
        }
        let filter = self.variable_filter.to_lowercase();
        let mut toggles: Vec<PlotSignal> = Vec::new();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("variable_header")
                    .num_columns(3)
                    .show(ui, |ui| {
                        ui.label(RichText::new("Variable").small().strong());
                        ui.label(RichText::new("Value").small().strong());
                        ui.label(RichText::new("Description").small().strong());
                        ui.end_row();
                    });
                for run in self.runs.iter().rev() {
                    let header = egui::CollapsingHeader::new(RichText::new(&run.label).strong())
                        .id_salt(("run", run.id))
                        .default_open(true);
                    header.show(ui, |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{}  ({:.2} s)",
                                run.outcome.result_file.display(),
                                run.outcome.elapsed_seconds
                            ))
                            .small()
                            .weak(),
                        );
                        egui::Grid::new(("vars", run.id))
                            .num_columns(3)
                            .striped(true)
                            .show(ui, |ui| {
                                let result = &run.outcome.result;
                                for (i, var) in result.variables.iter().enumerate() {
                                    if var.name == "time"
                                        || (!filter.is_empty()
                                            && !var.name.to_lowercase().contains(&filter))
                                    {
                                        continue;
                                    }
                                    let signal = PlotSignal {
                                        run_id: run.id,
                                        variable: var.name.clone(),
                                    };
                                    let mut on = self.plotted.contains(&signal);
                                    let label = if var.is_parameter {
                                        format!("{} (p)", var.name)
                                    } else {
                                        var.name.clone()
                                    };
                                    if ui.checkbox(&mut on, label).changed() {
                                        toggles.push(signal);
                                    }
                                    let last = result.values[i].last().copied().unwrap_or(f64::NAN);
                                    ui.label(RichText::new(format!("{last:.6}")).monospace());
                                    ui.label(RichText::new(&var.description).small().weak());
                                    ui.end_row();
                                }
                            });
                    });
                }
            });
        for signal in toggles {
            if let Some(pos) = self.plotted.iter().position(|s| *s == signal) {
                self.plotted.remove(pos);
            } else {
                self.plotted.push(signal);
            }
        }
    }
}

fn class_icon(kind: &str) -> &'static str {
    match kind {
        "package" => "[P]",
        "model" => "[M]",
        "block" => "[B]",
        "connector" => "[C]",
        "record" => "[R]",
        "function" => "[F]",
        "type" => "[T]",
        _ => "[ ]",
    }
}
