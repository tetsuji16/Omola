//! Background worker: every OMC call runs on one worker thread so the UI never blocks.
//!
//! The UI sends [`Job`]s and polls [`Event`]s once per frame. The worker writes the
//! editor buffer to a private run directory, so user files are never modified by a run.

use crate::edit::{self, EditOp};
use crate::library::{self, ClassDoc, LibrarySpec, LoadedLibrary};
use omola_core::{mat, Omc, SimulationConfig};
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread,
    time::Instant,
};

/// Settings from the Simulation Setup dialog.
///
/// Only `stop_time` and `parameters` reach OMC today; see `docs/issues/gui/` for the
/// issues that wire the remaining fields into `omola-core`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SimulationSetup {
    pub start_time: f64,
    pub stop_time: f64,
    pub number_of_intervals: u32,
    pub tolerance: f64,
    pub method: String,
    /// Parameter overrides `name=value` applied with OMC `-override`.
    pub parameters: Vec<(String, String)>,
}

impl SimulationSetup {
    /// Reject settings OMC would only fail on after a long translation (issue G44).
    ///
    /// The message is shown as-is in the Translation log and in the Simulation Setup
    /// dialog, so it names the offending field.
    pub fn validate(&self) -> Result<(), String> {
        if !self.start_time.is_finite() || !self.stop_time.is_finite() {
            return Err("Start time and stop time must be finite numbers.".into());
        }
        if self.stop_time <= self.start_time {
            return Err(format!(
                "Stop time ({} s) must be greater than start time ({} s).",
                self.stop_time, self.start_time
            ));
        }
        if self.number_of_intervals == 0 {
            return Err("Number of intervals must be at least 1.".into());
        }
        if !self.tolerance.is_finite() || self.tolerance <= 0.0 {
            return Err("Tolerance must be a positive number.".into());
        }
        if !omola_core::SIMULATION_METHODS.contains(&self.method.as_str()) {
            return Err(format!(
                "Unknown integration method \"{}\". Supported: {}.",
                self.method,
                omola_core::SIMULATION_METHODS.join(", ")
            ));
        }
        Ok(())
    }
}

/// Class name shown for a result file loaded from disk (issue G14).
///
/// OpenModelica writes `<Class>_res.mat`; other tools write `<name>.mat`. The suffix is
/// stripped so `BouncingBall_res.mat` is listed as `BouncingBall`. Never empty.
pub fn result_class_from_path(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    let stem = if lower.ends_with("_res.mat") {
        &name[..name.len() - "_res.mat".len()]
    } else if lower.ends_with(".mat") {
        &name[..name.len() - ".mat".len()]
    } else {
        name.as_str()
    };
    if stem.is_empty() {
        "result".to_owned()
    } else {
        stem.to_owned()
    }
}

/// Read a result file from disk and publish it like a finished simulation (issue G14).
///
/// Needs no OMC: the MAT v4 reader lives in `omola-core`.
fn load_result(path: &Path, send: &dyn Fn(Event)) {
    let started = Instant::now();
    match mat::read_trajectory_file(path) {
        Ok(trajectory) => {
            send(Event::Log(
                LogTab::Simulation,
                LogLevel::Info,
                format!(
                    "Loaded result \"{}\"\n... {} variables, {} result points",
                    path.display(),
                    trajectory.variables.len(),
                    trajectory.time.len()
                ),
            ));
            send(Event::SimulationFinished(Box::new(SimulationOutcome {
                class: result_class_from_path(path),
                result_file: path.to_path_buf(),
                result: trajectory,
                elapsed_seconds: started.elapsed().as_secs_f64(),
            })));
        }
        Err(error) => send(Event::Log(
            LogTab::Simulation,
            LogLevel::Error,
            format!("Could not load result {}: {error}", path.display()),
        )),
    }
}

#[cfg(test)]
mod edit_integration_test;

impl Default for SimulationSetup {
    fn default() -> Self {
        Self {
            start_time: 0.0,
            stop_time: 1.0,
            number_of_intervals: 500,
            tolerance: 1e-4,
            method: "dassl".into(),
            parameters: Vec::new(),
        }
    }
}

/// Where a class comes from: a loaded library, or the user's editor buffer.
#[derive(Debug, Clone)]
pub struct ClassContext {
    pub libraries: Vec<LibrarySpec>,
    /// Editor buffer that defines (or uses) the class; written to a private copy.
    pub user_source: Option<String>,
    /// Other open buffers, kept as distinct stored definitions.
    pub other_sources: Vec<String>,
}

pub enum Job {
    /// Locate OMC and report its version.
    Doctor,
    LoadLibrary(LibrarySpec),
    LoadClassDoc {
        class: String,
        context: ClassContext,
    },
    LoadClassText {
        class: String,
        context: ClassContext,
    },
    /// Component tree of a class, from `getModelInstance` (G06).
    LoadInstance {
        class: String,
        context: ClassContext,
    },
    LoadParameters {
        component_type: String,
        class: String,
        component: String,
        context: ClassContext,
    },
    /// Icon graphics of a class, from `getModelInstanceAnnotation` (G07).
    LoadIcon {
        class: String,
        context: ClassContext,
    },
    /// Diagram graphics + component placement + connections, from instance JSON + icon annotations (G12).
    LoadDiagram {
        class: String,
        context: ClassContext,
        cached_icons: std::collections::BTreeMap<String, crate::graphics::Icon>,
    },
    Check {
        class: String,
        context: ClassContext,
    },
    /// Build the model (`buildModel`) without running it (G46).
    Translate {
        class: String,
        context: ClassContext,
    },
    /// Read an existing result file (`*_res.mat`) into the Variable Browser (G14).
    LoadResult {
        path: PathBuf,
    },
    /// Copy a result file to a user-chosen destination off the UI thread (G14, G53).
    CopyResult {
        source: PathBuf,
        dest: PathBuf,
    },
    Simulate {
        class: String,
        context: ClassContext,
        setup: SimulationSetup,
    },
    /// Install the start-up libraries with OpenModelica's package manager
    /// (`scripts/install-libraries.mos`).
    InstallLibraries,
    /// A line typed into the command window, run as an OMC script with libraries loaded.
    Command {
        text: String,
        libraries: Vec<LibrarySpec>,
    },
    EditModel {
        doc_text: String,
        file_name: String,
        class: String,
        ops: Vec<EditOp>,
        libraries: Vec<LibrarySpec>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

/// Log tabs of the Logs window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogTab {
    Syntax,
    Translation,
    Simulation,
    Version,
}

impl LogTab {
    pub const ALL: [LogTab; 4] = [
        LogTab::Syntax,
        LogTab::Translation,
        LogTab::Simulation,
        LogTab::Version,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LogTab::Syntax => "Syntax",
            LogTab::Translation => "Translation",
            LogTab::Simulation => "Simulation",
            LogTab::Version => "Version",
        }
    }
}

/// A finished simulation, ready for the variable browser and plot window.
pub struct SimulationOutcome {
    pub class: String,
    pub result_file: PathBuf,
    pub result: mat::TrajectoryResult,
    pub elapsed_seconds: f64,
}

pub enum Event {
    Log(LogTab, LogLevel, String),
    /// Output of a command-window line (echoed command first).
    Command(String),
    OmcVersion(Option<String>),
    /// A job started (`true`) or finished (`false`); drives the busy indicator.
    Busy(bool),
    LibraryLoaded(LibrarySpec, Result<LoadedLibrary, String>),
    ClassDoc(String, Result<ClassDoc, String>),
    ClassText(String, Result<String, String>),
    /// Component tree of a class (empty when the class declares no components).
    Instance(String, Result<Vec<crate::instance::ComponentNode>, String>),
    Parameters {
        component: String,
        result: Result<Vec<crate::instance::ParamRow>, String>,
    },
    /// Icon graphics of a class; `Err` when the class has no usable Icon annotation.
    Icon(String, Result<crate::graphics::Icon, String>),
    /// Diagram graphics + component placement + connections (G12).
    Diagram(String, Result<crate::diagram::Diagram, String>),
    SimulationFinished(Box<SimulationOutcome>),
    /// A `Simulate` job of this class ended without a result (error or Stop). Every
    /// `Simulate` job ends in exactly one of `SimulationFinished` / `SimulationFailed`,
    /// which is what a parameter sweep waits for (G49).
    SimulationFailed(String),
    /// `InstallLibraries` finished (`true` when the Modelica Standard Library is installed).
    LibrariesInstalled(bool),
    /// A `CopyResult` job finished; `dest` is the file the user chose.
    ResultCopied {
        dest: PathBuf,
        result: Result<(), String>,
    },
    ModelEdited {
        class: String,
        result: Result<String, String>,
    },
}

/// How deep the Component Browser follows the inlined type definitions of a component.
const INSTANCE_DEPTH: usize = 4;

pub struct Worker {
    jobs: Sender<Job>,
    events: Receiver<Event>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Worker {
    /// Spawn the worker. `wake` is called after every event so the UI repaints.
    pub fn spawn(work_root: PathBuf, wake: impl Fn() + Send + 'static) -> Self {
        let (jobs, job_rx) = mpsc::channel::<Job>();
        let (event_tx, events) = mpsc::channel::<Event>();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel_worker = cancel.clone();
        thread::Builder::new()
            .name("omola-omc-worker".into())
            .spawn(move || {
                // Old work folders are removed here, never on the UI thread (G50). A work
                // root chosen by the caller (tests) is left alone.
                let _session_lock = if work_root == default_work_root() {
                    lock_session_dir(&work_root)
                } else {
                    None
                };
                if work_root == default_work_root() {
                    let removed = work_root
                        .parent()
                        .map_or(0, |root| remove_stale_sessions(root, &work_root));
                    if removed > 0 {
                        let _ = event_tx.send(Event::Log(
                            LogTab::Translation,
                            LogLevel::Info,
                            format!("Removed {removed} old work folders."),
                        ));
                        wake();
                    }
                }
                let mut run_counter = 0u32;
                for job in job_rx {
                    cancel_worker.store(false, std::sync::atomic::Ordering::Relaxed);
                    let send = |event: Event| {
                        let _ = event_tx.send(event);
                        wake();
                    };
                    send(Event::Busy(true));
                    run_counter += 1;
                    let run_dir = work_root.join(format!("run-{run_counter}"));
                    run_job(job, &run_dir, &cancel_worker, &send);
                    send(Event::Busy(false));
                }
            })
            .expect("spawn OMC worker thread");
        Self {
            jobs,
            events,
            cancel,
        }
    }

    pub fn submit(&self, job: Job) {
        let _ = self.jobs.send(job);
    }

    pub fn try_submit(&self, job: Job) -> bool {
        self.jobs.send(job).is_ok()
    }

    pub fn cancel(&self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Drain all pending events without blocking.
    pub fn poll(&self) -> Vec<Event> {
        let mut out = Vec::new();
        loop {
            match self.events.try_recv() {
                Ok(event) => out.push(event),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return out,
            }
        }
    }
}

/// Versioned libraries as `(name, version)` pairs for `omola-core` (unversioned are skipped).
fn pinned(libraries: &[LibrarySpec]) -> Vec<(String, String)> {
    libraries
        .iter()
        .filter_map(|l| l.version.clone().map(|v| (l.name.clone(), v)))
        .collect()
}

fn run_job(
    job: Job,
    run_dir: &Path,
    cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    send: &dyn Fn(Event),
) {
    // Reading a result file needs no OMC, so it also works before OpenModelica is set up.
    let job = match job {
        Job::LoadResult { path } => return load_result(&path, send),
        Job::CopyResult { source, dest } => {
            let result = copy_result_file(&source, &dest, true);
            return send(Event::ResultCopied { dest, result });
        }
        other => other,
    };
    let omc = match Omc::discover().map(|omc| omc.with_cancel_flag(cancel.clone())) {
        Ok(omc) => omc,
        Err(error) => {
            send(Event::OmcVersion(None));
            if !matches!(job, Job::LoadLibrary(_)) {
                send(Event::Log(
                    LogTab::Version,
                    LogLevel::Error,
                    error.to_string(),
                ));
            }
            fail_job(job, error.to_string(), send);
            return;
        }
    };
    match job {
        Job::Doctor => match omc.version() {
            Ok(version) => {
                send(Event::Log(
                    LogTab::Version,
                    LogLevel::Info,
                    format!(
                        "Backend: {}\nCompiler: {}",
                        version,
                        omc.executable().display()
                    ),
                ));
                send(Event::OmcVersion(Some(version)));
            }
            Err(error) => {
                send(Event::OmcVersion(None));
                send(Event::Log(
                    LogTab::Version,
                    LogLevel::Error,
                    error.to_string(),
                ));
            }
        },
        Job::LoadLibrary(spec) => {
            let result = library::library_index_script(&spec)
                .map_err(|e| e.to_string())
                .and_then(|script| omc.run_query_script(&script).map_err(|e| e.to_string()))
                .and_then(|out| library::parse_library_index(&spec, &out.output));
            match &result {
                Ok(lib) => {
                    // Best effort: a read-only cache directory must not fail the load.
                    let _ = crate::cache::store(&crate::cache::cache_root(), lib);
                    send(Event::Log(
                        LogTab::Version,
                        LogLevel::Info,
                        format!(
                            "Loaded {} {} ({} classes) from {}",
                            spec.name,
                            lib.version,
                            lib.classes.len(),
                            lib.source_file.display()
                        ),
                    ))
                }
                Err(message) => send(Event::Log(
                    LogTab::Version,
                    LogLevel::Warning,
                    message.clone(),
                )),
            }
            send(Event::LibraryLoaded(spec, result));
        }
        Job::LoadClassDoc { class, context } => {
            let result = with_user_files(run_dir, &class, &context, |file| {
                let script =
                    library::class_doc_script_with_user_files(&class, &context.libraries, file)
                        .map_err(|e| e.to_string())?;
                let out = omc.run_query_script(&script).map_err(|e| e.to_string())?;
                library::parse_class_doc(&class, &out.output)
            });
            send(Event::ClassDoc(class, result));
        }
        Job::LoadClassText { class, context } => {
            let result = library::class_text_script(&class, &context.libraries)
                .map_err(|e| e.to_string())
                .and_then(|script| omc.run_query_script(&script).map_err(|e| e.to_string()))
                .and_then(|out| {
                    library::parse_sections(&out.output)
                        .remove("text")
                        .ok_or_else(|| format!("OMC returned no text for {class}"))
                });
            send(Event::ClassText(class, result));
        }
        Job::LoadInstance { class, context } => {
            let result = with_user_files(run_dir, &class, &context, |file| {
                let script =
                    library::instance_script_with_user_files(&class, &context.libraries, file)
                        .map_err(|e| e.to_string())?;
                omc.run_query_script(&script)
                    .map(|out| out.output)
                    .map_err(|e| e.to_string())
            })
            .and_then(|output| {
                let sections = library::parse_sections(&output);
                let body = sections
                    .get("instance")
                    .ok_or_else(|| format!("OMC returned no instance JSON for {class}"))?;
                let json = crate::instance::extract_json(body)
                    .ok_or_else(|| format!("OMC returned no instance JSON for {class}"))?;
                crate::instance::parse_components(json, INSTANCE_DEPTH)
            });
            send(Event::Instance(class, result));
        }
        Job::LoadParameters {
            component_type,
            class,
            component,
            context,
        } => {
            let result = with_user_files(run_dir, &class, &context, |file| {
                let type_script = library::instance_script_with_user_files(
                    &component_type,
                    &context.libraries,
                    file,
                )
                .map_err(|e| e.to_string())?;
                let type_output = omc
                    .run_query_script(&type_script)
                    .map_err(|e| e.to_string())?
                    .output;
                let type_json = parse_instance_value(&type_output, &component_type)?;
                let class_script =
                    library::instance_script_with_user_files(&class, &context.libraries, file)
                        .map_err(|e| e.to_string())?;
                let class_output = omc
                    .run_query_script(&class_script)
                    .map_err(|e| e.to_string())?
                    .output;
                let class_json = parse_instance_value(&class_output, &class)?;
                let component_json = find_instance_component(&class_json, &component);
                Ok(crate::instance::parse_parameter_rows(
                    &type_json,
                    component_json,
                ))
            });
            send(Event::Parameters { component, result });
        }
        Job::LoadIcon { class, context } => {
            let result = with_user_files(run_dir, &class, &context, |file| {
                let script = library::icon_script_with_user_files(&class, &context.libraries, file)
                    .map_err(|e| e.to_string())?;
                omc.run_query_script(&script)
                    .map(|out| out.output)
                    .map_err(|e| e.to_string())
            })
            .and_then(|output| {
                let sections = library::parse_sections(&output);
                let body = sections.get("icon").map(|s| s.trim()).unwrap_or("");
                // OMC prints nothing at all when the class declares no Icon anywhere in its
                // `extends` chain.
                let json = crate::instance::extract_json(body)
                    .ok_or_else(|| format!("{class} declares no Icon annotation"))?;
                let value: serde_json::Value = serde_json::from_str(json)
                    .map_err(|e| format!("could not read the Icon JSON of {class}: {e}"))?;
                crate::graphics::parse_icon(&value)
            });
            if let Ok(icon) = &result {
                if !icon.skipped.is_empty() {
                    // One warning per load, however many graphics were dropped.
                    send(Event::Log(
                        LogTab::Translation,
                        LogLevel::Warning,
                        format!(
                            "Icon of {class}: skipped {} graphic(s) that could not be read:\n{}",
                            icon.skipped.len(),
                            icon.skipped.join("\n")
                        ),
                    ));
                }
            }
            send(Event::Icon(class, result));
        }
        Job::LoadDiagram {
            class,
            context,
            cached_icons,
        } => {
            // First load the instance JSON to get component placements and connections
            let instance_result = with_user_files(run_dir, &class, &context, |file| {
                let script =
                    library::instance_script_with_user_files(&class, &context.libraries, file)
                        .map_err(|e| e.to_string())?;
                omc.run_query_script(&script)
                    .map(|out| out.output)
                    .map_err(|e| e.to_string())
            });

            let instance_json = match instance_result {
                Ok(output) => {
                    let sections = library::parse_sections(&output);
                    let body = match sections
                        .get("instance")
                        .ok_or_else(|| format!("OMC returned no instance JSON for {class}"))
                    {
                        Ok(b) => b,
                        Err(e) => {
                            send(Event::Diagram(class.clone(), Err(e)));
                            return;
                        }
                    };
                    match crate::instance::extract_json(body)
                        .ok_or_else(|| format!("OMC returned no instance JSON for {class}"))
                    {
                        Ok(j) => j.to_owned(),
                        Err(e) => {
                            send(Event::Diagram(class.clone(), Err(e)));
                            return;
                        }
                    }
                }
                Err(e) => {
                    send(Event::Diagram(class.clone(), Err(e)));
                    return;
                }
            };

            // Parse the instance JSON to get component types
            let instance_value: serde_json::Value = match serde_json::from_str(&instance_json) {
                Ok(v) => v,
                Err(e) => {
                    send(Event::Diagram(
                        class.clone(),
                        Err(format!("could not parse instance JSON: {e}")),
                    ));
                    return;
                }
            };

            // Collect all component types that need icons
            let component_types = crate::diagram::placed_component_types(&instance_value);

            // Load icons for all component types
            let mut icons = cached_icons;
            let types: Vec<String> = component_types
                .into_iter()
                .filter(|name| !icons.contains_key(name))
                .collect();
            if types.is_empty() {
                let diagram = crate::diagram::parse_diagram(&instance_value, &icons);
                send(Event::Diagram(class, diagram));
                return;
            }
            let loaded = with_user_files(run_dir, &class, &context, |file| {
                let script =
                    library::icons_script_with_user_files(&types, &context.libraries, file)
                        .map_err(|e| e.to_string())?;
                omc.run_query_script(&script).map_err(|e| e.to_string())
            });
            match loaded {
                Ok(out) => {
                    let sections = library::parse_sections(&out.output);
                    for type_name in types {
                        let key = format!("icon:{type_name}");
                        let parsed = sections
                            .get(&key)
                            .and_then(|body| crate::instance::extract_json(body))
                            .ok_or_else(|| "OMC returned no icon JSON section".to_owned())
                            .and_then(|json| {
                                serde_json::from_str::<serde_json::Value>(json)
                                    .map_err(|e| e.to_string())
                            })
                            .and_then(|value| {
                                crate::graphics::parse_icon(&value).map_err(|e| e.to_string())
                            });
                        match parsed {
                            Ok(icon) => {
                                icons.insert(type_name, icon);
                            }
                            Err(reason) => send(Event::Log(
                                LogTab::Simulation,
                                LogLevel::Warning,
                                format!("icon of {type_name}: {reason}"),
                            )),
                        }
                    }
                }
                Err(reason) => {
                    for type_name in types {
                        send(Event::Log(
                            LogTab::Simulation,
                            LogLevel::Warning,
                            format!("icon of {type_name}: {reason}"),
                        ));
                    }
                }
            }

            // Now parse the diagram with the icons
            let diagram = crate::diagram::parse_diagram(&instance_value, &icons);
            send(Event::Diagram(class, diagram));
        }
        Job::Check { class, context } => {
            send(Event::Log(
                LogTab::Syntax,
                LogLevel::Info,
                format!("checkModel(\"{class}\")"),
            ));
            let result = with_user_files(run_dir, &class, &context, |file| {
                let script =
                    library::check_script_with_user_files(&class, &context.libraries, file)
                        .map_err(|e| e.to_string())?;
                omc.run_query_script(&script)
                    .map(|out| out.output)
                    .map_err(|e| e.to_string())
            });
            match result {
                Ok(output) => {
                    let sections = library::parse_sections(&output);
                    let check = sections.get("check").map(|s| s.trim()).unwrap_or("");
                    let errors = sections.get("errors").map(|s| s.trim()).unwrap_or("");
                    let ok = check.contains("completed successfully");
                    let text = [check, errors]
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n");
                    send(Event::Log(
                        LogTab::Syntax,
                        if ok { LogLevel::Info } else { LogLevel::Error },
                        if text.is_empty() {
                            format!("Check of {class} produced no output.")
                        } else {
                            text
                        },
                    ));
                }
                Err(message) => send(Event::Log(LogTab::Syntax, LogLevel::Error, message)),
            }
        }
        Job::Simulate {
            class,
            context,
            setup,
        } => {
            let finished = std::cell::Cell::new(false);
            let name = class.clone();
            simulate(&omc, run_dir, class, context, setup, &|event| {
                if matches!(event, Event::SimulationFinished(_)) {
                    finished.set(true);
                }
                send(event);
            });
            if !finished.get() {
                send(Event::SimulationFailed(name));
            }
        }
        Job::Translate { class, context } => translate(&omc, run_dir, &class, &context, send),
        // Handled before OMC discovery above.
        Job::LoadResult { path } => load_result(&path, send),
        Job::CopyResult { .. } => unreachable!("copy handled before OMC discovery"),
        Job::InstallLibraries => {
            send(Event::Log(
                LogTab::Version,
                LogLevel::Info,
                "Installing Modelica Standard Library 4.1.0 and ModelicaReference with the OpenModelica package manager (this downloads from the OpenModelica library index)...".into(),
            ));
            let script = include_str!("../../../scripts/install-libraries.mos");
            match omc.run_query_script(script) {
                Ok(out) => {
                    let ok = out.output.contains("OMOLA_INSTALLED Modelica 4.1.0");
                    let summary: Vec<&str> = out
                        .output
                        .lines()
                        .filter(|l| l.starts_with("OMOLA_") || l.contains("Error"))
                        .collect();
                    send(Event::Log(
                        LogTab::Version,
                        if ok { LogLevel::Info } else { LogLevel::Error },
                        summary.join("\n"),
                    ));
                    send(Event::LibrariesInstalled(ok));
                }
                Err(e) => {
                    send(Event::Log(LogTab::Version, LogLevel::Error, e.to_string()));
                    send(Event::LibrariesInstalled(false));
                }
            }
        }
        Job::Command { text, libraries } => {
            let mut script = String::new();
            let pins = pinned(&libraries);
            for (name, version) in &pins {
                script.push_str(&format!(
                    "omolaLoaded := loadModel({name}, {{\"{}\"}});\n",
                    omola_core::escape_modelica_string(version)
                ));
            }
            if omola_core::validate_library_pins(&pins).is_err() {
                return send(Event::Command(format!("{text}\n  invalid library pins")));
            }
            let line = text.trim().trim_end_matches(';');
            script.push_str(&format!("{line};\n"));
            let output = match omc.run_query_script(&script) {
                Ok(out) => clean_command_output(&out.output, pins.len()),
                Err(error) => error.to_string(),
            };
            send(Event::Command(format!("{text}\n{output}")));
        }
        Job::EditModel {
            doc_text,
            file_name,
            class,
            ops,
            libraries,
        } => {
            let result = edit::edit_script(&class, &ops).and_then(|script| {
                let mut prefix = String::new();
                let pins = pinned(&libraries);
                omola_core::validate_library_pins(&pins).map_err(|e| e.to_string())?;
                for (name, version) in &pins {
                    prefix.push_str(&format!(
                        "loadModel({name}, {{\"{}\"}});\n",
                        omola_core::escape_modelica_string(version)
                    ));
                }
                let source_name = if file_name.is_empty() {
                    format!("{class}.mo")
                } else {
                    file_name
                };
                prefix.push_str(&format!(
                    "loadString(\"{}\", \"{}\");\n",
                    omola_core::escape_modelica_string(&doc_text),
                    omola_core::escape_modelica_string(&source_name)
                ));
                prefix.push_str(&script);
                let output = omc
                    .run_query_script(&prefix)
                    .map_err(|e| e.to_string())?
                    .output;
                if output.contains("OMOLA_EDIT_FAIL") {
                    return Err(output);
                }
                let begin = output
                    .find("OMOLA_EDIT_BEGIN")
                    .ok_or_else(|| format!("OMC did not return edited class:\n{output}"))?
                    + "OMOLA_EDIT_BEGIN".len();
                let end = output[begin..]
                    .find("OMOLA_EDIT_END")
                    .ok_or_else(|| format!("OMC returned an incomplete edited class:\n{output}"))?
                    + begin;
                Ok(output[begin..end].trim().to_owned())
            });
            send(Event::ModelEdited { class, result });
        }
    }
}

fn parse_instance_value(output: &str, class: &str) -> Result<serde_json::Value, String> {
    let sections = library::parse_sections(output);
    let body = sections
        .get("instance")
        .ok_or_else(|| format!("OMC returned no instance JSON for {class}"))?;
    let json = crate::instance::extract_json(body)
        .ok_or_else(|| format!("OMC returned no instance JSON for {class}"))?;
    serde_json::from_str(json)
        .map_err(|e| format!("could not parse instance JSON for {class}: {e}"))
}

fn find_instance_component<'a>(
    root: &'a serde_json::Value,
    path: &str,
) -> Option<&'a serde_json::Value> {
    let name = path.split('.').next()?;
    root.get("elements")?.as_array()?.iter().find(|element| {
        element.get("$kind").and_then(serde_json::Value::as_str) == Some("component")
            && element.get("name").and_then(serde_json::Value::as_str) == Some(name)
    })
}

/// Drop the echo of `cd(...)` and the library `loadModel` results from command output.
fn clean_command_output(output: &str, loads: usize) -> String {
    let mut lines = output.lines();
    for _ in 0..(1 + loads) {
        lines.next();
    }
    lines
        .map(|l| format!("  {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn fail_job(job: Job, message: String, send: &dyn Fn(Event)) {
    match job {
        Job::LoadLibrary(spec) => send(Event::LibraryLoaded(spec, Err(message))),
        Job::LoadClassDoc { class, .. } => send(Event::ClassDoc(class, Err(message))),
        Job::LoadClassText { class, .. } => send(Event::ClassText(class, Err(message))),
        Job::LoadInstance { class, .. } => send(Event::Instance(class, Err(message))),
        Job::LoadParameters { component, .. } => send(Event::Parameters {
            component,
            result: Err(message),
        }),
        Job::LoadIcon { class, .. } => send(Event::Icon(class, Err(message))),
        Job::LoadDiagram { class, .. } => send(Event::Diagram(class, Err(message))),
        Job::Command { text, .. } => send(Event::Command(format!("{text}\n  {message}"))),
        Job::EditModel { class, .. } => send(Event::ModelEdited {
            class,
            result: Err(message),
        }),
        Job::InstallLibraries => send(Event::LibrariesInstalled(false)),
        Job::Simulate { class, .. } => send(Event::SimulationFailed(class)),
        Job::Doctor
        | Job::Check { .. }
        | Job::Translate { .. }
        | Job::LoadResult { .. }
        | Job::CopyResult { .. } => {}
    }
}

/// Write each editor buffer to a separate private file. The target is last.
fn write_user_files(
    run_dir: &Path,
    class: &str,
    context: &ClassContext,
) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    if let Some(source) = &context.user_source {
        for (index, text) in context.other_sources.iter().enumerate() {
            files.push(write_source(
                &run_dir.join("dependencies"),
                &format!("dependency_{index}"),
                text,
            )?);
        }
        files.push(write_source(run_dir, class, source)?);
    }
    Ok(files)
}

fn with_user_files<T>(
    run_dir: &Path,
    class: &str,
    context: &ClassContext,
    f: impl FnOnce(&[PathBuf]) -> Result<T, String>,
) -> Result<T, String> {
    let files = write_user_files(run_dir, class, context)?;
    f(&files)
}

fn simulate(
    omc: &Omc,
    run_dir: &Path,
    class: String,
    context: ClassContext,
    setup: SimulationSetup,
    send: &dyn Fn(Event),
) {
    let sources = match write_user_files(run_dir, &class, &context) {
        Ok(paths) => paths,
        Err(message) => return send(Event::Log(LogTab::Translation, LogLevel::Error, message)),
    };
    send(Event::Log(
        LogTab::Translation,
        LogLevel::Info,
        format!(
            "Translating and simulating {class} (start {} s, stop {} s, {} intervals, tolerance {}, method {}) ...",
            setup.start_time, setup.stop_time, setup.number_of_intervals, setup.tolerance, setup.method
        ),
    ));
    let started = Instant::now();
    if let Err(e) = std::fs::create_dir_all(run_dir) {
        return send(Event::Log(
            LogTab::Translation,
            LogLevel::Error,
            e.to_string(),
        ));
    }
    let prefix = run_dir.join(result_stem(&class));
    let libraries = pinned(&context.libraries);
    let outcome = omc.simulate_class_with_sources(
        &sources,
        &class,
        SimulationConfig {
            stop_time: setup.stop_time,
            start_time: Some(setup.start_time),
            number_of_intervals: Some(setup.number_of_intervals),
            tolerance: Some(setup.tolerance),
            method: Some(setup.method.as_str()),
            result: Some(&prefix),
            libraries: &libraries,
            package_roots: &[],
            parameters: &setup.parameters,
        },
    );
    let result = match outcome {
        Ok(result) => result,
        Err(omola_core::BackendError::Cancelled) => {
            return send(Event::Log(
                LogTab::Simulation,
                LogLevel::Warning,
                "Simulation stopped.".into(),
            ))
        }
        Err(error) => {
            return send(Event::Log(
                LogTab::Translation,
                LogLevel::Error,
                error.to_string(),
            ))
        }
    };
    let Some(result_file) = result.result_file else {
        return send(Event::Log(
            LogTab::Simulation,
            LogLevel::Error,
            "OMC reported success but no result file.".into(),
        ));
    };
    match mat::read_trajectory_file(&result_file) {
        Ok(trajectory) => {
            let elapsed_seconds = started.elapsed().as_secs_f64();
            send(Event::Log(
                LogTab::Translation,
                LogLevel::Info,
                format!("OpenModelica output for {class}:\n{}", result.output.trim()),
            ));
            send(Event::Log(
                LogTab::Simulation,
                LogLevel::Info,
                format!(
                    "Simulation of \"{class}\" (OpenModelica {})\n... finished in {elapsed_seconds:.2} s\n... {} variables, {} result points\n... \"{}\" creating (simulation result file)",
                    result.omc_version,
                    trajectory.variables.len(),
                    trajectory.time.len(),
                    result_file.display()
                ),
            ));
            send(Event::SimulationFinished(Box::new(SimulationOutcome {
                class,
                result_file,
                result: trajectory,
                elapsed_seconds,
            })));
        }
        Err(error) => send(Event::Log(
            LogTab::Simulation,
            LogLevel::Error,
            error.to_string(),
        )),
    }
}

/// `Translate` (G46): build the model in `<run_dir>/build` and report to the Translation tab.
fn translate(omc: &Omc, run_dir: &Path, class: &str, context: &ClassContext, send: &dyn Fn(Event)) {
    let log = |level, text: String| send(Event::Log(LogTab::Translation, level, text));
    log(
        LogLevel::Info,
        format!("Translating {class} (buildModel, no simulation) ..."),
    );
    let build_dir = run_dir.join("build");
    if let Err(error) = std::fs::create_dir_all(&build_dir) {
        return log(
            LogLevel::Error,
            format!("could not create {}: {error}", build_dir.display()),
        );
    }
    let cancelled = std::cell::Cell::new(false);
    let result = with_user_files(run_dir, class, context, |file| {
        let script =
            library::translate_script_with_user_files(class, &context.libraries, file, &build_dir)
                .map_err(|e| e.to_string())?;
        omc.run_query_script(&script)
            .map(|out| out.output)
            .map_err(|e| {
                cancelled.set(matches!(e, omola_core::BackendError::Cancelled));
                e.to_string()
            })
    });
    match result {
        Ok(output) => {
            let sections = library::parse_sections(&output);
            let build = sections.get("build").map(|s| s.trim()).unwrap_or("");
            let errors = sections.get("errors").map(|s| s.trim()).unwrap_or("");
            if build.is_empty() {
                log(
                    LogLevel::Error,
                    if errors.is_empty() {
                        format!("Translation of {class} failed.")
                    } else {
                        format!("Translation of {class} failed.\n{errors}")
                    },
                );
            } else {
                let mut text = format!("Translation of {class} finished ({build}).");
                if !errors.is_empty() {
                    text.push('\n');
                    text.push_str(errors);
                }
                log(LogLevel::Info, text);
            }
        }
        Err(_) if cancelled.get() => log(LogLevel::Warning, "Translation stopped.".into()),
        Err(message) => log(LogLevel::Error, message),
    }
}

/// Values of a parameter sweep (G49): `points` evenly spaced values from `from` to `to`.
///
/// The last value is exactly `to`, so rounding never produces `0.30000000000000004`
/// where the user typed `0.3` as the end.
pub fn sweep_values(from: f64, to: f64, points: usize) -> Result<Vec<f64>, String> {
    if !from.is_finite() || !to.is_finite() {
        return Err("From and To must be finite numbers.".into());
    }
    if from == to {
        return Err("From and To must differ.".into());
    }
    if !(2..=MAX_SWEEP_POINTS).contains(&points) {
        return Err(format!(
            "The number of runs must be between 2 and {MAX_SWEEP_POINTS}."
        ));
    }
    let step = (to - from) / (points - 1) as f64;
    let mut values: Vec<f64> = (0..points).map(|i| from + step * i as f64).collect();
    values[points - 1] = to;
    Ok(values)
}

/// Upper bound of [`sweep_values`]; every run is kept in memory.
pub const MAX_SWEEP_POINTS: usize = 50;

/// Copy a result file for `Save result as...` (G14).
///
/// Without `replace` an existing destination is never opened for writing (`create_new`).
/// With `replace` the copy goes to a temporary file next to the destination, which is
/// renamed over it only after the copy succeeded: a failed copy keeps the old file.
pub fn copy_result_file(source: &Path, dest: &Path, replace: bool) -> Result<(), String> {
    let src = std::fs::canonicalize(source)
        .map_err(|e| format!("could not read {}: {e}", source.display()))?;
    if std::fs::canonicalize(dest).is_ok_and(|d| d == src) {
        return Err("The result is already stored in that file.".into());
    }
    let mut input = std::fs::File::open(&src)
        .map_err(|e| format!("could not read {}: {e}", source.display()))?;
    // The file that is written: the destination itself, or a temporary sibling.
    let target = if replace {
        let mut name = dest
            .file_name()
            .ok_or_else(|| format!("{} is not a file name.", dest.display()))?
            .to_owned();
        name.push(format!(".omola-tmp-{}", std::process::id()));
        dest.with_file_name(name)
    } else {
        dest.to_path_buf()
    };
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                format!("{} already exists.", target.display())
            } else {
                format!("could not write {}: {e}", dest.display())
            }
        })?;
    let copied = std::io::copy(&mut input, &mut output).and_then(|_| output.sync_all());
    drop(output);
    let finished = copied.and_then(|()| {
        if replace {
            std::fs::rename(&target, dest)
        } else {
            Ok(())
        }
    });
    if let Err(error) = finished {
        // Only the file this call created is removed, never an older destination.
        let _ = std::fs::remove_file(&target);
        return Err(format!("could not write {}: {error}", dest.display()));
    }
    Ok(())
}

/// Write the editor buffer to `<run_dir>/<Class>.mo` (the user's file is never touched).
fn write_source(run_dir: &Path, class: &str, text: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(run_dir)
        .map_err(|e| format!("could not create run directory {}: {e}", run_dir.display()))?;
    let path = run_dir.join(format!("{}.mo", result_stem(class)));
    std::fs::write(&path, text).map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(path)
}

fn result_stem(class: &str) -> String {
    class.replace('.', "_")
}

/// Per-process scratch directory for GUI runs.
pub fn default_work_root() -> PathBuf {
    std::env::temp_dir()
        .join("omola-gui")
        .join(format!("session-{}", std::process::id()))
}

/// Work folders of earlier sessions are removed once they are this old (G50).
pub const STALE_SESSION_AGE: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 3600);

/// Name of the file a running Omola keeps locked in its work folder (G53).
pub const SESSION_LOCK_FILE: &str = "session.lock";

/// Create and lock the lock file of the work folder `dir`. The lock is held as long as
/// the returned file is alive and tells other Omola processes that the folder is in use.
pub fn lock_session_dir(dir: &Path) -> Option<std::fs::File> {
    std::fs::create_dir_all(dir).ok()?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(SESSION_LOCK_FILE))
        .ok()?;
    file.try_lock().ok()?;
    Some(file)
}

/// Whether another process holds the lock of the work folder `dir`. A folder without a
/// lock file (an older Omola, or one that crashed early) is not in use. A lock file
/// that exists but cannot be opened counts as in use: the folder is kept.
pub fn session_dir_in_use(dir: &Path) -> bool {
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(dir.join(SESSION_LOCK_FILE))
    {
        Ok(file) => file,
        Err(error) => return error.kind() != std::io::ErrorKind::NotFound,
    };
    match file.try_lock() {
        Ok(()) => false,
        Err(std::fs::TryLockError::WouldBlock) => true,
        // Not knowing is treated as "in use": the folder is kept.
        Err(std::fs::TryLockError::Error(_)) => true,
    }
}

/// The `session-*` directories directly under `root` that may be removed: not `current`,
/// not a symbolic link, last modified more than `max_age` before `now`, and not locked
/// by a running Omola (a window may stay open for longer than `max_age`).
pub fn stale_session_dirs(
    root: &Path,
    current: &Path,
    now: std::time::SystemTime,
    max_age: std::time::Duration,
) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut stale: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("session-"))
        .map(|entry| entry.path())
        .filter(|path| path != current)
        .filter(|path| {
            // `symlink_metadata` does not follow links: a link is never a directory here.
            std::fs::symlink_metadata(path).is_ok_and(|meta| {
                meta.is_dir()
                    && meta
                        .modified()
                        .ok()
                        .and_then(|modified| now.duration_since(modified).ok())
                        .is_some_and(|age| age > max_age)
            })
        })
        .filter(|path| !session_dir_in_use(path))
        .collect();
    stale.sort();
    stale
}

/// Remove the stale session folders under `root`; returns how many were removed. A
/// folder that cannot be removed is tried again at the next start.
pub fn remove_stale_sessions(root: &Path, current: &Path) -> usize {
    stale_session_dirs(
        root,
        current,
        std::time::SystemTime::now(),
        STALE_SESSION_AGE,
    )
    .into_iter()
    .filter(|dir| std::fs::remove_dir_all(dir).is_ok())
    .count()
}

/// Number of output intervals for an interval length (Simulation Setup, G44). A length
/// that is not positive and finite, or an empty time span, gives 1.
pub fn intervals_for_length(start: f64, stop: f64, length: f64) -> u32 {
    let count = (stop - start) / length;
    if !count.is_finite() || length <= 0.0 {
        return 1;
    }
    count.round().clamp(1.0, 1e7) as u32
}
#[cfg(test)]
mod tests;

#[cfg(test)]
mod audit_tests;
