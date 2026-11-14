//! Unit tests of the application state (no window is opened).

use super::simulation::merge_overrides;
use super::sweep::sweep_value_text;
use super::text_tools::{parse_goto_line, replace_all};
use super::*;

#[test]
fn rename_validation_rejects_keywords_invalid_names_and_duplicates() {
    assert_eq!(component_name_error("abc", &["old".into()], "old"), None);
    assert_eq!(
        component_name_error("1a", &[], "old"),
        Some("Not a valid Modelica identifier")
    );
    assert_eq!(
        component_name_error("model", &[], "old"),
        Some("Not a valid Modelica identifier")
    );
    assert_eq!(
        component_name_error("a b", &[], "old"),
        Some("Not a valid Modelica identifier")
    );
    assert_eq!(
        component_name_error("", &[], "old"),
        Some("Not a valid Modelica identifier")
    );
    assert_eq!(
        component_name_error("taken", &["taken".into()], "old"),
        Some("Name already used")
    );
}
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

fn history_app() -> OmolaApp {
    let mut app = app();
    app.documents
        .push(Document::new_untitled("model A\nend A;"));
    app.refresh_outline();
    app.active = Some("A".into());
    app
}

fn apply_snapshot_edit(app: &mut OmolaApp, text: &str) {
    let doc = app.active_doc().unwrap();
    let before = app.documents[doc].text.clone();
    app.record_undo_snapshot(doc, before);
    app.documents[doc].text = text.into();
}

#[test]
fn editing_user_source_invalidates_user_icons_but_keeps_library_icons() {
    let mut app = history_app();
    app.documents[0].text = "model A\n model Custom\n end Custom;\nend A;".into();
    app.refresh_outline();
    app.icons.insert("A.Custom".into(), Loadable::Loading);
    app.icons
        .insert("Modelica.Blocks.Math.Gain".into(), Loadable::Loading);
    app.user_text_changed(0);
    assert!(!app.icons.contains_key("A.Custom"));
    assert!(app.icons.contains_key("Modelica.Blocks.Math.Gain"));
}

#[test]
fn graphical_edit_snapshots_undo_twice_and_redo_once() {
    let mut app = history_app();
    let original = app.documents[0].text.clone();
    let after_add = "model A\n  Modelica.Blocks.Sources.Constant c;\nend A;";
    let after_move = "model A\n  Modelica.Blocks.Sources.Constant c annotation(Placement(transformation(origin={1, 2})));\nend A;";
    for source in [after_add, after_move] {
        app.handle_model_edited_event(Event::ModelEdited {
            class: "A".into(),
            result: Ok(source.into()),
        });
    }

    app.undo();
    assert!(app.documents[0].text.contains("Constant c;"));
    assert!(!app.documents[0].text.contains("origin={1, 2}"));
    app.undo();
    assert_eq!(app.documents[0].text, original);
    app.redo();
    assert!(app.documents[0].text.contains("Constant c;"));
}

#[test]
fn new_graphical_edit_after_undo_clears_redo() {
    let mut app = history_app();
    apply_snapshot_edit(&mut app, "model A\n  Real x;\nend A;");
    app.undo();
    assert!(app.can_redo());
    apply_snapshot_edit(&mut app, "model A\n  Real y;\nend A;");
    assert!(!app.can_redo());
}

#[test]
fn undo_history_is_capped_at_one_hundred_snapshots() {
    let mut app = history_app();
    for i in 0..101 {
        app.record_undo_snapshot(0, format!("snapshot {i}"));
    }
    assert_eq!(app.documents[0].undo.len(), 100);
    assert_eq!(app.documents[0].undo.first().unwrap(), "snapshot 1");
}

#[test]
fn log_history_is_bounded_and_discards_oldest_entries() {
    let mut app = app();
    for i in 0..=MAX_LOG_ENTRIES {
        app.info(LogTab::Syntax, i.to_string());
    }
    assert_eq!(app.logs.len(), MAX_LOG_ENTRIES);
    assert_eq!(app.logs.first().unwrap().text, "1");
}

#[test]
fn long_unicode_log_entries_keep_the_tail_at_character_boundaries() {
    let mut app = app();
    app.info(LogTab::Syntax, "あ".repeat(MAX_LOG_ENTRY_CHARS + 1));
    let text = &app.logs[0].text;
    assert!(text.starts_with("... (earlier output truncated)\n"));
    assert!(text.ends_with('あ'));
    assert_eq!(
        text.chars().count(),
        MAX_LOG_ENTRY_CHARS + "... (earlier output truncated)\n".chars().count()
    );
}

#[test]
fn clearing_logs_removes_only_the_selected_tab() {
    let mut app = app();
    app.info(LogTab::Simulation, "run output".into());
    app.info(LogTab::Syntax, "syntax output".into());
    app.clear_logs_for_tab(LogTab::Simulation);
    assert_eq!(app.logs.len(), 1);
    assert_eq!(app.logs[0].tab, LogTab::Syntax);
}

#[test]
fn variable_tree_cache_reuses_entries_and_tracks_filter_toggles() {
    let mut app = app();
    let mut run = outcome("BouncingBall");
    run.result.variables.extend(
        ["der(w)", "inertia1.w"]
            .into_iter()
            .map(|name| ResultVariable {
                name: name.into(),
                description: String::new(),
                is_parameter: false,
            }),
    );
    app.add_run(run);
    let id = app.runs[0].id;

    let first = app.var_tree_for(id).generation;
    assert_eq!(app.var_tree_for(id).generation, first);

    app.variable_filter = "w".into();
    let filtered_indices = {
        let filtered = app.var_tree_for(id);
        assert_eq!(filtered.generation, first + 1);
        filtered.flat.clone()
    };
    let matched: Vec<_> = filtered_indices
        .iter()
        .map(|&i| {
            app.run(id).unwrap().outcome.result.variables[i]
                .name
                .as_str()
        })
        .collect();
    assert!(matched.contains(&"der(w)"));
    assert!(matched.contains(&"inertia1.w"));

    app.variable_show_parameters = false;
    let no_parameters = app.var_tree_for(id).generation;
    assert_eq!(no_parameters, first + 2);
    app.variable_show_time_varying = false;
    assert_eq!(app.var_tree_for(id).generation, first + 3);
}

#[test]
fn kept_results_survive_the_keep_results_limit() {
    let mut app = app();
    app.keep_results = 1;
    app.add_run(outcome("First"));
    let first = app.runs[0].id;
    app.set_result_kept(first, true);
    app.add_run(outcome("Second"));
    app.add_run(outcome("Third"));
    let classes: Vec<&str> = app.runs.iter().map(|r| r.outcome.class.as_str()).collect();
    assert_eq!(classes, ["First", "Third"]);
    // Unpinning applies the limit again: the oldest unpinned result goes.
    app.set_result_kept(first, false);
    let classes: Vec<&str> = app.runs.iter().map(|r| r.outcome.class.as_str()).collect();
    assert_eq!(classes, ["Third"]);
}

#[test]
fn closing_a_result_removes_its_curves_and_cache() {
    let mut app = app();
    app.keep_results = 3;
    app.add_run(outcome("First"));
    app.add_run(outcome("Second"));
    let first = app.runs[0].id;
    app.var_tree_for(first);
    app.diagrams[0].signals.push(PlotSignal {
        run_id: first,
        variable: "height".into(),
    });
    app.close_result(first);
    assert_eq!(app.runs.len(), 1);
    assert!(!app.var_tree_cache.contains_key(&first));
    assert!(app.diagrams[0].signals.iter().all(|s| s.run_id != first));
}

#[test]
fn simulate_rejects_a_stop_time_before_the_start_time() {
    let mut app = history_app();
    app.setup.start_time = 2.0;
    app.setup.stop_time = 1.0;
    app.simulate();
    assert!(app
        .logs
        .iter()
        .any(|l| l.level == LogLevel::Error && l.text.contains("Stop time")));
}

#[test]
fn unsaved_documents_lists_only_dirty_documents() {
    let mut app = history_app();
    assert!(app.unsaved_documents().is_empty());
    assert!(!app.must_confirm_exit());
    app.documents[0].dirty = true;
    assert_eq!(app.unsaved_documents(), ["Unnamed.mo*"]);
    assert!(app.must_confirm_exit());
    app.exit_confirmed = true;
    assert!(!app.must_confirm_exit());
}

#[test]
fn evicting_a_result_removes_its_variable_tree_cache() {
    let mut app = app();
    app.keep_results = 1;
    app.add_run(outcome("First"));
    let removed_id = app.runs[0].id;
    app.var_tree_for(removed_id);
    app.add_run(outcome("Second"));
    assert!(!app.var_tree_cache.contains_key(&removed_id));
}

#[test]
fn package_browser_child_lookup_covers_user_and_library_classes() {
    let mut app = app();
    app.user_classes = vec![
        UserClass {
            kind: "package".into(),
            qualified_name: "UserPkg".into(),
            depth: 0,
            description: None,
        },
        UserClass {
            kind: "model".into(),
            qualified_name: "UserPkg.Child".into(),
            depth: 1,
            description: None,
        },
    ];
    assert!(app.has_children_of("UserPkg"));
    let spec = LibrarySpec::new("Modelica", None);
    app.libraries.push(LibraryState::Loaded(LoadedLibrary::new(
        spec,
        "4.1".into(),
        PathBuf::new(),
        vec![
            crate::library::ClassEntry {
                qualified_name: "Modelica".into(),
                restriction: "package".into(),
                comment: String::new(),
            },
            crate::library::ClassEntry {
                qualified_name: "Modelica.Blocks".into(),
                restriction: "package".into(),
                comment: String::new(),
            },
        ],
    )));
    assert!(app.has_children_of("Modelica"));
    assert!(!app.has_children_of("Modelica.Blocks"));
}

#[test]
fn package_search_cache_reuses_matches_and_caps_at_500() {
    let mut app = app();
    app.user_classes = (0..510)
        .map(|i| UserClass {
            kind: "model".into(),
            qualified_name: format!("Search{i}"),
            depth: 0,
            description: None,
        })
        .collect();
    app.package_filter = " Search ".into();
    app.update_package_search_cache();
    let first = app.package_search_cache.as_ref().unwrap().1.as_ptr();
    assert_eq!(app.package_search_cache.as_ref().unwrap().0, "search");
    assert_eq!(app.package_search_cache.as_ref().unwrap().1.len(), 501);
    app.update_package_search_cache();
    assert_eq!(app.package_search_cache.as_ref().unwrap().1.as_ptr(), first);
}

#[test]
fn failed_graphical_edit_does_not_add_an_undo_snapshot() {
    let mut app = history_app();
    app.pending_edit = Some("A".into());
    app.handle_model_edited_event(Event::ModelEdited {
        class: "A".into(),
        result: Err("OMC rejected the edit".into()),
    });
    assert!(app.documents[0].undo.is_empty());
    assert!(!app.can_undo());
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
fn first_run_autoplots_only_in_active_diagram() {
    let mut app = app();
    app.new_diagram();
    app.add_run(outcome("A"));
    assert!(app.diagrams[0].signals.is_empty());
    assert_eq!(app.diagrams[1].signals.len(), 1);
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
fn the_first_sweep_run_plots_next_to_another_models_curves() {
    let mut app = app();
    app.add_run(outcome("Other"));
    assert_eq!(app.active_signals().len(), 1, "the first result plots");
    app.add_run_tagged(outcome("A"), Some((" [e=0]".into(), false)));
    assert_eq!(
        app.active_signals(),
        [
            PlotSignal {
                run_id: 1,
                variable: "height".into()
            },
            PlotSignal {
                run_id: 2,
                variable: "height".into()
            },
        ],
        "the first sweep run joins the other model's curve"
    );
    app.add_run_tagged(outcome("A"), Some((" [e=1]".into(), true)));
    let ids: Vec<u64> = app.active_signals().iter().map(|s| s.run_id).collect();
    assert_eq!(ids, vec![1, 2, 3]);
}

#[test]
fn a_sweep_plots_into_the_active_diagram_only() {
    let mut app = app();
    app.add_run(outcome("Other"));
    app.new_diagram();
    app.add_run_tagged(outcome("A"), Some((" [e=0]".into(), false)));
    assert_eq!(
        app.diagrams[0].signals,
        [PlotSignal {
            run_id: 1,
            variable: "height".into()
        }],
        "the other model's diagram is untouched"
    );
    assert_eq!(
        app.diagrams[1].signals,
        [PlotSignal {
            run_id: 2,
            variable: "height".into()
        }]
    );
}

#[test]
fn a_sweep_plots_when_its_previous_run_is_not_plotted() {
    let mut app = app();
    app.add_run(outcome("Other"));
    app.add_run(outcome("A"));
    app.add_run_tagged(outcome("A"), Some((" [e=0]".into(), false)));
    assert_eq!(
        app.active_signals(),
        [
            PlotSignal {
                run_id: 1,
                variable: "height".into()
            },
            PlotSignal {
                run_id: 3,
                variable: "height".into()
            },
        ]
    );
}

#[test]
fn a_plain_run_does_not_autoplot_next_to_other_curves() {
    let mut app = app();
    app.add_run(outcome("Other"));
    app.add_run(outcome("A"));
    assert_eq!(
        app.active_signals(),
        [PlotSignal {
            run_id: 1,
            variable: "height".into()
        }]
    );
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
    app.active = Some("Annotated".into());
    app.apply_experiment("Annotated", "model", Some(experiment()));
    assert_eq!(app.setup.start_time, 0.5);
    assert_eq!(app.setup.stop_time, 5.0);
    assert_eq!(app.setup.tolerance, 1e-5);
    assert_eq!(app.setup.number_of_intervals, 450);
    assert_eq!(app.setup_from.as_deref(), Some("Annotated"));

    // Another model's annotation replaces the seeded values ...
    app.active = Some("Other".into());
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
    app.active = Some("Modelica.Blocks".into());
    app.apply_experiment("Modelica.Blocks", "package", Some(experiment()));
    assert_eq!(app.setup.stop_time, 2.0);
    app.active = Some("Other".into());
    app.apply_experiment("Other", "model", Some(experiment()));
    assert_eq!(app.setup.stop_time, 2.0);
}

#[test]
fn manual_setup_edits_survive_a_later_annotation() {
    let mut app = app();
    app.active = Some("Annotated".into());
    app.setup.stop_time = 7.0;
    app.setup_edited_by_user = true;
    app.apply_experiment("Annotated", "model", Some(experiment()));
    assert_eq!(app.setup.stop_time, 7.0);
    assert!(app.setup_from.is_none());
}

#[test]
fn a_late_annotation_of_a_class_the_user_left_is_ignored() {
    let mut app = app();
    app.active = Some("Current".into());
    app.apply_experiment("Current", "model", Some(experiment()));
    assert_eq!(app.setup.stop_time, 5.0);
    app.active = Some("Next".into());
    app.apply_experiment(
        "Left",
        "model",
        Some(library::Experiment {
            stop_time: 99.0,
            ..experiment()
        }),
    );
    assert_eq!(app.setup.stop_time, 5.0);
    assert_eq!(app.setup_from.as_deref(), Some("Current"));
}

#[test]
fn cached_experiment_is_applied_when_class_is_reopened() {
    let mut app = app();
    app.docs.insert(
        "Left".into(),
        Loadable::Ready(DocPage {
            doc: library::ClassDoc {
                class: "Left".into(),
                restriction: "model".into(),
                comment: String::new(),
                info_html: String::new(),
                revisions_html: String::new(),
                roots: BTreeMap::new(),
                experiment: Some(experiment()),
            },
            info: Vec::new(),
        }),
    );
    app.open_class("Left");
    assert_eq!(app.setup.stop_time, 5.0);
    assert_eq!(app.setup_from.as_deref(), Some("Left"));
}

#[test]
fn pending_result_copy_holds_window_close() {
    let mut app = app();
    app.pending_result_copies = 1;
    assert!(app.hold_close_for_edit());
    assert!(app.hold_close_for_edit());
    app.pending_result_copies = 0;
    assert!(!app.hold_close_for_edit());
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
fn an_invalid_edit_does_not_drop_the_setup_override() {
    let mut edited = BTreeMap::new();
    edited.insert("L".to_owned(), "-".to_owned());
    let merged = merge_overrides(
        &[("L".into(), "1".into()), ("m".into(), "3".into())],
        &edited,
    );
    assert_eq!(
        merged,
        vec![
            ("L".to_owned(), "1".to_owned()),
            ("m".to_owned(), "3".to_owned())
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

#[test]
fn context_includes_classes_from_other_open_documents() {
    let mut app = app();
    app.documents.push(Document::new_untitled(
        "model Helper\n  Real x;\nend Helper;",
    ));
    app.documents
        .push(Document::new_untitled("model Main\n  Helper h;\nend Main;"));
    app.refresh_outline();
    let source = app.context_for("Main").user_source.unwrap();
    assert!(source.contains("model Main"));
    assert!(
        source.contains("model Helper"),
        "the dependency's buffer is included"
    );
    assert_eq!(source.matches("model Main").count(), 1);
    assert!(app.context_for("Modelica.Blocks").user_source.is_none());
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
fn zoom_diagram_is_clamped_and_reset_restores_defaults() {
    let mut app = app();
    app.zoom_diagram(1000.0);
    assert_eq!(app.diagram_zoom, DIAGRAM_ZOOM_MAX);
    app.zoom_diagram(0.0001);
    assert_eq!(app.diagram_zoom, DIAGRAM_ZOOM_MIN);
    app.diagram_pan = egui::Vec2::new(5.0, 5.0);
    app.reset_diagram_view();
    assert_eq!(app.diagram_zoom, 1.0);
    assert_eq!(app.diagram_pan, egui::Vec2::ZERO);
}

#[test]
fn closing_active_tab_resets_the_next_diagram_view() {
    let mut app = app();
    app.open_in_new_tab("First");
    app.open_in_new_tab("Second");
    app.diagram_zoom = 2.0;
    app.diagram_pan = egui::Vec2::new(5.0, 5.0);
    app.close_tab("Second");
    assert_eq!(app.active.as_deref(), Some("First"));
    assert_eq!(app.diagram_zoom, 1.0);
    assert_eq!(app.diagram_pan, egui::Vec2::ZERO);
}

#[test]
fn persisted_state_roundtrips_ribbon_collapsed() {
    let mut original = app();
    original.ribbon_collapsed = true;
    let state = original.to_persisted();
    let json = serde_json::to_string(&state).unwrap();
    let restored: PersistedState = serde_json::from_str(&json).unwrap();
    let mut app2 = OmolaApp::with_worker(Worker::spawn(
        std::env::temp_dir().join("omola-gui-unused"),
        || {},
    ));
    app2.apply_persisted(restored, None);
    assert!(app2.ribbon_collapsed);
    let old: PersistedState = serde_json::from_str("{}").unwrap();
    assert_eq!(old.ribbon_collapsed, None);
    let mut app3 = app();
    app3.apply_persisted(old, None);
    assert!(!app3.ribbon_collapsed);
}

#[test]
fn missing_last_file_does_not_panic() {
    let state = PersistedState {
        last_file: Some(PathBuf::from("/nonexistent/path/file.mo")),
        tab: Some("Text".to_owned()),
        setup: Some(SimulationSetup::default()),
        keep_results: Some(3),
        recent_files: Vec::new(),
        ribbon_collapsed: None,
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
        ribbon_collapsed: None,
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
fn requests_are_ignored_while_a_dialog_is_open() {
    let mut app = app();
    app.path_dialog = Some(PathDialog::Open(String::new()));
    app.request_open_dialog();
    app.request_save_as_dialog();
    assert!(matches!(app.path_dialog, Some(PathDialog::Open(_))));
    assert!(app.dialog_rx.is_none());

    app.path_dialog = None;
    app.dialog_rx = Some(std::sync::mpsc::channel().1);
    app.request_open_dialog();
    assert!(app.path_dialog.is_none());
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
    app.push_log(LogEntry {
        tab: LogTab::Syntax,
        level: LogLevel::Error,
        text: "[A.mo:3:1-3:2:writable] Error: old".into(),
    });
    app.refresh_error_locations();
    assert_eq!(app.error_locations.len(), 1);

    // A new Check must not keep the markers of the previous run.
    app.start_error_scan();
    app.push_log(LogEntry {
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

// --- G42 close / G43 overwrite and disk conflict / G14 save result ---

/// A fresh directory for one test.
fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("omola-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// An app with `file` (a model named `name`) opened; returns the document index.
fn app_with_file(dir: &std::path::Path, name: &str) -> (OmolaApp, PathBuf, usize) {
    let file = dir.join(format!("{name}.mo"));
    std::fs::write(&file, format!("model {name}\n  Real x;\nend {name};\n")).unwrap();
    let mut app = app();
    app.open_file(file.clone());
    let doc = app
        .documents
        .iter()
        .position(|d| d.path.as_deref() == Some(file.as_path()))
        .expect("the file is open");
    (app, file, doc)
}

fn has_log(app: &OmolaApp, level: LogLevel, needle: &str) -> bool {
    app.logs
        .iter()
        .any(|l| l.level == level && l.text.contains(needle))
}

/// Drive the event loop until a log line arrives (worker jobs are asynchronous).
fn wait_for_log(app: &mut OmolaApp, level: LogLevel, needle: &str) {
    for _ in 0..500 {
        app.handle_events();
        if has_log(app, level, needle) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("log line {needle:?} did not arrive from the worker");
}

#[test]
fn closing_a_clean_document_removes_it_without_a_question() {
    let mut app = history_app();
    app.model_tabs.push("A".into());
    app.request_close_document(0);
    assert!(app.pending_close.is_none());
    assert!(app.documents.is_empty());
    assert!(app.active.is_none());
    assert!(!app.model_tabs.iter().any(|t| t == "A"));
}

#[test]
fn closing_a_modified_document_asks_and_discard_drops_it() {
    let mut app = history_app();
    app.documents[0].dirty = true;
    app.close_active();
    assert_eq!(app.pending_close, Some(0));
    assert_eq!(app.documents.len(), 1, "nothing closes before the answer");
    app.confirm_close_discard();
    assert!(app.pending_close.is_none());
    assert!(app.documents.is_empty());
    assert!(!app.must_confirm_exit());
}

#[test]
fn save_in_the_close_question_writes_the_file_and_closes() {
    let dir = test_dir("g42-save");
    let (mut app, file, doc) = app_with_file(&dir, "Keep");
    app.documents[doc].text = "model Keep\n  Real y;\nend Keep;\n".into();
    app.documents[doc].dirty = true;
    app.request_close_document(doc);
    assert_eq!(app.pending_close, Some(doc));
    app.confirm_close_save();
    assert!(std::fs::read_to_string(&file).unwrap().contains("Real y;"));
    assert!(!app
        .documents
        .iter()
        .any(|d| d.path.as_deref() == Some(file.as_path())));
    assert!(app.close_after_save.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_document_with_a_running_edit_is_not_closed() {
    let mut app = history_app();
    app.pending_edit = Some("A".into());
    app.close_document(0);
    assert_eq!(app.documents.len(), 1);
    assert!(has_log(&app, LogLevel::Error, "still running"));
}

#[test]
fn save_all_writes_named_documents_and_reports_the_rest() {
    let dir = test_dir("g42-save-all");
    let (mut app, file, doc) = app_with_file(&dir, "Named");
    app.documents[doc].text.push_str("// edited\n");
    app.documents[doc].dirty = true;
    app.documents
        .push(Document::new_untitled("model B\nend B;"));
    let untitled = app.documents.len() - 1;
    app.documents[untitled].dirty = true;
    assert!(app.can_save_all());

    assert!(!app.save_all_and_exit());
    assert!(!app.exit_confirmed);
    assert!(has_log(&app, LogLevel::Error, "use Save As"));
    assert!(std::fs::read_to_string(&file)
        .unwrap()
        .contains("// edited"));
    assert!(!app.documents[doc].dirty);

    app.documents[untitled].dirty = false;
    app.documents[doc].dirty = true;
    assert!(app.save_all_and_exit());
    assert!(app.exit_confirmed);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_changed_on_disk_is_not_overwritten_silently() {
    let dir = test_dir("g43-conflict");
    let (mut app, file, doc) = app_with_file(&dir, "Shared");
    let other = "model Shared\n  Real fromOtherProgram;\nend Shared;\n";
    std::fs::write(&file, other).unwrap();
    app.documents[doc].text = "model Shared\n  Real mine;\nend Shared;\n".into();
    app.documents[doc].dirty = true;

    assert!(!app.save_document(doc, false));
    assert_eq!(app.save_conflict, Some(doc));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), other);

    // Cancel changes nothing.
    app.resolve_save_conflict(ConflictAnswer::Cancel);
    assert!(app.save_conflict.is_none());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), other);
    assert!(app.documents[doc].dirty);

    // Reload takes the text of the other program.
    assert!(!app.save_document(doc, false));
    app.resolve_save_conflict(ConflictAnswer::Reload);
    assert_eq!(app.documents[doc].text, other);
    assert!(!app.documents[doc].dirty);

    // Overwrite keeps the text of the editor.
    std::fs::write(&file, "model Shared\nend Shared;\n").unwrap();
    app.documents[doc].text = "model Shared\n  Real mine;\nend Shared;\n".into();
    app.documents[doc].dirty = true;
    assert!(!app.save_document(doc, false));
    app.resolve_save_conflict(ConflictAnswer::Overwrite);
    assert!(std::fs::read_to_string(&file)
        .unwrap()
        .contains("Real mine;"));
    assert!(!app.documents[doc].dirty);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_all_skips_a_file_changed_on_disk() {
    let dir = test_dir("g43-save-all");
    let (mut app, file, doc) = app_with_file(&dir, "Busy");
    std::fs::write(&file, "model Busy\n  Real other;\nend Busy;\n").unwrap();
    app.documents[doc].dirty = true;
    let problems = app.save_all();
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("changed on disk"));
    assert!(std::fs::read_to_string(&file)
        .unwrap()
        .contains("Real other;"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn writing_over_an_existing_file_waits_for_the_answer() {
    let dir = test_dir("g43-replace");
    let target = dir.join("Existing.mo");
    std::fs::write(&target, "old").unwrap();
    let mut app = history_app();

    app.request_write(PendingWrite::SaveAs(target.clone()), true);
    assert_eq!(
        app.overwrite_confirm,
        Some(PendingWrite::SaveAs(target.clone()))
    );
    app.resolve_overwrite(false);
    assert!(app.overwrite_confirm.is_none());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "old");
    assert!(app.documents[0].path.is_none());

    app.request_write(PendingWrite::SaveAs(target.clone()), true);
    app.resolve_overwrite(true);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "model A\nend A;");
    assert_eq!(app.documents[0].path.as_deref(), Some(target.as_path()));

    // Saving to its own file again is no "replace" question.
    app.request_write(PendingWrite::SaveAs(target.clone()), true);
    assert!(app.overwrite_confirm.is_none());

    // A native dialog asked already: no second question.
    let csv = dir.join("new.mo");
    app.request_write(PendingWrite::SaveAs(csv.clone()), false);
    assert!(app.overwrite_confirm.is_none());
    assert!(csv.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_as_refuses_a_file_that_is_open_in_another_document() {
    let dir = test_dir("g43-open-twice");
    let (mut app, file, _doc) = app_with_file(&dir, "First");
    app.documents
        .push(Document::new_untitled("model B\nend B;"));
    let second = app.documents.len() - 1;
    app.save_as_doc = Some(second);
    app.save_as(file.clone());
    assert!(app.documents[second].path.is_none());
    assert!(std::fs::read_to_string(&file)
        .unwrap()
        .contains("model First"));
    assert!(has_log(&app, LogLevel::Error, "open in another document"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_result_as_copies_the_result_file() {
    let dir = test_dir("g14-save-result");
    let source = dir.join("A_res.mat");
    std::fs::write(&source, b"result bytes").unwrap();
    let mut app = app();
    let mut run = outcome("A");
    run.result_file = source.clone();
    app.add_run(run);
    let id = app.runs[0].id;

    let dest = dir.join("copy.mat");
    app.save_result_as(id, &dest);
    // The copy runs on the worker thread: nothing is logged on the UI thread yet (G53).
    assert!(!has_log(&app, LogLevel::Info, "Saved the result to"));
    wait_for_log(&mut app, LogLevel::Info, "Saved the result to");
    assert_eq!(std::fs::read(&dest).unwrap(), b"result bytes");

    app.save_result_as(id, &source);
    wait_for_log(&mut app, LogLevel::Error, "already stored in that file");
    app.save_result_as(id + 100, &dest);
    assert!(has_log(&app, LogLevel::Error, "no longer open"));
    let _ = std::fs::remove_dir_all(&dir);
}

// --- G45 go to line / replace ---

#[test]
fn goto_line_input_is_validated() {
    assert_eq!(parse_goto_line(" 3 ", 10), Ok(3));
    assert_eq!(parse_goto_line("10", 10), Ok(10));
    assert!(parse_goto_line("0", 10).is_err());
    assert!(parse_goto_line("11", 10).is_err());
    assert!(parse_goto_line("x", 10).is_err());
    assert!(parse_goto_line("-1", 10).is_err());
    // An empty text still has line 1.
    assert_eq!(parse_goto_line("1", 0), Ok(1));
}

#[test]
fn replace_all_counts_the_replacements() {
    assert_eq!(replace_all("a b a", "a", "c"), ("c b c".to_owned(), 2));
    assert_eq!(replace_all("a b a", "", "c"), ("a b a".to_owned(), 0));
    assert_eq!(replace_all("a b a", "z", "c"), ("a b a".to_owned(), 0));
    assert_eq!(replace_all("aaa", "aa", "b"), ("ba".to_owned(), 1));
}

#[test]
fn goto_line_selects_the_text_layer_and_the_line() {
    let mut app = history_app();
    assert_eq!(app.goto_line("2"), Ok(2));
    assert_eq!(app.documents[0].editor.highlight_line, Some(2));
    assert_eq!(app.tab, RibbonTab::Text);
    assert!(app.goto_line("3").is_err());

    // A library class is read-only: the dialog does not open.
    app.active = Some("Modelica.Blocks.Sources.Step".into());
    app.open_goto_line_dialog();
    assert!(app.goto_line_dialog.is_none());
    app.open_replace_dialog();
    assert!(app.replace_dialog.is_none());
}

#[test]
fn replace_in_the_active_document_is_one_undo_step() {
    let mut app = history_app();
    let before = "model A\n  Real x;\n  Real y;\nend A;";
    app.documents[0].text = before.into();
    assert_eq!(app.replace_in_active("Real", "Integer"), Ok(2));
    assert_eq!(
        app.documents[0].text,
        "model A\n  Integer x;\n  Integer y;\nend A;"
    );
    assert!(app.documents[0].dirty);
    app.undo();
    assert_eq!(app.documents[0].text, before);

    assert!(app.replace_in_active("", "x").is_err());
    assert!(app.replace_in_active("missing", "x").is_err());
    assert_eq!(app.documents[0].text, before);
}

// --- G46 translate / G48 save in model / G49 sweep ---

#[test]
fn translate_rejects_a_package_and_a_missing_model() {
    let mut app = app();
    app.translate();
    assert!(has_log(&app, LogLevel::Error, "No model is open."));

    app.documents
        .push(Document::new_untitled("package P\nend P;"));
    app.refresh_outline();
    app.active = Some("P".into());
    app.translate();
    assert!(has_log(&app, LogLevel::Error, "cannot be translated"));
}

#[test]
fn save_in_model_needs_a_user_model_and_queues_one_edit() {
    let mut app = app();
    assert!(!app.can_store_setup());
    app.save_setup_in_model();
    assert!(has_log(&app, LogLevel::Error, "Only a user model"));

    let mut app = history_app();
    assert!(app.can_store_setup());
    app.setup.start_time = 2.0;
    app.setup.stop_time = 1.0;
    app.save_setup_in_model();
    assert!(app.pending_edit.is_none(), "an invalid setup is not stored");

    app.setup.start_time = 0.0;
    app.setup.stop_time = 4.0;
    app.save_setup_in_model();
    assert_eq!(app.pending_edit.as_deref(), Some("A"));
    assert!(!app.can_store_setup(), "one edit at a time");
}

#[test]
fn sweep_value_text_is_short() {
    assert_eq!(sweep_value_text(0.0), "0");
    assert_eq!(sweep_value_text(0.5), "0.5");
    assert_eq!(sweep_value_text(0.1 + 0.2), "0.3");
    assert_eq!(sweep_value_text(-2.0), "-2");
    assert_eq!(sweep_value_text(10.0), "10");
}

#[test]
fn sweep_input_is_validated() {
    let mut app = history_app();
    assert!(app.start_sweep("", "0", "1", "3").is_err());
    assert!(app.start_sweep("e", "x", "1", "3").is_err());
    assert!(app.start_sweep("e", "0", "", "3").is_err());
    assert!(app.start_sweep("e", "0", "1", "1.5").is_err());
    assert!(app.start_sweep("e", "0", "1", "1").is_err());
    assert!(app.start_sweep("e", "0", "0", "3").is_err());
    assert!(app.start_sweep("e; system(1)", "0", "1", "3").is_err());
    assert!(app.sweep.is_none());

    app.busy = true;
    assert!(app.start_sweep("e", "0", "1", "3").is_err());
    app.busy = false;
    app.active = None;
    assert!(app.start_sweep("e", "0", "1", "3").is_err());
}

/// What `handle_events` does for `Event::SimulationFinished`.
fn finish_sweep_run(app: &mut OmolaApp) {
    let tag = app.sweep_run_tag();
    app.add_run_tagged(outcome("A"), tag);
    app.sweep_run_finished();
}

#[test]
fn sweep_runs_every_value_and_keeps_all_results() {
    let mut app = history_app();
    app.keep_results = 1;
    assert_eq!(app.start_sweep(" e ", "0", "1", "3"), Ok(()));
    let plan = app.sweep.clone().unwrap();
    assert_eq!(plan.parameter, "e");
    assert_eq!(plan.values, [0.0, 0.5, 1.0]);
    assert_eq!(plan.next, 1);

    // A plain Simulate does not cut into the sweep.
    app.simulate();
    assert!(has_log(&app, LogLevel::Warning, "sweep is running"));

    finish_sweep_run(&mut app);
    assert_eq!(app.sweep.as_ref().unwrap().next, 2);
    finish_sweep_run(&mut app);
    finish_sweep_run(&mut app);
    assert!(app.sweep.is_none());
    assert!(has_log(&app, LogLevel::Info, "Sweep finished: 3 runs."));

    assert_eq!(app.runs.len(), 3, "sweep runs survive keep_results = 1");
    assert!(app.runs.iter().all(|r| r.kept));
    let labels: Vec<&str> = app.runs.iter().map(|r| r.label.as_str()).collect();
    assert!(labels[0].ends_with("[e=0]"), "{labels:?}");
    assert!(labels[1].ends_with("[e=0.5]"), "{labels:?}");
    assert!(labels[2].ends_with("[e=1]"), "{labels:?}");
}

#[test]
fn a_failed_sweep_run_stops_the_sweep() {
    let mut app = history_app();
    app.start_sweep("e", "0", "1", "3").unwrap();
    finish_sweep_run(&mut app);
    app.sweep_run_failed("A");
    assert!(app.sweep.is_none());
    assert!(has_log(
        &app,
        LogLevel::Warning,
        "stopped after 1 of 3 runs"
    ));
    assert_eq!(app.runs.len(), 1);

    // A failed simulation outside a sweep reports nothing extra.
    let warnings = app.logs.len();
    app.sweep_run_failed("A");
    assert_eq!(app.logs.len(), warnings);
}

#[test]
fn the_sweep_dialog_starts_with_an_edited_parameter() {
    let mut app = history_app();
    app.open_sweep_dialog();
    assert_eq!(app.sweep_dialog.as_ref().unwrap().parameter, "");
    app.pending_overrides.insert("e".into(), "0.7".into());
    app.open_sweep_dialog();
    let dialog = app.sweep_dialog.clone().unwrap();
    assert_eq!(dialog.parameter, "e");
    assert_eq!(dialog.points, "5");
}

// --- G44 ---

#[test]
fn lowering_keep_results_evicts_old_results() {
    let mut app = app();
    app.keep_results = 5;
    for class in ["First", "Second", "Third"] {
        app.add_run(outcome(class));
    }
    app.keep_results = 1;
    app.evict_results();
    assert_eq!(app.runs.len(), 1);
    assert_eq!(app.runs[0].outcome.class, "Third");

    // A kept result does not count and is never closed.
    app.runs[0].kept = true;
    app.keep_results = 5;
    app.add_run(outcome("Fourth"));
    app.add_run(outcome("Fifth"));
    app.keep_results = 1;
    app.evict_results();
    let classes: Vec<&str> = app.runs.iter().map(|r| r.outcome.class.as_str()).collect();
    assert_eq!(classes, ["Third", "Fifth"]);
}

// --- G53: review follow-up ---

#[test]
fn replace_question_blocks_another_save_as() {
    let dir = test_dir("g53-overwrite");
    let (mut app, _file, doc) = app_with_file(&dir, "A");
    let other = dir.join("Other.mo");
    std::fs::write(&other, "model Other end Other;").unwrap();
    app.save_as_doc = Some(doc);
    app.request_write(PendingWrite::SaveAs(other.clone()), true);
    assert!(app.overwrite_confirm.is_some());

    // While "Replace file?" is open no other Save As may take over `save_as_doc`.
    assert!(app.dialog_in_flight());
    app.new_model();
    let second = app.documents.len() - 1;
    app.request_save_as_dialog_for(second);
    assert_eq!(app.save_as_doc, Some(doc));

    app.resolve_overwrite(true);
    assert!(std::fs::read_to_string(&other).unwrap().contains("model A"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_a_document_cancels_a_pending_save_as() {
    let dir = test_dir("g53-stale-save-as");
    let (mut app, file_a, doc_a) = app_with_file(&dir, "A");
    let file_b = dir.join("B.mo");
    std::fs::write(&file_b, "model B\nend B;\n").unwrap();
    app.open_file(file_b.clone());
    let doc_b = app
        .documents
        .iter()
        .position(|d| d.path.as_deref() == Some(file_b.as_path()))
        .unwrap();

    // Text fallback: the path question of A is open, then B is closed.
    app.save_as_doc = Some(doc_a);
    app.path_dialog = Some(PathDialog::SaveAs(String::new()));
    app.close_document(doc_b);
    assert!(app.path_dialog.is_none() && app.save_as_doc.is_none());
    assert!(!app.stale_save_as, "no native dialog is open");

    // Native dialog: it cannot be closed, so its answer is ignored.
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog_rx = Some(rx);
    app.save_as_doc = Some(0);
    app.open_file(file_b.clone());
    let doc_b = app.documents.len() - 1;
    app.close_document(doc_b);
    assert!(app.stale_save_as);
    let target = dir.join("Target.mo");
    tx.send(DialogResult::SaveAs(Some(target.clone()))).unwrap();
    app.poll_dialog();
    assert!(!target.exists(), "the stale answer wrote nothing");
    assert!(!app.stale_save_as);
    assert!(has_log(&app, LogLevel::Warning, "Save As was cancelled"));
    assert!(file_a.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_as_onto_the_own_file_still_detects_a_change_on_disk() {
    let dir = test_dir("g53-own-file");
    let (mut app, file, doc) = app_with_file(&dir, "A");
    app.documents[doc].text.push_str("// mine\n");
    app.documents[doc].dirty = true;
    std::fs::write(&file, "model A\n  Real other;\nend A;\n").unwrap();

    app.save_as_doc = Some(doc);
    app.request_write(PendingWrite::SaveAs(file.clone()), true);
    assert!(
        app.overwrite_confirm.is_none(),
        "the own file needs no Replace question"
    );
    assert_eq!(app.save_conflict, Some(doc));
    assert!(std::fs::read_to_string(&file).unwrap().contains("other"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_all_and_exit_waits_for_a_running_model_edit() {
    let dir = test_dir("g53-exit-edit");
    let (mut app, file, doc) = app_with_file(&dir, "A");
    let on_disk = std::fs::read_to_string(&file).unwrap();
    app.documents[doc].text.push_str("// edited\n");
    app.documents[doc].dirty = true;
    app.pending_edit = Some("A".to_owned());

    assert!(!app.save_all_and_exit());
    assert!(!app.exit_confirmed);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), on_disk);
    assert!(has_log(
        &app,
        LogLevel::Error,
        "model edit is still running"
    ));

    app.pending_edit = None;
    assert!(app.save_all_and_exit());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_edit_result_for_an_older_text_is_not_applied() {
    let dir = test_dir("g53-edit-race");
    let (mut app, _file, doc) = app_with_file(&dir, "A");
    let submitted = app.documents[doc].text.clone();
    app.pending_edit = Some("A".to_owned());
    app.pending_edit_text = Some(submitted.clone());

    // The user types in the Text layer before OMC answers.
    app.documents[doc].text = submitted.replace("Real x;", "Real x;\n  Real typed;");
    let typed = app.documents[doc].text.clone();
    app.handle_model_edited_event(Event::ModelEdited {
        class: "A".to_owned(),
        result: Ok("model A\n  Real fromOmc;\nend A;".to_owned()),
    });
    assert_eq!(app.documents[doc].text, typed);
    assert!(app.pending_edit.is_none() && app.pending_edit_text.is_none());
    assert!(has_log(&app, LogLevel::Warning, "the edit was not applied"));

    // An unchanged text takes the result as before.
    app.pending_edit = Some("A".to_owned());
    app.pending_edit_text = Some(typed);
    app.handle_model_edited_event(Event::ModelEdited {
        class: "A".to_owned(),
        result: Ok("model A\n  Real fromOmc;\nend A;".to_owned()),
    });
    assert!(app.documents[doc].text.contains("fromOmc"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unreadable_file_counts_as_changed_on_disk() {
    let dir = test_dir("g53-unreadable");
    let (mut app, file, doc) = app_with_file(&dir, "A");
    // A missing file is not a conflict: saving recreates it.
    std::fs::remove_file(&file).unwrap();
    assert!(!app.documents[doc].changed_on_disk());
    // A path that exists but cannot be read as a file (here: a folder).
    std::fs::create_dir_all(&file).unwrap();
    assert!(app.documents[doc].changed_on_disk());
    app.documents[doc].dirty = true;
    assert!(!app.save_document(doc, false));
    assert_eq!(app.save_conflict, Some(doc));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_as_onto_the_own_file_is_recognised_by_another_spelling() {
    let dir = test_dir("g53-own-file-spelling");
    let (mut app, file, doc) = app_with_file(&dir, "A");
    app.documents[doc].dirty = true;
    std::fs::write(&file, "model A\n  Real other;\nend A;\n").unwrap();

    // The same file, spelled with a detour through a sub-folder.
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let spelled = dir.join("sub").join("..").join(file.file_name().unwrap());
    assert_ne!(spelled, file);
    app.save_as_doc = Some(doc);
    app.request_write(PendingWrite::SaveAs(spelled), true);
    assert!(app.overwrite_confirm.is_none());
    assert_eq!(app.save_conflict, Some(doc));
    assert!(std::fs::read_to_string(&file).unwrap().contains("other"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_first_close_request_waits_for_a_running_model_edit() {
    let mut app = app();
    assert!(!app.hold_close_for_edit(), "nothing is running");
    app.pending_edit = Some("A".to_owned());
    app.docks.bottom = false;
    app.bottom_pane = BottomPane::Commands;
    assert!(app.hold_close_for_edit());
    assert!(has_log(&app, LogLevel::Warning, "close again to exit"));
    assert!(app.docks.bottom && app.bottom_pane == BottomPane::Logs);
    assert!(app.log_tab == LogTab::Syntax);
    // A second request is let through: a hanging edit must not trap the user.
    assert!(!app.hold_close_for_edit());

    // The next edit is protected again.
    app.handle_model_edited_event(Event::ModelEdited {
        class: "A".to_owned(),
        result: Err("failed".to_owned()),
    });
    app.pending_edit = Some("A".to_owned());
    assert!(app.hold_close_for_edit());
}
