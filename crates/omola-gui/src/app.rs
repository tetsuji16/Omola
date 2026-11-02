//! Top-level application state and the per-frame layout.
//!
//! The window follows the classic Modelica-workbench arrangement documented in
//! `docs/reference/dymola-gui.md`: quick access toolbar and title, ribbon tabs with
//! grouped commands, docked browsers on the left, model tabs and the edit window in the
//! centre, the command/log window at the bottom, and a browser bar with the layer toolbar.

use crate::backend::{
    ClassContext, Event, Job, LogLevel, LogTab, SimulationOutcome, SimulationSetup, Worker,
};
use crate::html::{self, Block};
use crate::library::{self, ClassDoc, LibrarySpec, LoadedLibrary};
use crate::model::{default_class, scan_classes, ClassEntry as UserClass, Document};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

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
}

/// A curve in the plot window: (run id, variable name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlotSignal {
    pub run_id: u64,
    pub variable: String,
}

/// Small text-input dialogs (fallback when `OMOLA_GUI_TEXT_DIALOGS=1` or no native portal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathDialog {
    Open(String),
    SaveAs(String),
    RunScript(String),
}

/// Async result from a native file dialog (picked on a background thread).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogResult {
    Open(Option<PathBuf>),
    SaveAs(Option<PathBuf>),
}

/// Maximum number of entries in the "Recent files" menu.
pub const MAX_RECENT_FILES: usize = 8;

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
    /// Open model tabs (qualified class names) and the active one.
    pub model_tabs: Vec<String>,
    pub active: Option<String>,
    pub history: Vec<String>,
    pub history_pos: usize,
    pub docs: BTreeMap<String, Loadable<DocPage>>,
    pub texts: BTreeMap<String, Loadable<String>>,
    pub doc_view: DocView,
    pub find_open: bool,
    pub find_query: String,
    pub setup: SimulationSetup,
    /// Classes whose experiment annotation has already been applied to `setup`.
    pub setup_from: Option<String>,
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
    pub plotted: Vec<PlotSignal>,
    /// Values edited in the variable browser, applied as overrides on the next simulation.
    pub pending_overrides: BTreeMap<String, String>,
    /// Zoomed plot range (x0, x1, y0, y1); `None` = fit to data.
    pub plot_view: Option<(f64, f64, f64, f64)>,
    pub logs: Vec<LogEntry>,
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
            model_tabs: Vec::new(),
            active: None,
            history: Vec::new(),
            history_pos: 0,
            docs: BTreeMap::new(),
            texts: BTreeMap::new(),
            doc_view: DocView::Formatted,
            find_open: false,
            find_query: String::new(),
            setup: SimulationSetup::default(),
            setup_from: None,
            show_setup: false,
            path_dialog: None,
            dialog_rx: None,
            recent_files: Vec::new(),
            runs: Vec::new(),
            next_run_id: 1,
            keep_results: 2,
            variable_filter: String::new(),
            plotted: Vec::new(),
            pending_overrides: BTreeMap::new(),
            plot_view: None,
            logs: Vec::new(),
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
        }
    }

    // ---------------------------------------------------------------- libraries & classes

    pub fn load_library(&mut self, spec: LibrarySpec) {
        self.libraries.retain(|l| l.spec().name != spec.name);
        self.libraries.push(LibraryState::Loading(spec.clone()));
        self.worker.submit(Job::LoadLibrary(spec));
    }

    /// Install a library from the on-disk index cache without starting OMC.
    ///
    /// Returns `false` on a cache miss (or a stale/corrupt entry), so the caller can fall
    /// back to [`Self::load_library`]. `load_library` itself always queries OMC, which is
    /// what the Tools > Reload Libraries command wants.
    pub fn load_library_from_cache(&mut self, spec: LibrarySpec) -> bool {
        let Some(lib) = crate::cache::load(&self.cache_root, &spec) else {
            return false;
        };
        let (name, version, count, source) = (
            spec.name.clone(),
            lib.version.clone(),
            lib.classes.len(),
            lib.source_file.clone(),
        );
        self.libraries.retain(|l| l.spec().name != name);
        self.libraries.push(LibraryState::Loaded(lib));
        self.info(
            LogTab::Version,
            format!(
                "Loaded {name} {version} ({count} classes) from cache {}",
                source.display()
            ),
        );
        true
    }

    pub fn loaded_specs(&self) -> Vec<LibrarySpec> {
        self.libraries
            .iter()
            .filter_map(|l| match l {
                LibraryState::Loaded(lib) => Some(lib.spec.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn is_user_class(&self, class: &str) -> bool {
        self.user_classes.iter().any(|c| c.qualified_name == class)
    }

    /// Index of the document that defines a user class.
    pub fn doc_of(&self, class: &str) -> Option<usize> {
        self.user_classes
            .iter()
            .position(|c| c.qualified_name == class)
            .map(|i| self.user_class_doc[i])
    }

    /// The document of the active class, if it is a user class.
    pub fn active_doc(&self) -> Option<usize> {
        self.active.as_deref().and_then(|c| self.doc_of(c))
    }

    /// Context for OMC queries about `class`: loaded libraries plus the user buffer if needed.
    pub fn context_for(&self, class: &str) -> ClassContext {
        let top = class.split('.').next().unwrap_or(class);
        let user = self.is_user_class(class);
        let mut libraries = self.loaded_specs();
        if !user {
            // Load only the library that owns the class (faster), plus its dependencies
            // which OMC resolves through `uses` annotations.
            libraries.retain(|l| l.name == top);
            if libraries.is_empty() {
                libraries.push(LibrarySpec::new(top, None));
            }
        }
        ClassContext {
            libraries,
            user_source: user
                .then(|| self.doc_of(class).map(|i| self.documents[i].text.clone()))
                .flatten(),
        }
    }

    /// Restriction (`model`, `package`, ...) if known from an index.
    pub fn restriction_of(&self, class: &str) -> Option<String> {
        if let Some(c) = self.user_classes.iter().find(|c| c.qualified_name == class) {
            return Some(c.kind.clone());
        }
        self.libraries.iter().find_map(|l| match l {
            LibraryState::Loaded(lib) => lib.class(class).map(|c| c.restriction.clone()),
            _ => None,
        })
    }

    /// Children of a library or user class, as (qualified, restriction, comment).
    pub fn children_of(&self, class: &str) -> Vec<(String, String, String)> {
        if self.is_user_class(class) {
            return self
                .user_classes
                .iter()
                .filter(|c| library::parent_name(&c.qualified_name) == Some(class))
                .map(|c| {
                    (
                        c.qualified_name.clone(),
                        c.kind.clone(),
                        c.description.clone().unwrap_or_default(),
                    )
                })
                .collect();
        }
        self.libraries
            .iter()
            .find_map(|l| match l {
                LibraryState::Loaded(lib)
                    if class.split('.').next() == Some(lib.spec.name.as_str()) =>
                {
                    Some(
                        lib.children(class)
                            .into_iter()
                            .map(|c| {
                                (
                                    c.qualified_name.clone(),
                                    c.restriction.clone(),
                                    c.comment.clone(),
                                )
                            })
                            .collect(),
                    )
                }
                _ => None,
            })
            .unwrap_or_default()
    }

    /// Show a class in the edit window (new tab if needed) and record history.
    pub fn open_class(&mut self, class: &str) {
        self.navigate(class);
        if self
            .history
            .get(self.history_pos.wrapping_sub(1))
            .map(String::as_str)
            != Some(class)
            || self.history.is_empty()
        {
            self.history.truncate(self.history_pos);
            self.history.push(class.to_owned());
            self.history_pos = self.history.len();
        }
    }

    fn navigate(&mut self, class: &str) {
        if !self.model_tabs.iter().any(|t| t == class) {
            // Replace the active tab (like single-clicking in the package browser),
            // unless it is a user model, which keeps its own tab.
            match self
                .active
                .as_ref()
                .and_then(|a| self.model_tabs.iter().position(|t| t == a))
            {
                Some(i) if !self.is_user_class(&self.model_tabs[i]) => {
                    self.model_tabs[i] = class.to_owned()
                }
                _ => self.model_tabs.push(class.to_owned()),
            }
        }
        self.active = Some(class.to_owned());
        // Expand the package browser down to the class.
        let mut prefix = String::new();
        for part in class.split('.') {
            if !prefix.is_empty() {
                self.expanded.insert(prefix.clone());
                prefix.push('.');
            }
            prefix.push_str(part);
        }
        self.request_doc(class);
        if self.tab == RibbonTab::Text {
            self.request_text(class);
        }
    }

    pub fn open_in_new_tab(&mut self, class: &str) {
        if !self.model_tabs.iter().any(|t| t == class) {
            self.model_tabs.push(class.to_owned());
        }
        self.open_class(class);
    }

    pub fn close_tab(&mut self, class: &str) {
        self.model_tabs.retain(|t| t != class);
        if self.active.as_deref() == Some(class) {
            self.active = self.model_tabs.last().cloned();
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.history_pos > 1
    }

    pub fn can_go_forward(&self) -> bool {
        self.history_pos < self.history.len()
    }

    pub fn go_back(&mut self) {
        if self.can_go_back() {
            self.history_pos -= 1;
            let class = self.history[self.history_pos - 1].clone();
            self.navigate(&class);
        }
    }

    pub fn go_forward(&mut self) {
        if self.can_go_forward() {
            self.history_pos += 1;
            let class = self.history[self.history_pos - 1].clone();
            self.navigate(&class);
        }
    }

    pub fn request_doc(&mut self, class: &str) {
        let stale = matches!(self.docs.get(class), Some(Loadable::Failed(_)))
            || self.doc_of(class).is_some_and(|i| self.documents[i].dirty);
        if self.docs.contains_key(class) && !stale {
            return;
        }
        if self
            .libraries
            .iter()
            .any(|l| matches!(l, LibraryState::Loading(_)))
            && !self.is_user_class(class)
            && self.restriction_of(class).is_none()
        {
            // Wait for the library index; re-requested when it arrives.
            return;
        }
        self.docs.insert(class.to_owned(), Loadable::Loading);
        self.worker.submit(Job::LoadClassDoc {
            class: class.to_owned(),
            context: self.context_for(class),
        });
    }

    pub fn request_text(&mut self, class: &str) {
        if self.is_user_class(class) || self.texts.contains_key(class) {
            return;
        }
        if self.restriction_of(class).as_deref() == Some("package") {
            self.texts.insert(
                class.to_owned(),
                Loadable::Failed(
                    "The Modelica text of a whole package is not shown; open one of its classes."
                        .into(),
                ),
            );
            return;
        }
        self.texts.insert(class.to_owned(), Loadable::Loading);
        self.worker.submit(Job::LoadClassText {
            class: class.to_owned(),
            context: self.context_for(class),
        });
    }

    // ---------------------------------------------------------------- user files

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

    fn first_class_of(&self, doc: usize) -> Option<String> {
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
        let Some(i) = self.active_doc() else {
            return;
        };
        if self.documents[i].path.is_none() {
            self.request_save_as_dialog();
            return;
        }
        match self.documents[i].save() {
            Ok(()) => {
                let title = self.documents[i].title();
                if let Some(path) = self.documents[i].path.clone() {
                    push_recent(&mut self.recent_files, path);
                }
                self.info(LogTab::Syntax, format!("Saved {title}"))
            }
            Err(e) => self.error(LogTab::Syntax, format!("Save failed: {e}")),
        }
    }

    pub fn refresh_outline(&mut self) {
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
        // Drop cached documentation of user classes; it is re-queried when shown.
        for name in before {
            self.docs.remove(&name);
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

    // ---------------------------------------------------------------- check & simulate

    pub fn check(&mut self) {
        let Some(class) = self.active.clone() else {
            return self.error(LogTab::Syntax, "No model is open.".into());
        };
        self.log_tab = LogTab::Syntax;
        self.worker.submit(Job::Check {
            context: self.context_for(&class),
            class,
        });
    }

    pub fn simulate(&mut self) {
        let Some(class) = self.active.clone() else {
            return self.error(LogTab::Translation, "No model is open.".into());
        };
        if self.restriction_of(&class).as_deref() == Some("package") {
            return self.error(
                LogTab::Translation,
                format!("{class} is a package and cannot be simulated. Open a model, for example an Examples model."),
            );
        }
        let mut setup = self.setup.clone();
        setup.parameters = merge_overrides(&setup.parameters, &self.pending_overrides);
        if let Err(e) = omola_core::validate_parameter_overrides(&setup.parameters) {
            return self.error(LogTab::Translation, e.to_string());
        }
        self.log_tab = LogTab::Translation;
        self.bottom_pane = BottomPane::Logs;
        self.worker.submit(Job::Simulate {
            context: self.context_for(&class),
            class,
            setup,
        });
    }

    pub fn run_command(&mut self) {
        let text = self.command_input.trim().to_owned();
        if text.is_empty() {
            return;
        }
        self.command_input.clear();
        self.command_history.push(text.clone());
        self.worker.submit(Job::Command {
            text,
            libraries: self.loaded_specs(),
        });
    }

    pub fn info(&mut self, tab: LogTab, text: String) {
        self.logs.push(LogEntry {
            tab,
            level: LogLevel::Info,
            text,
        });
    }

    pub fn error(&mut self, tab: LogTab, text: String) {
        self.log_tab = tab;
        self.logs.push(LogEntry {
            tab,
            level: LogLevel::Error,
            text,
        });
    }

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
                    self.logs.push(LogEntry { tab, level, text });
                }
                Event::Command(text) => self.command_history.push(text),
                Event::OmcVersion(version) => {
                    if version.is_none() && self.omc_version.is_none() && !self.omc_hint_shown {
                        self.omc_hint_shown = true;
                        self.log_tab = LogTab::Version;
                        self.logs.push(LogEntry {
                            tab: LogTab::Version,
                            level: LogLevel::Warning,
                            text: OMC_SETUP_HINT.into(),
                        });
                    }
                    self.omc_version = version;
                }
                Event::Busy(busy) => self.busy = busy,
                Event::LibraryLoaded(spec, result) => {
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
                Event::ClassDoc(class, result) => {
                    let page = match result {
                        Ok(doc) => {
                            if let Some(exp) = doc.experiment {
                                if doc.restriction != "package"
                                    && self.setup_from.as_deref() != Some(class.as_str())
                                {
                                    self.setup.start_time = exp.start_time;
                                    self.setup.stop_time = exp.stop_time;
                                    self.setup_from = Some(class.clone());
                                }
                            }
                            let info = html::parse(&doc.info_html);
                            Loadable::Ready(DocPage { doc, info })
                        }
                        Err(message) => Loadable::Failed(message),
                    };
                    self.docs.insert(class, page);
                }
                Event::ClassText(class, result) => {
                    self.texts.insert(
                        class,
                        match result {
                            Ok(text) => Loadable::Ready(text),
                            Err(e) => Loadable::Failed(e),
                        },
                    );
                }
                Event::SimulationFinished(outcome) => self.add_run(*outcome),
                Event::LibrariesInstalled(ok) => {
                    if ok {
                        self.docs.clear();
                        for spec in library::default_libraries() {
                            self.load_library(spec);
                        }
                    }
                }
            }
        }
    }

    /// Store a new result, drop the oldest beyond `keep_results`, and keep curves on the
    /// newest result of the same model (as a re-simulation replots the same variables).
    pub fn add_run(&mut self, outcome: SimulationOutcome) {
        let id = self.next_run_id;
        self.next_run_id += 1;
        let short = library::short_name(&outcome.class).to_owned();
        let same = self
            .runs
            .iter()
            .filter(|r| r.outcome.class == outcome.class)
            .count();
        let label = if same == 0 {
            short
        } else {
            format!("{short} {}", same + 1)
        };
        let latest_previous = self
            .runs
            .iter()
            .filter(|r| r.outcome.class == outcome.class)
            .map(|r| r.id)
            .max();
        let mut replot: Vec<PlotSignal> = self
            .plotted
            .iter()
            .filter(|s| Some(s.run_id) == latest_previous)
            .map(|s| PlotSignal {
                run_id: id,
                variable: s.variable.clone(),
            })
            .collect();
        if replot.is_empty() && self.plotted.is_empty() {
            if let Some(var) = outcome.result.variables.iter().find(|v| {
                let leaf = v.name.rsplit('.').next().unwrap_or(&v.name);
                !v.is_parameter && leaf != "time" && !v.name.contains("der(")
            }) {
                replot.push(PlotSignal {
                    run_id: id,
                    variable: var.name.clone(),
                });
            }
        }
        let moving = !replot.is_empty() && latest_previous.is_some();
        self.plotted
            .retain(|s| !(moving && Some(s.run_id) == latest_previous));
        self.plotted.extend(replot);
        self.runs.push(ResultRun { id, label, outcome });
        while self.runs.len() > self.keep_results.max(1) {
            let removed = self.runs.remove(0);
            self.plotted.retain(|s| s.run_id != removed.id);
        }
        self.tab = RibbonTab::Simulation;
        self.log_tab = LogTab::Simulation;
    }

    pub fn run(&self, id: u64) -> Option<&ResultRun> {
        self.runs.iter().find(|r| r.id == id)
    }

    /// The edit-window layer for the current ribbon tab.
    pub fn layer(&self) -> Layer {
        match self.tab {
            RibbonTab::Graphics | RibbonTab::Tools | RibbonTab::Simulation => self.graphics_layer,
            RibbonTab::Documentation => Layer::Documentation,
            RibbonTab::Text => Layer::Text,
        }
    }

    // ---------------------------------------------------------------- persistence

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

    // ---------------------------------------------------------------- native dialogs

    /// True when text-input path dialogs should be used instead of native OS dialogs.
    pub fn use_text_dialogs() -> bool {
        std::env::var("OMOLA_GUI_TEXT_DIALOGS").as_deref() == Ok("1")
    }

    /// Open a native "Open file" dialog on a background thread.
    pub fn request_open_dialog(&mut self) {
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
        if Self::use_text_dialogs() {
            self.path_dialog = Some(PathDialog::SaveAs(String::new()));
            return;
        }
        let file_name = self
            .active_doc()
            .and_then(|i| {
                self.documents[i]
                    .path
                    .as_ref()
                    .and_then(|p| p.file_name().map(|n| n.to_owned()))
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

    /// Poll the native dialog channel and apply results.
    pub fn poll_dialog(&mut self) {
        let result = self.dialog_rx.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(result) = result {
            self.dialog_rx = None;
            match result {
                DialogResult::Open(Some(path)) => self.open_file(path),
                DialogResult::Open(None) => {}
                DialogResult::SaveAs(Some(path)) => {
                    if let Some(i) = self.active_doc() {
                        self.documents[i].path = Some(path);
                    }
                    self.save();
                }
                DialogResult::SaveAs(None) => {}
            }
        }
    }

    pub fn set_layer(&mut self, layer: Layer) {
        match layer {
            Layer::Icon | Layer::Diagram => {
                self.graphics_layer = layer;
                self.tab = RibbonTab::Graphics;
            }
            Layer::Documentation => self.tab = RibbonTab::Documentation,
            Layer::Text => {
                self.tab = RibbonTab::Text;
                if let Some(active) = self.active.clone() {
                    self.request_text(&active);
                }
            }
        }
    }

    /// Window title, e.g. `Modelica - Modelica (Read-Only) - [Documentation]`.
    pub fn window_title(&self) -> String {
        let layer = if self.tab == RibbonTab::Simulation {
            "Plot [1*]".to_owned()
        } else {
            self.layer().label().to_owned()
        };
        match &self.active {
            None => format!("Omola - Modelica Workbench - [{layer}]"),
            Some(class) => {
                let read_only = if self.is_user_class(class) {
                    ""
                } else {
                    " (Read-Only)"
                };
                format!(
                    "{} - {class}{read_only} - [{layer}]",
                    library::short_name(class)
                )
            }
        }
    }
}

/// Setup overrides plus values edited in the variable browser (edited values win).
pub fn merge_overrides(
    setup: &[(String, String)],
    edited: &BTreeMap<String, String>,
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = setup
        .iter()
        .filter(|(n, _)| !edited.contains_key(n))
        .cloned()
        .collect();
    out.extend(
        edited
            .iter()
            .filter(|(_, v)| v.trim().parse::<f64>().is_ok())
            .map(|(n, v)| (n.clone(), v.trim().to_owned())),
    );
    out
}

impl eframe::App for OmolaApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_events();
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
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let state = self.to_persisted();
        eframe::set_value(storage, STATE_KEY, &state);
    }
}

impl OmolaApp {
    fn handle_shortcuts(&mut self, ui: &egui::Ui) {
        use egui::{Key, KeyboardShortcut, Modifiers};
        let save = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
        let simulate = KeyboardShortcut::new(Modifiers::NONE, Key::F9);
        let find = KeyboardShortcut::new(Modifiers::COMMAND, Key::F);
        if ui.input_mut(|i| i.consume_shortcut(&save)) {
            self.save();
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
mod tests {
    use super::*;
    use omola_core::mat::{ResultVariable, TrajectoryResult};

    fn outcome(class: &str) -> SimulationOutcome {
        let var = |name: &str, p: bool| ResultVariable {
            name: name.into(),
            description: String::new(),
            is_parameter: p,
        };
        SimulationOutcome {
            class: class.into(),
            result_file: PathBuf::from("x_res.mat"),
            result: TrajectoryResult {
                variables: vec![var("time", false), var("g", true), var("height", false)],
                time: vec![0.0, 1.0],
                values: vec![vec![0.0, 1.0], vec![9.81, 9.81], vec![1.0, 0.5]],
            },
            elapsed_seconds: 0.0,
        }
    }

    fn app() -> OmolaApp {
        OmolaApp::with_worker(Worker::spawn(
            std::env::temp_dir().join("omola-gui-unused"),
            || {},
        ))
    }

    #[test]
    fn first_run_autoplots_and_old_runs_are_dropped() {
        let mut app = app();
        app.add_run(outcome("BouncingBall"));
        assert_eq!(app.tab, RibbonTab::Simulation);
        assert_eq!(
            app.plotted,
            vec![PlotSignal {
                run_id: 1,
                variable: "height".into()
            }]
        );
        app.add_run(outcome("BouncingBall"));
        app.add_run(outcome("BouncingBall"));
        let ids: Vec<_> = app.runs.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![2, 3]);
        assert!(app.plotted.iter().all(|s| s.run_id == 3));
        assert_eq!(app.plotted.len(), 1);
    }

    #[test]
    fn edited_values_override_setup() {
        let mut edited = BTreeMap::new();
        edited.insert("L".to_owned(), "2".to_owned());
        edited.insert("bad".to_owned(), "x".to_owned());
        let merged = merge_overrides(
            &[("L".into(), "1".into()), ("m".into(), "3".into())],
            &edited,
        );
        assert_eq!(
            merged,
            vec![
                ("m".to_owned(), "3".to_owned()),
                ("L".to_owned(), "2".to_owned())
            ]
        );
    }

    #[test]
    fn library_class_title_and_history() {
        let mut app = app();
        app.open_class("Modelica");
        app.open_class("Modelica.Blocks");
        assert_eq!(
            app.window_title(),
            "Blocks - Modelica.Blocks (Read-Only) - [Documentation]"
        );
        assert_eq!(app.model_tabs, vec!["Modelica.Blocks".to_owned()]);
        assert!(app.expanded.contains("Modelica"));
        app.go_back();
        assert_eq!(app.active.as_deref(), Some("Modelica"));
        assert!(app.can_go_forward());
        app.go_forward();
        assert_eq!(app.active.as_deref(), Some("Modelica.Blocks"));
    }

    #[test]
    fn several_user_documents_and_library_classes() {
        let mut app = app();
        let dir = std::env::temp_dir().join(format!("omola-app-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Pendulum.mo");
        std::fs::write(
            &file,
            "model Pendulum\n  Real x;\nequation\n  x = 1;\nend Pendulum;\n",
        )
        .unwrap();
        app.new_model();
        app.open_file(file.clone());
        assert_eq!(
            app.model_tabs,
            vec!["Unnamed".to_owned(), "Pendulum".to_owned()]
        );
        assert_eq!(app.active.as_deref(), Some("Pendulum"));
        assert_eq!(app.doc_of("Pendulum"), Some(1));
        assert!(app
            .context_for("Pendulum")
            .user_source
            .unwrap()
            .contains("model Pendulum"));
        // Opening the same file again only activates it.
        app.open_class("Unnamed");
        app.open_file(file);
        assert_eq!(app.documents.len(), 2);
        assert_eq!(app.active.as_deref(), Some("Pendulum"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn user_model_gets_its_own_tab_and_context() {
        let mut app = app();
        app.open_class("Modelica");
        app.new_model();
        assert_eq!(
            app.model_tabs,
            vec!["Modelica".to_owned(), "Unnamed".to_owned()]
        );
        let ctx = app.context_for("Unnamed");
        assert!(ctx.user_source.unwrap().contains("model Unnamed"));
        assert_eq!(app.window_title(), "Unnamed - Unnamed - [Diagram]");
        let lib_ctx = app.context_for("Modelica.Blocks");
        assert!(lib_ctx.user_source.is_none());
        assert_eq!(lib_ctx.libraries[0].name, "Modelica");
    }

    // --- G02 persistence tests ---

    #[test]
    fn persisted_state_roundtrips_setup_keep_results_and_tab() {
        let mut app = app();
        app.setup.stop_time = 7.0;
        app.setup.tolerance = 1e-6;
        app.keep_results = 5;
        app.tab = RibbonTab::Simulation;

        let state = app.to_persisted();
        let json = serde_json::to_string(&state).unwrap();
        let restored: PersistedState = serde_json::from_str(&json).unwrap();

        let mut app2 = OmolaApp::with_worker(Worker::spawn(
            std::env::temp_dir().join("omola-gui-unused"),
            || {},
        ));
        app2.apply_persisted(restored, None);

        assert_eq!(app2.setup.stop_time, 7.0);
        assert_eq!(app2.setup.tolerance, 1e-6);
        assert_eq!(app2.keep_results, 5);
        assert_eq!(app2.tab, RibbonTab::Simulation);
    }

    #[test]
    fn missing_last_file_does_not_panic() {
        let state = PersistedState {
            last_file: Some(PathBuf::from("/nonexistent/path/file.mo")),
            tab: Some("Text".to_owned()),
            setup: Some(SimulationSetup::default()),
            keep_results: Some(3),
            recent_files: Vec::new(),
        };
        let mut app = app();
        app.apply_persisted(state, None);
        // Should fall back to the default sample ("Modelica") without panicking.
        assert_eq!(app.active.as_deref(), Some("Modelica"));
        // Tab and keep_results should still be restored.
        assert_eq!(app.tab, RibbonTab::Text);
        assert_eq!(app.keep_results, 3);
    }

    #[test]
    fn cli_file_takes_priority_over_saved_last_file() {
        let dir = std::env::temp_dir().join(format!("omola-persist-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cli_file = dir.join("FromCLI.mo");
        std::fs::write(
            &cli_file,
            "model FromCLI\n  Real x;\nequation\n  x = 1;\nend FromCLI;\n",
        )
        .unwrap();

        let state = PersistedState {
            last_file: Some(PathBuf::from("/nonexistent/saved.mo")),
            tab: Some("Simulation".to_owned()),
            setup: Some(SimulationSetup {
                stop_time: 7.0,
                ..SimulationSetup::default()
            }),
            keep_results: Some(3),
            recent_files: Vec::new(),
        };
        let mut app = app();
        app.apply_persisted(state, Some(cli_file.clone()));

        // CLI file wins.
        assert_eq!(app.active.as_deref(), Some("FromCLI"));
        // But setup/tab/keep from persisted state are still applied.
        assert_eq!(app.setup.stop_time, 7.0);
        assert_eq!(app.tab, RibbonTab::Simulation);
        assert_eq!(app.keep_results, 3);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- G03 recent files tests ---

    #[test]
    fn push_recent_deduplicates_and_caps_at_max() {
        let mut list = Vec::new();
        let a = PathBuf::from("/tmp/a.mo");
        let b = PathBuf::from("/tmp/b.mo");
        push_recent(&mut list, a.clone());
        push_recent(&mut list, b.clone());
        assert_eq!(list, vec![b.clone(), a.clone()]);

        // Re-adding `a` moves it to the front and removes the duplicate.
        push_recent(&mut list, a.clone());
        assert_eq!(list, vec![a.clone(), b.clone()]);
        assert_eq!(list.len(), 2);

        // Fill beyond the cap.
        for i in 0..(MAX_RECENT_FILES + 3) {
            push_recent(&mut list, PathBuf::from(format!("/tmp/{i}.mo")));
        }
        assert_eq!(list.len(), MAX_RECENT_FILES);
        // The most recently added should be first.
        assert_eq!(
            list[0],
            PathBuf::from(format!("/tmp/{}.mo", MAX_RECENT_FILES + 2))
        );
    }

    #[test]
    fn open_file_adds_to_recent_files() {
        let dir = std::env::temp_dir().join(format!("omola-recent-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Recent.mo");
        std::fs::write(
            &file,
            "model Recent\n  Real x;\nequation\n  x = 1;\nend Recent;\n",
        )
        .unwrap();

        let mut app = app();
        app.open_file(file.clone());
        assert_eq!(app.recent_files.len(), 1);
        assert_eq!(app.recent_files[0], file);

        // Opening the same file again should not create a duplicate.
        app.open_class("Unnamed");
        app.open_file(file.clone());
        assert_eq!(app.recent_files.len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- G04 library index cache ---

    #[test]
    fn library_index_is_reused_from_the_cache() {
        use crate::library::ClassEntry;

        let dir = std::env::temp_dir().join(format!("omola-lib-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("package.mo");
        std::fs::write(&source, "package Modelica end Modelica;").unwrap();

        let spec = LibrarySpec::new("Modelica", Some("4.1.0"));
        let lib = LoadedLibrary {
            spec: spec.clone(),
            version: "4.1.0".into(),
            source_file: source,
            classes: vec![
                ClassEntry {
                    qualified_name: "Modelica".into(),
                    restriction: "package".into(),
                    comment: "Modelica Standard Library".into(),
                },
                ClassEntry {
                    qualified_name: "Modelica.Blocks".into(),
                    restriction: "package".into(),
                    comment: String::new(),
                },
            ],
        };
        crate::cache::store(&dir.join("cache"), &lib).unwrap();

        let mut app = app();
        app.cache_root = dir.join("cache");
        assert!(app.load_library_from_cache(spec.clone()));
        // Installed synchronously: no `Loading` placeholder and no OMC job was needed.
        assert!(matches!(app.libraries[0], LibraryState::Loaded(_)));
        assert_eq!(app.children_of("Modelica").len(), 1);

        // A version that was never indexed is a cache miss.
        assert!(!app.load_library_from_cache(LibrarySpec::new("Modelica", Some("3.2.3"))));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
