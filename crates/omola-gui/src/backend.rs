//! Background worker: every OMC call runs on one worker thread so the UI never blocks.
//!
//! The UI sends [`Job`]s and polls [`Event`]s once per frame. The worker writes the
//! editor buffer to a private run directory, so user files are never modified by a run.

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
#[derive(Debug, Clone, PartialEq)]
pub struct SimulationSetup {
    pub start_time: f64,
    pub stop_time: f64,
    pub number_of_intervals: u32,
    pub tolerance: f64,
    pub method: String,
    /// Parameter overrides `name=value` applied with OMC `-override`.
    pub parameters: Vec<(String, String)>,
}

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
    Check {
        class: String,
        context: ClassContext,
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
    SimulationFinished(Box<SimulationOutcome>),
    /// `InstallLibraries` finished (`true` when the Modelica Standard Library is installed).
    LibrariesInstalled(bool),
}

pub struct Worker {
    jobs: Sender<Job>,
    events: Receiver<Event>,
}

impl Worker {
    /// Spawn the worker. `wake` is called after every event so the UI repaints.
    pub fn spawn(work_root: PathBuf, wake: impl Fn() + Send + 'static) -> Self {
        let (jobs, job_rx) = mpsc::channel::<Job>();
        let (event_tx, events) = mpsc::channel::<Event>();
        thread::Builder::new()
            .name("omola-omc-worker".into())
            .spawn(move || {
                let mut run_counter = 0u32;
                for job in job_rx {
                    let send = |event: Event| {
                        let _ = event_tx.send(event);
                        wake();
                    };
                    send(Event::Busy(true));
                    run_counter += 1;
                    let run_dir = work_root.join(format!("run-{run_counter}"));
                    run_job(job, &run_dir, &send);
                    send(Event::Busy(false));
                }
            })
            .expect("spawn OMC worker thread");
        Self { jobs, events }
    }

    pub fn submit(&self, job: Job) {
        let _ = self.jobs.send(job);
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

fn run_job(job: Job, run_dir: &Path, send: &dyn Fn(Event)) {
    let omc = match Omc::discover() {
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
                Ok(lib) => send(Event::Log(
                    LogTab::Version,
                    LogLevel::Info,
                    format!(
                        "Loaded {} {} ({} classes) from {}",
                        spec.name,
                        lib.version,
                        lib.classes.len(),
                        lib.source_file.display()
                    ),
                )),
                Err(message) => send(Event::Log(
                    LogTab::Version,
                    LogLevel::Warning,
                    message.clone(),
                )),
            }
            send(Event::LibraryLoaded(spec, result));
        }
        Job::LoadClassDoc { class, context } => {
            let result = with_user_file(run_dir, &class, &context, |file| {
                let script = library::class_doc_script(&class, &context.libraries, file)
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
        Job::Check { class, context } => {
            send(Event::Log(
                LogTab::Syntax,
                LogLevel::Info,
                format!("checkModel(\"{class}\")"),
            ));
            let result = with_user_file(run_dir, &class, &context, |file| {
                let script = library::check_script(&class, &context.libraries, file)
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
        } => simulate(&omc, run_dir, class, context, setup, send),
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
    }
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
        Job::Command { text, .. } => send(Event::Command(format!("{text}\n  {message}"))),
        Job::InstallLibraries => send(Event::LibrariesInstalled(false)),
        Job::Doctor | Job::Check { .. } | Job::Simulate { .. } => {}
    }
}

/// Run `f` with the user's buffer written to a private file (or `None`).
fn with_user_file<T>(
    run_dir: &Path,
    class: &str,
    context: &ClassContext,
    f: impl FnOnce(Option<&Path>) -> Result<T, String>,
) -> Result<T, String> {
    match &context.user_source {
        Some(text) => {
            let path = write_source(run_dir, class, text)?;
            f(Some(&path))
        }
        None => f(None),
    }
}

fn simulate(
    omc: &Omc,
    run_dir: &Path,
    class: String,
    context: ClassContext,
    setup: SimulationSetup,
    send: &dyn Fn(Event),
) {
    let source = match &context.user_source {
        Some(text) => match write_source(run_dir, &class, text) {
            Ok(path) => Some(path),
            Err(message) => return send(Event::Log(LogTab::Translation, LogLevel::Error, message)),
        },
        None => None,
    };
    if setup.start_time != 0.0
        || setup.number_of_intervals != SimulationSetup::default().number_of_intervals
        || setup.tolerance != SimulationSetup::default().tolerance
        || setup.method != SimulationSetup::default().method
    {
        send(Event::Log(
            LogTab::Translation,
            LogLevel::Warning,
            "Start time, intervals, tolerance and method are not yet passed to OMC (see issue G05); OMC defaults are used.".into(),
        ));
    }
    send(Event::Log(
        LogTab::Translation,
        LogLevel::Info,
        format!(
            "Translating and simulating {class} (stop time {} s) ...",
            setup.stop_time
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
    let outcome = omc.simulate_class(
        source.as_deref(),
        &class,
        SimulationConfig {
            stop_time: setup.stop_time,
            result: Some(&prefix),
            libraries: &libraries,
            package_roots: &[],
            parameters: &setup.parameters,
        },
    );
    let result = match outcome {
        Ok(result) => result,
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
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::library::LibrarySpec;
    use std::{os::unix::fs::PermissionsExt, time::Duration};

    /// End-to-end through the real `omola-core` adapter with a fake `omc` executable that
    /// copies a pre-built OpenModelica-style MAT file. Real OMC is covered by CI integration.
    #[test]
    fn simulate_job_reads_result_through_fake_omc() {
        let root = std::env::temp_dir().join(format!("omola-gui-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let mat_path = root.join("fixture.mat");
        let var = |name: &str| mat::ResultVariable {
            name: name.into(),
            description: String::new(),
            is_parameter: false,
        };
        let fixture = mat::TrajectoryResult {
            variables: vec![var("time"), var("x")],
            time: vec![0.0, 0.5, 1.0],
            values: vec![vec![0.0, 0.5, 1.0], vec![1.0, 0.6, 0.3]],
        };
        std::fs::write(&mat_path, mat::write_trajectory(&fixture).unwrap()).unwrap();
        let fake = root.join("fake-omc");
        std::fs::write(
            &fake,
            format!(
                "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'OpenModelica 1.27.1 (fake)'; exit 0; fi\ncp '{}' ./fake_res.mat\necho 'OMOLA_RESULT_FILE:fake_res.mat'\n",
                mat_path.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("OMOLA_OMC", &fake);

        let worker = Worker::spawn(root.join("work"), || {});
        worker.submit(Job::Simulate {
            class: "M".into(),
            context: ClassContext {
                libraries: vec![],
                user_source: Some("model M Real x(start=1); equation der(x) = -x; end M;".into()),
            },
            setup: SimulationSetup::default(),
        });
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut outcome = None;
        while outcome.is_none() && Instant::now() < deadline {
            for event in worker.poll() {
                if let Event::SimulationFinished(done) = event {
                    outcome = Some(done);
                } else if let Event::Log(_, LogLevel::Error, message) = event {
                    panic!("worker error: {message}");
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
        let outcome = outcome.expect("simulation finished");
        assert_eq!(outcome.class, "M");
        assert_eq!(outcome.result.series("x").unwrap(), [1.0, 0.6, 0.3]);
        assert!(outcome.result_file.ends_with("M_res.mat"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Real OpenModelica through the GUI worker. Ignored by default because it needs `omc`
    /// and mutates nothing global (it only reads `OMOLA_OMC`); CI runs it on its own with
    /// `cargo test -p omola-gui -- --ignored --test-threads=1 simulate_bouncing_ball_with_real_omc`.
    #[test]
    #[ignore = "needs OpenModelica; run with --ignored and OMOLA_OMC set"]
    fn simulate_bouncing_ball_with_real_omc() {
        if std::env::var_os("OMOLA_OMC").is_none() {
            eprintln!("OMOLA_OMC is not set; skipping the real-OMC GUI backend test");
            return;
        }
        let root = std::env::temp_dir().join(format!("omola-gui-real-omc-{}", std::process::id()));
        let worker = Worker::spawn(root.join("work"), || {});
        worker.submit(Job::Simulate {
            class: "BouncingBall".into(),
            context: ClassContext {
                libraries: vec![],
                user_source: Some(include_str!("../../../examples/BouncingBall.mo").into()),
            },
            setup: SimulationSetup {
                stop_time: 3.0,
                ..SimulationSetup::default()
            },
        });
        // OMC translates and compiles C code, which can take a while on a cold CI runner.
        let deadline = Instant::now() + Duration::from_secs(180);
        let mut outcome = None;
        while outcome.is_none() && Instant::now() < deadline {
            for event in worker.poll() {
                match event {
                    Event::SimulationFinished(done) => outcome = Some(done),
                    Event::Log(_, LogLevel::Error, message) => panic!("worker error: {message}"),
                    _ => {}
                }
            }
            thread::sleep(Duration::from_millis(50));
        }
        let outcome = outcome.expect("simulation finished within 180 s");
        let result = &outcome.result;

        assert_eq!(result.time.first(), Some(&0.0));
        let last = *result.time.last().expect("time samples");
        assert!((last - 3.0).abs() < 1e-6, "last time was {last}");

        let height = result.series("height").expect("height variable");
        assert!(
            (height[0] - 1.0).abs() < 1e-6,
            "initial height was {}",
            height[0]
        );
        // BouncingBall has no Zeno handling: after the bounces converge (about t = 2.56 s)
        // `when height <= 0` never fires again and OMC lets the ball fall through the floor.
        // That is real model behavior, so physics is only checked before that point.
        let before_zeno: Vec<(f64, f64)> = result
            .time
            .iter()
            .cloned()
            .zip(height.iter().cloned())
            .filter(|(t, _)| *t <= 2.0)
            .collect();
        assert!(
            before_zeno.iter().all(|(_, h)| *h >= -1e-4),
            "ball went below the ground before t = 2 s: {:?}",
            before_zeno
                .iter()
                .find(|(_, h)| *h < -1e-4)
                .expect("a violating sample")
        );
        // First impact happens at sqrt(2 * 1 / 9.81) = 0.4515 s: an event sample sits on the floor.
        assert!(
            before_zeno
                .iter()
                .any(|(t, h)| (0.44..=0.47).contains(t) && h.abs() < 1e-3),
            "no floor contact near t = 0.4515 s"
        );
        // After the bounce (restitution 0.7) the ball rises again to about 0.49 m.
        let peak = before_zeno
            .iter()
            .filter(|(t, _)| (0.5..=0.8).contains(t))
            .map(|(_, h)| *h)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            (0.4..=0.6).contains(&peak),
            "expected a rebound peak near 0.49 m, got {peak}"
        );

        let gravity = result.index_of("gravity").expect("gravity variable");
        assert!(result.variables[gravity].is_parameter);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Wait for the first event accepted by `pick`, failing on worker errors.
    fn wait_for<T>(worker: &Worker, secs: u64, mut pick: impl FnMut(Event) -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < deadline {
            for event in worker.poll() {
                if let Event::Log(_, LogLevel::Error, message) = &event {
                    panic!("worker error: {message}");
                }
                if let Some(value) = pick(event) {
                    return value;
                }
            }
            thread::sleep(Duration::from_millis(50));
        }
        panic!("timed out after {secs} s");
    }

    /// Modelica Standard Library 4.1.0 through the GUI worker: class index, the top-level
    /// documentation shown in the Documentation layer (with its images), and simulating a
    /// library example without any user file. Needs real OMC with MSL 4.1.0 installed.
    #[test]
    #[ignore = "needs OpenModelica with MSL 4.1.0; run with --ignored and OMOLA_OMC set"]
    fn msl_browse_document_and_simulate_with_real_omc() {
        if std::env::var_os("OMOLA_OMC").is_none() {
            eprintln!("OMOLA_OMC is not set; skipping");
            return;
        }
        let root = std::env::temp_dir().join(format!("omola-gui-msl-{}", std::process::id()));
        let worker = Worker::spawn(root.join("work"), || {});
        let msl = LibrarySpec::new("Modelica", Some("4.1.0"));

        worker.submit(Job::LoadLibrary(msl.clone()));
        let lib = wait_for(&worker, 300, |e| match e {
            Event::LibraryLoaded(_, result) => Some(result.expect("MSL index")),
            _ => None,
        });
        assert_eq!(lib.version, "4.1.0");
        assert!(
            lib.classes.len() > 1000,
            "only {} classes",
            lib.classes.len()
        );
        let top: Vec<&str> = lib
            .children("Modelica")
            .iter()
            .map(|c| c.short_name())
            .collect();
        for expected in ["UsersGuide", "Blocks", "Electrical", "Mechanics"] {
            assert!(top.contains(&expected), "missing {expected} in {top:?}");
        }
        assert_eq!(
            lib.class("Modelica.Blocks.Examples.PID_Controller")
                .map(|c| c.restriction.as_str()),
            Some("model")
        );

        let context = ClassContext {
            libraries: vec![msl.clone()],
            user_source: None,
        };
        worker.submit(Job::LoadClassDoc {
            class: "Modelica".into(),
            context: context.clone(),
        });
        let doc = wait_for(&worker, 300, |e| match e {
            Event::ClassDoc(_, result) => Some(result.expect("Modelica documentation")),
            _ => None,
        });
        assert_eq!(doc.restriction, "package");
        assert_eq!(doc.comment, "Modelica Standard Library");
        assert!(doc.info_html.contains("Modelica_Libraries.svg"));
        let blocks = crate::html::parse(&doc.info_html);
        assert!(crate::html::plain_text(&blocks).contains("Modelica Standard Library"));
        for src in [
            "modelica://Modelica/Resources/Images/Logos/Modelica_Libraries.svg",
            "modelica://Modelica/Resources/Images/UsersGuide/ModelicaLibraries.png",
        ] {
            match crate::library::resolve_link(src, "Modelica", &doc.roots) {
                crate::library::LinkTarget::File(path) => {
                    assert!(path.is_file(), "image not found: {}", path.display())
                }
                other => panic!("unexpected link target {other:?}"),
            }
        }

        worker.submit(Job::LoadClassDoc {
            class: "Modelica.Blocks.Examples.PID_Controller".into(),
            context: context.clone(),
        });
        let example = wait_for(&worker, 300, |e| match e {
            Event::ClassDoc(_, result) => Some(result.expect("example documentation")),
            _ => None,
        });
        let experiment = example.experiment.expect("experiment annotation");
        assert!((experiment.stop_time - 4.0).abs() < 1e-9, "{experiment:?}");

        worker.submit(Job::Simulate {
            class: "Modelica.Blocks.Examples.PID_Controller".into(),
            context,
            setup: SimulationSetup {
                stop_time: experiment.stop_time,
                ..SimulationSetup::default()
            },
        });
        let outcome = wait_for(&worker, 600, |e| match e {
            Event::SimulationFinished(done) => Some(done),
            _ => None,
        });
        let result = &outcome.result;
        assert!((result.time.last().copied().unwrap() - 4.0).abs() < 1e-6);
        let speed = result.series("inertia1.w").expect("inertia1.w");
        assert!(speed.iter().all(|v| v.is_finite()));
        assert!(
            speed.iter().cloned().fold(f64::MIN, f64::max) > 1.0,
            "drive never spun up"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
