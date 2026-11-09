//! Modal-ish windows: Simulation Setup and the temporary path dialogs.

use crate::app::{component_name_error, OmolaApp, PathDialog, RenameDialog};
use std::path::PathBuf;

impl OmolaApp {
    pub fn dialogs(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.setup_dialog(&ctx);
        self.path_dialog_window(&ctx);
        self.rename_dialog_window(&ctx);
        self.parameter_dialog_window(&ctx);
    }

    fn parameter_dialog_window(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.parameter_dialog.clone() else {
            return;
        };
        let title = format!(
            "Parameters: {} ({})",
            dialog.component, dialog.component_type
        );
        let mut open = true;
        let mut accepted = false;
        let mut cancel = false;
        egui::Window::new(title)
            .open(&mut open)
            .resizable(true)
            .show(ctx, |ui| {
                if dialog.readonly {
                    ui.label("Library components are read-only.");
                }
                if let Some(message) = &dialog.error {
                    ui.colored_label(egui::Color32::RED, message);
                }
                match dialog.rows.as_ref() {
                    None => {
                        ui.spinner();
                        ui.label("Loading parameters ...");
                    }
                    Some(rows) if rows.is_empty() => {
                        ui.label("This component has no parameters.");
                    }
                    Some(rows) => {
                        egui::Grid::new("component_parameters")
                            .striped(true)
                            .show(ui, |ui| {
                                ui.strong("Name");
                                ui.strong("Value");
                                ui.strong("Unit");
                                ui.strong("Description");
                                ui.end_row();
                                for (i, row) in rows.iter().enumerate() {
                                    ui.label(&row.name);
                                    let value = &mut dialog.values[i];
                                    let choices: Vec<String> = if row.type_name == "Boolean" {
                                        vec!["true".into(), "false".into()]
                                    } else {
                                        row.choices.clone()
                                    };
                                    if choices.is_empty() {
                                        let response = ui.add_enabled(
                                            !dialog.readonly,
                                            egui::TextEdit::singleline(value)
                                                .desired_width(150.0)
                                                .hint_text(
                                                    row.default_value.as_deref().unwrap_or(""),
                                                ),
                                        );
                                        if response.changed() {
                                            let string_parameter = row.type_name == "String";
                                            value.retain(|c| {
                                                c != '\n'
                                                    && c != '\r'
                                                    && (string_parameter || c != ';')
                                            });
                                        }
                                    } else {
                                        let selected = if value.is_empty() {
                                            row.default_value
                                                .clone()
                                                .unwrap_or_else(|| "(default)".into())
                                        } else {
                                            value.clone()
                                        };
                                        ui.add_enabled_ui(!dialog.readonly, |ui| {
                                            egui::ComboBox::from_id_salt((
                                                "parameter_value",
                                                &dialog.component,
                                                &row.name,
                                            ))
                                            .selected_text(selected)
                                            .show_ui(
                                                ui,
                                                |ui| {
                                                    for choice in &choices {
                                                        if ui
                                                            .selectable_label(
                                                                value == choice,
                                                                choice,
                                                            )
                                                            .clicked()
                                                        {
                                                            *value = choice.clone();
                                                        }
                                                    }
                                                },
                                            );
                                        });
                                    }
                                    ui.label(&row.unit);
                                    ui.label(&row.comment).on_hover_text(&row.comment);
                                    ui.end_row();
                                }
                            });
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    let ready = dialog.rows.is_some();
                    if ui
                        .add_enabled(
                            !dialog.readonly && ready && !self.busy,
                            egui::Button::new("OK"),
                        )
                        .clicked()
                    {
                        accepted = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if accepted {
            if let Some(rows) = dialog.rows.as_ref() {
                let ops = crate::instance::parameter_edits(&dialog.component, rows, &dialog.values);
                self.parameter_dialog = None;
                if !ops.is_empty() {
                    self.apply_edit(&dialog.class, ops);
                }
            }
        } else if cancel || !open {
            self.parameter_dialog = None;
        } else {
            self.parameter_dialog = Some(dialog);
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

    fn rename_dialog_window(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.rename_dialog.clone() else {
            return;
        };
        let mut open = true;
        let mut submit = false;
        let mut cancel = false;
        let mut existing = Vec::new();
        if let Some(crate::app::Loadable::Ready(d)) = self.model_diagrams.get(&dialog.class) {
            existing.extend(d.components.iter().map(|c| c.name.clone()));
        }
        egui::Window::new("Rename component")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(format!("Rename {}", dialog.old));
                let edit = ui.text_edit_singleline(&mut dialog.new_text);
                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }
                if let Some(error) = component_name_error(&dialog.new_text, &existing, &dialog.old)
                {
                    ui.colored_label(egui::Color32::RED, error);
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            component_name_error(&dialog.new_text, &existing, &dialog.old)
                                .is_none(),
                            egui::Button::new("OK"),
                        )
                        .clicked()
                    {
                        submit = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if !open || cancel {
            self.rename_dialog = None;
        } else if submit && component_name_error(&dialog.new_text, &existing, &dialog.old).is_none()
        {
            self.rename_dialog = None;
            let new_name = dialog.new_text.clone();
            self.apply_edit(
                &dialog.class,
                vec![crate::edit::EditOp::RenameComponent {
                    old: dialog.old,
                    new: new_name.clone(),
                }],
            );
            self.selected_component = Some(new_name);
        } else {
            self.rename_dialog = Some(RenameDialog {
                class: dialog.class,
                old: dialog.old,
                new_text: dialog.new_text,
            });
        }
    }
}
