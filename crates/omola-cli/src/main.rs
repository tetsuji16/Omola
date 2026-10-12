use omola_core::{BackendError, Omc};
use std::{env, path::PathBuf, process::ExitCode};

mod compat;

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
    Ok(format!(
        "OpenModelica {}\n{}",
        result.omc_version, result.output
    ))
}

fn simulate(args: &[String]) -> Result<String, BackendError> {
    let (model, class, values) = required_with_values(args, true)?;
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
    let output = values.get("--output").map(PathBuf::from);
    let result = Omc::discover()?.simulate(&model, &class, stop_time, output.as_deref())?;
    Ok(format!(
        "OpenModelica {}\nResult: {}\n{}",
        result.omc_version,
        result
            .result_file
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<missing>".into()),
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
            || (allow_simulation_options && matches!(key.as_str(), "--stop-time" | "--output"));
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
    eprintln!("Omola 0.1 — OpenModelica-backed Modelica workflow\n\nUsage:\n  omola doctor\n  omola check --model FILE.mo --class Modelica.Class\n  omola simulate --model FILE.mo --class Modelica.Class [--stop-time SECONDS] [--output PREFIX]\n  omola compat run --manifest FILE.toml --format json|markdown\n\nRequires OpenModelica `omc` on PATH or OMOLA_OMC set to its executable path.");
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
}
