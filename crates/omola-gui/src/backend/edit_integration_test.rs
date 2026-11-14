//! Graphical edits through a real `omc` (ignored without one).

use super::*;
use crate::edit::EditOp;
use std::time::Duration;

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
    let deadline = Instant::now() + Duration::from_secs(60);
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

#[test]
#[ignore = "needs OpenModelica with MSL 4.1.0; run with --ignored and OMOLA_OMC set"]
fn edit_model_adds_connection_with_real_omc() {
    if std::env::var_os("OMOLA_OMC").is_none() {
        return;
    }
    let root = std::env::temp_dir().join(format!("omola-gui-connect-{}", std::process::id()));
    let worker = Worker::spawn(root.join("work"), || {});
    worker.submit(Job::EditModel {
            doc_text: "model A Modelica.Blocks.Sources.Sine sine; Modelica.Blocks.Continuous.FirstOrder firstOrder; end A;".into(),
            file_name: "A.mo".into(), class: "A".into(),
            ops: vec![EditOp::AddConnection { from: "sine.y".into(), to: "firstOrder.u".into(), points: vec![[0.0, 0.0], [10.0, 0.0]] }],
            libraries: vec![LibrarySpec::new("Modelica", Some("4.1.0"))],
        });
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut result = None;
    while result.is_none() && Instant::now() < deadline {
        for event in worker.poll() {
            if let Event::ModelEdited { result: r, .. } = event {
                result = Some(r);
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(result
        .expect("edit result")
        .unwrap()
        .contains("connect(sine.y, firstOrder.u)"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[ignore = "needs OpenModelica with MSL 4.1.0; run with --ignored and OMOLA_OMC set"]
fn load_parameter_rows_and_current_modifier_with_real_omc() {
    if std::env::var_os("OMOLA_OMC").is_none() {
        return;
    }
    let root = std::env::temp_dir().join(format!("omola-gui-parameters-{}", std::process::id()));
    let worker = Worker::spawn(root.join("work"), || {});
    worker.submit(Job::LoadParameters {
        component_type: "Modelica.Blocks.Continuous.FirstOrder".into(),
        class: "A".into(),
        component: "firstOrder".into(),
        context: ClassContext {
            other_sources: Vec::new(),
            libraries: vec![LibrarySpec::new("Modelica", Some("4.1.0"))],
            user_source: Some(
                "model A Modelica.Blocks.Continuous.FirstOrder firstOrder(T=2); end A;".into(),
            ),
        },
    });
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut result = None;
    while result.is_none() && Instant::now() < deadline {
        for event in worker.poll() {
            if let Event::Parameters { result: r, .. } = event {
                result = Some(r);
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    let rows = result.expect("parameter result").unwrap();
    let t = rows.iter().find(|p| p.name == "T").expect("T parameter");
    assert_eq!(t.default_value.as_deref(), Some("1"));
    assert_eq!(t.current_value.as_deref(), Some("2"));
    assert_eq!(t.unit, "s");
    worker.submit(Job::EditModel {
        doc_text: "model A Modelica.Blocks.Continuous.FirstOrder firstOrder; end A;".into(),
        file_name: "A.mo".into(),
        class: "A".into(),
        ops: vec![EditOp::SetModifier {
            path: "firstOrder.T".into(),
            value: Some("2".into()),
        }],
        libraries: vec![LibrarySpec::new("Modelica", Some("4.1.0"))],
    });
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut edited = None;
    while edited.is_none() && Instant::now() < deadline {
        for event in worker.poll() {
            if let Event::ModelEdited { result, .. } = event {
                edited = Some(result);
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    let edited = edited.expect("modifier edit result").unwrap();
    assert!(edited.contains("T = 2"), "{edited}");
    let _ = std::fs::remove_dir_all(root);
}
