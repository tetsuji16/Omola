//! Modal-ish windows: Simulation Setup, the path dialogs and the component rename dialog.

use crate::app::{OmolaApp, PathDialog, RenameDialog};
use crate::theme;
use std::path::PathBuf;

impl OmolaApp {
    pub fn dialogs(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.setup_dialog(&ctx);
        self.path_dialog_window(&ctx);
        self.rename_dialog_window(&ctx);
        self.parameter_dialog_window(&ctx);
    }

    /// Parameters of one component (G33).
    ///
    /// The dialog edits text only; the validation is OMC's. `;` and line breaks are
    /// stripped as they are typed, because they would end the statement in the edit script.
    fn parameter_dialog_window(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.parameter_dialog.clone() else {
            return;
        };
        let mut rows = dialog.rows.clone().unwrap_or_default();
        let mut open = true;
        let mut submitted = false;
        let mut cancelled = false;
        egui::Window::new(format!("Parameters: {}", dialog.component))
            .collapsible(false)
            .resizable(true)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(&dialog.type_name).color(theme::TEXT_WEAK));
                if let Some(error) = &dialog.error {
                    ui.colored_label(theme::ERROR, error);
                } else if dialog.rows.is_none() {
                    ui.label(
                        egui::RichText::new("Reading the parameters ...").color(theme::TEXT_WEAK),
                    );
                }
                if !rows.is_empty() {
                    egui::Grid::new("parameter_rows")
                        .striped(true)
                        .num_columns(4)
                        .show(ui, |ui| {
                            ui.strong("Name");
                            ui.strong("Value");
                            ui.strong("Unit");
                            ui.strong("Description");
                            ui.end_row();
                            for row in &mut rows {
                                ui.label(&row.name);
                                if !row.choices.is_empty() {
                                    let current =
                                        row.value.rsplit('.').next().unwrap_or_default().to_owned();
                                    egui::ComboBox::from_id_salt(("param", &row.name))
                                        .selected_text(if current.is_empty() {
                                            row.default.clone()
                                        } else {
                                            current
                                        })
                                        .show_ui(ui, |ui| {
                                            for choice in &row.choices {
                                                if ui
                                                    .selectable_label(row.value == *choice, choice)
                                                    .clicked()
                                                {
                                                    row.value = choice.clone();
                                                }
                                            }
                                        });
                                } else if row.is_boolean {
                                    egui::ComboBox::from_id_salt(("param", &row.name))
                                        .selected_text(if row.value.is_empty() {
                                            row.default.clone()
                                        } else {
                                            row.value.clone()
                                        })
                                        .show_ui(ui, |ui| {
                                            for choice in ["true", "false"] {
                                                if ui
                                                    .selectable_label(row.value == choice, choice)
                                                    .clicked()
                                                {
                                                    row.value = choice.to_owned();
                                                }
                                            }
                                        });
                                } else {
                                    // An unchanged row shows the default greyed out as a hint.
                                    let hint = if row.default.is_empty() {
                                        row.original.clone()
                                    } else {
                                        row.default.clone()
                                    };
                                    ui.add(
                                        egui::TextEdit::singleline(&mut row.value)
                                            .hint_text(hint)
                                            .desired_width(120.0),
                                    );
                                }
                                ui.label(&row.unit);
                                ui.label(&row.comment);
                                ui.end_row();
                            }
                        });
                }
                ui.horizontal(|ui| {
                    let ready = dialog.rows.is_some() && dialog.error.is_none();
                    if ui
                        .add_enabled(dialog.editable && ready, egui::Button::new("OK"))
                        .clicked()
                    {
                        submitted = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancelled = true;
                    }
                    if !dialog.editable {
                        ui.label(
                            egui::RichText::new("read-only (library class)")
                                .color(theme::TEXT_WEAK),
                        );
                    }
                });
            });
        // A `;` or a line break would end the statement the edit script builds.
        for row in &mut rows {
            row.value.retain(|c| c != ';' && c != '\n' && c != '\r');
        }
        if submitted {
            self.parameter_dialog = None;
            let ops = crate::app::param_ops(&dialog.component, &rows);
            if !ops.is_empty() {
                self.apply_edit(&dialog.class, ops);
            }
        } else if cancelled || !open {
            self.parameter_dialog = None;
        } else if dialog.rows.is_some() {
            if let Some(state) = &mut self.parameter_dialog {
                state.rows = Some(rows);
            }
        }
    }

    /// Rename a Diagram component (G31).
    ///
    /// OK stays disabled until the text is an identifier that no other component uses,
    /// so a name that OMC would reject never reaches the edit script.
    fn rename_dialog_window(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.rename_dialog.clone() else {
            return;
        };
        // The old name is left out: the dialog starts with it in the box.
        let existing: Vec<String> = self
            .component_names(&dialog.class)
            .into_iter()
            .filter(|name| name != &dialog.old)
            .collect();
        let mut text = dialog.new_text.clone();
        let mut open = true;
        let mut cancelled = false;
        let mut submitted = false;
        egui::Window::new("Rename component")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(format!("{} in {}", dialog.old, dialog.class));
                let response = ui.add(egui::TextEdit::singleline(&mut text).desired_width(240.0));
                let name = text.trim();
                let problem = crate::edit::check_component_name(name, &existing).err();
                let unchanged = name == dialog.old;
                if let Some(message) = problem {
                    ui.colored_label(theme::ERROR, message);
                } else if unchanged {
                    ui.label(egui::RichText::new("Type a new name.").color(theme::TEXT_WEAK));
                }
                let can_ok = problem.is_none() && !unchanged;
                let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.horizontal(|ui| {
                    if ui.add_enabled(can_ok, egui::Button::new("OK")).clicked()
                        || (enter && can_ok)
                    {
                        submitted = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancelled = true;
                    }
                });
            });
        if submitted {
            self.rename_dialog = None;
            let new = text.trim().to_owned();
            let op = crate::edit::EditOp::RenameComponent {
                old: dialog.old.clone(),
                new: new.clone(),
            };
            if self.apply_edit(&dialog.class, vec![op])
                && self.selected_component.as_deref() == Some(dialog.old.as_str())
            {
                self.selected_component = Some(new);
            }
        } else if cancelled || !open {
            self.rename_dialog = None;
        } else {
            self.rename_dialog = Some(RenameDialog {
                new_text: text,
                ..dialog
            });
        }
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
            PathDialog::ExportCsv(t) => ("Export plot as CSV", "Path to a .csv file", t.clone()),
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
                PathDialog::ExportCsv(_) => self.export_csv(&path),
            }
        } else if !keep_open {
            self.path_dialog = None;
        } else {
            self.path_dialog = Some(match dialog {
                PathDialog::Open(_) => PathDialog::Open(text),
                PathDialog::SaveAs(_) => PathDialog::SaveAs(text),
                PathDialog::RunScript(_) => PathDialog::RunScript(text),
                PathDialog::ExportCsv(_) => PathDialog::ExportCsv(text),
            });
        }
    }
}
