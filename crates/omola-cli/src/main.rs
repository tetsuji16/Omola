use omola_core::{BackendError, Omc, SimulationConfig};
use std::{env, path::PathBuf, process::ExitCode};

mod bridge;
mod compat;
mod fmi;
mod licenses;
mod omedit;
mod omedit_evidence;
mod project;
mod release;
mod results;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("compat") {
        return match compat::run(&args[1..]) {
            Ok((text, has_failures)) => {
                print!("{text}");
                if has_failures {
                    ExitCode::FAILURE
                } else {
                    ExitCode::SUCCESS
                }
            }
            Err(error) => {
                eprintln!("omola: {error}");
                ExitCode::FAILURE
            }
        };
    }
    if args.first().map(String::as_str) == Some("bridge") {
        return match bridge::run(&args[1..]) {
            Ok(text) => {
                print!("{text}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("omola: {error}");
                ExitCode::FAILURE
            }
        };
    }
    if args.first().map(String::as_str) == Some("results") {
        return match results::run(&args[1..]) {
            Ok(text) => {
                print!("{text}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("omola: {error}");
                ExitCode::FAILURE
            }
        };
    }
    if args.first().map(String::as_str) == Some("fmi") {
        return match fmi::run(&args[1..]) {
            Ok(text) => {
                print!("{text}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("omola: {error}");
                ExitCode::FAILURE
            }
        };
    }
    if args.first().map(String::as_str) == Some("project") {
        return match project::run(&args[1..]) {
            Ok(text) => {
                print!("{text}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("omola: {error}");
                ExitCode::FAILURE
            }
        };
    }
    if args.first().map(String::as_str) == Some("omedit") {
        return match omedit_evidence::run(&args[1..]) {
            Ok(text) => {
                print!("{text}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("omola: {error}");
                ExitCode::FAILURE
            }
        };
    }
    if args.first().map(String::as_str) == Some("licenses") {
        return match licenses::run(&args[1..]) {
            Ok(text) => {
                print!("{text}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("omola: {error}");
                ExitCode::FAILURE
            }
        };
    }
    if args.first().map(String::as_str) == Some("release") {
        return match release::run(&args[1..]) {
            Ok(text) => {
                print!("{text}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("omola: {error}");
                ExitCode::FAILURE
            }
        };
    }
    let result = match args.first().map(String::as_str) {
        Some("doctor") if args.len() == 1 => doctor(),
        Some("check") => check(&args[1..]),
        Some("simulate") => simulate(&args[1..]),
        _ => {
            usage();
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("omola: {error}");
            ExitCode::FAILURE
        }
    }
}

fn doctor() -> Result<String, BackendError> {
    let omc = Omc::discover()?;
    Ok(format!(
        "OpenModelica: {}\nBackend: external `omc` process\n",
        omc.version()?
    ))
}

fn check(args: &[String]) -> Result<String, BackendError> {
    let (model, class) = required(args, false)?;
    let result = Omc::discover()?.check_model(&model, &class)?;
    Ok(format!("{}\n{}", result.omc_version, result.output))
}

pub(crate) fn simulate(args: &[String]) -> Result<String, BackendError> {
    let (model, class, values) = required_with_values(args, true)?;
    let started_at_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let stop_time = match values.get("--stop-time") {
        Some(value) => value.parse::<f64>().map_err(|_| {
            BackendError::InvalidInput("--stop-time must be a finite non-negative number".into())
        })?,
        None => 1.0,
    };
    if !stop_time.is_finite() || stop_time < 0.0 {
        return Err(BackendError::InvalidInput(
            "--stop-time must be finite and non-negative".into(),
        ));
    }
    let parameters = parse_parameter_overrides(values.get("--parameters"))?;
    omola_core::validate_parameter_overrides(&parameters)?;
    let output = values.get("--output").map(PathBuf::from);
    let result = Omc::discover()?.simulate_project_model_with_parameters(
        &model,
        &class,
        SimulationConfig {
            stop_time,
            result: output.as_deref(),
            libraries: &[],
            package_roots: &[],
            parameters: &parameters,
        },
    )?;
    let metadata =
        results::write_run_metadata(&result, &model, &class, started_at_unix_ms, &parameters)?;
    Ok(format!(
        "{}\nResult: {}\nRun metadata: {}\n{}",
        result.omc_version,
        result
            .result_file
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<missing>".into()),
        metadata.display(),
        result.output
    ))
}

fn required(
    args: &[String],
    allow_simulation_options: bool,
) -> Result<(PathBuf, String), BackendError> {
    let (model, class, _) = required_with_values(args, allow_simulation_options)?;
    Ok((model, class))
}

fn required_with_values(
    args: &[String],
    allow_simulation_options: bool,
) -> Result<(PathBuf, String, std::collections::HashMap<String, String>), BackendError> {
    let mut values = std::collections::HashMap::new();
    let mut i = 0;
    while i < args.len() {
        let key = &args[i];
        let allowed = matches!(key.as_str(), "--model" | "--class")
            || (allow_simulation_options
                && matches!(key.as_str(), "--stop-time" | "--output" | "--parameters"));
        if !allowed {
            return Err(BackendError::InvalidInput(format!(
                "unknown or unsupported option: {key}"
            )));
        }
        let value = args
            .get(i + 1)
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| BackendError::InvalidInput(format!("missing value for {key}")))?;
        if values.insert(key.clone(), value.clone()).is_some() {
            return Err(BackendError::InvalidInput(format!(
                "option repeated: {key}"
            )));
        }
        i += 2;
    }
    let model = values
        .get("--model")
        .map(PathBuf::from)
        .ok_or_else(|| BackendError::InvalidInput("missing --model PATH".into()))?;
    let class = values
        .get("--class")
        .cloned()
        .ok_or_else(|| BackendError::InvalidInput("missing --class Modelica.Class".into()))?;
    if !model.is_file() {
        return Err(BackendError::InvalidInput(format!(
            "model file is not readable: {}",
            model.display()
        )));
    }
    Ok((model, class, values))
}

fn usage() {
    eprintln!("Omola 0.1 — OpenModelica-backed Modelica workflow\n\nUsage:\n  omola doctor\n  omola check --model FILE.mo --class Modelica.Class\n  omola simulate --model FILE.mo --class Modelica.Class [--stop-time SECONDS] [--parameters name=value,name=value] [--output PREFIX]\n  omola project validate MANIFEST.toml\n  omola project simulate MANIFEST.toml [--output PREFIX]\n  omola project open MANIFEST.toml [--omedit PATH]\n  omola omedit evidence verify --evidence FILE.json --manifest FILE.toml [--require-verified]\n  omola licenses check [--output REPORT.md]\n  omola results inspect FILE.csv|FILE.mat\n  omola results export-csv FILE.mat --variables x,y --output FILE.csv\n  omola compat run --manifest FILE.toml --format json|markdown\n  omola release verify --scorecard FILE.json --manifest FILE.toml --gates FILE.toml\n  omola bridge inventory [--format json|markdown]\n  omola bridge call --function Dymola.function [--model FILE.mo --class Modelica.Class --stop-time SECONDS]\n\nRequires OpenModelica `omc` on PATH or OMOLA_OMC set to its executable path.");
}

fn parse_parameter_overrides(
    value: Option<&String>,
) -> Result<Vec<(String, String)>, BackendError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    if value.is_empty() {
        return Err(BackendError::InvalidInput(
            "--parameters must be a comma-separated list of name=value numeric overrides".into(),
        ));
    }
    let mut result = Vec::new();
    for item in value.split(',') {
        let (name, number) = item.split_once('=').ok_or_else(|| {
            BackendError::InvalidInput(format!(
                "invalid parameter override `{item}`; expected name=value"
            ))
        })?;
        result.push((name.to_owned(), number.to_owned()));
    }
    omola_core::validate_parameter_overrides(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_values_unknown_options_and_duplicates() {
        assert!(required_with_values(&["--model".into()], false)
            .unwrap_err()
            .to_string()
            .contains("missing value"));
        assert!(required_with_values(&["--odd".into(), "x".into()], false)
            .unwrap_err()
            .to_string()
            .contains("unknown"));
        assert!(required_with_values(
            &[
                "--model".into(),
                "a.mo".into(),
                "--model".into(),
                "b.mo".into(),
                "--class".into(),
                "A".into()
            ],
            false
        )
        .unwrap_err()
        .to_string()
        .contains("repeated"));
    }

    #[test]
    fn parameter_override_list_is_numeric_and_injection_safe() {
        let valid = Some("gravity=9.81,Demo.initialHeight=2".to_owned());
        assert_eq!(parse_parameter_overrides(valid.as_ref()).unwrap().len(), 2);
        let invalid = Some("gravity=9.81,other=2;print(1)".to_owned());
        assert!(parse_parameter_overrides(invalid.as_ref()).is_err());
    }
}
