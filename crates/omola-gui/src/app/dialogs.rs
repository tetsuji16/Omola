//! File dialogs: native ones on a helper thread, or the text-input fallback, and the
//! handling of their answers.

use super::*;

impl OmolaApp {
    /// Ask where to store a copy of the result file of run `run_id` (G14).
    pub fn request_save_result_dialog(&mut self, run_id: u64) {
        if self.dialog_in_flight() {
            return;
        }
        let Some(run) = self.run(run_id) else {
            return;
        };
        if Self::use_text_dialogs() {
            self.path_dialog = Some(PathDialog::SaveResult(run_id, String::new()));
            return;
        }
        let file_name = run.outcome.result_file.file_name().map_or_else(
            || "result_res.mat".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        );
        let (tx, rx) = std::sync::mpsc::channel::<DialogResult>();
        self.dialog_rx = Some(rx);
        std::thread::spawn(move || {
            let result = rfd::FileDialog::new()
                .add_filter("OpenModelica result", &["mat"])
                .set_file_name(file_name)
                .save_file();
            let _ = tx.send(DialogResult::SaveResult(run_id, result));
        });
    }

    /// True when text-input path dialogs should be used instead of native OS dialogs.
    pub fn use_text_dialogs() -> bool {
        std::env::var("OMOLA_GUI_TEXT_DIALOGS").as_deref() == Ok("1")
    }

    /// True while a file dialog (native or text fallback) is already open.
    pub(super) fn dialog_in_flight(&self) -> bool {
        self.dialog_rx.is_some() || self.path_dialog.is_some()
    }

    /// Open a native "Open file" dialog on a background thread.
    pub fn request_open_dialog(&mut self) {
        if self.dialog_in_flight() {
            return;
        }
        if Self::use_text_dialogs() {
            self.path_dialog = Some(PathDialog::Open(String::new()));
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel::<DialogResult>();
        self.dialog_rx = Some(rx);
        std::thread::spawn(move || {
            let result = rfd::FileDialog::new()
                .add_filter("Modelica", &["mo"])
                .pick_file();
            let _ = tx.send(DialogResult::Open(result));
        });
    }

    /// Open a native "Save as" dialog on a background thread.
    pub fn request_save_as_dialog(&mut self) {
        if let Some(i) = self.active_doc() {
            self.request_save_as_dialog_for(i);
        }
    }

    /// "Save as" for document `doc`, which need not be the active one (closing a tab,
    /// G42). The dialog result is applied to that document.
    pub fn request_save_as_dialog_for(&mut self, doc: usize) {
        if self.dialog_in_flight() || doc >= self.documents.len() {
            self.close_after_save = None;
            return;
        }
        self.save_as_doc = Some(doc);
        if Self::use_text_dialogs() {
            self.path_dialog = Some(PathDialog::SaveAs(String::new()));
            return;
        }
        let file_name = self.documents[doc]
            .path
            .as_ref()
            .and_then(|p| p.file_name().map(|n| n.to_owned()))
            .or_else(|| {
                self.first_class_of(doc)
                    .map(|class| std::ffi::OsString::from(format!("{class}.mo")))
            })
            .unwrap_or_else(|| std::ffi::OsString::from("Unnamed.mo"));
        let (tx, rx) = std::sync::mpsc::channel::<DialogResult>();
        self.dialog_rx = Some(rx);
        std::thread::spawn(move || {
            let result = rfd::FileDialog::new()
                .add_filter("Modelica", &["mo"])
                .set_file_name(file_name.to_string_lossy())
                .save_file();
            let _ = tx.send(DialogResult::SaveAs(result));
        });
    }

    /// Open a native "Save as" dialog for the plot CSV on a background thread.
    pub fn request_export_csv(&mut self) {
        if self.dialog_in_flight() {
            return;
        }
        if self.active_signals().is_empty() {
            self.info(
                LogTab::Simulation,
                "Nothing to export: the active diagram has no curves.".into(),
            );
            return;
        }
        if Self::use_text_dialogs() {
            self.path_dialog = Some(PathDialog::ExportCsv(String::new()));
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel::<DialogResult>();
        self.dialog_rx = Some(rx);
        std::thread::spawn(move || {
            let result = rfd::FileDialog::new()
                .add_filter("CSV", &["csv"])
                .set_file_name("plot.csv")
                .save_file();
            let _ = tx.send(DialogResult::ExportCsv(result));
        });
    }

    /// Open a native "Run script" dialog on a background thread.
    pub fn request_run_script_dialog(&mut self) {
        if self.dialog_in_flight() {
            return;
        }
        if Self::use_text_dialogs() {
            self.path_dialog = Some(PathDialog::RunScript(String::new()));
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel::<DialogResult>();
        self.dialog_rx = Some(rx);
        std::thread::spawn(move || {
            let result = rfd::FileDialog::new()
                .add_filter("OpenModelica script", &["mos"])
                .pick_file();
            let _ = tx.send(DialogResult::RunScript(result));
        });
    }

    /// Poll the native dialog channel and apply results.
    pub fn poll_dialog(&mut self) {
        let result = self.dialog_rx.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(result) = result {
            self.dialog_rx = None;
            // The native dialog already asked before replacing an existing file.
            self.apply_dialog_result(result, false);
        }
    }

    /// Act on the answer of a file dialog (native, or the text fallback with
    /// `confirm_overwrite = true` because it cannot ask "Replace file?" itself).
    pub fn apply_dialog_result(&mut self, result: DialogResult, confirm_overwrite: bool) {
        match result {
            DialogResult::Open(Some(path)) => self.open_file(path),
            DialogResult::RunScript(Some(path)) => self.run_script(&path),
            DialogResult::LoadResult(Some(path)) => self.load_result(path),
            DialogResult::SaveAs(Some(path)) => {
                self.request_write(PendingWrite::SaveAs(path), confirm_overwrite)
            }
            DialogResult::ExportCsv(Some(path)) => {
                self.request_write(PendingWrite::ExportCsv(path), confirm_overwrite)
            }
            DialogResult::SaveResult(run_id, Some(path)) => {
                self.request_write(PendingWrite::SaveResult(run_id, path), confirm_overwrite)
            }
            DialogResult::SaveAs(None) => {
                self.save_as_doc = None;
                self.close_after_save = None;
            }
            DialogResult::Open(None)
            | DialogResult::RunScript(None)
            | DialogResult::LoadResult(None)
            | DialogResult::ExportCsv(None)
            | DialogResult::SaveResult(_, None) => {}
        }
    }
}
