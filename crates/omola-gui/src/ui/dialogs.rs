//! Modal-ish windows: Simulation Setup and the temporary path dialogs.

use crate::app::{OmolaApp, PathDialog};
use std::path::PathBuf;

impl OmolaApp {
    pub fn dialogs(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.setup_dialog(&ctx);
        self.path_dialog_window(&ctx);
    }

    fn setup_dialog(&mut self, ctx: &egui::Context) {
        let mut open = self.show_setup;
        // Snapshot the general fields so a manual edit can be detected: from then on the
        // `experiment` annotation of a newly opened class must not overwrite them.
        let before = (
            self.setup.start_time,
            self.setup.stop_time,
            self.setup.number_of_intervals,
            self.setup.tolerance,
            self.setup.method.clone(),
        );
        egui::Window::new("Simulation Setup")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Model: {}",
                    self.active.clone().unwrap_or_else(|| "-".into())
                ));
                ui.separator();
                ui.strong("General");
                egui::Grid::new("setup_general")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Start time");
                        ui.add(
                            egui::DragValue::new(&mut self.setup.start_time)
                                .speed(0.1)
                                .suffix(" s"),
                        );
                        ui.end_row();
                        ui.label("Stop time");
                        ui.add(
                            egui::DragValue::new(&mut self.setup.stop_time)
                                .range(0.0..=f64::MAX)
                                .speed(0.1)
                                .suffix(" s"),
                        );
                        ui.end_row();
                        ui.label("Number of intervals");
                        ui.add(
                            egui::DragValue::new(&mut self.setup.number_of_intervals)
                                .range(1..=10_000_000),
                        );
                        ui.end_row();
                        ui.label("Tolerance");
                        ui.add(
                            egui::DragValue::new(&mut self.setup.tolerance)
                                .speed(1e-6)
                                .range(1e-12..=1.0),
                        );
                        ui.end_row();
                        ui.label("Algorithm");
                        egui::ComboBox::from_id_salt("algorithm")
                            .selected_text(self.setup.method.clone())
                            .show_ui(ui, |ui| {
                                for m in omola_core::SIMULATION_METHODS {
                                    ui.selectable_value(
                                        &mut self.setup.method,
                                        (*m).to_owned(),
                                        *m,
                                    );
                                }
                            });
                        ui.end_row();
                    });
                ui.separator();
                ui.strong("Parameter overrides");
                let mut remove = None;
                for (i, (name, value)) in self.setup.parameters.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(name)
                                .desired_width(140.0)
                                .hint_text("name"),
                        );
                        ui.label("=");
                        ui.add(
                            egui::TextEdit::singleline(value)
                                .desired_width(80.0)
                                .hint_text("value"),
                        );
                        if ui.small_button("x").clicked() {
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    self.setup.parameters.remove(i);
                }
                if ui.small_button("+ Add parameter").clicked() {
                    self.setup.parameters.push((String::new(), String::new()));
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!self.busy, egui::Button::new("Simulate"))
                        .clicked()
                    {
                        self.setup
                            .parameters
                            .retain(|(n, v)| !n.trim().is_empty() && !v.trim().is_empty());
                        self.simulate();
                        self.show_setup = false;
                    }
                    if ui.button("OK").clicked() {
                        self.setup
                            .parameters
                            .retain(|(n, v)| !n.trim().is_empty() && !v.trim().is_empty());
                        self.show_setup = false;
                    }
                });
            });
        self.show_setup = open && self.show_setup;
        let after = (
            self.setup.start_time,
            self.setup.stop_time,
            self.setup.number_of_intervals,
            self.setup.tolerance,
            self.setup.method.clone(),
        );
        if after != before {
            self.setup_edited_by_user = true;
        }
    }

    fn path_dialog_window(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.path_dialog.clone() else {
            return;
        };
        let (title, prompt, mut text) = match &dialog {
            PathDialog::Open(t) => ("Open Modelica file", "Path to a .mo file", t.clone()),
            PathDialog::SaveAs(t) => ("Save as", "Path to a .mo file", t.clone()),
            PathDialog::RunScript(t) => (
                "Run Script",
                "Path to an OpenModelica script (.mos)",
                t.clone(),
            ),
        };
        let mut keep_open = true;
        let mut submitted = false;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .open(&mut keep_open)
            .show(ctx, |ui| {
                ui.label(format!("{prompt} (native file dialog: issue G03):"));
                let response = ui.add(egui::TextEdit::singleline(&mut text).desired_width(420.0));
                let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("OK").clicked() || enter {
                    submitted = true;
                }
            });
        if submitted && !text.trim().is_empty() {
            self.path_dialog = None;
            let path = PathBuf::from(text.trim());
            match dialog {
                PathDialog::Open(_) => self.open_file(path),
                PathDialog::SaveAs(_) => {
                    if let Some(i) = self.active_doc() {
                        self.documents[i].path = Some(path);
                    }
                    self.save();
                }
                PathDialog::RunScript(_) => {
                    self.command_input = format!(
                        "runScript(\"{}\")",
                        omola_core::escape_modelica_string(&path.to_string_lossy())
                    );
                    self.bottom_pane = crate::app::BottomPane::Commands;
                    self.docks.bottom = true;
                    self.run_command();
                }
            }
        } else if !keep_open {
            self.path_dialog = None;
        } else {
            self.path_dialog = Some(match dialog {
                PathDialog::Open(_) => PathDialog::Open(text),
                PathDialog::SaveAs(_) => PathDialog::SaveAs(text),
                PathDialog::RunScript(_) => PathDialog::RunScript(text),
            });
        }
    }
}
