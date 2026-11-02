//! Background worker: every OMC call runs on one worker thread so the UI never blocks.
//!
//! The UI sends [`Job`]s and polls [`Event`]s once per frame. The worker writes the
//! editor buffer to a private run directory, so user files are never modified by a run.

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

pub enum Job {
    /// Locate OMC and report its version.
    Doctor,
    Check {
        source_text: String,
        class: String,
    },
    Simulate {
        source_text: String,
        class: String,
        setup: SimulationSetup,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

/// A finished simulation, ready for the variable browser and plot window.
pub struct SimulationOutcome {
    pub class: String,
    pub result_file: PathBuf,
    pub result: mat::TrajectoryResult,
    pub elapsed_seconds: f64,
}

pub enum Event {
    Log(LogLevel, String),
    OmcVersion(Option<String>),
    /// A job started (`true`) or finished (`false`); drives the busy indicator.
    Busy(bool),
    SimulationFinished(Box<SimulationOutcome>),
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

fn run_job(job: Job, run_dir: &Path, send: &dyn Fn(Event)) {
    let omc = match Omc::discover() {
        Ok(omc) => omc,
        Err(error) => {
            send(Event::OmcVersion(None));
            send(Event::Log(LogLevel::Error, error.to_string()));
            return;
        }
    };
    match job {
        Job::Doctor => match omc.version() {
            Ok(version) => {
                send(Event::Log(
                    LogLevel::Info,
                    format!("OMC: {} ({version})", omc.executable().display()),
                ));
                send(Event::OmcVersion(Some(version)));
            }
            Err(error) => {
                send(Event::OmcVersion(None));
                send(Event::Log(LogLevel::Error, error.to_string()));
            }
        },
        Job::Check { source_text, class } => {
            let source = match write_source(run_dir, &class, &source_text) {
                Ok(path) => path,
                Err(message) => return send(Event::Log(LogLevel::Error, message)),
            };
            send(Event::Log(LogLevel::Info, format!("Checking {class} ...")));
            match omc.check_model(&source, &class) {
                Ok(result) => send(Event::Log(
                    LogLevel::Info,
                    format!("Check of {class} succeeded.\n{}", result.output.trim()),
                )),
                Err(error) => send(Event::Log(LogLevel::Error, error.to_string())),
            }
        }
        Job::Simulate {
            source_text,
            class,
            setup,
        } => {
            let source = match write_source(run_dir, &class, &source_text) {
                Ok(path) => path,
                Err(message) => return send(Event::Log(LogLevel::Error, message)),
            };
            if setup.start_time != 0.0
                || setup.number_of_intervals != SimulationSetup::default().number_of_intervals
                || setup.tolerance != SimulationSetup::default().tolerance
                || setup.method != SimulationSetup::default().method
            {
                send(Event::Log(
                    LogLevel::Warning,
                    "Start time, intervals, tolerance and method are not yet passed to OMC (see issue G05); OMC defaults are used.".into(),
                ));
            }
            send(Event::Log(
                LogLevel::Info,
                format!("Simulating {class} until t = {} s ...", setup.stop_time),
            ));
            let started = Instant::now();
            let prefix = run_dir.join(result_stem(&class));
            let outcome = omc.simulate_project_model_with_parameters(
                &source,
                &class,
                SimulationConfig {
                    stop_time: setup.stop_time,
                    result: Some(&prefix),
                    libraries: &[],
                    package_roots: &[],
                    parameters: &setup.parameters,
                },
            );
            let result = match outcome {
                Ok(result) => result,
                Err(error) => return send(Event::Log(LogLevel::Error, error.to_string())),
            };
            let Some(result_file) = result.result_file else {
                return send(Event::Log(
                    LogLevel::Error,
                    "OMC reported success but no result file.".into(),
                ));
            };
            match mat::read_trajectory_file(&result_file) {
                Ok(trajectory) => {
                    let elapsed_seconds = started.elapsed().as_secs_f64();
                    send(Event::Log(
                        LogLevel::Info,
                        format!(
                            "Simulation finished in {elapsed_seconds:.2} s: {} variables, {} points.\nResult: {}",
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
                Err(error) => send(Event::Log(LogLevel::Error, error.to_string())),
            }
        }
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
            source_text: "model M Real x(start=1); equation der(x) = -x; end M;".into(),
            class: "M".into(),
            setup: SimulationSetup::default(),
        });
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut outcome = None;
        while outcome.is_none() && Instant::now() < deadline {
            for event in worker.poll() {
                if let Event::SimulationFinished(done) = event {
                    outcome = Some(done);
                } else if let Event::Log(LogLevel::Error, message) = event {
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
            source_text: include_str!("../../../examples/BouncingBall.mo").into(),
            class: "BouncingBall".into(),
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
                    Event::Log(LogLevel::Error, message) => panic!("worker error: {message}"),
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
        if let Some((at, min)) = height
            .iter()
            .cloned()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .filter(|(_, min)| *min < -1e-6)
        {
            let velocity = result.series("velocity");
            let around: Vec<String> = (at.saturating_sub(3)..(at + 4).min(height.len()))
                .map(|i| {
                    format!(
                        "i={i} t={} h={} v={:?}",
                        result.time[i],
                        height[i],
                        velocity.map(|v| v[i])
                    )
                })
                .collect();
            let head: Vec<String> = (0..8.min(height.len()))
                .map(|i| format!("i={i} t={} h={}", result.time[i], height[i]))
                .collect();
            let vars: Vec<String> = result
                .variables
                .iter()
                .map(|v| format!("{}{}", v.name, if v.is_parameter { "(p)" } else { "" }))
                .collect();
            panic!(
                "ball went below the ground: min {min} at sample {at} of {}\nvariables: {vars:?}\nhead: {head:#?}\naround min: {around:#?}",
                height.len()
            );
        }

        let gravity = result.index_of("gravity").expect("gravity variable");
        assert!(result.variables[gravity].is_parameter);
        let _ = std::fs::remove_dir_all(&root);
    }
}
