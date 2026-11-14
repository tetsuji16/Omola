//! Top-level application state and the per-frame layout.
//!
//! The window follows the classic Modelica-workbench arrangement documented in
//! `docs/reference/dymola-gui.md`: quick access toolbar and title, ribbon tabs with
//! grouped commands, docked browsers on the left, model tabs and the edit window in the
//! centre, the command/log window at the bottom, and a browser bar with the layer toolbar.

use crate::backend::{
    ClassContext, Event, Job, LogLevel, LogTab, SimulationOutcome, SimulationSetup, Worker,
};
use crate::diagram::Placement;
use crate::edit::EditOp;
use crate::html::{self, Block};
use crate::library::{self, ClassDoc, LibrarySpec, LoadedLibrary};
use crate::model::{default_class, scan_classes, ClassEntry as UserClass, Document};
use crate::text_editor::{location_is_for, parse_omc_locations, SourceLocation};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

mod classes;
mod dialogs;
mod editing;
mod files;
mod logs;
mod navigation;
mod results;
mod simulation;
mod sweep;
mod text_tools;

pub use sweep::{SweepDialog, SweepPlan};
pub use text_tools::ReplaceDialog;

/// Ribbon tabs (File is a menu button left of the tabs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RibbonTab {
    Graphics,
    Documentation,
    Text,
    Simulation,
    Tools,
}

impl RibbonTab {
    pub const ALL: [RibbonTab; 5] = [
        RibbonTab::Graphics,
        RibbonTab::Documentation,
        RibbonTab::Text,
        RibbonTab::Simulation,
        RibbonTab::Tools,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RibbonTab::Graphics => "Graphics",
            RibbonTab::Documentation => "Documentation",
            RibbonTab::Text => "Text",
            RibbonTab::Simulation => "Simulation",
            RibbonTab::Tools => "Tools",
        }
    }

    pub fn from_label(s: &str) -> Option<RibbonTab> {
        Self::ALL.iter().copied().find(|t| t.label() == s)
    }
}

/// Model layers shown in the edit window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Icon,
    Diagram,
    Documentation,
    Text,
}

impl Layer {
    pub fn label(self) -> &'static str {
        match self {
            Layer::Icon => "Icon",
            Layer::Diagram => "Diagram",
            Layer::Documentation => "Documentation",
            Layer::Text => "Modelica Text",
        }
    }
}

/// What the Documentation layer shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocView {
    Formatted,
    InfoSource,
    RevisionSource,
}

/// Bottom window: command line or logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BottomPane {
    Commands,
    Logs,
}

/// Visibility of docked browsers and windows (toggled from the bottom bar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Docks {
    pub package_browser: bool,
    pub component_browser: bool,
    pub variable_browser: bool,
    pub bottom: bool,
}

impl Default for Docks {
    fn default() -> Self {
        Self {
            package_browser: true,
            component_browser: true,
            variable_browser: true,
            bottom: true,
        }
    }
}

pub enum LibraryState {
    Loading(LibrarySpec),
    Loaded(LoadedLibrary),
    Failed(LibrarySpec, String),
}

impl LibraryState {
    pub fn spec(&self) -> &LibrarySpec {
        match self {
            LibraryState::Loading(spec) | LibraryState::Failed(spec, _) => spec,
            LibraryState::Loaded(lib) => &lib.spec,
        }
    }
}

pub enum Loadable<T> {
    Loading,
    Ready(T),
    Failed(String),
}

/// Parsed documentation, cached per class.
pub struct DocPage {
    pub doc: ClassDoc,
    pub info: Vec<Block>,
}

/// One kept simulation result, shown as a top-level node in the variable browser.
pub struct ResultRun {
    pub id: u64,
    pub label: String,
    pub outcome: SimulationOutcome,
    /// Pinned by "Keep result": never evicted by the `keep_results` limit (G14).
    pub kept: bool,
}

/// Cached Variable Browser model for a result and its current filters.
pub struct VarTreeCache {
    pub key: VarTreeKey,
    pub tree: crate::var_tree::VarNode,
    /// Matching variable indices when the text filter is active.
    pub flat: Vec<usize>,
    pub(crate) generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarTreeKey {
    pub run_id: u64,
    pub filter: String,
    pub show_parameters: bool,
    pub show_time_varying: bool,
}

/// A curve in the plot window: (run id, variable name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlotSignal {
    pub run_id: u64,
    pub variable: String,
}

/// Time axis plus named value columns of a diagram, shaped for `plot::plotted_csv`.
pub type DiagramColumns = (Vec<f64>, Vec<(String, Vec<f64>)>);

/// Small text-input dialogs (fallback when `OMOLA_GUI_TEXT_DIALOGS=1` or no native portal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathDialog {
    Open(String),
    SaveAs(String),
    RunScript(String),
    ExportCsv(String),
    LoadResult(String),
    /// Store a copy of the result file of this run (G14).
    SaveResult(u64, String),
}

/// A write to a file chosen by the user. It waits in `overwrite_confirm` while the
/// "Replace file?" question is open (G43).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingWrite {
    SaveAs(PathBuf),
    ExportCsv(PathBuf),
    SaveResult(u64, PathBuf),
}

impl PendingWrite {
    pub fn path(&self) -> &std::path::Path {
        match self {
            Self::SaveAs(path) | Self::ExportCsv(path) | Self::SaveResult(_, path) => path,
        }
    }
}

/// Answers of the "File changed on disk" dialog (G43).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictAnswer {
    Overwrite,
    Reload,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameDialog {
    pub class: String,
    pub old: String,
    pub new_text: String,
}

pub fn component_name_error(new: &str, existing: &[String], old: &str) -> Option<&'static str> {
    const KEYWORDS: &[&str] = &[
        "algorithm",
        "and",
        "annotation",
        "block",
        "break",
        "class",
        "connect",
        "connector",
        "constant",
        "der",
        "discrete",
        "each",
        "else",
        "elseif",
        "elsewhen",
        "encapsulated",
        "end",
        "enumeration",
        "equation",
        "expandable",
        "extends",
        "external",
        "false",
        "final",
        "flow",
        "for",
        "function",
        "if",
        "import",
        "impure",
        "in",
        "initial",
        "inner",
        "input",
        "loop",
        "model",
        "not",
        "operator",
        "or",
        "outer",
        "output",
        "package",
        "parameter",
        "partial",
        "protected",
        "public",
        "pure",
        "record",
        "redeclare",
        "replaceable",
        "return",
        "stream",
        "then",
        "true",
        "type",
        "when",
        "while",
        "within",
    ];
    let mut chars = new.chars();
    if !matches!(chars.next(),Some(c) if c=='_'||c.is_ascii_alphabetic())
        || !chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
    {
        return Some("Not a valid Modelica identifier");
    }
    if KEYWORDS.contains(&new) {
        return Some("Not a valid Modelica identifier");
    }
    if new != old && existing.iter().any(|n| n == new) {
        return Some("Name already used");
    }
    None
}

/// Async result from a native file dialog (picked on a background thread).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogResult {
    Open(Option<PathBuf>),
    SaveAs(Option<PathBuf>),
    ExportCsv(Option<PathBuf>),
    RunScript(Option<PathBuf>),
    LoadResult(Option<PathBuf>),
    SaveResult(u64, Option<PathBuf>),
}

/// Maximum number of entries in the "Recent files" menu.
pub const MAX_RECENT_FILES: usize = 8;
pub const MAX_LOG_ENTRIES: usize = 2_000;
pub const MAX_LOG_ENTRY_CHARS: usize = 200_000;

/// Add `path` to the front of `list`, removing duplicates and trimming to `MAX_RECENT_FILES`.
pub fn push_recent(list: &mut Vec<PathBuf>, path: PathBuf) {
    list.retain(|p| p != &path);
    list.insert(0, path);
    list.truncate(MAX_RECENT_FILES);
}

pub struct LogEntry {
    pub tab: LogTab,
    pub level: LogLevel,
    pub text: String,
}

/// Serializable subset of [`OmolaApp`] persisted across restarts via eframe storage.
///
/// `OmolaApp` itself is not serialized (it contains a worker thread, channels, etc.).
#[derive(serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct PersistedState {
    pub last_file: Option<PathBuf>,
    pub tab: Option<String>,
    pub setup: Option<SimulationSetup>,
    pub keep_results: Option<usize>,
    pub recent_files: Vec<PathBuf>,
}

/// Storage key for [`PersistedState`] in eframe's `Storage`.
const STATE_KEY: &str = "omola_gui_state";

#[derive(Debug, Clone, PartialEq)]
pub struct ParameterDialog {
    pub class: String,
    pub component: String,
    pub component_type: String,
    pub rows: Option<Vec<crate::instance::ParamRow>>,
    pub values: Vec<String>,
    pub readonly: bool,
    pub error: Option<String>,
}

pub struct OmolaApp {
    pub tab: RibbonTab,
    pub graphics_layer: Layer,
    /// User files being edited (their classes appear in the package browser).
    pub documents: Vec<Document>,
    pub user_classes: Vec<UserClass>,
    /// For each entry of `user_classes`, the index of its document.
    pub user_class_doc: Vec<usize>,
    pub libraries: Vec<LibraryState>,
    pub expanded: BTreeSet<String>,
    pub package_filter: String,
    /// Cached Package Browser matches for the normalized query.
    pub package_search_cache: Option<(String, Vec<String>)>,
    /// Open model tabs (qualified class names) and the active one.
    pub model_tabs: Vec<String>,
    pub active: Option<String>,
    pub history: Vec<String>,
    pub history_pos: usize,
    pub docs: BTreeMap<String, Loadable<DocPage>>,
    pub texts: BTreeMap<String, Loadable<String>>,
    /// Tokenized layout jobs for read-only library source text, keyed by class.
    pub library_text_layouts: BTreeMap<String, crate::text_editor::EditorState>,
    /// Component tree of the active class (`getModelInstance`, G06).
    pub components: Option<(String, Vec<crate::instance::ComponentNode>)>,
    /// Component nodes the user expanded, as dotted paths from the active class.
    pub component_expanded: BTreeSet<String>,
    /// Class whose component tree has been requested but not received yet.
    pub components_pending: Option<String>,
    /// Icon graphics per class, for the Icon layer (`getModelInstanceAnnotation`, G07).
    pub icons: BTreeMap<String, Loadable<crate::graphics::Icon>>,
    /// Diagram graphics + placement + connections per class (G12).
    pub model_diagrams: BTreeMap<String, Loadable<crate::diagram::Diagram>>,
    /// Viewport zoom for the Diagram layer.
    pub diagram_zoom: f32,
    /// Viewport pan for the Diagram layer (in diagram coordinates).
    pub diagram_pan: egui::Vec2,
    /// Selected component in the Diagram layer (for tooltip and component browser sync).
    pub selected_component: Option<String>,
    pub doc_view: DocView,
    pub find_open: bool,
    pub find_query: String,
    pub setup: SimulationSetup,
    /// Classes whose experiment annotation has already been applied to `setup`.
    pub setup_from: Option<String>,
    /// Set once the user changes the general setup fields by hand; the `experiment`
    /// annotation of a later class must then not overwrite their values.
    pub setup_edited_by_user: bool,
    pub show_setup: bool,
    pub path_dialog: Option<PathDialog>,
    /// Channel for async results from native file dialogs.
    pub dialog_rx: Option<std::sync::mpsc::Receiver<DialogResult>>,
    /// Recently opened/saved files (max [`MAX_RECENT_FILES`]).
    pub recent_files: Vec<PathBuf>,
    pub runs: Vec<ResultRun>,
    pub next_run_id: u64,
    pub keep_results: usize,
    pub variable_filter: String,
    pub var_tree_cache: BTreeMap<u64, VarTreeCache>,
    /// Time shown in the Value column; `None` = the last sample of the newest result.
    pub variable_time: Option<f64>,
    /// Explicit expand/collapse state of the Variable Browser tree, keyed by
    /// `<run id>:<path>`. A node that was never touched follows the default rule
    /// (a top-level node with at most 20 children starts open).
    pub variable_expanded: BTreeMap<String, bool>,
    /// Filter toggles of the Variable Browser (both on by default).
    pub variable_show_parameters: bool,
    pub variable_show_time_varying: bool,
    /// The `More >>` row of the Variable Browser is open.
    pub variable_advanced: bool,
    /// Diagrams of the plot window, stacked vertically; never empty.
    pub diagrams: Vec<crate::plot::PlotDiagram>,
    /// Diagram the Variable Browser checkboxes and the CSV export work on.
    pub active_diagram: usize,
    /// Draw the measurement cursor (a vertical line with the value of every curve).
    pub cursor_enabled: bool,
    /// Values edited in the variable browser, applied as overrides on the next simulation.
    pub pending_overrides: BTreeMap<String, String>,
    /// Whether the contextual Plot Options ribbon is selected (Simulation remains active).
    pub plot_options: bool,
    pub logs: Vec<LogEntry>,
    /// Source positions parsed from the log entries of the current Check / Simulate (G25).
    pub error_locations: Vec<SourceLocation>,
    /// `logs.len()` when [`Self::error_locations`] was built; `None` = rebuild next frame.
    error_locations_log_len: Option<usize>,
    /// Index of the first log entry written by the current Check / Simulate.
    error_from_log_index: usize,
    pub log_tab: LogTab,
    pub bottom_pane: BottomPane,
    pub command_input: String,
    pub command_history: Vec<String>,
    pub docks: Docks,
    /// Height of the command/log window (drag its top edge to change).
    pub bottom_height: f32,
    pub busy: bool,
    pub omc_version: Option<String>,
    /// Root of the on-disk library index cache (see [`crate::cache`]).
    pub cache_root: PathBuf,
    pub worker: Worker,
    /// Last window title sent to the OS.
    pub title_sent: String,
    /// A link to open in the system browser at the end of the frame.
    pub pending_url: Option<String>,
    pub omc_hint_shown: bool,
    /// The "Unsaved changes" dialog is open because the window was asked to close (G42).
    pub confirm_exit: bool,
    /// The user chose to discard unsaved changes: the next close request goes through.
    pub exit_confirmed: bool,
    pub frame_stats: crate::perf::FrameStats,
    pub pending_edit: Option<String>,
    /// Result copies still queued or running on the worker.
    pub pending_result_copies: usize,
    pub result_copy_close_warned: bool,
    /// Text of the document when the running edit was submitted: a result for an
    /// older text is not applied (G53).
    pub pending_edit_text: Option<String>,
    /// The native "Save as" dialog that is still open belongs to a document whose
    /// index is no longer valid; its answer is ignored (G53).
    pub stale_save_as: bool,
    /// A close request was already held back for the running model edit; the next
    /// one is let through, so a hanging edit cannot keep the window open (G53).
    pub edit_close_warned: bool,
    pub rename_dialog: Option<RenameDialog>,
    pub parameter_dialog: Option<ParameterDialog>,
    pub diagram_drag: Option<DiagramDrag>,
    pub connection_drag: Option<ConnectionDrag>,
    pub selected_connection: Option<usize>,
    pub pending_placement: Option<(String, Placement)>,
    /// Document waiting for the answer of the "Save changes?" dialog (G42).
    pub pending_close: Option<usize>,
    /// Document that closes as soon as its pending save succeeds (G42).
    pub close_after_save: Option<usize>,
    /// Document the open "Save as" dialog belongs to; `None` = the active one.
    pub save_as_doc: Option<usize>,
    /// A write to an existing file waiting for the "Replace file?" answer (G43).
    pub overwrite_confirm: Option<PendingWrite>,
    /// Document whose file was changed by another program since it was read (G43).
    pub save_conflict: Option<usize>,
    /// The running parameter sweep (G49).
    pub sweep: Option<SweepPlan>,
    pub sweep_dialog: Option<SweepDialog>,
    /// Input of the "Go To Line" dialog (G45).
    pub goto_line_dialog: Option<String>,
    pub replace_dialog: Option<ReplaceDialog>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiagramDrag {
    pub component: String,
    pub start: [f64; 2],
    pub offset: [f64; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectionDrag {
    pub from: String,
    pub start: [f64; 2],
    pub current: [f64; 2],
}

/// Shown once when no OpenModelica compiler is found.
pub const OMC_SETUP_HINT: &str =
    "OpenModelica (omc) is required for checking, simulation and libraries.\n\
Install it together with the Modelica Standard Library:\n\
  Windows:        powershell -ExecutionPolicy Bypass -File scripts\\setup-windows.ps1\n\
  Debian/Ubuntu:  scripts/setup-linux.sh\n\
  macOS / Docker: scripts/setup-docker.sh\n\
or set OMOLA_OMC to an existing omc executable, then use Tools > Locate OMC. See docs/install.md.";

impl OmolaApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_file: Option<PathBuf>) -> Self {
        crate::theme::apply(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let ctx = cc.egui_ctx.clone();
        let worker = Worker::spawn(crate::backend::default_work_root(), move || {
            ctx.request_repaint()
        });
        let mut app = Self::with_worker(worker);
        app.frame_stats = crate::perf::FrameStats::from_env();
        app.worker.submit(Job::Doctor);
        // Serve the library tree from the on-disk index cache when it is still valid, so
        // start-up does not re-run `getClassNames(recursive=true)` over the whole MSL.
        for spec in library::default_libraries() {
            if !app.load_library_from_cache(spec.clone()) {
                app.load_library(spec);
            }
        }
        // Like a fresh workbench session: an empty model "Unnamed" is always available.
        let unnamed = app.add_unnamed();
        app.model_tabs.push(unnamed);

        // Restore persisted state if available.
        let persisted = cc
            .storage
            .and_then(|storage| eframe::get_value::<PersistedState>(storage, STATE_KEY));
        app.apply_persisted(persisted.unwrap_or_default(), initial_file);
        app
    }

    /// Construct without a window (tests and `new`).
    pub fn with_worker(worker: Worker) -> Self {
        Self {
            tab: RibbonTab::Documentation,
            graphics_layer: Layer::Diagram,
            documents: Vec::new(),
            user_classes: Vec::new(),
            user_class_doc: Vec::new(),
            libraries: Vec::new(),
            expanded: BTreeSet::new(),
            package_filter: String::new(),
            package_search_cache: None,
            model_tabs: Vec::new(),
            active: None,
            history: Vec::new(),
            history_pos: 0,
            docs: BTreeMap::new(),
            texts: BTreeMap::new(),
            library_text_layouts: BTreeMap::new(),
            components: None,
            component_expanded: BTreeSet::new(),
            components_pending: None,
            icons: BTreeMap::new(),
            model_diagrams: BTreeMap::new(),
            diagram_zoom: 1.0,
            diagram_pan: egui::Vec2::ZERO,
            selected_component: None,
            doc_view: DocView::Formatted,
            find_open: false,
            find_query: String::new(),
            setup: SimulationSetup::default(),
            setup_from: None,
            setup_edited_by_user: false,
            show_setup: false,
            path_dialog: None,
            dialog_rx: None,
            recent_files: Vec::new(),
            runs: Vec::new(),
            next_run_id: 1,
            keep_results: 2,
            variable_filter: String::new(),
            var_tree_cache: BTreeMap::new(),
            variable_time: None,
            variable_expanded: BTreeMap::new(),
            variable_show_parameters: true,
            variable_show_time_varying: true,
            variable_advanced: false,
            diagrams: vec![crate::plot::PlotDiagram::default()],
            active_diagram: 0,
            cursor_enabled: false,
            pending_overrides: BTreeMap::new(),
            plot_options: false,
            logs: Vec::new(),
            error_locations: Vec::new(),
            error_locations_log_len: None,
            error_from_log_index: 0,
            log_tab: LogTab::Version,
            bottom_pane: BottomPane::Logs,
            command_input: String::new(),
            command_history: Vec::new(),
            docks: Docks::default(),
            bottom_height: 135.0,
            busy: false,
            omc_version: None,
            cache_root: crate::cache::cache_root(),
            worker,
            title_sent: String::new(),
            pending_url: None,
            omc_hint_shown: false,
            confirm_exit: false,
            exit_confirmed: false,
            frame_stats: crate::perf::FrameStats::disabled(),
            pending_edit: None,
            pending_result_copies: 0,
            result_copy_close_warned: false,
            pending_edit_text: None,
            stale_save_as: false,
            edit_close_warned: false,
            rename_dialog: None,
            parameter_dialog: None,
            diagram_drag: None,
            connection_drag: None,
            selected_connection: None,
            pending_placement: None,
            pending_close: None,
            close_after_save: None,
            save_as_doc: None,
            overwrite_confirm: None,
            save_conflict: None,
            sweep: None,
            sweep_dialog: None,
            goto_line_dialog: None,
            replace_dialog: None,
        }
    }

    // ---------------------------------------------------------------- libraries & classes

    // ---------------------------------------------------------------- user files

    // ---------------------------------------------------------------- check & simulate

    /// Apply worker events. Called once per frame.
    pub fn handle_events(&mut self) {
        for event in self.worker.poll() {
            match event {
                Event::Log(tab, level, text) => {
                    if level == LogLevel::Error {
                        self.log_tab = tab;
                        self.bottom_pane = BottomPane::Logs;
                    } else if matches!(tab, LogTab::Translation | LogTab::Simulation) {
                        self.log_tab = tab;
                    }
                    self.push_log(LogEntry { tab, level, text });
                }
                Event::Command(text) => self.command_history.push(text),
                Event::OmcVersion(version) => {
                    if version.is_none() && self.omc_version.is_none() && !self.omc_hint_shown {
                        self.omc_hint_shown = true;
                        self.log_tab = LogTab::Version;
                        self.push_log(LogEntry {
                            tab: LogTab::Version,
                            level: LogLevel::Warning,
                            text: OMC_SETUP_HINT.into(),
                        });
                    }
                    self.omc_version = version;
                }
                Event::Busy(busy) => self.busy = busy,
                Event::LibraryLoaded(spec, result) => {
                    self.package_search_cache = None;
                    let state = match result {
                        Ok(lib) => LibraryState::Loaded(lib),
                        Err(message) => LibraryState::Failed(spec.clone(), message),
                    };
                    match self
                        .libraries
                        .iter_mut()
                        .find(|l| l.spec().name == spec.name)
                    {
                        Some(slot) => *slot = state,
                        None => self.libraries.push(state),
                    }
                    // Re-request documentation that waited for this index.
                    if let Some(active) = self.active.clone() {
                        if !self.docs.contains_key(&active) {
                            self.request_doc(&active);
                        }
                    }
                }
                Event::ClassDoc(class, source, result) => {
                    self.accept_class_doc(class, source, result)
                }
                Event::ClassText(class, result) => {
                    self.library_text_layouts.remove(&class);
                    self.texts.insert(
                        class,
                        match result {
                            Ok(text) => Loadable::Ready(text),
                            Err(e) => Loadable::Failed(e),
                        },
                    );
                }
                Event::Instance(class, result) => {
                    if self.components_pending.as_deref() == Some(class.as_str()) {
                        self.components_pending = None;
                    }
                    match result {
                        Ok(nodes) => self.components = Some((class, nodes)),
                        Err(message) => self.info(
                            LogTab::Translation,
                            format!("Component tree of {class} is not available: {message}"),
                        ),
                    }
                }
                Event::Parameters { component, result } => {
                    if let Some(dialog) = self
                        .parameter_dialog
                        .as_mut()
                        .filter(|d| d.component == component)
                    {
                        match result {
                            Ok(rows) => {
                                dialog.values = rows
                                    .iter()
                                    .map(|r| r.current_value.clone().unwrap_or_default())
                                    .collect();
                                dialog.rows = Some(rows);
                            }
                            Err(message) => dialog.error = Some(message),
                        }
                    }
                }
                Event::Icon(class, result) => {
                    self.icons.insert(
                        class,
                        match result {
                            Ok(icon) => Loadable::Ready(icon),
                            Err(message) => Loadable::Failed(message),
                        },
                    );
                }
                Event::Diagram(class, result) => {
                    if let Ok(diagram) = &result {
                        for component in &diagram.components {
                            if let Some(icon) = &component.icon {
                                self.icons.insert(
                                    component.type_name.clone(),
                                    Loadable::Ready(icon.clone()),
                                );
                            }
                            for pin in &component.connectors {
                                if let Some(icon) = &pin.icon {
                                    self.icons.insert(
                                        pin.type_name.clone(),
                                        Loadable::Ready(icon.clone()),
                                    );
                                }
                            }
                        }
                    }
                    self.model_diagrams.insert(
                        class,
                        match result {
                            Ok(diagram) => Loadable::Ready(diagram),
                            Err(message) => Loadable::Failed(message),
                        },
                    );
                }
                Event::SimulationFinished(outcome) => {
                    let sweep = self.sweep_run_tag();
                    self.add_run_tagged(*outcome, sweep);
                    self.sweep_run_finished();
                }
                Event::SimulationFailed(class) => self.sweep_run_failed(&class),
                Event::ResultCopied { dest, result } => {
                    self.pending_result_copies = self.pending_result_copies.saturating_sub(1);
                    if self.pending_result_copies == 0 {
                        self.result_copy_close_warned = false;
                    }
                    match result {
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
                Event::LibrariesInstalled(ok) => {
                    if ok {
                        self.docs.clear();
                        for spec in library::default_libraries() {
                            self.load_library(spec);
                        }
                    }
                }
                Event::ModelEdited { class, result } => {
                    self.pending_placement = None;
                    self.selected_connection = None;
                    self.handle_model_edited_event(Event::ModelEdited { class, result });
                }
            }
        }
    }
}

impl eframe::App for OmolaApp {
    /// Runs even when nothing is painted (a minimized window), so a close request can
    /// never slip past the unsaved-changes question (G42).
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let close_requested = ctx.input(|i| i.viewport().close_requested());
        if close_requested && self.hold_close_for_edit() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            // The warning must be visible: bring a minimized window back.
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            ctx.request_repaint();
        } else if close_requested && self.must_confirm_exit() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            // The question must be visible: bring a minimized window back.
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            self.confirm_exit = true;
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let started = std::time::Instant::now();
        self.handle_events();
        self.refresh_error_locations();
        self.poll_dialog();
        self.handle_shortcuts(ui);
        let title = self.window_title();
        if title != self.title_sent {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Title(format!("{title} - Omola")));
            self.title_sent = title;
        }
        self.layout(ui);
        self.dialogs(ui);
        if let Some(url) = self.pending_url.take() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(url));
        }
        if let Some(s) = self
            .frame_stats
            .record(started.elapsed(), std::time::Instant::now())
        {
            eprintln!(
                "[omola-frame] frames={} avg={:.2}ms max={:.2}ms tab={} layer={}",
                s.frames,
                s.avg_ms,
                s.max_ms,
                self.tab.label(),
                self.layer().label()
            );
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let state = self.to_persisted();
        eframe::set_value(storage, STATE_KEY, &state);
    }
}

impl OmolaApp {
    fn accept_class_doc(
        &mut self,
        class: String,
        source: Option<String>,
        result: Result<library::ClassDoc, String>,
    ) {
        if !matches!(self.docs.get(&class), Some(Loadable::Loading))
            || (source.is_none() && self.is_user_class(&class))
            || source.as_ref().is_some_and(|text| {
                self.doc_of(&class)
                    .is_none_or(|i| self.documents[i].text != *text)
            })
        {
            return;
        }
        let page = match result {
            Ok(doc) => {
                self.apply_experiment(&class, &doc.restriction, doc.experiment);
                let info = html::parse(&doc.info_html);
                Loadable::Ready(DocPage { doc, info })
            }
            Err(message) => Loadable::Failed(message),
        };
        self.docs.insert(class, page);
    }
    fn handle_shortcuts(&mut self, ui: &egui::Ui) {
        use egui::{Key, KeyboardShortcut, Modifiers};
        let save = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
        let save_as = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::S);
        let new = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
        let open = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
        let close = KeyboardShortcut::new(Modifiers::COMMAND, Key::W);
        let goto = KeyboardShortcut::new(Modifiers::COMMAND, Key::G);
        let replace = KeyboardShortcut::new(Modifiers::COMMAND, Key::H);
        let simulate = KeyboardShortcut::new(Modifiers::NONE, Key::F9);
        let find = KeyboardShortcut::new(Modifiers::COMMAND, Key::F);
        let undo = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
        let redo = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
        let redo_alternate = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
        let text_edit_has_focus =
            self.layer() == Layer::Text && ui.ctx().memory(|memory| memory.focused().is_some());
        if !text_edit_has_focus {
            if ui.input_mut(|i| i.consume_shortcut(&undo)) {
                self.undo();
            } else if ui.input_mut(|i| i.consume_shortcut(&redo))
                || ui.input_mut(|i| i.consume_shortcut(&redo_alternate))
            {
                self.redo();
            }
        }
        // The more specific shortcut first: Ctrl+S also matches Ctrl+Shift+S.
        if ui.input_mut(|i| i.consume_shortcut(&save_as)) {
            self.request_save_as_dialog();
        } else if ui.input_mut(|i| i.consume_shortcut(&save)) {
            self.save();
        }
        if ui.input_mut(|i| i.consume_shortcut(&new)) {
            self.new_model();
        }
        if ui.input_mut(|i| i.consume_shortcut(&open)) {
            self.request_open_dialog();
        }
        if ui.input_mut(|i| i.consume_shortcut(&close)) {
            self.close_active();
        }
        if ui.input_mut(|i| i.consume_shortcut(&goto)) {
            self.open_goto_line_dialog();
        }
        if ui.input_mut(|i| i.consume_shortcut(&replace)) {
            self.open_replace_dialog();
        }
        if !self.busy && ui.input_mut(|i| i.consume_shortcut(&simulate)) {
            self.simulate();
        }
        if ui.input_mut(|i| i.consume_shortcut(&find)) {
            self.find_open = true;
        }
    }
}

#[cfg(test)]
mod tests;
