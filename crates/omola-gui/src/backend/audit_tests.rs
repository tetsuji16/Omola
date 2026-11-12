//! Tests of the result, sweep and translate helpers.

use super::*;

#[test]
fn result_class_strips_the_result_suffix() {
    let class = |p: &str| result_class_from_path(Path::new(p));
    assert_eq!(class("work/BouncingBall_res.mat"), "BouncingBall");
    assert_eq!(class("A_B_res.MAT"), "A_B");
    assert_eq!(class("other.mat"), "other");
    assert_eq!(class("noextension"), "noextension");
    assert_eq!(class("_res.mat"), "result");
}

#[test]
fn setup_validation_names_the_offending_field() {
    let ok = SimulationSetup::default();
    assert_eq!(ok.validate(), Ok(()));
    let bad = |edit: fn(&mut SimulationSetup)| {
        let mut setup = SimulationSetup::default();
        edit(&mut setup);
        setup.validate().unwrap_err()
    };
    assert!(bad(|s| s.stop_time = s.start_time).contains("Stop time"));
    assert!(bad(|s| s.stop_time = f64::NAN).contains("finite"));
    assert!(bad(|s| s.number_of_intervals = 0).contains("intervals"));
    assert!(bad(|s| s.tolerance = 0.0).contains("Tolerance"));
    assert!(bad(|s| s.method = "magic".into()).contains("magic"));
}

#[test]
fn load_result_job_reports_a_missing_file_without_omc() {
    let events = std::cell::RefCell::new(Vec::new());
    load_result(Path::new("definitely-missing_res.mat"), &|e| {
        events.borrow_mut().push(e)
    });
    let events = events.into_inner();
    assert!(matches!(
        events.as_slice(),
        [Event::Log(LogTab::Simulation, LogLevel::Error, text)]
            if text.contains("definitely-missing_res.mat")
    ));
}

// --- G50 work folder cleanup / G44 interval length ---

#[test]
fn stale_session_dirs_skips_the_current_and_recent_sessions() {
    use std::time::{Duration, SystemTime};
    let root = std::env::temp_dir().join(format!("omola-g50-list-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for dir in ["session-1", "session-2", "session-3", "other"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    std::fs::write(root.join("session-file"), "not a folder").unwrap();
    let current = root.join("session-2");
    let week = Duration::from_secs(7 * 24 * 3600);

    // Everything was just created: nothing is old enough.
    assert!(stale_session_dirs(&root, &current, SystemTime::now(), week).is_empty());

    // Eight days later the other session folders are stale; the rest is never listed.
    let later = SystemTime::now() + Duration::from_secs(8 * 24 * 3600);
    assert_eq!(
        stale_session_dirs(&root, &current, later, week),
        [root.join("session-1"), root.join("session-3")]
    );
    // A missing root is not an error.
    assert!(stale_session_dirs(&root.join("missing"), &current, later, week).is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn remove_stale_sessions_deletes_only_listed_directories() {
    let root = std::env::temp_dir().join(format!("omola-g50-remove-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for dir in ["session-1", "session-2", "other"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
        std::fs::write(root.join(dir).join("x_res.mat"), "data").unwrap();
    }
    // Fresh folders are younger than a week: nothing is removed.
    assert_eq!(remove_stale_sessions(&root, &root.join("session-2")), 0);
    assert!(root.join("session-1").exists());

    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(30 * 24 * 3600);
    let stale = stale_session_dirs(&root, &root.join("session-2"), later, STALE_SESSION_AGE);
    assert_eq!(stale, [root.join("session-1")]);
    assert!(root.join("other").join("x_res.mat").exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn intervals_for_length_rounds_and_clamps() {
    assert_eq!(intervals_for_length(0.0, 10.0, 0.02), 500);
    assert_eq!(intervals_for_length(0.0, 1.0, 0.3), 3);
    assert_eq!(intervals_for_length(0.0, 1.0, 0.0), 1);
    assert_eq!(intervals_for_length(0.0, 1.0, -0.1), 1);
    assert_eq!(intervals_for_length(0.0, 1.0, f64::NAN), 1);
    assert_eq!(intervals_for_length(0.0, 1.0, 1e-12), 10_000_000);
    assert_eq!(intervals_for_length(1.0, 1.0, 0.1), 1);
}
