//! Simulation results, the variable tree cache and the plot diagrams.

use super::*;

impl OmolaApp {
    /// Store a new result, drop the oldest beyond `keep_results`, and keep curves on the
    /// newest result of the same model (as a re-simulation replots the same variables).
    #[cfg(test)]
    pub fn add_run(&mut self, outcome: SimulationOutcome) {
        self.add_run_tagged(outcome, None);
    }

    /// Add a result. `sweep` is set for a run of a parameter sweep (G49): the label
    /// suffix, and whether the curves of the previous run stay next to the new ones.
    /// Sweep runs are kept so that the whole family of curves survives `keep_results`.
    pub fn add_run_tagged(&mut self, outcome: SimulationOutcome, sweep: Option<(String, bool)>) {
        let keep_previous_curves = sweep.as_ref().is_some_and(|(_, keep)| *keep);
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
        let label = match &sweep {
            Some((suffix, _)) => format!("{label}{suffix}"),
            None => label,
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
            let moving = !replot.is_empty() && latest_previous.is_some() && !keep_previous_curves;
            diagram
                .signals
                .retain(|s| !(moving && Some(s.run_id) == latest_previous));
            diagram.signals.extend(replot);
        }
        self.runs.push(ResultRun {
            id,
            label,
            outcome,
            kept: sweep.is_some(),
        });
        self.evict_results();
        // A new result has its own time axis: show its last sample again.
        self.variable_time = None;
        self.tab = RibbonTab::Simulation;
        self.log_tab = LogTab::Simulation;
    }

    /// Drop the oldest results beyond `keep_results`. Results pinned with "Keep result"
    /// do not count towards the limit and are never dropped (G14).
    pub fn evict_results(&mut self) {
        let limit = self.keep_results.max(1);
        loop {
            let unpinned = self.runs.iter().filter(|r| !r.kept).count();
            if unpinned <= limit {
                return;
            }
            let Some(oldest) = self.runs.iter().find(|r| !r.kept).map(|r| r.id) else {
                return;
            };
            self.close_result(oldest);
        }
    }

    /// Remove one result together with its curves and cached variable tree (G14).
    pub fn close_result(&mut self, run_id: u64) {
        self.runs.retain(|r| r.id != run_id);
        self.var_tree_cache.remove(&run_id);
        for diagram in &mut self.diagrams {
            diagram.signals.retain(|s| s.run_id != run_id);
        }
        self.variable_time = None;
    }

    /// Pin or unpin a result. Unpinning re-applies the `keep_results` limit (G14).
    pub fn set_result_kept(&mut self, run_id: u64, kept: bool) {
        if let Some(run) = self.runs.iter_mut().find(|r| r.id == run_id) {
            run.kept = kept;
        }
        if !kept {
            self.evict_results();
        }
    }

    /// Read a result file from disk into the Variable Browser (G14).
    pub fn load_result(&mut self, path: PathBuf) {
        self.log_tab = LogTab::Simulation;
        self.bottom_pane = BottomPane::Logs;
        self.worker.submit(Job::LoadResult { path });
    }

    /// Ask for a result file (`*.mat`) and load it (G14).
    pub fn request_load_result_dialog(&mut self) {
        if self.dialog_in_flight() {
            return;
        }
        if Self::use_text_dialogs() {
            self.path_dialog = Some(PathDialog::LoadResult(String::new()));
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel::<DialogResult>();
        self.dialog_rx = Some(rx);
        std::thread::spawn(move || {
            let result = rfd::FileDialog::new()
                .add_filter("Simulation result", &["mat"])
                .pick_file();
            let _ = tx.send(DialogResult::LoadResult(result));
        });
    }

    /// Return the cached variable tree, rebuilding it only when its inputs change.
    pub fn var_tree_for(&mut self, run_id: u64) -> &VarTreeCache {
        let key = VarTreeKey {
            run_id,
            filter: self.variable_filter.to_lowercase(),
            show_parameters: self.variable_show_parameters,
            show_time_varying: self.variable_show_time_varying,
        };
        let current_generation = self
            .var_tree_cache
            .get(&run_id)
            .map_or(0, |cache| cache.generation);
        if self
            .var_tree_cache
            .get(&run_id)
            .is_none_or(|cache| cache.key != key)
        {
            let run = self
                .runs
                .iter()
                .find(|run| run.id == run_id)
                .expect("Variable Browser run id must exist");
            let vars = &run.outcome.result.variables;
            let mut names = Vec::new();
            let mut flat = Vec::new();
            for i in crate::ui::browsers::variable_order(self, run) {
                let var = &vars[i];
                if var.name == "time" {
                    continue;
                }
                if (var.is_parameter && !key.show_parameters)
                    || (!var.is_parameter && !key.show_time_varying)
                {
                    continue;
                }
                if !key.filter.is_empty() && !var.name.to_lowercase().contains(&key.filter) {
                    continue;
                }
                names.push((i, var.name.as_str()));
                if !key.filter.is_empty() {
                    flat.push(i);
                }
            }
            let tree = if key.filter.is_empty() {
                crate::var_tree::build_variable_tree(&names)
            } else {
                crate::var_tree::VarNode::default()
            };
            self.var_tree_cache.insert(
                run_id,
                VarTreeCache {
                    key,
                    tree,
                    flat,
                    generation: current_generation + 1,
                },
            );
        }
        self.var_tree_cache
            .get(&run_id)
            .expect("cache inserted for existing run")
    }

    pub fn run(&self, id: u64) -> Option<&ResultRun> {
        self.runs.iter().find(|r| r.id == id)
    }

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
}
