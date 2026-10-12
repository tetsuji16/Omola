use omola_core::BackendError;
use serde::Serialize;
#[derive(Serialize)]
struct Mapping {
    dymola_function: &'static str,
    status: &'static str,
    omola_equivalent: Option<&'static str>,
    limitation: &'static str,
    evidence_fixtures: &'static [&'static str],
}

const MAPPINGS: &[Mapping] = &[
    Mapping {
        dymola_function: "simulateModel",
        status: "partial",
        omola_equivalent: Some("omola simulate --model FILE.mo --class Modelica.Class [--stop-time SECONDS] [--output PREFIX]"),
        limitation: "Maps a file-backed model, stopTime, and an output prefix. Start time, solver selection, tolerances, output intervals, parameter overrides, exact Dymola resultFile naming, and Dymola session state are not mapped.",
        evidence_fixtures: &["bouncing-ball-simulate", "msl-first-order-blocks"],
    },
    Mapping {
        dymola_function: "translateModel",
        status: "unsupported",
        omola_equivalent: None,
        limitation: "No standalone translation operation or generated simulation-code artifact is exposed; translation currently occurs only as part of simulation.",
        evidence_fixtures: &[],
    },
    Mapping {
        dymola_function: "plot",
        status: "unsupported",
        omola_equivalent: None,
        limitation: "No interactive plotting or plot export capability is implemented.",
        evidence_fixtures: &[],
    },
    Mapping {
        dymola_function: "getLastError",
        status: "unsupported",
        omola_equivalent: None,
        limitation: "Omola returns diagnostics on the operation that produced them; it has no persistent Dymola session or stateful last-error query.",
        evidence_fixtures: &[],
    },
];

pub fn run(args: &[String]) -> Result<String, BackendError> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err(usage());
    };
    match subcommand {
        "inventory" => inventory(&args[1..]),
        "call" => call(&args[1..]),
        _ => Err(usage()),
    }
}

fn inventory(args: &[String]) -> Result<String, BackendError> {
    let format = match args {
        [] => "markdown",
        [flag, format] if flag == "--format" => format.as_str(),
        _ => return Err(usage()),
    };
    let mut counts = [0usize; 3];
    for mapping in MAPPINGS {
        let index = match mapping.status {
            "implemented" => 0,
            "partial" => 1,
            _ => 2,
        };
        counts[index] += 1;
    }
    match format {
        "json" => serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": 1,
            "label": "Omola via OpenModelica",
            "compatibility_claim": "This inventory documents measured mappings only and does not claim Dymola parity.",
            "counts": {"implemented": counts[0], "partial": counts[1], "unsupported": counts[2]},
            "functions": MAPPINGS,
        }))
        .map_err(|error| BackendError::InvalidInput(format!("could not encode bridge inventory: {error}"))),
        "markdown" => {
            let mut text = format!(
                "# Dymola scripting bridge inventory\n\nLabel: Omola via OpenModelica  \nCounts: {} implemented, {} partial, {} unsupported\n\nThis inventory documents measured mappings only and does not claim Dymola parity.\n\n| Dymola function | Status | Omola equivalent | Limitations | Evidence fixtures |\n| --- | --- | --- | --- | --- |\n",
                counts[0], counts[1], counts[2]
            );
            for mapping in MAPPINGS {
                let evidence = mapping.evidence_fixtures.join(", ");
                text.push_str(&format!(
                    "| `{}` | {} | {} | {} | {} |\n",
                    mapping.dymola_function,
                    mapping.status,
                    mapping.omola_equivalent.unwrap_or("none"),
                    mapping.limitation,
                    if mapping.evidence_fixtures.is_empty() { "none" } else { &evidence }
                ));
            }
            Ok(text)
        }
        _ => Err(BackendError::InvalidInput(
            "bridge inventory format must be json or markdown".into(),
        )),
    }
}

fn call(args: &[String]) -> Result<String, BackendError> {
    let values = parse_options(args)?;
    let function = values.get("--function").ok_or_else(|| {
        BackendError::InvalidInput("missing --function Dymola.functionName".into())
    })?;
    match function.as_str() {
        "simulateModel" | "Dymola.simulateModel" => {
            for (option, parameter, capability) in [
                ("--start-time", "startTime", "non-zero start-time control"),
                ("--method", "method", "solver selection"),
                ("--tolerance", "tolerance", "solver tolerance control"),
                ("--intervals", "numberOfIntervals", "output interval count"),
                ("--output-interval", "outputInterval", "output interval control"),
                ("--fixed-step-size", "fixedstepsize", "fixed-step solver configuration"),
            ] {
                if values.contains_key(option) {
                    return Err(BackendError::InvalidInput(format!(
                        "unsupported Dymola.simulateModel parameter `{parameter}`: Omola does not map {capability}"
                    )));
                }
            }
            let mut forwarded = Vec::new();
            for option in ["--model", "--class", "--stop-time", "--output"] {
                if let Some(value) = values.get(option) {
                    forwarded.push(option.to_owned());
                    forwarded.push(value.clone());
                }
            }
            let output = super::simulate(&forwarded)?;
            Ok(format!(
            "Partial mapping of Dymola.simulateModel through Omola simulate. Only model file, class, stopTime, and output prefix are mapped; this is not a Dymola implementation.\n{output}"
            ))
        }
        "translateModel" | "Dymola.translateModel" => unsupported(
            "translateModel",
            "standalone translation and generated-code artifacts are unavailable",
        ),
        "plot" | "Dymola.plot" => unsupported(
            "plot",
            "interactive plotting and plot export are unavailable",
        ),
        "getLastError" | "Dymola.getLastError" => unsupported(
            "getLastError",
            "persistent Dymola session state is unavailable; inspect the diagnostics from the failed Omola operation",
        ),
        other => Err(BackendError::InvalidInput(format!(
            "unsupported Dymola scripting function `{other}`: no Omola mapping is declared"
        ))),
    }
}

fn unsupported<T>(function: &str, capability: &str) -> Result<T, BackendError> {
    Err(BackendError::InvalidInput(format!(
        "unsupported Dymola scripting capability `{function}`: {capability}"
    )))
}

fn parse_options(
    args: &[String],
) -> Result<std::collections::HashMap<String, String>, BackendError> {
    let mut values = std::collections::HashMap::new();
    let mut index = 0;
    while index < args.len() {
        let key = &args[index];
        if !matches!(
            key.as_str(),
            "--function"
                | "--model"
                | "--class"
                | "--stop-time"
                | "--output"
                | "--start-time"
                | "--method"
                | "--tolerance"
                | "--intervals"
                | "--output-interval"
                | "--fixed-step-size"
        ) {
            return Err(BackendError::InvalidInput(format!(
                "unsupported bridge option: {key}"
            )));
        }
        let value = args
            .get(index + 1)
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| BackendError::InvalidInput(format!("missing value for {key}")))?;
        if values.insert(key.clone(), value.clone()).is_some() {
            return Err(BackendError::InvalidInput(format!(
                "option repeated: {key}"
            )));
        }
        index += 2;
    }
    Ok(values)
}

fn usage() -> BackendError {
    BackendError::InvalidInput(
        "usage: bridge inventory [--format json|markdown] | bridge call --function Dymola.function [--model FILE.mo --class Modelica.Class --stop-time SECONDS]".into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_counts_and_limits_are_explicit() {
        let report = inventory(&["--format".into(), "json".into()]).unwrap();
        assert!(report.contains("\"partial\": 1"));
        assert!(report.contains("\"unsupported\": 3"));
        assert!(report.contains("Dymola parity"));
    }

    #[test]
    fn unsupported_function_names_the_missing_capability() {
        let error = call(&["--function".into(), "plot".into()]).unwrap_err();
        assert!(error
            .to_string()
            .contains("Dymola scripting capability `plot`"));
        assert!(error.to_string().contains("interactive plotting"));
    }

    #[test]
    fn unknown_function_is_not_silently_accepted() {
        let error = call(&["--function".into(), "someVendorFunction".into()]).unwrap_err();
        assert!(error.to_string().contains("no Omola mapping is declared"));
    }

    #[test]
    fn unmapped_simulation_parameter_names_the_missing_capability() {
        let error = call(&[
            "--function".into(),
            "Dymola.simulateModel".into(),
            "--method".into(),
            "Dassl".into(),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("parameter `method`"));
        assert!(error.to_string().contains("solver selection"));
    }
}
