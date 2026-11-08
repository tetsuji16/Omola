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
use crate::text_editor::{location_is_for, parse_omc_locations, SourceLocation};
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

/// Time axis plus named value columns of a diagram, shaped for `plot::plotted_csv`.
pub type DiagramColumns = (Vec<f64>, Vec<(String, Vec<f64>)>);

/// Small text-input dialogs (fallback when `OMOLA_GUI_TEXT_DIALOGS=1` or no native portal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathDialog {
    Open(String),
    SaveAs(String),
    RunScript(String),
    ExportCsv(String),
}

/// Async result from a native file dialog (picked on a background thread).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogResult {
    Open(Option<PathBuf>),
    SaveAs(Option<PathBuf>),
    ExportCsv(Option<PathBuf>),
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

/// One row of the Package Browser search (G19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub name: String,
    pub restriction: String,
    pub comment: String,
}

/// Hits of the last Package Browser search.
///
/// The search used to lower-case every class name of the MSL on every frame. It is now
/// computed once per query and kept until the query or the class lists change (G19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSearch {
    /// Lower-case query these hits belong to.
    pub query: String,
    pub hits: Vec<SearchHit>,
    /// More classes matched than [`MAX_SEARCH_HITS`]; the list stops there.
    pub truncated: bool,
    /// Bumped whenever the hits are rebuilt (used by the tests).
    pub generation: u64,
}

/// Upper bound for one search, so a one-letter query cannot build a list of thousands.
pub const MAX_SEARCH_HITS: usize = 500;

/// Variable tree of one result for one set of browser filters (G20).
pub struct VarTreeCache {
    pub key: VarTreeKey,
    pub tree: crate::var_tree::VarNode,
    /// Matching variable indices when a text filter is active (flat list).
    pub flat: Vec<usize>,
    /// Bumped whenever the tree is rebuilt (used by the tests).
    pub generation: u64,
}

/// What a [`VarTreeCache`] was built from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarTreeKey {
    pub run_id: u64,
    /// Lower-case filter text (already trimmed).
    pub filter: String,
    pub show_parameters: bool,
    pub show_time_varying: bool,
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
    /// Hits of [`Self::package_filter`], rebuilt only when the query changes (G19).
    pub package_search_cache: Option<PackageSearch>,
    /// Open model tabs (qualified class names) and the active one.
    pub model_tabs: Vec<String>,
    pub active: Option<String>,
    pub history: Vec<String>,
    pub history_pos: usize,
    pub docs: BTreeMap<String, Loadable<DocPage>>,
    pub texts: BTreeMap<String, Loadable<String>>,
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
    /// Variable trees, one per result, rebuilt only when the filters change (G20).
    pub var_tree_cache: BTreeMap<u64, VarTreeCache>,
    pub keep_results: usize,
    pub variable_filter: String,
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
    /// Shared x range of every diagram (x0, x1); `None` = fit to the data.
    pub plot_x: Option<(f64, f64)>,
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
    /// Per-frame timing, printed once a second when `OMOLA_FRAME_STATS=1` (G16).
    pub frame_stats: crate::perf::FrameStats,
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
            var_tree_cache: BTreeMap::new(),
            keep_results: 2,
            variable_filter: String::new(),
            variable_time: None,
            variable_expanded: BTreeMap::new(),
            variable_show_parameters: true,
            variable_show_time_varying: true,
            variable_advanced: false,
            diagrams: vec![crate::plot::PlotDiagram::default()],
            active_diagram: 0,
            cursor_enabled: false,
            pending_overrides: BTreeMap::new(),
            plot_x: None,
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
            frame_stats: crate::perf::FrameStats::disabled(),
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

    /// Whether `class` has children, without building the child list (G19).
    ///
    /// User classes are few and are scanned; library classes use the O(1) index from
    /// G18, so a collapsed row costs a hash lookup instead of a `Vec` per frame.
    pub fn has_children_of(&self, class: &str) -> bool {
        if self.is_user_class(class) {
            return self
                .user_classes
                .iter()
                .any(|c| library::parent_name(&c.qualified_name) == Some(class));
        }
        self.libraries
            .iter()
            .find_map(|l| match l {
                LibraryState::Loaded(lib)
                    if class.split('.').next() == Some(lib.spec.name.as_str()) =>
                {
                    Some(lib.has_children(class))
                }
                _ => None,
            })
            .unwrap_or(false)
    }

    /// Hits of the Package Browser search, rebuilt only when the query changes (G19).
    pub fn search_hits(&mut self, query: &str) -> &PackageSearch {
        let query = query.trim().to_lowercase();
        let stale = self
            .package_search_cache
            .as_ref()
            .is_none_or(|cache| cache.query != query);
        if stale {
            let (hits, truncated) = self.search_classes(&query);
            let generation = self
                .package_search_cache
                .as_ref()
                .map_or(1, |cache| cache.generation + 1);
            self.package_search_cache = Some(PackageSearch {
                query,
                hits,
                truncated,
                generation,
            });
        }
        self.package_search_cache
            .as_ref()
            .expect("the search cache was just filled")
    }

    /// All matches for a lower-case query, at most [`MAX_SEARCH_HITS`] of them.
    ///
    /// A user class matches on its qualified name (like before); a library class matches
    /// on its short name, which is what the search box used to compare.
    fn search_classes(&self, query: &str) -> (Vec<SearchHit>, bool) {
        let mut hits: Vec<SearchHit> = Vec::new();
        for c in &self.user_classes {
            if !c.qualified_name.to_lowercase().contains(query) {
                continue;
            }
            if hits.len() >= MAX_SEARCH_HITS {
                return (hits, true);
            }
            hits.push(SearchHit {
                name: c.qualified_name.clone(),
                restriction: c.kind.clone(),
                comment: c.description.clone().unwrap_or_default(),
            });
        }
        for lib in &self.libraries {
            let LibraryState::Loaded(lib) = lib else {
                continue;
            };
            for c in &lib.classes {
                if !library::short_name(&c.qualified_name)
                    .to_lowercase()
                    .contains(query)
                {
                    continue;
                }
                if hits.len() >= MAX_SEARCH_HITS {
                    return (hits, true);
                }
                hits.push(SearchHit {
                    name: c.qualified_name.clone(),
                    restriction: c.restriction.clone(),
                    comment: c.comment.clone(),
                });
            }
        }
        (hits, false)
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
        self.request_components(class);
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

    /// Ask the worker for the component tree of `class` (once per class).
    ///
    /// This starts an OMC process, so it is only sent when the active class changes or
    /// after a successful Check - never per keystroke.
    pub fn request_components(&mut self, class: &str) {
        if self.components.as_ref().map(|(c, _)| c.as_str()) == Some(class)
            || self.components_pending.as_deref() == Some(class)
        {
            return;
        }
        self.components_pending = Some(class.to_owned());
        self.worker.submit(Job::LoadInstance {
            class: class.to_owned(),
            context: self.context_for(class),
        });
    }

    /// Ask the worker for the Icon graphics of `class` (once per class).
    ///
    /// The Icon layer calls this on every frame it is visible, so an entry is added to
    /// [`Self::icons`] before the job is sent; a failed load is remembered as well and is
    /// only retried after the next Check.
    pub fn request_icon(&mut self, class: &str) {
        if self.icons.contains_key(class) {
            return;
        }
        self.icons.insert(class.to_owned(), Loadable::Loading);
        self.worker.submit(Job::LoadIcon {
            class: class.to_owned(),
            context: self.context_for(class),
        });
    }

    /// Ask the worker for the Diagram graphics of `class` (once per class).
    ///
    /// The Diagram layer calls this on every frame it is visible, so an entry is added to
    /// [`Self::model_diagrams`] before the job is sent; a failed load is remembered as well and is
    /// only retried after the next Check.
    pub fn request_diagram(&mut self, class: &str) {
        if self.model_diagrams.contains_key(class) {
            return;
        }
        self.model_diagrams
            .insert(class.to_owned(), Loadable::Loading);
        self.worker.submit(Job::LoadDiagram {
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
        // The search results depend on the user classes.
        self.package_search_cache = None;
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
        self.start_error_scan();
        self.worker.submit(Job::Check {
            context: self.context_for(&class),
            class: class.clone(),
        });
        // The buffer may have been edited since the last tree was fetched.
        self.components = None;
        self.components_pending = None;
        self.request_components(&class);
        self.icons.remove(&class);
        self.request_icon(&class);
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
        // A half-typed value in the Variable Browser must not silently change the run.
        let ignored: Vec<&str> = self
            .pending_overrides
            .iter()
            .filter(|(_, v)| v.trim().parse::<f64>().is_err())
            .map(|(n, _)| n.as_str())
            .collect();
        if !ignored.is_empty() {
            self.warn(
                LogTab::Simulation,
                format!(
                    "Ignoring overrides that are not numbers: {}",
                    ignored.join(", ")
                ),
            );
        }
        if let Err(e) = omola_core::validate_parameter_overrides(&setup.parameters) {
            return self.error(LogTab::Translation, e.to_string());
        }
        self.log_tab = LogTab::Translation;
        self.bottom_pane = BottomPane::Logs;
        self.start_error_scan();
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

    /// Re-parse the source positions of the current Check / Simulate when the log grew.
    ///
    /// Only the entries written since the current run started are looked at, so a fix and
    /// a re-check clear the markers of the previous run. Runs once per new log entry, not
    /// once per frame.
    fn refresh_error_locations(&mut self) {
        let len = self.logs.len();
        if self.error_locations_log_len == Some(len) {
            return;
        }
        let from = match self.error_locations_log_len {
            // The pane was cleared (G22): parse everything that is left.
            Some(previous) if previous > len => 0,
            _ => self.error_from_log_index.min(len),
        };
        self.error_locations_log_len = Some(len);
        self.error_locations = self.logs[from..]
            .iter()
            .filter(|entry| {
                matches!(
                    entry.tab,
                    LogTab::Syntax | LogTab::Translation | LogTab::Simulation
                )
            })
            .flat_map(|entry| parse_omc_locations(&entry.text))
            .collect();
    }

    /// Forget the locations of the previous run when a new Check / Simulate starts.
    fn start_error_scan(&mut self) {
        self.error_from_log_index = self.logs.len();
        self.error_locations_log_len = None;
        self.error_locations.clear();
    }

    /// Open the document a log location belongs to, show the Text layer and mark the line.
    ///
    /// G23 makes the editor scroll to `highlight_line` and put the caret on it.
    pub fn jump_to_location(&mut self, location: &SourceLocation) {
        // A user model is checked through a private copy named after its class, so the
        // class has to be part of the match.
        let document = (0..self.documents.len()).find(|&i| {
            let class = self.first_class_of(i);
            location_is_for(
                &location.file,
                self.documents[i].path.as_deref(),
                class.as_deref(),
            )
        });
        let class = document.and_then(|i| {
            self.user_classes
                .iter()
                .zip(&self.user_class_doc)
                .find(|(_, doc)| **doc == i)
                .map(|(class, _)| (i, class.qualified_name.clone()))
        });
        let Some((i, class)) = class else {
            let tab = self.log_tab;
            self.info(tab, format!("{} is not open.", location.file));
            return;
        };
        self.open_class(&class);
        self.select_tab(RibbonTab::Text);
        self.documents[i].editor.highlight_line = Some(location.line);
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

    pub fn warn(&mut self, tab: LogTab, text: String) {
        self.logs.push(LogEntry {
            tab,
            level: LogLevel::Warning,
            text,
        });
    }

    /// Seed the Simulation Setup dialog from a class's `experiment` annotation.
    ///
    /// The annotation only provides the initial values: once the user has changed the
    /// general fields by hand (`setup_edited_by_user`) their numbers are kept, and the
    /// annotation of the class already applied (`setup_from`) is not applied twice.
    pub fn apply_experiment(
        &mut self,
        class: &str,
        restriction: &str,
        experiment: Option<library::Experiment>,
    ) {
        let Some(exp) = experiment else {
            return;
        };
        if restriction == "package"
            || self.setup_edited_by_user
            || self.setup_from.as_deref() == Some(class)
        {
            return;
        }
        self.setup.start_time = exp.start_time;
        self.setup.stop_time = exp.stop_time;
        self.setup.tolerance = exp.tolerance;
        self.setup.number_of_intervals = exp.intervals;
        self.setup_from = Some(class.to_owned());
        self.info(
            LogTab::Translation,
            format!(
                "Experiment annotation of {class}: start {} s, stop {} s, {} intervals, tolerance {}",
                exp.start_time, exp.stop_time, exp.intervals, exp.tolerance
            ),
        );
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
                    // The search results depend on the library index.
                    self.package_search_cache = None;
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
                            self.apply_experiment(&class, &doc.restriction, doc.experiment);
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
                    self.model_diagrams.insert(
                        class,
                        match result {
                            Ok(diagram) => Loadable::Ready(diagram),
                            Err(message) => Loadable::Failed(message),
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
        let any_curves = self.diagrams.iter().any(|d| !d.signals.is_empty());
        // Only the very first result is plotted automatically, and only into the active
        // diagram: the first state variable.
        let autoplot = if any_curves {
            None
        } else {
            outcome
                .result
                .variables
                .iter()
                .find(|v| {
                    let leaf = v.name.rsplit('.').next().unwrap_or(&v.name);
                    !v.is_parameter && leaf != "time" && !v.name.contains("der(")
                })
                .map(|v| v.name.clone())
        };
        for diagram in &mut self.diagrams {
            let mut replot: Vec<PlotSignal> = diagram
                .signals
                .iter()
                .filter(|s| Some(s.run_id) == latest_previous)
                .map(|s| PlotSignal {
                    run_id: id,
                    variable: s.variable.clone(),
                })
                .collect();
            if replot.is_empty() {
                if let Some(variable) = &autoplot {
                    replot.push(PlotSignal {
                        run_id: id,
                        variable: variable.clone(),
                    });
                }
            }
            let moving = !replot.is_empty() && latest_previous.is_some();
            diagram
                .signals
                .retain(|s| !(moving && Some(s.run_id) == latest_previous));
            diagram.signals.extend(replot);
        }
        self.runs.push(ResultRun { id, label, outcome });
        while self.runs.len() > self.keep_results.max(1) {
            let removed = self.runs.remove(0);
            self.var_tree_cache.remove(&removed.id);
            for diagram in &mut self.diagrams {
                diagram.signals.retain(|s| s.run_id != removed.id);
            }
        }
        // A new result has its own time axis: show its last sample again.
        self.variable_time = None;
        self.tab = RibbonTab::Simulation;
        self.log_tab = LogTab::Simulation;
    }

    pub fn run(&self, id: u64) -> Option<&ResultRun> {
        self.runs.iter().find(|r| r.id == id)
    }

    /// The variable tree of `run_id` for the current browser filters (G20).
    ///
    /// The tree used to be rebuilt for every result on every frame: all names were
    /// lower-cased, filtered and inserted into a fresh tree. It is now kept until the
    /// filter or the run changes; the expand/collapse state is not part of it.
    pub fn var_tree_for(&mut self, run_id: u64) -> &VarTreeCache {
        let key = VarTreeKey {
            run_id,
            filter: self.variable_filter.trim().to_lowercase(),
            show_parameters: self.variable_show_parameters,
            show_time_varying: self.variable_show_time_varying,
        };
        let stale = self
            .var_tree_cache
            .get(&run_id)
            .is_none_or(|cache| cache.key != key);
        if stale {
            let generation = self
                .var_tree_cache
                .get(&run_id)
                .map_or(1, |cache| cache.generation + 1);
            let (tree, flat) = self.build_var_tree(&key);
            self.var_tree_cache.insert(
                run_id,
                VarTreeCache {
                    key,
                    tree,
                    flat,
                    generation,
                },
            );
        }
        self.var_tree_cache
            .get(&run_id)
            .expect("the tree cache was just filled")
    }

    /// Tree (no filter) or flat list (filter) of the variables of one result.
    fn build_var_tree(&self, key: &VarTreeKey) -> (crate::var_tree::VarNode, Vec<usize>) {
        let Some(run) = self.run(key.run_id) else {
            return (crate::var_tree::VarNode::default(), Vec::new());
        };
        let vars = &run.outcome.result.variables;
        let source = self
            .doc_of(&run.outcome.class)
            .map(|i| self.documents[i].text.as_str());
        let mut names: Vec<(usize, &str)> = Vec::new();
        let mut flat: Vec<usize> = Vec::new();
        for i in variable_order(vars, source) {
            let var = &vars[i];
            if var.name == "time" {
                continue;
            }
            let shown = if var.is_parameter {
                key.show_parameters
            } else {
                key.show_time_varying
            };
            if !shown {
                continue;
            }
            if !key.filter.is_empty() && !var.name.to_lowercase().contains(&key.filter) {
                continue;
            }
            if !key.filter.is_empty() {
                flat.push(i);
            }
            names.push((i, var.name.as_str()));
        }
        if key.filter.is_empty() {
            (crate::var_tree::build_variable_tree(&names), flat)
        } else {
            // A search shows a flat list; the tree is not needed.
            (crate::var_tree::VarNode::default(), flat)
        }
    }

    // ---------------------------------------------------------------- plot diagrams

    /// Curves of the active diagram.
    pub fn active_signals(&self) -> &[PlotSignal] {
        self.diagrams
            .get(self.active_diagram)
            .map(|d| d.signals.as_slice())
            .unwrap_or(&[])
    }

    /// Curves of the active diagram, for the Variable Browser checkboxes.
    pub fn active_signals_mut(&mut self) -> &mut Vec<PlotSignal> {
        if self.diagrams.is_empty() {
            self.diagrams.push(crate::plot::PlotDiagram::default());
        }
        let index = self.active_diagram.min(self.diagrams.len() - 1);
        self.active_diagram = index;
        &mut self.diagrams[index].signals
    }

    /// Append an empty diagram and make it active.
    pub fn new_diagram(&mut self) {
        self.diagrams.push(crate::plot::PlotDiagram::default());
        self.active_diagram = self.diagrams.len() - 1;
    }

    /// Drop the active diagram; the last one is kept so the window is never empty.
    pub fn remove_active_diagram(&mut self) {
        if self.diagrams.len() <= 1 {
            return;
        }
        self.diagrams.remove(self.active_diagram);
        self.active_diagram = self.active_diagram.min(self.diagrams.len() - 1);
    }

    /// Delete every curve of the active diagram.
    pub fn erase_curves(&mut self) {
        self.active_signals_mut().clear();
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

    /// Open a native "Save as" dialog for the plot CSV on a background thread.
    pub fn request_export_csv(&mut self) {
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
    fn diagram_columns(&mut self) -> Option<DiagramColumns> {
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
                DialogResult::ExportCsv(Some(path)) => self.export_csv(&path),
                DialogResult::ExportCsv(None) => {}
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

/// Display order of the variables: declaration order in the user's source when
/// available, else the order the result file lists them.
fn variable_order(vars: &[omola_core::mat::ResultVariable], source: Option<&str>) -> Vec<usize> {
    let mut order: Vec<usize> = (0..vars.len()).collect();
    if let Some(text) = source {
        let key = |i: &usize| -> (usize, usize) {
            let name = &vars[*i].name;
            let (base, is_der) = match name.strip_prefix("der(").and_then(|n| n.strip_suffix(')')) {
                Some(inner) => (inner, 1),
                None => (name.as_str(), 0),
            };
            let leaf = base.split('.').next().unwrap_or(base);
            (find_identifier(text, leaf).unwrap_or(usize::MAX), is_der)
        };
        order.sort_by_key(key);
    }
    order
}

/// Byte offset of `ident` in `text` as a whole word, if it appears.
fn find_identifier(text: &str, ident: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(pos) = text[from..].find(ident) {
        let at = from + pos;
        let before = at.checked_sub(1).map(|i| bytes[i]);
        let after = bytes.get(at + ident.len()).copied();
        let is_ident = |b: Option<u8>| b.is_some_and(|b| b == b'_' || b.is_ascii_alphanumeric());
        if !is_ident(before) && !is_ident(after) {
            return Some(at);
        }
        from = at + ident.len();
    }
    None
}

impl eframe::App for OmolaApp {
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
            app.active_signals(),
            [PlotSignal {
                run_id: 1,
                variable: "height".into()
            }]
        );
        app.add_run(outcome("BouncingBall"));
        app.add_run(outcome("BouncingBall"));
        let ids: Vec<_> = app.runs.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![2, 3]);
        assert!(app.active_signals().iter().all(|s| s.run_id == 3));
        assert_eq!(app.active_signals().len(), 1);
    }

    #[test]
    fn a_new_diagram_takes_its_own_curves_and_the_last_one_stays() {
        let mut app = app();
        app.add_run(outcome("BouncingBall"));
        app.new_diagram();
        assert_eq!(app.active_diagram, 1);
        assert!(
            app.active_signals().is_empty(),
            "a new diagram starts empty"
        );
        // The first diagram keeps its curve.
        assert_eq!(app.diagrams[0].signals.len(), 1);
        app.remove_active_diagram();
        assert_eq!(app.diagrams.len(), 1);
        assert_eq!(app.active_diagram, 0);
        app.remove_active_diagram();
        assert_eq!(app.diagrams.len(), 1, "the last diagram is kept");
    }

    #[test]
    fn removing_a_diagram_keeps_the_active_index_in_range() {
        let mut app = app();
        app.new_diagram();
        app.new_diagram();
        assert_eq!(app.active_diagram, 2);
        app.remove_active_diagram();
        assert_eq!(app.active_diagram, 1);
        assert_eq!(app.diagrams.len(), 2);
    }

    #[test]
    fn a_second_run_rebinds_the_curves_of_every_diagram() {
        let mut app = app();
        app.add_run(outcome("BouncingBall"));
        app.new_diagram();
        app.active_signals_mut().push(PlotSignal {
            run_id: 1,
            variable: "g".into(),
        });
        app.add_run(outcome("BouncingBall"));
        for diagram in &app.diagrams {
            assert_eq!(diagram.signals.len(), 1);
            assert_eq!(
                diagram.signals[0].run_id, 2,
                "both diagrams follow the new run"
            );
        }
        assert_eq!(app.diagrams[1].signals[0].variable, "g");
    }

    #[test]
    fn the_csv_carries_the_curves_of_the_newest_result_only() {
        let mut app = app();
        app.add_run(outcome("BouncingBall"));
        app.new_diagram();
        app.active_signals_mut().push(PlotSignal {
            run_id: 1,
            variable: "g".into(),
        });
        // A stale curve of an older run in the same diagram.
        app.active_signals_mut().push(PlotSignal {
            run_id: 0,
            variable: "height".into(),
        });
        let (time, columns) = app.diagram_columns().expect("columns");
        assert_eq!(time, vec![0.0, 1.0]);
        assert_eq!(columns.len(), 1, "only the newest run is exported");
        assert_eq!(columns[0].0, "g");
        assert!(
            app.logs
                .iter()
                .any(|e| e.level == crate::backend::LogLevel::Warning),
            "mixing results warns"
        );
    }

    fn experiment() -> library::Experiment {
        library::Experiment {
            start_time: 0.5,
            stop_time: 5.0,
            tolerance: 1e-5,
            intervals: 450,
        }
    }

    #[test]
    fn experiment_annotation_seeds_the_setup_once() {
        let mut app = app();
        app.apply_experiment("Annotated", "model", Some(experiment()));
        assert_eq!(app.setup.start_time, 0.5);
        assert_eq!(app.setup.stop_time, 5.0);
        assert_eq!(app.setup.tolerance, 1e-5);
        assert_eq!(app.setup.number_of_intervals, 450);
        assert_eq!(app.setup_from.as_deref(), Some("Annotated"));

        // Another model's annotation replaces the seeded values ...
        app.apply_experiment(
            "Other",
            "model",
            Some(library::Experiment {
                stop_time: 2.0,
                ..experiment()
            }),
        );
        assert_eq!(app.setup.stop_time, 2.0);

        // ... but a package never does, and the same class is not applied twice.
        app.apply_experiment("Modelica.Blocks", "package", Some(experiment()));
        assert_eq!(app.setup.stop_time, 2.0);
        app.apply_experiment("Other", "model", Some(experiment()));
        assert_eq!(app.setup.stop_time, 2.0);
    }

    #[test]
    fn manual_setup_edits_survive_a_later_annotation() {
        let mut app = app();
        app.setup.stop_time = 7.0;
        app.setup_edited_by_user = true;
        app.apply_experiment("Annotated", "model", Some(experiment()));
        assert_eq!(app.setup.stop_time, 7.0);
        assert!(app.setup_from.is_none());
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

    // --- G25 error locations ---

    #[test]
    fn error_locations_are_rebuilt_only_from_the_current_run() {
        let mut app = app();
        app.logs.push(LogEntry {
            tab: LogTab::Syntax,
            level: LogLevel::Error,
            text: "[A.mo:3:1-3:2:writable] Error: old".into(),
        });
        app.refresh_error_locations();
        assert_eq!(app.error_locations.len(), 1);

        // A new Check must not keep the markers of the previous run.
        app.start_error_scan();
        app.logs.push(LogEntry {
            tab: LogTab::Syntax,
            level: LogLevel::Error,
            text: "[A.mo:9:1-9:2:writable] Error: new".into(),
        });
        app.refresh_error_locations();
        assert_eq!(app.error_locations.len(), 1);
        assert_eq!(app.error_locations[0].line, 9);

        // Without a new entry the cache is not rebuilt (and nothing is duplicated).
        app.refresh_error_locations();
        assert_eq!(app.error_locations.len(), 1);
    }

    #[test]
    fn jump_to_location_opens_the_text_layer_on_the_error_line() {
        let dir = std::env::temp_dir().join(format!("omola-g25-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Jump.mo");
        std::fs::write(
            &file,
            "model Jump\n  Real x;\nequation\n  x = ;\nend Jump;\n",
        )
        .unwrap();

        let mut app = app();
        app.open_file(file.clone());
        app.tab = RibbonTab::Graphics;

        // OMC echoes the path it was given, which may use another directory and slashes.
        app.jump_to_location(&SourceLocation {
            file: "C:/elsewhere/Jump.mo".into(),
            line: 4,
            column: 7,
        });
        assert_eq!(app.tab, RibbonTab::Text);
        assert_eq!(app.active.as_deref(), Some("Jump"));
        let doc = app
            .documents
            .iter()
            .find(|d| d.path.as_deref() == Some(file.as_path()))
            .unwrap();
        assert_eq!(doc.editor.highlight_line, Some(4));

        // A file that is not open only produces a message.
        let before = app.logs.len();
        app.jump_to_location(&SourceLocation {
            file: "/nope/Missing.mo".into(),
            line: 1,
            column: 1,
        });
        assert_eq!(app.logs.len(), before + 1);
        assert!(app.logs.last().unwrap().text.contains("is not open"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- G19 package browser ---

    fn library_with(classes: &[&str]) -> LibraryState {
        use crate::library::ClassEntry;
        let entries = classes
            .iter()
            .map(|name| ClassEntry {
                qualified_name: (*name).to_owned(),
                restriction: "package".into(),
                comment: String::new(),
            })
            .collect();
        LibraryState::Loaded(LoadedLibrary::new(
            LibrarySpec::new("Modelica", Some("4.1.0")),
            "4.1.0".into(),
            PathBuf::from("/lib/Modelica/package.mo"),
            entries,
        ))
    }

    #[test]
    fn has_children_of_knows_library_and_user_classes() {
        let mut app = app();
        app.libraries = vec![library_with(&[
            "Modelica",
            "Modelica.Blocks",
            "Modelica.Blocks.Gain",
        ])];
        assert!(app.has_children_of("Modelica"));
        assert!(app.has_children_of("Modelica.Blocks"));
        assert!(!app.has_children_of("Modelica.Blocks.Gain"));

        app.documents.push(Document::new_untitled(
            "package P\n  model M end M;\nend P;\n".to_owned(),
        ));
        app.refresh_outline();
        assert!(app.has_children_of("P"));
        assert!(!app.has_children_of("P.M"));
        assert!(!app.has_children_of("Nope"));
    }

    #[test]
    fn the_search_is_rebuilt_only_when_the_query_changes() {
        let mut app = app();
        app.libraries = vec![library_with(&["Modelica", "Modelica.Blocks.Gain"])];
        let generation = |app: &OmolaApp| app.package_search_cache.as_ref().unwrap().generation;

        assert_eq!(app.search_hits("gain").hits.len(), 1);
        assert_eq!(generation(&app), 1);
        // The same query twice does not rebuild the list.
        assert_eq!(app.search_hits("  Gain ").hits.len(), 1);
        assert_eq!(generation(&app), 1, "the query was rebuilt");
        // A different query does.
        app.search_hits("blocks");
        assert_eq!(generation(&app), 2);
        assert_eq!(app.search_hits("nothing-matches").hits.len(), 0);

        // A changed outline (a user file was edited) invalidates the cached hits.
        app.refresh_outline();
        assert!(app.package_search_cache.is_none());
    }

    #[test]
    fn the_search_stops_at_five_hundred_hits() {
        let mut app = app();
        let names: Vec<String> = (0..MAX_SEARCH_HITS + 20)
            .map(|i| format!("Modelica.M{i}"))
            .collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        app.libraries = vec![library_with(&names)];
        let hits = app.search_hits("m");
        assert_eq!(hits.hits.len(), MAX_SEARCH_HITS);
        assert!(hits.truncated, "the list says it was cut");
    }

    // --- G20 variable browser cache ---

    /// A result with the variables the variable browser tests use.
    fn inertia_outcome() -> SimulationOutcome {
        let var = |name: &str, p: bool| ResultVariable {
            name: name.into(),
            description: String::new(),
            is_parameter: p,
        };
        SimulationOutcome {
            class: "Inertia".into(),
            result_file: PathBuf::from("x_res.mat"),
            result: TrajectoryResult {
                variables: vec![
                    var("time", false),
                    var("inertia1.w", false),
                    var("der(inertia1.w)", false),
                    var("inertia1.J", true),
                    var("inertia2.w", false),
                ],
                time: vec![0.0, 1.0],
                values: vec![vec![0.0, 1.0]; 5],
            },
            elapsed_seconds: 0.0,
        }
    }

    #[test]
    fn the_variable_tree_is_built_once_per_state() {
        let mut app = app();
        app.add_run(inertia_outcome());
        let id = app.runs[0].id;
        assert_eq!(app.var_tree_for(id).generation, 1);
        assert_eq!(
            app.var_tree_for(id).generation,
            1,
            "rebuilt without a change"
        );

        // A new filter rebuilds it.
        app.variable_filter = "w".into();
        assert_eq!(app.var_tree_for(id).generation, 2);
        // Toggling parameters away rebuilds it too.
        app.variable_show_parameters = false;
        assert_eq!(app.var_tree_for(id).generation, 3);
        // Whitespace around the same filter is not a change.
        app.variable_filter = "  w ".into();
        assert_eq!(app.var_tree_for(id).generation, 3);
    }

    #[test]
    fn a_filtered_tree_lists_the_matching_variables() {
        let mut app = app();
        app.add_run(inertia_outcome());
        let id = app.runs[0].id;
        app.variable_filter = "w".into();
        let indices = app.var_tree_for(id).flat.clone();
        let names: Vec<String> = indices
            .iter()
            .map(|&i| app.runs[0].outcome.result.variables[i].name.clone())
            .collect();
        assert_eq!(names, ["inertia1.w", "der(inertia1.w)", "inertia2.w"]);

        // A parameter filter without the Parameters toggle stays empty.
        app.variable_show_parameters = false;
        app.variable_filter = "j".into();
        assert!(app.var_tree_for(id).flat.is_empty());
        // `time` is never listed.
        app.variable_filter = "time".into();
        assert!(app.var_tree_for(id).flat.is_empty());
    }

    #[test]
    fn dropping_a_result_drops_its_tree() {
        let mut app = app();
        app.keep_results = 1;
        app.add_run(inertia_outcome());
        let oldest = app.runs[0].id;
        app.var_tree_for(oldest);
        app.add_run(inertia_outcome());
        let newest = app.runs[0].id;
        app.var_tree_for(newest);
        assert!(!app.var_tree_cache.contains_key(&oldest));
        assert!(app.var_tree_cache.contains_key(&newest));
    }

    #[test]
    fn identifiers_match_whole_words() {
        let text = "parameter Real m=1; parameter Real mm=2; Real phi;";
        assert_eq!(find_identifier(text, "m"), Some(15));
        assert_eq!(find_identifier(text, "phi"), Some(46));
        assert_eq!(find_identifier(text, "x"), None);
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
        let lib = LoadedLibrary::new(
            spec.clone(),
            "4.1.0".into(),
            source,
            vec![
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
        );
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
