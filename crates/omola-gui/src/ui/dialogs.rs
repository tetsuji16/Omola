//! Modal-ish windows: Simulation Setup and the temporary path dialogs.

use crate::app::{
    component_name_error, ConflictAnswer, DialogResult, OmolaApp, PathDialog, RenameDialog,
};
use std::path::PathBuf;

impl OmolaApp {
    pub fn dialogs(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.setup_dialog(&ctx);
        self.path_dialog_window(&ctx);
        self.rename_dialog_window(&ctx);
        self.parameter_dialog_window(&ctx);
        self.exit_dialog_window(&ctx);
        self.close_dialog_window(&ctx);
        self.overwrite_dialog_window(&ctx);
        self.save_conflict_window(&ctx);
        self.sweep_dialog_window(&ctx);
        self.goto_line_window(&ctx);
        self.replace_window(&ctx);
    }

    /// "Save changes?" before a single document is closed (G42).
    fn close_dialog_window(&mut self, ctx: &egui::Context) {
        let Some(doc) = self.pending_close else {
            return;
        };
        let Some(title) = self.documents.get(doc).map(|d| d.title()) else {
            self.pending_close = None;
            return;
        };
        let title = title.trim_end_matches('*').to_owned();
        egui::Window::new("Save changes?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("{title} has unsaved changes."));
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        self.confirm_close_save();
                    }
                    if ui.button("Discard").clicked() {
                        self.confirm_close_discard();
                    }
                    if ui.button("Cancel").clicked() {
                        self.pending_close = None;
                    }
                });
            });
    }

    /// "Replace file?" for the text-input path dialogs, which cannot ask themselves (G43).
    fn overwrite_dialog_window(&mut self, ctx: &egui::Context) {
        let Some(path) = self
            .overwrite_confirm
            .as_ref()
            .map(|w| w.path().display().to_string())
        else {
            return;
        };
        egui::Window::new("Replace file?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("{path} already exists."));
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Replace").clicked() {
                        self.resolve_overwrite(true);
                    }
                    if ui.button("Cancel").clicked() {
                        self.resolve_overwrite(false);
                    }
                });
            });
    }

    /// "File changed on disk": another program wrote the file since Omola read it (G43).
    fn save_conflict_window(&mut self, ctx: &egui::Context) {
        let Some(doc) = self.save_conflict else {
            return;
        };
        let Some(path) = self.documents.get(doc).and_then(|d| d.path.clone()) else {
            self.save_conflict = None;
            return;
        };
        egui::Window::new("File changed on disk")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!(
                    "{} was changed by another program since Omola read it.",
                    path.display()
                ));
                ui.label("Overwrite keeps your text. Reload from disk drops your changes.");
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Overwrite").clicked() {
                        self.resolve_save_conflict(ConflictAnswer::Overwrite);
                    }
                    if ui.button("Reload from disk").clicked() {
                        self.resolve_save_conflict(ConflictAnswer::Reload);
                    }
                    if ui.button("Cancel").clicked() {
                        self.resolve_save_conflict(ConflictAnswer::Cancel);
                    }
                });
            });
    }

    /// "Sweep Parameters": one parameter, evenly spaced values, one kept run each (G49).
    fn sweep_dialog_window(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.sweep_dialog.clone() else {
            return;
        };
        let mut open = true;
        let mut start = false;
        let mut cancel = false;
        egui::Window::new("Sweep Parameters")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Model: {}",
                    self.active.clone().unwrap_or_else(|| "-".into())
                ));
                egui::Grid::new("sweep_fields")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Parameter");
                        ui.add(
                            egui::TextEdit::singleline(&mut dialog.parameter)
                                .desired_width(180.0)
                                .hint_text("for example e"),
                        );
                        ui.end_row();
                        ui.label("From");
                        ui.add(egui::TextEdit::singleline(&mut dialog.from).desired_width(90.0));
                        ui.end_row();
                        ui.label("To");
                        ui.add(egui::TextEdit::singleline(&mut dialog.to).desired_width(90.0));
                        ui.end_row();
                        ui.label("Points");
                        ui.add(egui::TextEdit::singleline(&mut dialog.points).desired_width(90.0));
                        ui.end_row();
                    });
                ui.label(format!(
                    "2 to {} runs. Each run is kept until you close it.",
                    crate::backend::MAX_SWEEP_POINTS
                ));
                if let Some(message) = &dialog.error {
                    ui.colored_label(egui::Color32::RED, message);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!self.busy, egui::Button::new("Start"))
                        .clicked()
                    {
                        start = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if cancel || !open {
            self.sweep_dialog = None;
        } else if start {
            match self.start_sweep(&dialog.parameter, &dialog.from, &dialog.to, &dialog.points) {
                Ok(()) => self.sweep_dialog = None,
                Err(message) => {
                    dialog.error = Some(message);
                    self.sweep_dialog = Some(dialog);
                }
            }
        } else {
            self.sweep_dialog = Some(dialog);
        }
    }

    /// "Go To Line" of the Text layer (G45).
    fn goto_line_window(&mut self, ctx: &egui::Context) {
        let Some(mut text) = self.goto_line_dialog.clone() else {
            return;
        };
        let mut open = true;
        let mut go = false;
        let mut error = None;
        egui::Window::new("Go To Line")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Line");
                    let edit = ui.add(egui::TextEdit::singleline(&mut text).desired_width(80.0));
                    if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        go = true;
                    }
                    if ui.button("Go").clicked() {
                        go = true;
                    }
                });
                if go {
                    error = self.goto_line(&text).err();
                }
                if let Some(message) = &error {
                    ui.colored_label(egui::Color32::RED, message);
                }
            });
        if !open || (go && error.is_none()) {
            self.goto_line_dialog = None;
        } else {
            self.goto_line_dialog = Some(text);
        }
    }

    /// "Replace" of the Text layer: replaces every occurrence in one undo step (G45).
    fn replace_window(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.replace_dialog.clone() else {
            return;
        };
        let mut open = true;
        let mut replace = false;
        egui::Window::new("Replace")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                egui::Grid::new("replace_fields")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Find");
                        ui.add(egui::TextEdit::singleline(&mut dialog.find).desired_width(220.0));
                        ui.end_row();
                        ui.label("Replace with");
                        ui.add(
                            egui::TextEdit::singleline(&mut dialog.replace).desired_width(220.0),
                        );
                        ui.end_row();
                    });
                ui.label("Case-sensitive. Undo restores the text.");
                if let Some(message) = &dialog.error {
                    ui.label(message);
                }
                if ui.button("Replace All").clicked() {
                    replace = true;
                }
            });
        if !open {
            self.replace_dialog = None;
            return;
        }
        if replace {
            dialog.error = Some(
                match self.replace_in_active(&dialog.find, &dialog.replace) {
                    Ok(count) => format!("Replaced {count} occurrence(s)."),
                    Err(message) => message,
                },
            );
        }
        // `replace_in_active` never closes a document, so the dialog stays valid.
        self.replace_dialog = Some(dialog);
    }

    /// "Unsaved changes" confirmation shown when the window is asked to close (G42).
    fn exit_dialog_window(&mut self, ctx: &egui::Context) {
        if !self.confirm_exit {
            return;
        }
        let unsaved = self.unsaved_documents();
        if unsaved.is_empty() {
            self.confirm_exit = false;
            return;
        }
        egui::Window::new("Unsaved changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("These documents have unsaved changes:");
                for title in &unsaved {
                    ui.label(format!("    {title}"));
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Save all and exit").clicked() && self.save_all_and_exit() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button("Discard and exit").clicked() {
                        self.exit_confirmed = true;
                        self.confirm_exit = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button("Cancel").clicked() {
                        self.confirm_exit = false;
                    }
                });
            });
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
        let keep_before = self.keep_results;
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
                        // Another view of the same value: nothing extra is stored.
                        ui.label("Interval length");
                        let (start, stop) = (self.setup.start_time, self.setup.stop_time);
                        let mut length =
                            (stop - start) / f64::from(self.setup.number_of_intervals.max(1));
                        if ui
                            .add_enabled(
                                stop > start,
                                egui::DragValue::new(&mut length)
                                    .speed(0.001)
                                    .range(0.0..=f64::MAX)
                                    .suffix(" s"),
                            )
                            .changed()
                        {
                            self.setup.number_of_intervals =
                                crate::backend::intervals_for_length(start, stop, length);
                        }
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
                ui.strong("Output");
                ui.horizontal(|ui| {
                    ui.label("Number of results to keep");
                    ui.add(egui::DragValue::new(&mut self.keep_results).range(1..=20))
                        .on_hover_text("Older results are closed; kept results stay");
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
                let problem = self.setup.validate().err();
                if let Some(message) = &problem {
                    ui.colored_label(egui::Color32::RED, message);
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !self.busy && problem.is_none(),
                            egui::Button::new("Simulate"),
                        )
                        .clicked()
                    {
                        self.setup
                            .parameters
                            .retain(|(n, v)| !n.trim().is_empty() && !v.trim().is_empty());
                        self.simulate();
                        self.show_setup = false;
                    }
                    if ui
                        .add_enabled(self.can_store_setup(), egui::Button::new("Store in model"))
                        .on_hover_text("Write these values into the experiment annotation")
                        .on_disabled_hover_text("Open a user model to store the setup")
                        .clicked()
                    {
                        self.save_setup_in_model();
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
        if self.keep_results < keep_before {
            self.evict_results();
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
            PathDialog::LoadResult(t) => {
                ("Load Result", "Path to a result file (_res.mat)", t.clone())
            }
            PathDialog::SaveResult(_, t) => {
                ("Save result as", "Path to a result file (.mat)", t.clone())
            }
        };
        let mut keep_open = true;
        let mut submitted = false;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .open(&mut keep_open)
            .show(ctx, |ui| {
                ui.label(format!("{prompt} (text input fallback):"));
                let response = ui.add(egui::TextEdit::singleline(&mut text).desired_width(420.0));
                let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("OK").clicked() || enter {
                    submitted = true;
                }
            });
        if submitted && !text.trim().is_empty() {
            self.path_dialog = None;
            let path = Some(PathBuf::from(text.trim()));
            let result = match dialog {
                PathDialog::Open(_) => DialogResult::Open(path),
                PathDialog::SaveAs(_) => DialogResult::SaveAs(path),
                PathDialog::RunScript(_) => DialogResult::RunScript(path),
                PathDialog::ExportCsv(_) => DialogResult::ExportCsv(path),
                PathDialog::LoadResult(_) => DialogResult::LoadResult(path),
                PathDialog::SaveResult(run_id, _) => DialogResult::SaveResult(run_id, path),
            };
            // This dialog has no "Replace file?" question of its own.
            self.apply_dialog_result(result, true);
        } else if !keep_open {
            self.path_dialog = None;
            if matches!(dialog, PathDialog::SaveAs(_)) {
                self.apply_dialog_result(DialogResult::SaveAs(None), true);
            }
        } else {
            self.path_dialog = Some(match dialog {
                PathDialog::Open(_) => PathDialog::Open(text),
                PathDialog::SaveAs(_) => PathDialog::SaveAs(text),
                PathDialog::RunScript(_) => PathDialog::RunScript(text),
                PathDialog::ExportCsv(_) => PathDialog::ExportCsv(text),
                PathDialog::LoadResult(_) => PathDialog::LoadResult(text),
                PathDialog::SaveResult(run_id, _) => PathDialog::SaveResult(run_id, text),
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
