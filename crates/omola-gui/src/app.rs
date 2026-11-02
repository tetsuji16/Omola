//! Top-level application state and the per-frame layout.
//!
//! Layout follows the familiar Modelica-workbench arrangement (see
//! `docs/reference/dymola-gui.md`): menu bar, ribbon tabs with a context toolbar,
//! browsers on the left, the edit/plot window in the centre, messages at the bottom.

use crate::backend::{Event, Job, LogLevel, SimulationOutcome, SimulationSetup, Worker};
use crate::model::{default_class, scan_classes, ClassEntry, Document};
use std::path::PathBuf;

/// Built-in sample shown when the GUI starts without a file argument.
pub const SAMPLE_MODEL: &str = include_str!("../../../examples/BouncingBall.mo");

/// Ribbon tabs. Graphics/Documentation/Text are the modeling layers; Simulation shows
/// the variable browser and plot window.
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

    pub fn is_modeling(self) -> bool {
        !matches!(self, RibbonTab::Simulation)
    }
}

/// Diagram or Icon layer inside the Graphics tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsLayer {
    Diagram,
    Icon,
}

/// One kept simulation result, shown as a top-level node in the variable browser.
pub struct ResultRun {
    pub id: u64,
    /// e.g. `BouncingBall 1`, `BouncingBall 2` (newest has the highest number).
    pub label: String,
    pub outcome: SimulationOutcome,
}

/// A curve in the plot window: (run id, variable name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlotSignal {
    pub run_id: u64,
    pub variable: String,
}

/// Small text-input dialogs (until native file dialogs land, issue G03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathDialog {
    Open(String),
    SaveAs(String),
}

pub struct OmolaApp {
    pub tab: RibbonTab,
    pub graphics_layer: GraphicsLayer,
    pub document: Document,
    pub classes: Vec<ClassEntry>,
    pub selected_class: Option<String>,
    pub setup: SimulationSetup,
    pub show_setup: bool,
    pub path_dialog: Option<PathDialog>,
    pub runs: Vec<ResultRun>,
    pub next_run_id: u64,
    /// How many results the variable browser keeps (Dymola keeps the last two by default).
    pub keep_results: usize,
    pub variable_filter: String,
    pub plotted: Vec<PlotSignal>,
    pub log: Vec<(LogLevel, String)>,
    pub busy: bool,
    pub omc_version: Option<String>,
    pub worker: Worker,
}

impl OmolaApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_file: Option<PathBuf>) -> Self {
        let ctx = cc.egui_ctx.clone();
        let worker = Worker::spawn(crate::backend::default_work_root(), move || {
            ctx.request_repaint()
        });
        let mut app = Self::with_worker(worker);
        if let Some(path) = initial_file {
            app.open_file(path);
        }
        app.worker.submit(Job::Doctor);
        app
    }

    /// Construct without a window (used by tests and `new`).
    pub fn with_worker(worker: Worker) -> Self {
        let document = Document::new_untitled(SAMPLE_MODEL);
        let mut app = Self {
            tab: RibbonTab::Text,
            graphics_layer: GraphicsLayer::Diagram,
            document,
            classes: Vec::new(),
            selected_class: None,
            setup: SimulationSetup {
                stop_time: 3.0,
                ..SimulationSetup::default()
            },
            show_setup: false,
            path_dialog: None,
            runs: Vec::new(),
            next_run_id: 1,
            keep_results: 2,
            variable_filter: String::new(),
            plotted: Vec::new(),
            log: vec![(
                LogLevel::Info,
                "Omola GUI started. Calculations are delegated to OpenModelica (omc).".into(),
            )],
            busy: false,
            omc_version: None,
            worker,
        };
        app.refresh_outline();
        app
    }

    pub fn open_file(&mut self, path: PathBuf) {
        match Document::open(path.clone()) {
            Ok(doc) => {
                self.document = doc;
                self.selected_class = None;
                self.refresh_outline();
                self.info(format!("Opened {}", path.display()));
            }
            Err(e) => self.error(format!("Could not open {}: {e}", path.display())),
        }
    }

    pub fn save(&mut self) {
        if self.document.path.is_none() {
            self.path_dialog = Some(PathDialog::SaveAs(String::new()));
            return;
        }
        match self.document.save() {
            Ok(()) => self.info(format!("Saved {}", self.document.title())),
            Err(e) => self.error(format!("Save failed: {e}")),
        }
    }

    /// Re-scan the class outline after the text changed.
    pub fn refresh_outline(&mut self) {
        self.classes = scan_classes(&self.document.text);
        let still_valid = self
            .selected_class
            .as_ref()
            .is_some_and(|c| self.classes.iter().any(|e| &e.qualified_name == c));
        if !still_valid {
            self.selected_class = default_class(&self.classes);
        }
    }

    pub fn check(&mut self) {
        let Some(class) = self.selected_class.clone() else {
            return self.error("No model class found in the edit window.".into());
        };
        self.worker.submit(Job::Check {
            source_text: self.document.text.clone(),
            class,
        });
    }

    pub fn simulate(&mut self) {
        let Some(class) = self.selected_class.clone() else {
            return self.error("No model class found in the edit window.".into());
        };
        self.worker.submit(Job::Simulate {
            source_text: self.document.text.clone(),
            class,
            setup: self.setup.clone(),
        });
    }

    pub fn info(&mut self, message: String) {
        self.log.push((LogLevel::Info, message));
    }

    pub fn error(&mut self, message: String) {
        self.log.push((LogLevel::Error, message));
    }

    /// Apply worker events. Called once per frame.
    pub fn handle_events(&mut self) {
        for event in self.worker.poll() {
            match event {
                Event::Log(level, message) => self.log.push((level, message)),
                Event::OmcVersion(version) => self.omc_version = version,
                Event::Busy(busy) => self.busy = busy,
                Event::SimulationFinished(outcome) => self.add_run(*outcome),
            }
        }
    }

    /// Store a new result, drop the oldest beyond `keep_results`, and plot like Dymola does
    /// on first run (the first state-like variable) when nothing is plotted yet.
    pub fn add_run(&mut self, outcome: SimulationOutcome) {
        let id = self.next_run_id;
        self.next_run_id += 1;
        let label = format!("{} {}", outcome.class, id);
        // Re-point existing curves of the same class to the new run (Dymola replots).
        let previous: Vec<u64> = self
            .runs
            .iter()
            .filter(|r| r.outcome.class == outcome.class)
            .map(|r| r.id)
            .collect();
        let latest_previous = previous.iter().max().copied();
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
    }

    pub fn run(&self, id: u64) -> Option<&ResultRun> {
        self.runs.iter().find(|r| r.id == id)
    }
}

impl eframe::App for OmolaApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_events();
        self.handle_shortcuts(ui);

        egui::Panel::top("menu_bar").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("ribbon").show(ui, |ui| self.ribbon(ui));
        egui::Panel::bottom("status_bar").show(ui, |ui| self.status_bar(ui));
        egui::Panel::bottom("messages")
            .resizable(true)
            .default_size(150.0)
            .show(ui, |ui| self.messages_window(ui));
        if self.tab.is_modeling() {
            egui::Panel::left("browsers")
                .resizable(true)
                .default_size(260.0)
                .show(ui, |ui| self.modeling_browsers(ui));
            egui::CentralPanel::default().show(ui, |ui| self.edit_window(ui));
        } else {
            egui::Panel::left("variable_browser")
                .resizable(true)
                .default_size(320.0)
                .show(ui, |ui| self.variable_browser(ui));
            egui::CentralPanel::default().show(ui, |ui| self.plot_window(ui));
        }
        self.dialogs(ui);
    }
}

impl OmolaApp {
    fn handle_shortcuts(&mut self, ui: &egui::Ui) {
        use egui::{Key, KeyboardShortcut, Modifiers};
        let save = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
        let simulate = KeyboardShortcut::new(Modifiers::NONE, Key::F9);
        if ui.input_mut(|i| i.consume_shortcut(&save)) {
            self.save();
        }
        if !self.busy && ui.input_mut(|i| i.consume_shortcut(&simulate)) {
            self.simulate();
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
    fn sample_model_selects_bouncing_ball() {
        assert_eq!(app().selected_class.as_deref(), Some("BouncingBall"));
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
        assert_eq!(app.runs[1].label, "BouncingBall 3");
        // Curves follow the newest run of the same class.
        assert!(app.plotted.iter().all(|s| s.run_id == 3));
        assert_eq!(app.plotted.len(), 1);
    }
}
