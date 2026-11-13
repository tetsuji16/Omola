//! Check, Simulate, the command window and the simulation setup.

use super::*;

impl OmolaApp {
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
        if self.sweep.is_some() {
            return self.warn(
                LogTab::Simulation,
                "A parameter sweep is running; stop it first.".into(),
            );
        }
        let Some(class) = self.active.clone() else {
            return self.error(LogTab::Translation, "No model is open.".into());
        };
        self.submit_simulation(class, None);
    }

    /// Validate the setup and queue one simulation of `class`. `extra_override` (a sweep
    /// value, G49) wins over the setup and the Variable Browser. Returns `false`, with
    /// the reason in the log, when nothing was queued.
    pub(super) fn submit_simulation(
        &mut self,
        class: String,
        extra_override: Option<(String, String)>,
    ) -> bool {
        if self.restriction_of(&class).as_deref() == Some("package") {
            self.error(
                LogTab::Translation,
                format!("{class} is a package and cannot be simulated. Open a model, for example an Examples model."),
            );
            return false;
        }
        let mut setup = self.setup.clone();
        setup.parameters = merge_overrides(&setup.parameters, &self.pending_overrides);
        if let Some((name, value)) = extra_override {
            setup.parameters.retain(|(n, _)| n != &name);
            setup.parameters.push((name, value));
        }
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
            self.error(LogTab::Translation, e.to_string());
            return false;
        }
        if let Err(message) = setup.validate() {
            self.error(LogTab::Translation, message);
            return false;
        }
        self.log_tab = LogTab::Translation;
        self.bottom_pane = BottomPane::Logs;
        self.start_error_scan();
        self.worker.submit(Job::Simulate {
            context: self.context_for(&class),
            class,
            setup,
        });
        true
    }

    /// Translate the active model (`buildModel`) without simulating it (G46).
    pub fn translate(&mut self) {
        let Some(class) = self.active.clone() else {
            return self.error(LogTab::Translation, "No model is open.".into());
        };
        if self.restriction_of(&class).as_deref() == Some("package") {
            return self.error(
                LogTab::Translation,
                format!("{class} is a package and cannot be translated. Open a model."),
            );
        }
        self.log_tab = LogTab::Translation;
        self.bottom_pane = BottomPane::Logs;
        self.start_error_scan();
        self.worker.submit(Job::Translate {
            context: self.context_for(&class),
            class,
        });
    }

    /// Whether "Save in Model" can run: a user model is active and no edit is pending.
    pub fn can_store_setup(&self) -> bool {
        self.pending_edit.is_none() && self.active.as_ref().is_some_and(|c| self.is_user_class(c))
    }

    /// Write the general setup into the `experiment` annotation of the active user
    /// model (G48). Other annotations are kept: the edit goes through the OMC API.
    pub fn save_setup_in_model(&mut self) {
        let Some(class) = self.active.clone().filter(|c| self.is_user_class(c)) else {
            return self.error(
                LogTab::Syntax,
                "Only a user model can store the simulation setup.".into(),
            );
        };
        if let Err(message) = self.setup.validate() {
            return self.error(LogTab::Syntax, message);
        }
        let setup = &self.setup;
        let interval =
            (setup.stop_time - setup.start_time) / (setup.number_of_intervals.max(1) as f64);
        let op = EditOp::SetExperiment {
            start: setup.start_time,
            stop: setup.stop_time,
            tolerance: setup.tolerance,
            interval,
        };
        self.apply_edit(&class, vec![op]);
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

    /// Seed the Simulation Setup dialog from a class's `experiment` annotation.
    ///
    /// The annotation only provides the initial values: once the user has changed the
    /// general fields by hand (`setup_edited_by_user`) their numbers are kept, and the
    /// annotation of the class already applied (`setup_from`) is not applied twice.
    /// A late answer for a class the user has already left is ignored: only the
    /// annotation of the class that is open now seeds the setup (G53).
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
            || self.active.as_deref() != Some(class)
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
