use omola_core::{BackendError, CommandResult, Omc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Deserialize)]
struct Manifest {
    schema_version: u32,
    name: String,
    modelica_spec_version: String,
    fixtures: Vec<Fixture>,
}

#[derive(Debug, Deserialize)]
struct Fixture {
    id: String,
    model: Option<String>,
    class: Option<String>,
    operation: Option<String>,
    expected: String,
    feature_tags: Vec<String>,
    provenance: String,
    license: String,
    omc_version_prefix: Option<String>,
    libraries: Vec<String>,
    solver: Option<String>,
    skip_reason: Option<String>,
    stop_time: Option<f64>,
    comparison: Option<Comparison>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Comparison {
    golden_csv: String,
    variables: Vec<String>,
    time_alignment: String,
    units: BTreeMap<String, String>,
    absolute_tolerance: f64,
    relative_tolerance: f64,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: u32,
    manifest: String,
    manifest_sha256: String,
    execution_label: &'static str,
    omola_version: &'static str,
    omc_version: Option<String>,
    operating_system: &'static str,
    libraries: Vec<String>,
    libraries_measured: BTreeMap<String, Counts>,
    totals: Counts,
    compatibility_claim: &'static str,
    features: BTreeMap<String, Counts>,
    fixtures: Vec<FixtureResult>,
    numeric_comparisons: String,
}

#[derive(Debug, Default, Serialize)]
struct Counts {
    pass: usize,
    fail: usize,
    skip: usize,
}

#[derive(Debug, Serialize)]
struct FixtureResult {
    id: String,
    feature_tags: Vec<String>,
    modelica_spec_version: String,
    reproduction_command: String,
    expected: String,
    outcome: String,
    message: String,
    omc_version: Option<String>,
    libraries: Vec<String>,
    solver: Option<String>,
    solver_version: String,
    provenance: String,
    license: String,
    comparison: Option<Comparison>,
    numeric_comparison: Option<NumericComparison>,
}

#[derive(Debug, Serialize)]
struct NumericComparison {
    time_samples_checked: usize,
    values_compared: usize,
    maximum_absolute_error: f64,
}

pub fn run(args: &[String]) -> Result<(String, bool), BackendError> {
    let (manifest_path, format) = parse_args(args)?;
    let manifest_text = fs::read_to_string(&manifest_path).map_err(|error| {
        BackendError::InvalidInput(format!(
            "could not read compatibility manifest {}: {error}",
            manifest_path.display()
        ))
    })?;
    let manifest: Manifest = toml::from_str(&manifest_text).map_err(|error| {
        BackendError::InvalidInput(format!(
            "invalid compatibility manifest {}: {error}",
            manifest_path.display()
        ))
    })?;
    validate_manifest(&manifest, &manifest_path)?;

    let backend = match Omc::discover() {
        Ok(omc) => Some(omc),
        Err(BackendError::MissingCompiler { .. }) => None,
        Err(error) => {
            return Err(BackendError::InvalidInput(format!(
                "could not initialize OpenModelica for compatibility run: {error}"
            )))
        }
    };
    let omc_version = backend.as_ref().map(Omc::version).transpose()?;
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let mut results = Vec::with_capacity(manifest.fixtures.len());

    for fixture in &manifest.fixtures {
        let mut result = base_result(fixture, &manifest.modelica_spec_version, &manifest_path);
        if fixture.expected == "skip" {
            result.outcome = "skip".into();
            result.message = fixture.skip_reason.clone().unwrap_or_default();
        } else if backend.is_none() {
            result.outcome = "skip".into();
            result.message = "OpenModelica is not installed; backend fixture was not run".into();
        } else if fixture.omc_version_prefix.as_ref().is_some_and(|prefix| {
            !omc_version
                .as_deref()
                .unwrap_or_default()
                .starts_with(prefix)
        }) {
            result.outcome = "skip".into();
            result.message = format!(
                "requires OMC version prefix {}; found {}",
                fixture.omc_version_prefix.as_deref().unwrap_or_default(),
                omc_version.as_deref().unwrap_or("unknown")
            );
        } else {
            let omc = backend.as_ref().expect("checked above");
            let source = manifest_dir.join(fixture.model.as_deref().unwrap_or_default());
            let source = source.canonicalize().map_err(|error| {
                BackendError::InvalidInput(format!(
                    "{} fixture '{}': model path {} could not be resolved: {error}",
                    manifest_path.display(),
                    fixture.id,
                    source.display()
                ))
            })?;
            let class = fixture.class.as_deref().unwrap_or_default();
            let actual = match fixture.operation.as_deref() {
                Some("check") => omc
                    .check_model(&source, class)
                    .map(|command| (command, None)),
                Some("simulate") => {
                    simulate_fixture(omc, &source, class, fixture.stop_time.unwrap_or(1.0))
                        .map(|(command, temp)| (command, Some(temp)))
                }
                _ => unreachable!("operation validated"),
            };
            result.omc_version = omc_version.clone();
            match actual {
                Ok((command, temp)) if fixture.expected == "pass" => {
                    let numeric = match (&fixture.comparison, &command.result_file) {
                        (Some(comparison), Some(path)) => {
                            compare_golden(omc, path, manifest_dir, comparison)
                        }
                        (Some(_), None) => Err("simulation produced no result file".into()),
                        (None, _) => Ok(None),
                    };
                    match numeric {
                        Ok(comparison) => {
                            result.outcome = "pass".into();
                            result.message = comparison.as_ref().map_or_else(
                                || "backend operation completed successfully".into(),
                                |value| format!("golden comparison passed at {} time samples ({} values); max absolute error {:.9e}", value.time_samples_checked, value.values_compared, value.maximum_absolute_error),
                            );
                            result.numeric_comparison = comparison;
                        }
                        Err(message) => {
                            result.outcome = "fail".into();
                            result.message = message;
                        }
                    }
                    result.solver = reported_solver(&command).or_else(|| fixture.solver.clone());
                    result.solver_version = solver_version(&result.solver);
                    drop(temp);
                }
                Err(_) if fixture.expected == "fail" => {
                    result.outcome = "pass".into();
                    result.message = "backend reported the expected failure".into();
                }
                Ok(_) => {
                    result.outcome = "fail".into();
                    result.message = "expected backend failure but operation succeeded".into();
                }
                Err(_) => {
                    result.outcome = "fail".into();
                    result.message = "backend operation failed unexpectedly".into();
                }
            }
        }
        results.push(result);
    }

    let mut totals = Counts::default();
    let mut features: BTreeMap<String, Counts> = BTreeMap::new();
    let mut libraries: Vec<String> = manifest
        .fixtures
        .iter()
        .flat_map(|fixture| fixture.libraries.iter().cloned())
        .collect();
    libraries.sort();
    libraries.dedup();
    let mut libraries_measured: BTreeMap<String, Counts> = BTreeMap::new();
    for result in &results {
        increment(&mut totals, &result.outcome);
        for tag in &result.feature_tags {
            increment(features.entry(tag.clone()).or_default(), &result.outcome);
        }
        if result.libraries.is_empty() {
            increment(
                libraries_measured
                    .entry("(no external library)".into())
                    .or_default(),
                &result.outcome,
            );
        } else {
            for library in &result.libraries {
                increment(
                    libraries_measured.entry(library.clone()).or_default(),
                    &result.outcome,
                );
            }
        }
    }
    let has_failures = totals.fail > 0;
    let report = Report {
        schema_version: 3,
        manifest: manifest.name,
        manifest_sha256: format!("{:x}", Sha256::digest(manifest_text.as_bytes())),
        execution_label: "Omola via OpenModelica",
        omola_version: env!("CARGO_PKG_VERSION"),
        omc_version,
        operating_system: std::env::consts::OS,
        libraries,
        libraries_measured,
        totals,
        compatibility_claim: "Only the listed fixtures were measured through OpenModelica; this subset does not establish Dymola compatibility.",
        features,
        fixtures: results,
        numeric_comparisons: "Declared golden CSV samples are compared through OpenModelica val() using the stated variable, time interpolation, unit, and tolerance metadata; absent comparisons are not inferred.".into(),
    };
    match format.as_str() {
        "json" => serde_json::to_string_pretty(&report)
            .map(|text| (text, has_failures))
            .map_err(|error| {
                BackendError::InvalidInput(format!("could not encode report: {error}"))
            }),
        "markdown" => Ok((render_markdown(&report), has_failures)),
        _ => unreachable!("format validated"),
    }
}

fn parse_args(args: &[String]) -> Result<(PathBuf, String), BackendError> {
    if args.len() != 5 || args[0] != "run" || args[1] != "--manifest" || args[3] != "--format" {
        return Err(BackendError::InvalidInput(
            "usage: compat run --manifest FILE.toml --format json|markdown".into(),
        ));
    }
    if !matches!(args[4].as_str(), "json" | "markdown") {
        return Err(BackendError::InvalidInput(
            "compatibility report format must be json or markdown".into(),
        ));
    }
    Ok((PathBuf::from(&args[2]), args[4].clone()))
}

fn validate_manifest(manifest: &Manifest, path: &Path) -> Result<(), BackendError> {
    let invalid = |field: &str, message: &str| {
        BackendError::InvalidInput(format!("{}: {field}: {message}", path.display()))
    };
    if manifest.schema_version != 2 {
        return Err(invalid("schema_version", "supported value is 2"));
    }
    if manifest.name.trim().is_empty() {
        return Err(invalid("name", "must not be empty"));
    }
    if manifest.modelica_spec_version.trim().is_empty() {
        return Err(invalid(
            "modelica_spec_version",
            "must pin a Modelica specification release",
        ));
    }
    if manifest.fixtures.is_empty() {
        return Err(invalid("fixtures", "must contain at least one entry"));
    }
    let mut ids = std::collections::BTreeSet::new();
    for (index, fixture) in manifest.fixtures.iter().enumerate() {
        let field = |name: &str| format!("fixtures[{index}].{name}");
        if fixture.id.trim().is_empty() || !ids.insert(fixture.id.as_str()) {
            return Err(invalid(&field("id"), "must be non-empty and unique"));
        }
        if fixture.feature_tags.is_empty()
            || fixture.feature_tags.iter().any(|tag| tag.trim().is_empty())
        {
            return Err(invalid(
                &field("feature_tags"),
                "must contain non-empty tags",
            ));
        }
        if fixture.provenance.trim().is_empty() || fixture.license.trim().is_empty() {
            return Err(invalid(
                &field("provenance/license"),
                "must document source and license",
            ));
        }
        if !matches!(fixture.expected.as_str(), "pass" | "fail" | "skip") {
            return Err(invalid(&field("expected"), "must be pass, fail, or skip"));
        }
        if fixture.expected == "skip" {
            if fixture
                .skip_reason
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                return Err(invalid(
                    &field("skip_reason"),
                    "is required for skipped fixtures",
                ));
            }
        } else {
            if fixture
                .model
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                return Err(invalid(
                    &field("model"),
                    "is required unless expected is skip",
                ));
            }
            if fixture
                .class
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                return Err(invalid(
                    &field("class"),
                    "is required unless expected is skip",
                ));
            }
            if !matches!(fixture.operation.as_deref(), Some("check" | "simulate")) {
                return Err(invalid(&field("operation"), "must be check or simulate"));
            }
            if fixture.operation.as_deref() == Some("simulate")
                && (!fixture.stop_time.unwrap_or(1.0).is_finite()
                    || fixture.stop_time.unwrap_or(1.0) < 0.0)
            {
                return Err(invalid(&field("stop_time"), "must be finite"));
            }
        }
        if let Some(comparison) = &fixture.comparison {
            if comparison.variables.is_empty()
                || comparison.absolute_tolerance < 0.0
                || comparison.relative_tolerance < 0.0
                || !comparison.absolute_tolerance.is_finite()
                || !comparison.relative_tolerance.is_finite()
                || comparison.golden_csv.trim().is_empty()
                || comparison.units.len() != comparison.variables.len()
                || comparison
                    .units
                    .keys()
                    .any(|variable| !comparison.variables.contains(variable))
                || comparison
                    .variables
                    .iter()
                    .any(|variable| !is_valid_variable(variable))
            {
                return Err(invalid(&field("comparison"), "requires variables, non-negative finite tolerances, and units keyed by declared variables"));
            }
            if comparison.time_alignment != "interpolate" {
                return Err(invalid(
                    &field("comparison.time_alignment"),
                    "currently only interpolate is supported by OMC val()",
                ));
            }
        }
    }
    Ok(())
}

fn base_result(fixture: &Fixture, spec_version: &str, manifest_path: &Path) -> FixtureResult {
    FixtureResult {
        id: fixture.id.clone(),
        feature_tags: fixture.feature_tags.clone(),
        modelica_spec_version: spec_version.to_owned(),
        reproduction_command: format!(
            "omola compat run --manifest {} --format markdown",
            manifest_path.display()
        ),
        expected: fixture.expected.clone(),
        outcome: String::new(),
        message: String::new(),
        omc_version: None,
        libraries: fixture.libraries.clone(),
        solver: fixture.solver.clone(),
        solver_version: "not reported".into(),
        provenance: fixture.provenance.clone(),
        license: fixture.license.clone(),
        comparison: fixture.comparison.as_ref().map(|comparison| Comparison {
            golden_csv: comparison.golden_csv.clone(),
            variables: comparison.variables.clone(),
            time_alignment: comparison.time_alignment.clone(),
            units: comparison.units.clone(),
            absolute_tolerance: comparison.absolute_tolerance,
            relative_tolerance: comparison.relative_tolerance,
        }),
        numeric_comparison: None,
    }
}

fn simulate_fixture(
    omc: &Omc,
    source: &Path,
    class: &str,
    stop_time: f64,
) -> Result<(CommandResult, TempResult), BackendError> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "omola-compat-{}-{unique}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&directory)?;
    let temp = TempResult { directory };
    let result = omc.simulate(
        source,
        class,
        stop_time,
        Some(&temp.directory.join("result")),
    )?;
    Ok((result, temp))
}

struct TempResult {
    directory: PathBuf,
}

impl Drop for TempResult {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn compare_golden(
    omc: &Omc,
    result_file: &Path,
    manifest_dir: &Path,
    comparison: &Comparison,
) -> Result<Option<NumericComparison>, String> {
    let csv_path = manifest_dir.join(&comparison.golden_csv);
    let csv = fs::read_to_string(&csv_path)
        .map_err(|error| format!("could not read golden CSV {}: {error}", csv_path.display()))?;
    let mut lines = csv.lines();
    let header: Vec<_> = lines
        .next()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .collect();
    let expected_header: Vec<_> = std::iter::once("time")
        .chain(comparison.variables.iter().map(String::as_str))
        .collect();
    if header != expected_header {
        return Err(format!(
            "golden CSV {} header must be `{}`",
            csv_path.display(),
            expected_header.join(",")
        ));
    }

    let mut script = String::from("clearMessages();\n");
    let result_path = modelica_string(&result_file.to_string_lossy());
    let mut queries = Vec::new();
    let mut sample_count = 0usize;
    let mut previous_time = f64::NEG_INFINITY;
    for (line_index, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<_> = line.split(',').map(str::trim).collect();
        if fields.len() != expected_header.len() {
            return Err(format!(
                "golden CSV {} line {} has {} columns; expected {}",
                csv_path.display(),
                line_index + 2,
                fields.len(),
                expected_header.len()
            ));
        }
        let time = parse_finite(fields[0]).ok_or_else(|| {
            format!(
                "golden CSV {} line {} has an invalid time",
                csv_path.display(),
                line_index + 2
            )
        })?;
        if time < previous_time {
            return Err(format!(
                "golden CSV {} time values must be monotonic",
                csv_path.display()
            ));
        }
        previous_time = time;
        for (column, variable) in comparison.variables.iter().enumerate() {
            let expected = parse_finite(fields[column + 1]).ok_or_else(|| {
                format!(
                    "golden CSV {} line {} has an invalid value for {variable}",
                    csv_path.display(),
                    line_index + 2
                )
            })?;
            let marker = format!("OMOLA_COMPAT_VALUE_{}:", queries.len());
            script.push_str(&format!("print(\"{marker}\" + String(val({variable},{time:.17},\"{result_path}\")) + \"\\n\");\n"));
            queries.push((marker, expected));
        }
        sample_count += 1;
    }
    if sample_count == 0 {
        return Err(format!("golden CSV {} has no samples", csv_path.display()));
    }
    let values = omc
        .run_script(&script)
        .map_err(|_| "OMC could not read simulation values for golden comparison".to_string())?;
    let mut max_error = 0.0f64;
    for (index, (marker, expected)) in queries.iter().enumerate() {
        let actual = values
            .output
            .lines()
            .find_map(|line| line.trim().strip_prefix(marker))
            .and_then(parse_finite)
            .ok_or_else(|| {
                format!(
                    "OMC did not return a finite result for golden sample {}",
                    index + 1
                )
            })?;
        let error = (actual - expected).abs();
        max_error = max_error.max(error);
        if error > comparison.absolute_tolerance + comparison.relative_tolerance * expected.abs() {
            return Err(format!(
                "golden value mismatch at sample {}: absolute error {:.9e} exceeds tolerance",
                index + 1,
                error
            ));
        }
    }
    Ok(Some(NumericComparison {
        time_samples_checked: sample_count,
        values_compared: queries.len(),
        maximum_absolute_error: max_error,
    }))
}

fn parse_finite(value: &str) -> Option<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

fn is_valid_variable(value: &str) -> bool {
    value.split('.').all(|part| {
        let mut chars = part.chars();
        chars
            .next()
            .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
            && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
    })
}

fn modelica_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn reported_solver(result: &CommandResult) -> Option<String> {
    let output = &result.output;
    let marker = "method = '";
    let start = output.find(marker)? + marker.len();
    let end = output[start..].find('\'')? + start;
    Some(output[start..end].to_owned())
}

fn solver_version(solver: &Option<String>) -> String {
    match solver.as_deref() {
        Some(name) => format!("{name}; version not exposed by OMC"),
        None => "not reported by backend".into(),
    }
}

fn increment(counts: &mut Counts, outcome: &str) {
    match outcome {
        "pass" => counts.pass += 1,
        "fail" => counts.fail += 1,
        _ => counts.skip += 1,
    }
}

fn render_markdown(report: &Report) -> String {
    let mut output = format!(
        "# Compatibility report: {}\n\nExecution: {}  \nSchema: {}  \nOmola: {}  \nOMC: {}  \nOS: {}  \nLibraries: {}  \nResults: {} pass, {} fail, {} skip\n\n",
        report.manifest,
        report.execution_label,
        report.schema_version,
        report.omola_version,
        report.omc_version.as_deref().unwrap_or("not available"),
        report.operating_system,
        if report.libraries.is_empty() { "none declared".into() } else { report.libraries.join(", ") },
        report.totals.pass,
        report.totals.fail,
        report.totals.skip
    );
    output.push_str(&format!(
        "{}\n\n## Feature counts\n\n| Feature (measured) | Pass | Fail | Skip |\n| --- | ---: | ---: | ---: |\n",
        report.compatibility_claim,
    ));
    for (feature, counts) in &report.features {
        output.push_str(&format!(
            "| {feature} | {} | {} | {} |\n",
            counts.pass, counts.fail, counts.skip
        ));
    }
    output.push_str(
        "\n## Library counts\n\n| Library | Pass | Fail | Skip |\n| --- | ---: | ---: | ---: |\n",
    );
    for (library, counts) in &report.libraries_measured {
        output.push_str(&format!(
            "| {library} | {} | {} | {} |\n",
            counts.pass, counts.fail, counts.skip
        ));
    }
    output.push_str("\n## Fixtures\n\n| ID | Outcome | Expected | Message | OMC | Modelica spec | Solver | Solver version | Libraries | Source/license | Reproduction command |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for fixture in &report.fixtures {
        output.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} / {} | {} |\n",
            fixture.id,
            fixture.outcome,
            fixture.expected,
            fixture.message,
            fixture.omc_version.as_deref().unwrap_or("not run"),
            fixture.modelica_spec_version,
            fixture.solver.as_deref().unwrap_or("not set"),
            fixture.solver_version,
            if fixture.libraries.is_empty() {
                "none".into()
            } else {
                fixture.libraries.join(", ")
            },
            fixture.provenance,
            fixture.license,
            fixture.reproduction_command
        ));
    }
    output.push_str(&format!(
        "\nNumeric comparisons: {}\n",
        report.numeric_comparisons
    ));
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_schema_and_fixture_diagnostics_include_file_and_field() {
        let text = r#"schema_version = 1
name = "bad"
modelica_spec_version = "3.6"
[[fixtures]]
id = "fixture"
expected = "pass"
feature_tags = ["syntax"]
provenance = "original"
license = "MIT"
libraries = []
"#;
        let manifest: Manifest = toml::from_str(text).unwrap();
        let error = validate_manifest(&manifest, Path::new("manifest.toml")).unwrap_err();
        assert!(error.to_string().contains("manifest.toml: schema_version"));
    }

    #[test]
    fn renders_reports_with_stable_field_order_and_counts() {
        let counts = Counts {
            pass: 2,
            fail: 0,
            skip: 1,
        };
        assert_eq!(
            serde_json::to_string(&counts).unwrap(),
            r#"{"pass":2,"fail":0,"skip":1}"#
        );
    }
}
