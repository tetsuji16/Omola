//! Worker tests: fake `omc` scripts (unix) and ignored real-OMC tests.

use super::*;
use crate::library::LibrarySpec;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

/// `OMOLA_OMC` is process-wide; tests that point it at a fake `omc` take this lock.
#[cfg(unix)]
static FAKE_OMC: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// `Translate` through the real adapter with a fake `omc` that prints the build section.
#[test]
#[cfg(unix)]
fn translate_job_reports_success_through_fake_omc() {
    let _env = FAKE_OMC.lock().unwrap_or_else(|e| e.into_inner());
    let root = std::env::temp_dir().join(format!("omola-gui-translate-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let fake = root.join("fake-omc");
    std::fs::write(
            &fake,
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'OpenModelica 1.27.1 (fake)'; exit 0; fi\nprintf 'OMOLA_BEGIN build\\nM\\nOMOLA_END build\\nOMOLA_BEGIN errors\\n\\nOMOLA_END errors\\n'\n",
        )
        .unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("OMOLA_OMC", &fake);

    let worker = Worker::spawn(root.join("work"), || {});
    worker.submit(Job::Translate {
        class: "M".into(),
        context: ClassContext {
            libraries: vec![],
            user_source: Some("model M Real x = 1; end M;".into()),
        },
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut finished = None;
    while finished.is_none() && Instant::now() < deadline {
        for event in worker.poll() {
            match event {
                Event::Log(LogTab::Translation, LogLevel::Info, message)
                    if message.contains("finished") =>
                {
                    finished = Some(message)
                }
                Event::Log(_, LogLevel::Error, message) => panic!("worker error: {message}"),
                Event::SimulationFinished(_) => panic!("Translate must not simulate"),
                _ => {}
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        finished.expect("translation finished"),
        "Translation of M finished (M)."
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn sweep_values_are_evenly_spaced_and_validated() {
    assert_eq!(
        sweep_values(0.0, 1.0, 5).unwrap(),
        [0.0, 0.25, 0.5, 0.75, 1.0]
    );
    let values = sweep_values(0.1, 0.3, 3).unwrap();
    assert_eq!(values.len(), 3);
    assert_eq!(values[2], 0.3, "the last value is exactly To");
    assert_eq!(sweep_values(1.0, 0.0, 2).unwrap(), [1.0, 0.0]);
    assert!(sweep_values(0.0, 1.0, 1).is_err());
    assert!(sweep_values(0.0, 1.0, MAX_SWEEP_POINTS + 1).is_err());
    assert!(sweep_values(0.0, 0.0, 3).is_err());
    assert!(sweep_values(f64::NAN, 1.0, 3).is_err());
    assert!(sweep_values(0.0, f64::INFINITY, 3).is_err());
}

#[test]
fn copy_result_file_refuses_to_overwrite() {
    let dir = std::env::temp_dir().join(format!("omola-copy-result-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("M_res.mat");
    let dest = dir.join("kept.mat");
    std::fs::write(&source, b"result").unwrap();
    copy_result_file(&source, &dest, false).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), b"result");
    std::fs::write(&dest, b"other").unwrap();
    let error = copy_result_file(&source, &dest, false).unwrap_err();
    assert!(error.contains("already exists"), "{error}");
    assert_eq!(std::fs::read(&dest).unwrap(), b"other", "left untouched");
    copy_result_file(&source, &dest, true).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), b"result");
    assert!(
        copy_result_file(&source, &source, true).is_err(),
        "same file"
    );
    assert_eq!(std::fs::read(&source).unwrap(), b"result");
    assert!(copy_result_file(&dir.join("missing.mat"), &dest, true).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "needs OpenModelica; run with --ignored and OMOLA_OMC set"]
fn translate_bouncing_ball_with_real_omc() {
    if std::env::var_os("OMOLA_OMC").is_none() {
        eprintln!("OMOLA_OMC is not set; skipping the real-OMC GUI backend test");
        return;
    }
    let root =
        std::env::temp_dir().join(format!("omola-gui-real-translate-{}", std::process::id()));
    let worker = Worker::spawn(root.join("work"), || {});
    worker.submit(Job::Translate {
        class: "BouncingBall".into(),
        context: ClassContext {
            libraries: vec![],
            user_source: Some(include_str!("../../../../examples/BouncingBall.mo").into()),
        },
    });
    let message = wait_for(&worker, 180, |event| match event {
        Event::Log(LogTab::Translation, LogLevel::Info, message)
            if message.contains("finished") =>
        {
            Some(message)
        }
        Event::SimulationFinished(_) => panic!("Translate must not simulate"),
        _ => None,
    });
    assert!(message.contains("BouncingBall"), "{message}");
    let _ = std::fs::remove_dir_all(&root);
}

/// Two runs of a sweep over `restitution` give different trajectories (G49).
#[test]
#[ignore = "needs OpenModelica; run with --ignored and OMOLA_OMC set"]
fn sweep_bouncing_ball_e_with_real_omc() {
    if std::env::var_os("OMOLA_OMC").is_none() {
        eprintln!("OMOLA_OMC is not set; skipping the real-OMC GUI backend test");
        return;
    }
    let root = std::env::temp_dir().join(format!("omola-gui-real-sweep-{}", std::process::id()));
    let worker = Worker::spawn(root.join("work"), || {});
    let mut heights = Vec::new();
    for value in sweep_values(0.5, 0.9, 2).unwrap() {
        worker.submit(Job::Simulate {
            class: "BouncingBall".into(),
            context: ClassContext {
                libraries: vec![],
                user_source: Some(include_str!("../../../../examples/BouncingBall.mo").into()),
            },
            setup: SimulationSetup {
                stop_time: 1.0,
                parameters: vec![("restitution".into(), value.to_string())],
                ..SimulationSetup::default()
            },
        });
        let outcome = wait_for(&worker, 180, |event| match event {
            Event::SimulationFinished(done) => Some(done),
            Event::SimulationFailed(class) => panic!("simulation of {class} failed"),
            _ => None,
        });
        heights.push(outcome.result.series("height").expect("height").to_vec());
    }
    assert_ne!(heights[0], heights[1], "restitution must change the result");
    let _ = std::fs::remove_dir_all(&root);
}

/// `Save in Model` (G48) replaces only `experiment`; the Icon annotation survives.
#[test]
#[ignore = "needs OpenModelica; run with --ignored and OMOLA_OMC set"]
fn set_experiment_keeps_other_annotations_with_real_omc() {
    if std::env::var_os("OMOLA_OMC").is_none() {
        eprintln!("OMOLA_OMC is not set; skipping the real-OMC GUI backend test");
        return;
    }
    let root =
        std::env::temp_dir().join(format!("omola-gui-real-experiment-{}", std::process::id()));
    let worker = Worker::spawn(root.join("work"), || {});
    worker.submit(Job::EditModel {
            doc_text: "model M\n  Real x;\nequation\n  x = 1;\n  annotation(Icon(graphics = {Rectangle(extent = {{-100, -100}, {100, 100}})}), experiment(StopTime = 1));\nend M;\n".into(),
            file_name: "M.mo".into(),
            class: "M".into(),
            ops: vec![EditOp::SetExperiment {
                start: 0.0,
                stop: 2.5,
                tolerance: 1e-6,
                interval: 0.005,
            }],
            libraries: vec![],
        });
    let text = wait_for(&worker, 120, |event| match event {
        Event::ModelEdited { result, .. } => Some(result.expect("set experiment")),
        _ => None,
    });
    assert!(text.contains("Rectangle"), "Icon annotation kept: {text}");
    assert!(text.contains("2.5"), "new StopTime stored: {text}");
    assert_eq!(text.matches("experiment(").count(), 1, "{text}");
    let _ = std::fs::remove_dir_all(&root);
}

/// End-to-end through the real `omola-core` adapter with a fake `omc` executable that
/// copies a pre-built OpenModelica-style MAT file. Real OMC is covered by CI integration.
#[test]
#[cfg(unix)]
fn simulate_job_reads_result_through_fake_omc() {
    let _env = FAKE_OMC.lock().unwrap_or_else(|e| e.into_inner());
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
            user_source: Some(include_str!("../../../../examples/BouncingBall.mo").into()),
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

#[test]
#[ignore = "needs OpenModelica; run with --ignored and OMOLA_OMC set"]
fn edit_model_adds_component_with_real_omc() {
    if std::env::var_os("OMOLA_OMC").is_none() {
        eprintln!("OMOLA_OMC is not set; skipping real-OMC edit test");
        return;
    }
    let root = std::env::temp_dir().join(format!("omola-gui-edit-{}", std::process::id()));
    let worker = Worker::spawn(root.join("work"), || {});
    worker.submit(Job::EditModel {
        doc_text: "model A Real x; equation der(x)=-x; end A;".into(),
        file_name: "A.mo".into(),
        class: "A".into(),
        ops: vec![EditOp::AddComponent {
            name: "k".into(),
            type_name: "Modelica.Blocks.Sources.Constant".into(),
            extent: [[-10., -10.], [10., 10.]],
            origin: [0., 0.],
        }],
        libraries: vec![LibrarySpec::new("Modelica", Some("4.1.0"))],
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut result = None;
    while result.is_none() && Instant::now() < deadline {
        for event in worker.poll() {
            if let Event::ModelEdited { result: r, .. } = event {
                result = Some(r);
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(result.expect("edit result").unwrap().contains("Constant k"));
    let _ = std::fs::remove_dir_all(root);
}

/// Icon annotations of three Modelica Standard Library classes through the GUI worker:
/// `getModelInstanceAnnotation(..., {"Icon"})` is parsed into paintable shapes, and a
/// class without an Icon of its own inherits the one of its base class.
#[test]
#[ignore = "needs OpenModelica with MSL 4.1.0; run with --ignored and OMOLA_OMC set"]
fn icon_annotations_with_real_omc() {
    if std::env::var_os("OMOLA_OMC").is_none() {
        eprintln!("OMOLA_OMC is not set; skipping");
        return;
    }
    let root = std::env::temp_dir().join(format!("omola-gui-icons-{}", std::process::id()));
    let worker = Worker::spawn(root.join("work"), || {});
    let context = ClassContext {
        libraries: vec![LibrarySpec::new("Modelica", Some("4.1.0"))],
        user_source: None,
    };
    let load = |class: &str| {
        worker.submit(Job::LoadIcon {
            class: class.into(),
            context: context.clone(),
        });
        wait_for(&worker, 300, |e| match e {
            Event::Icon(_, result) => Some(result.expect("icon annotation")),
            _ => None,
        })
    };

    let resistor = load("Modelica.Electrical.Analog.Basic.Resistor");
    // Rectangle + two lines + two texts; the conditional `useHeatPort` line is dropped.
    assert_eq!(resistor.len(), 5, "{:?}", resistor.skipped);
    assert_eq!(resistor.coordinate_extent.x1, -100.0);

    let first_order = load("Modelica.Blocks.Continuous.FirstOrder");
    assert_eq!(first_order.len(), 9);

    let inertia = load("Modelica.Mechanics.Rotational.Components.Inertia");
    assert_eq!(inertia.len(), 15);

    // A model without an Icon of its own inherits the one of `Modelica.Blocks.Icons.Block`
    // through two `extends` levels (see `tests/fixtures/icon_InheritedBlock.json`).
    worker.submit(Job::LoadIcon {
        class: "InheritsBlockIcon".into(),
        context: ClassContext {
            libraries: context.libraries.clone(),
            user_source: Some(include_str!("../../tests/fixtures/icon_InheritedBlock.mo").into()),
        },
    });
    let inherited = wait_for(&worker, 300, |e| match e {
        Event::Icon(_, result) => Some(result.expect("inherited icon")),
        _ => None,
    });
    assert!(!inherited.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
#[ignore = "needs OpenModelica with MSL 4.1.0; run with --ignored and OMOLA_OMC set"]
fn diagram_reuses_cached_icons_with_real_omc() {
    if std::env::var_os("OMOLA_OMC").is_none() {
        eprintln!("OMOLA_OMC is not set; skipping");
        return;
    }
    let root = std::env::temp_dir().join(format!("omola-gui-cache-{}", std::process::id()));
    let worker = Worker::spawn(root.join("work"), || {});
    let class = "Modelica.Blocks.Examples.PID_Controller";
    let context = ClassContext {
        libraries: vec![LibrarySpec::new("Modelica", Some("4.1.0"))],
        user_source: None,
    };
    let load = |cached_icons| {
        worker.submit(Job::LoadDiagram {
            class: class.into(),
            context: context.clone(),
            cached_icons,
        });
        wait_for(&worker, 180, |event| match event {
            Event::Diagram(_, result) => Some(result.expect("diagram")),
            _ => None,
        })
    };
    let first = load(Default::default());
    let component = first
        .components
        .iter()
        .find(|c| c.icon.is_some())
        .expect("component icon");
    let mut cached = component.icon.clone().unwrap();
    cached.skipped.push("cache verification marker".into());
    let type_name = component.type_name.clone();
    let second = load([(type_name.clone(), cached.clone())].into_iter().collect());
    assert_eq!(
        second
            .components
            .iter()
            .find(|c| c.type_name == type_name)
            .unwrap()
            .icon
            .as_ref(),
        Some(&cached)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
#[ignore = "needs OpenModelica; run with --ignored and OMOLA_OMC set"]
fn diagram_loads_user_component_icon_with_real_omc() {
    if std::env::var_os("OMOLA_OMC").is_none() {
        eprintln!("OMOLA_OMC is not set; skipping");
        return;
    }
    let root = std::env::temp_dir().join(format!("omola-gui-user-icon-{}", std::process::id()));
    let worker = Worker::spawn(root.join("work"), || {});
    worker.submit(Job::LoadDiagram {
        class: "A".into(),
        context: ClassContext {
            libraries: vec![],
            user_source: Some(
                r#"model A
  model Custom
    annotation(Icon(graphics={Rectangle(extent={{-100,-100},{100,100}})}));
  end Custom;
  Custom part annotation(Placement(transformation(extent={{-10,-10},{10,10}})));
end A;"#
                    .into(),
            ),
        },
        cached_icons: Default::default(),
    });
    let diagram = wait_for(&worker, 120, |event| match event {
        Event::Diagram(_, result) => Some(result.expect("diagram")),
        _ => None,
    });
    let part = diagram
        .components
        .iter()
        .find(|c| c.name == "part")
        .expect("placed part");
    assert!(
        part.icon.as_ref().is_some_and(|icon| !icon.is_empty()),
        "user component icon: {:?}",
        part.icon
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Exercise the component editing flow used by G29-G32 against a real OMC installation.
/// This does not replace the GUI acceptance screenshots, but catches scripting API and
/// Modelica source regressions in placement, connection, rename, delete, check, and simulate.
#[test]
#[ignore = "needs OpenModelica with MSL 4.1.0; run with --ignored and OMOLA_OMC set"]
fn diagram_edit_flow_with_real_omc() {
    if std::env::var_os("OMOLA_OMC").is_none() {
        eprintln!("OMOLA_OMC is not set; skipping real-OMC diagram edit flow");
        return;
    }
    let root = std::env::temp_dir().join(format!("omola-gui-diagram-flow-{}", std::process::id()));
    let worker = Worker::spawn(root.join("work"), || {});
    let libraries = vec![LibrarySpec::new("Modelica", Some("4.1.0"))];
    let context_for = |source: &str| ClassContext {
        libraries: libraries.clone(),
        user_source: Some(source.to_owned()),
    };
    let mut source = "model A end A;".to_owned();

    worker.submit(Job::EditModel {
        doc_text: source.clone(),
        file_name: "A.mo".into(),
        class: "A".into(),
        ops: vec![
            EditOp::AddComponent {
                name: "sine".into(),
                type_name: "Modelica.Blocks.Sources.Sine".into(),
                extent: [[-10.0, -10.0], [10.0, 10.0]],
                origin: [-40.0, 0.0],
            },
            EditOp::AddComponent {
                name: "firstOrder".into(),
                type_name: "Modelica.Blocks.Continuous.FirstOrder".into(),
                extent: [[-10.0, -10.0], [10.0, 10.0]],
                origin: [40.0, 0.0],
            },
        ],
        libraries: libraries.clone(),
    });
    source = wait_for(&worker, 60, |event| match event {
        Event::ModelEdited { result, .. } => Some(result.expect("add Sine and FirstOrder")),
        _ => None,
    });
    assert!(source.contains("Sine sine"), "{source}");
    assert!(source.contains("FirstOrder firstOrder"), "{source}");
    assert!(
        source.contains("Placement"),
        "component Placement missing: {source}"
    );

    worker.submit(Job::EditModel {
        doc_text: source.clone(),
        file_name: "A.mo".into(),
        class: "A".into(),
        ops: vec![EditOp::SetPlacement {
            component: "firstOrder".into(),
            type_name: "Modelica.Blocks.Continuous.FirstOrder".into(),
            extent: [[-10.0, -10.0], [10.0, 10.0]],
            origin: [60.0, 20.0],
            rotation: 90.0,
        }],
        libraries: libraries.clone(),
    });
    source = wait_for(&worker, 60, |event| match event {
        Event::ModelEdited { result, .. } => Some(result.expect("move FirstOrder")),
        _ => None,
    });
    assert!(source.contains("origin = {60, 20}"), "{source}");
    assert!(source.contains("rotation = 90"), "{source}");

    worker.submit(Job::EditModel {
        doc_text: source.clone(),
        file_name: "A.mo".into(),
        class: "A".into(),
        ops: vec![EditOp::AddConnection {
            from: "sine.y".into(),
            to: "firstOrder.u".into(),
            points: vec![[-30.0, 0.0], [0.0, 0.0], [30.0, 0.0]],
        }],
        libraries: libraries.clone(),
    });
    source = wait_for(&worker, 60, |event| match event {
        Event::ModelEdited { result, .. } => Some(result.expect("connect Sine to FirstOrder")),
        _ => None,
    });
    assert!(source.contains("connect(sine.y, firstOrder.u)"), "{source}");

    worker.submit(Job::Check {
        class: "A".into(),
        context: context_for(&source),
    });
    let check = wait_for(&worker, 60, |event| match event {
        Event::Log(LogTab::Syntax, LogLevel::Info, message)
            if message.contains("completed successfully") =>
        {
            Some(message)
        }
        _ => None,
    });
    assert!(check.contains("completed successfully"), "{check}");

    worker.submit(Job::Simulate {
        class: "A".into(),
        context: context_for(&source),
        setup: SimulationSetup {
            stop_time: 1.0,
            ..SimulationSetup::default()
        },
    });
    let outcome = wait_for(&worker, 180, |event| match event {
        Event::SimulationFinished(outcome) => Some(outcome),
        _ => None,
    });
    assert!(outcome.result.series("firstOrder.y").is_some());

    worker.submit(Job::EditModel {
        doc_text: source.clone(),
        file_name: "A.mo".into(),
        class: "A".into(),
        ops: vec![EditOp::RenameComponent {
            old: "firstOrder".into(),
            new: "filter".into(),
        }],
        libraries: libraries.clone(),
    });
    source = wait_for(&worker, 60, |event| match event {
        Event::ModelEdited { result, .. } => Some(result.expect("rename FirstOrder")),
        _ => None,
    });
    assert!(source.contains("connect(sine.y, filter.u)"), "{source}");

    worker.submit(Job::EditModel {
        doc_text: source.clone(),
        file_name: "A.mo".into(),
        class: "A".into(),
        ops: vec![
            EditOp::DeleteConnection {
                from: "sine.y".into(),
                to: "filter.u".into(),
            },
            EditOp::DeleteComponent {
                name: "filter".into(),
            },
        ],
        libraries: libraries.clone(),
    });
    source = wait_for(&worker, 60, |event| match event {
        Event::ModelEdited { result, .. } => Some(result.expect("delete connected FirstOrder")),
        _ => None,
    });
    assert!(!source.contains("FirstOrder filter"), "{source}");
    assert!(!source.contains("connect(sine.y, filter.u)"), "{source}");

    worker.submit(Job::Check {
        class: "A".into(),
        context: context_for(&source),
    });
    let check = wait_for(&worker, 60, |event| match event {
        Event::Log(LogTab::Syntax, LogLevel::Info, message)
            if message.contains("completed successfully") =>
        {
            Some(message)
        }
        _ => None,
    });
    assert!(check.contains("completed successfully"), "{check}");
    let _ = std::fs::remove_dir_all(root);
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
