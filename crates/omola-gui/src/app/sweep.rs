//! Parameter sweep: the same model simulated once per value of one parameter (G49).
//!
//! The runs are submitted one after another, so `Stop` ends the whole sweep and a failed
//! run does not start the remaining ones. Every finished run is kept and plotted next to
//! the earlier ones, which gives a family of curves.

use super::*;

/// A running sweep. `next` is the index of the value that is submitted next.
#[derive(Debug, Clone, PartialEq)]
pub struct SweepPlan {
    pub class: String,
    pub parameter: String,
    pub values: Vec<f64>,
    pub next: usize,
}

/// Text fields of the "Sweep Parameters" dialog.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SweepDialog {
    pub parameter: String,
    pub from: String,
    pub to: String,
    pub points: String,
    pub error: Option<String>,
}

/// Short text of a sweep value for result labels (`0.7`, not `0.7000000000000001`).
pub fn sweep_value_text(value: f64) -> String {
    let text = format!("{value:.6}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

impl OmolaApp {
    /// Open the "Sweep Parameters" dialog, pre-filled with an edited parameter if any.
    pub fn open_sweep_dialog(&mut self) {
        let parameter = self
            .pending_overrides
            .keys()
            .next()
            .cloned()
            .unwrap_or_default();
        self.sweep_dialog = Some(SweepDialog {
            parameter,
            points: "5".to_owned(),
            ..SweepDialog::default()
        });
    }

    /// Validate the dialog input and start the first run.
    pub fn start_sweep(
        &mut self,
        parameter: &str,
        from: &str,
        to: &str,
        points: &str,
    ) -> Result<(), String> {
        if self.busy || self.sweep.is_some() {
            return Err("Wait for the running job to finish.".to_owned());
        }
        let class = self.active.clone().ok_or("No model is open.")?;
        let parameter = parameter.trim();
        if parameter.is_empty() {
            return Err("Enter the name of a parameter.".to_owned());
        }
        omola_core::validate_parameter_overrides(&[(parameter.to_owned(), "0".to_owned())])
            .map_err(|e| e.to_string())?;
        let from: f64 = from
            .trim()
            .parse()
            .map_err(|_| "From must be a number.".to_owned())?;
        let to: f64 = to
            .trim()
            .parse()
            .map_err(|_| "To must be a number.".to_owned())?;
        let points: usize = points
            .trim()
            .parse()
            .map_err(|_| "Points must be a whole number.".to_owned())?;
        let values = crate::backend::sweep_values(from, to, points)?;
        self.sweep = Some(SweepPlan {
            class,
            parameter: parameter.to_owned(),
            values,
            next: 0,
        });
        self.submit_next_sweep_run();
        if self.sweep.is_none() {
            return Err("The sweep could not be started; see the log.".to_owned());
        }
        Ok(())
    }

    fn submit_next_sweep_run(&mut self) {
        let Some(plan) = self.sweep.as_mut() else {
            return;
        };
        let Some(&value) = plan.values.get(plan.next) else {
            self.sweep = None;
            return;
        };
        plan.next += 1;
        let (class, parameter) = (plan.class.clone(), plan.parameter.clone());
        let (number, total) = (plan.next, plan.values.len());
        self.info(
            LogTab::Simulation,
            format!("Sweep run {number} of {total}: {parameter} = {value}"),
        );
        if !self.submit_simulation(class, Some((parameter, format!("{value}")))) {
            self.sweep = None;
        }
    }

    /// Label suffix of the sweep run that just finished, and whether the curves of the
    /// previous run stay in the diagrams (every run after the first one).
    pub(super) fn sweep_run_tag(&self) -> Option<(String, bool)> {
        let plan = self.sweep.as_ref()?;
        let index = plan.next.checked_sub(1)?;
        let value = *plan.values.get(index)?;
        Some((
            format!(" [{}={}]", plan.parameter, sweep_value_text(value)),
            index > 0,
        ))
    }

    /// A sweep run finished: start the next one or report the end of the sweep.
    pub(super) fn sweep_run_finished(&mut self) {
        let Some(plan) = self.sweep.as_ref() else {
            return;
        };
        if plan.next < plan.values.len() {
            self.submit_next_sweep_run();
        } else {
            let total = plan.values.len();
            self.sweep = None;
            self.info(LogTab::Simulation, format!("Sweep finished: {total} runs."));
        }
    }

    /// A sweep run failed or was stopped: the remaining values are not simulated.
    pub(super) fn sweep_run_failed(&mut self, class: &str) {
        let Some(plan) = self.sweep.take() else {
            return;
        };
        let done = plan.next.saturating_sub(1);
        self.warn(
            LogTab::Simulation,
            format!(
                "Sweep of {class} stopped after {done} of {} runs.",
                plan.values.len()
            ),
        );
    }
}
