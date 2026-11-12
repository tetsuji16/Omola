//! Opening, saving and closing documents; persisted window state.

use super::*;

impl OmolaApp {
    /// Create an empty model `Unnamed`, `Unnamed2`, ... in its own (unsaved) document.
    pub fn new_model(&mut self) {
        let name = self.add_unnamed();
        self.open_in_new_tab(&name);
        self.tab = RibbonTab::Graphics;
    }

    /// Add an empty `Unnamed` model without opening it (start-up state).
    pub fn add_unnamed(&mut self) -> String {
        let name = (1..)
            .map(|i| {
                if i == 1 {
                    "Unnamed".to_owned()
                } else {
                    format!("Unnamed{i}")
                }
            })
            .find(|n| !self.user_classes.iter().any(|c| &c.qualified_name == n))
            .unwrap_or_else(|| "Unnamed".into());
        self.documents.push(Document::new_untitled(format!(
            "model {name}\nend {name};\n"
        )));
        self.refresh_outline();
        name
    }

    pub fn open_file(&mut self, path: PathBuf) {
        if let Some(i) = self
            .documents
            .iter()
            .position(|d| d.path.as_deref() == Some(path.as_path()))
        {
            let first = self.first_class_of(i);
            if let Some(class) = first {
                self.open_in_new_tab(&class);
            }
            return;
        }
        match Document::open(path.clone()) {
            Ok(doc) => {
                self.documents.push(doc);
                self.refresh_outline();
                self.info(LogTab::Syntax, format!("Opened {}", path.display()));
                push_recent(&mut self.recent_files, path);
                if let Some(class) = self.first_class_of(self.documents.len() - 1) {
                    self.open_in_new_tab(&class);
                }
            }
            Err(e) => self.error(
                LogTab::Syntax,
                format!("Could not open {}: {e}", path.display()),
            ),
        }
    }

    pub(super) fn first_class_of(&self, doc: usize) -> Option<String> {
        let classes: Vec<UserClass> = self
            .user_classes
            .iter()
            .zip(&self.user_class_doc)
            .filter(|(_, d)| **d == doc)
            .map(|(c, _)| c.clone())
            .collect();
        default_class(&classes)
    }

    /// Save the document of the active class (asks for a path if it was never saved).
    pub fn save(&mut self) {
        if let Some(i) = self.active_doc() {
            self.save_document(i, false);
        }
    }

    /// Save document `i`. Returns `true` when the file was written by this call.
    ///
    /// `false` means the save waits for an answer (the "Save as" dialog, or the "changed
    /// on disk" question of G43 unless `force`) or failed (the reason is logged).
    pub fn save_document(&mut self, i: usize, force: bool) -> bool {
        let Some(doc) = self.documents.get(i) else {
            return false;
        };
        if doc.path.is_none() {
            self.request_save_as_dialog_for(i);
            return false;
        }
        if !force && doc.changed_on_disk() {
            self.save_conflict = Some(i);
            return false;
        }
        match self.documents[i].save() {
            Ok(()) => {
                let title = self.documents[i].title();
                if let Some(path) = self.documents[i].path.clone() {
                    push_recent(&mut self.recent_files, path);
                }
                self.info(LogTab::Syntax, format!("Saved {title}"));
                if self.close_after_save == Some(i) {
                    self.close_after_save = None;
                    self.close_document(i);
                }
                true
            }
            Err(e) => {
                self.close_after_save = None;
                self.error(LogTab::Syntax, format!("Save failed: {e}"));
                false
            }
        }
    }

    /// Whether "Save All" has anything to write.
    pub fn can_save_all(&self) -> bool {
        self.documents.iter().any(|d| d.dirty)
    }

    /// Save every modified document that can be written without a question.
    ///
    /// Returns one line per document that was *not* saved: it has no file name yet, it was
    /// changed on disk by another program (G43), or writing failed. The lines are logged.
    pub fn save_all(&mut self) -> Vec<String> {
        let mut problems = Vec::new();
        for i in 0..self.documents.len() {
            if !self.documents[i].dirty {
                continue;
            }
            let title = self.documents[i].title().trim_end_matches('*').to_owned();
            if self.documents[i].path.is_none() {
                problems.push(format!("{title} has no file name yet; use Save As."));
            } else if self.documents[i].changed_on_disk() {
                problems.push(format!(
                    "{title} was changed on disk; save it on its own to decide."
                ));
            } else {
                match self.documents[i].save() {
                    Ok(()) => {
                        if let Some(path) = self.documents[i].path.clone() {
                            push_recent(&mut self.recent_files, path);
                        }
                        self.info(LogTab::Syntax, format!("Saved {title}"));
                    }
                    Err(e) => problems.push(format!("Save of {title} failed: {e}")),
                }
            }
        }
        for problem in &problems {
            self.error(LogTab::Syntax, problem.clone());
        }
        problems
    }

    /// "Save all and exit" of the exit dialog: `true` when nothing is left unsaved, so the
    /// window may close. Otherwise the dialog closes and the log shows what went wrong.
    pub fn save_all_and_exit(&mut self) -> bool {
        self.confirm_exit = false;
        if self.pending_edit.is_some() {
            // The edit changes the buffer when it returns: saving now would lose it.
            self.error(
                LogTab::Syntax,
                "A model edit is still running; try again when it has finished.".into(),
            );
        } else if self.save_all().is_empty() {
            self.exit_confirmed = true;
            return true;
        }
        self.log_tab = LogTab::Syntax;
        self.bottom_pane = BottomPane::Logs;
        self.docks.bottom = true;
        false
    }

    /// Qualified names of the classes defined in document `doc`.
    pub fn classes_of_document(&self, doc: usize) -> Vec<String> {
        self.user_classes
            .iter()
            .zip(&self.user_class_doc)
            .filter(|(_, d)| **d == doc)
            .map(|(c, _)| c.qualified_name.clone())
            .collect()
    }

    /// Close document `doc`, asking first when it has unsaved changes (G42).
    pub fn request_close_document(&mut self, doc: usize) {
        match self.documents.get(doc) {
            Some(d) if d.dirty => self.pending_close = Some(doc),
            Some(_) => self.close_document(doc),
            None => {}
        }
    }

    /// File > Close: close the document of the active user model, or the tab of a
    /// library class.
    pub fn close_active(&mut self) {
        let Some(class) = self.active.clone() else {
            return;
        };
        match self.doc_of(&class) {
            Some(doc) if self.is_user_class(&class) => self.request_close_document(doc),
            _ => self.close_tab(&class),
        }
    }

    /// Answer "Save" in the "Save changes?" dialog: the document closes once it is saved.
    pub fn confirm_close_save(&mut self) {
        if let Some(doc) = self.pending_close.take() {
            self.close_after_save = Some(doc);
            self.save_document(doc, false);
        }
    }

    /// Answer "Discard" in the "Save changes?" dialog.
    pub fn confirm_close_discard(&mut self) {
        if let Some(doc) = self.pending_close.take() {
            self.close_document(doc);
        }
    }

    /// Remove document `doc` with its tabs and cached views. Unsaved changes are lost:
    /// callers that must ask first use [`Self::request_close_document`].
    pub fn close_document(&mut self, doc: usize) {
        if doc >= self.documents.len() {
            return;
        }
        let classes = self.classes_of_document(doc);
        if self
            .pending_edit
            .as_ref()
            .is_some_and(|c| classes.contains(c))
        {
            return self.error(LogTab::Syntax, "A model edit is still running.".into());
        }
        let title = self.documents[doc].title().trim_end_matches('*').to_owned();
        let active_closed = self
            .active
            .as_ref()
            .is_some_and(|a| classes.iter().any(|c| c == a));
        for class in &classes {
            self.model_tabs.retain(|t| t != class);
            self.docs.remove(class);
            self.icons.remove(class);
            self.model_diagrams.remove(class);
            self.texts.remove(class);
        }
        if self
            .components
            .as_ref()
            .is_some_and(|(name, _)| classes.contains(name))
        {
            self.components = None;
        }
        self.history.retain(|c| !classes.contains(c));
        self.history_pos = self.history_pos.min(self.history.len());
        self.documents.remove(doc);
        // Document indices changed: questions about a document are no longer valid.
        self.pending_close = None;
        self.close_after_save = None;
        if self.save_as_doc.take().is_some() {
            if matches!(self.path_dialog, Some(PathDialog::SaveAs(_))) {
                self.path_dialog = None;
            }
            if matches!(self.overwrite_confirm, Some(PendingWrite::SaveAs(_))) {
                self.overwrite_confirm = None;
            }
            // A native dialog cannot be closed from here: its answer is ignored.
            self.stale_save_as = self.dialog_rx.is_some();
        }
        self.save_conflict = None;
        self.goto_line_dialog = None;
        self.replace_dialog = None;
        self.refresh_outline();
        if active_closed {
            self.active = None;
            self.selected_component = None;
            if let Some(next) = self.model_tabs.last().cloned() {
                self.open_class(&next);
            }
        }
        self.info(LogTab::Syntax, format!("Closed {title}"));
    }

    /// Answer of the "File changed on disk" dialog (G43).
    pub fn resolve_save_conflict(&mut self, answer: ConflictAnswer) {
        let Some(doc) = self.save_conflict.take() else {
            return;
        };
        if doc >= self.documents.len() {
            return;
        }
        match answer {
            ConflictAnswer::Overwrite => {
                self.save_document(doc, true);
            }
            ConflictAnswer::Reload => {
                self.close_after_save = None;
                let title = self.documents[doc].title().trim_end_matches('*').to_owned();
                match self.documents[doc].reload() {
                    Ok(()) => {
                        self.user_text_changed(doc);
                        self.documents[doc].dirty = false;
                        self.info(LogTab::Syntax, format!("Reloaded {title} from disk"));
                    }
                    Err(e) => self.error(LogTab::Syntax, format!("Reload failed: {e}")),
                }
            }
            ConflictAnswer::Cancel => self.close_after_save = None,
        }
    }

    /// Write `write`, asking first when `confirm` is set and the target file exists.
    ///
    /// Native save dialogs ask the question themselves, so they pass `confirm = false`.
    pub fn request_write(&mut self, write: PendingWrite, confirm: bool) {
        let own_file = match &write {
            PendingWrite::SaveAs(path) => self
                .save_as_doc
                .or_else(|| self.active_doc())
                .and_then(|i| self.documents.get(i))
                .is_some_and(|d| d.path.as_deref().is_some_and(|own| same_file(own, path))),
            _ => false,
        };
        if confirm && !own_file && write.path().exists() {
            self.overwrite_confirm = Some(write);
        } else {
            self.perform_write(write);
        }
    }

    /// Answer of the "Replace file?" dialog (G43).
    pub fn resolve_overwrite(&mut self, replace: bool) {
        let Some(write) = self.overwrite_confirm.take() else {
            return;
        };
        if replace {
            self.perform_write(write);
        } else if matches!(write, PendingWrite::SaveAs(_)) {
            self.save_as_doc = None;
            self.close_after_save = None;
        }
    }

    fn perform_write(&mut self, write: PendingWrite) {
        match write {
            PendingWrite::SaveAs(path) => self.save_as(path),
            PendingWrite::ExportCsv(path) => self.export_csv(&path),
            PendingWrite::SaveResult(run_id, path) => self.save_result_as(run_id, &path),
        }
    }

    /// Give the document of the open "Save as" dialog the file `path` and write it.
    pub fn save_as(&mut self, path: PathBuf) {
        let Some(i) = self.save_as_doc.take().or_else(|| self.active_doc()) else {
            return;
        };
        if i >= self.documents.len() {
            return;
        }
        let open_elsewhere = self
            .documents
            .iter()
            .enumerate()
            .any(|(j, d)| j != i && d.path.as_deref() == Some(path.as_path()));
        if open_elsewhere {
            self.close_after_save = None;
            return self.error(
                LogTab::Syntax,
                format!(
                    "{} is open in another document; close that one first.",
                    path.display()
                ),
            );
        }
        if self.documents[i]
            .path
            .as_deref()
            .is_some_and(|own| same_file(own, &path))
        {
            // The document's own file: an ordinary save, with the changed-on-disk check.
            self.save_document(i, false);
            return;
        }
        let previous = (self.documents[i].path.clone(), self.documents[i].disk_stamp);
        self.documents[i].path = Some(path);
        // The user chose this file (and confirmed replacing it): no conflict question.
        if !self.save_document(i, true) {
            // A successful save may have closed the document; a failed one never does.
            if let Some(doc) = self.documents.get_mut(i) {
                doc.path = previous.0;
                doc.disk_stamp = previous.1;
            }
        }
    }

    /// Copy the result file of run `run_id` to `dest` (G14).
    pub fn save_result_as(&mut self, run_id: u64, dest: &std::path::Path) {
        let Some(source) = self.run(run_id).map(|r| r.outcome.result_file.clone()) else {
            return self.error(LogTab::Simulation, "The result is no longer open.".into());
        };
        match crate::backend::copy_result_file(&source, dest, true) {
            Ok(()) => self.info(
                LogTab::Simulation,
                format!("Saved the result to {}", dest.display()),
            ),
            Err(message) => self.error(
                LogTab::Simulation,
                format!("Could not save the result: {message}"),
            ),
        }
    }

    pub fn refresh_outline(&mut self) {
        self.package_search_cache = None;
        self.user_classes.clear();
        self.user_class_doc.clear();
        for (i, doc) in self.documents.iter().enumerate() {
            for class in scan_classes(&doc.text) {
                self.user_classes.push(class);
                self.user_class_doc.push(i);
            }
        }
    }

    /// Text of document `doc` was edited in the Text layer.
    pub fn user_text_changed(&mut self, doc: usize) {
        if let Some(d) = self.documents.get_mut(doc) {
            d.dirty = true;
        }
        let before: Vec<String> = self
            .user_classes
            .iter()
            .map(|c| c.qualified_name.clone())
            .collect();
        self.refresh_outline();
        // User-defined icons can change with the source, including nested component types.
        // Invalidate both old and current names so Diagram and Icon reload fresh graphics.
        for name in before {
            self.docs.remove(&name);
            self.icons.remove(&name);
        }
        for class in &self.user_classes {
            self.icons.remove(&class.qualified_name);
        }
        if let Some(active) = self.active.clone() {
            if !self.is_user_class(&active) && self.restriction_of(&active).is_none() {
                // The class was renamed in the editor: follow the document's first class.
                if let Some(first) = self.first_class_of(doc) {
                    self.model_tabs.retain(|t| t != &active);
                    self.open_in_new_tab(&first);
                }
            }
        }
    }

    /// Whether a request to close the window is held back because a model edit is still
    /// running: closing now would drop the edit without a word. Only the first request
    /// is held back; closing again exits anyway (G53).
    pub fn hold_close_for_edit(&mut self) -> bool {
        if self.pending_edit.is_none() || self.exit_confirmed || self.edit_close_warned {
            return false;
        }
        self.edit_close_warned = true;
        self.warn(
            LogTab::Syntax,
            "A model edit is still running; close again to exit without it.".into(),
        );
        true
    }

    /// Whether a request to close the window must be held back for confirmation (G42).
    pub fn must_confirm_exit(&self) -> bool {
        !self.exit_confirmed && !self.unsaved_documents().is_empty()
    }

    /// Titles of the documents with unsaved changes, in tab order (G42).
    ///
    /// Empty means closing the window or the application loses nothing.
    pub fn unsaved_documents(&self) -> Vec<String> {
        self.documents
            .iter()
            .filter(|d| d.dirty)
            .map(|d| d.title())
            .collect()
    }

    /// Build a serializable snapshot of the state that should survive a restart.
    pub fn to_persisted(&self) -> PersistedState {
        let last_file = self
            .active_doc()
            .and_then(|i| self.documents[i].path.clone());
        PersistedState {
            last_file,
            tab: Some(self.tab.label().to_owned()),
            setup: Some(self.setup.clone()),
            keep_results: Some(self.keep_results),
            recent_files: self.recent_files.clone(),
        }
    }

    /// Apply persisted state. Priority: **CLI file > saved last_file > default sample**.
    pub fn apply_persisted(&mut self, state: PersistedState, cli_file: Option<PathBuf>) {
        if let Some(setup) = state.setup {
            self.setup = setup;
        }
        if let Some(keep) = state.keep_results {
            self.keep_results = keep;
        }
        if let Some(tab) = state.tab.and_then(|s| RibbonTab::from_label(&s)) {
            self.tab = tab;
        }
        // Restore recent files, keeping only those that still exist.
        self.recent_files = state
            .recent_files
            .into_iter()
            .filter(|p| p.is_file())
            .collect();

        let file_to_open = cli_file.or_else(|| {
            state.last_file.as_ref().and_then(|path| {
                if path.is_file() {
                    Some(path.clone())
                } else {
                    self.info(
                        LogTab::Syntax,
                        format!("Previous file not found: {}", path.display()),
                    );
                    None
                }
            })
        });

        match file_to_open {
            Some(path) => self.open_file(path),
            None => self.open_class("Modelica"),
        }
    }

    /// Run an OpenModelica script through the Commands pane.
    pub fn run_script(&mut self, path: &std::path::Path) {
        self.command_input = format!(
            "runScript(\"{}\")",
            omola_core::escape_modelica_string(&path.to_string_lossy())
        );
        self.bottom_pane = BottomPane::Commands;
        self.docks.bottom = true;
        self.run_command();
    }

    /// Write the curves of the active diagram to `path` as Omola CSV.
    pub fn export_csv(&mut self, path: &std::path::Path) {
        let Some((time, columns)) = self.diagram_columns() else {
            self.info(
                LogTab::Simulation,
                "Nothing to export: the active diagram has no curves.".into(),
            );
            return;
        };
        let csv_columns: Vec<(&str, &[f64])> = columns
            .iter()
            .map(|(name, values)| (name.as_str(), values.as_slice()))
            .collect();
        let csv = crate::plot::plotted_csv(&time, &csv_columns);
        let names: Vec<&str> = csv_columns.iter().map(|(name, _)| *name).collect();
        match std::fs::write(path, csv) {
            Ok(()) => self.info(
                LogTab::Simulation,
                format!(
                    "Wrote {} curve(s) ({}) to {}",
                    names.len(),
                    names.join(", "),
                    path.display()
                ),
            ),
            Err(error) => self.error(
                LogTab::Simulation,
                format!("Could not write {}: {error}", path.display()),
            ),
        }
    }

    /// Time axis and curves of the active diagram, limited to one result.
    ///
    /// A diagram can mix results; the CSV then only carries the newest one, because a
    /// single `time` column cannot describe two different runs.
    pub(super) fn diagram_columns(&mut self) -> Option<DiagramColumns> {
        let signals = self.active_signals().to_vec();
        let newest = signals.iter().map(|s| s.run_id).max()?;
        if signals.iter().any(|s| s.run_id != newest) {
            self.warn(
                LogTab::Simulation,
                "The active diagram mixes results; the CSV gets the newest one only.".into(),
            );
        }
        let run = self.run(newest)?;
        let time = run.outcome.result.time.clone();
        let columns: Vec<(String, Vec<f64>)> = signals
            .iter()
            .filter(|s| s.run_id == newest)
            .filter_map(|s| {
                let values = run.outcome.result.series(&s.variable)?;
                Some((s.variable.clone(), values.to_vec()))
            })
            .collect();
        if columns.is_empty() {
            return None;
        }
        Some((time, columns))
    }
}

/// Whether two paths name the same file, also when they are spelled differently
/// (relative and absolute, another case on Windows).
fn same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    a == b
        || matches!(
            (std::fs::canonicalize(a), std::fs::canonicalize(b)),
            (Ok(a), Ok(b)) if a == b
        )
}
