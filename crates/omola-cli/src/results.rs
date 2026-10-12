use omola_core::{BackendError, CommandResult, Omc};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Serialize)]
pub struct RunMetadata {
    schema_version: u32,
    started_at_unix_ms: u128,
    completed_at_unix_ms: u128,
    tool: String,
    tool_version: String,
    backend: String,
    backend_version: String,
    model: String,
    class: String,
    parameters: Vec<String>,
    output_result_file: String,
    solver_settings: String,
    units: String,
}

pub fn write_run_metadata(
    result: &CommandResult,
    model: &Path,
    class: &str,
    started_at_unix_ms: u128,
    parameters: &[(String, String)],
) -> Result<PathBuf, BackendError> {
    let result_path =
        result
            .result_file
            .as_deref()
            .ok_or_else(|| BackendError::InvalidCompilerResponse {
                version: result.omc_version.clone(),
                output: "simulation produced no result file for run metadata".into(),
            })?;
    let metadata_path = result_path.with_extension("json");
    let settings = extract_simulation_options(&result.output).unwrap_or_else(|| {
        "OpenModelica simulation options were not present in compiler output".into()
    });
    let metadata = RunMetadata {
        schema_version: 1,
        started_at_unix_ms,
        completed_at_unix_ms: now_unix_ms(),
        tool: "Omola".into(),
        tool_version: env!("CARGO_PKG_VERSION").into(),
        backend: "OpenModelica Compiler".into(),
        backend_version: result.omc_version.clone(),
        model: model
            .canonicalize()
            .unwrap_or_else(|_| model.to_path_buf())
            .display()
            .to_string(),
        class: class.into(),
        parameters: parameters
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect(),
        output_result_file: result_path.display().to_string(),
        solver_settings: settings,
        units: "Not reported by the OMC result metadata used by this adapter".into(),
    };
    let bytes = serde_json::to_vec_pretty(&metadata).map_err(|error| {
        BackendError::InvalidInput(format!("could not encode run metadata: {error}"))
    })?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&metadata_path)
        .map_err(|error| {
            BackendError::InvalidInput(format!(
                "could not create run metadata {} without overwriting an existing file: {error}",
                metadata_path.display()
            ))
        })?;
    file.write_all(&bytes).map_err(BackendError::Io)?;
    file.write_all(b"\n").map_err(BackendError::Io)?;
    Ok(metadata_path)
}

pub fn run(args: &[String]) -> Result<String, BackendError> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage());
    };
    match command {
        "inspect" if args.len() == 2 => inspect(Path::new(&args[1])),
        "export-csv" => export_csv(&args[1..]),
        _ => Err(usage()),
    }
}

fn inspect(path: &Path) -> Result<String, BackendError> {
    if !path.is_file() {
        return Err(BackendError::InvalidInput(format!(
            "result file does not exist or is not a file: {}",
            path.display()
        )));
    }
    match extension(path).as_deref() {
        Some("csv") => inspect_csv(path),
        Some("mat") => inspect_omc_mat(path),
        Some("dsres") => Err(BackendError::InvalidInput(
            "unsupported result format `.dsres`: Dymola DSRES decoding is not implemented".into(),
        )),
        Some(other) => Err(BackendError::InvalidInput(format!(
            "unsupported or ambiguous result format `.{other}`; supported inputs are OMC `.mat` and Omola CSV"
        ))),
        None => Err(BackendError::InvalidInput(
            "result file has no extension; format is ambiguous (expected .csv or .mat)".into(),
        )),
    }
}

fn inspect_csv(path: &Path) -> Result<String, BackendError> {
    let text = fs::read_to_string(path).map_err(|error| {
        BackendError::InvalidInput(format!("could not read CSV {}: {error}", path.display()))
    })?;
    let records = parse_csv(&text).map_err(|message| {
        BackendError::InvalidInput(format!("invalid CSV {}: {message}", path.display()))
    })?;
    let (headers, row_count) = validate_csv_records(&records).map_err(|message| {
        BackendError::InvalidInput(format!("invalid CSV {}: {message}", path.display()))
    })?;
    Ok(format!(
        "Format: Omola CSV\nFile: {}\nRows: {row_count}\nTime column: time\nVariables: {}\nMissing values: rejected\nUnits: not available in CSV\n",
        path.display(),
        headers.iter().skip(1).cloned().collect::<Vec<_>>().join(", ")
    ))
}

fn inspect_omc_mat(path: &Path) -> Result<String, BackendError> {
    let omc = Omc::discover()?;
    let quoted_path = modelica_string(&path.canonicalize()?.to_string_lossy());
    let script = format!(
        "clearMessages();\nfile := \"{quoted_path}\";\nsize := readSimulationResultSize(file);\nvars := readSimulationResultVars(file, false, true);\nprint(\"OMOLA_SIZE:\" + String(size) + \"\\n\");\nfor i in 1:size(vars, 1) loop print(\"OMOLA_VAR:\" + vars[i] + \"\\n\"); end for;\n"
    );
    let response = omc.run_script(&script)?;
    let size = response
        .output
        .lines()
        .find_map(|line| line.trim().strip_prefix("OMOLA_SIZE:"))
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|size| *size > 0)
        .ok_or_else(|| BackendError::InvalidCompilerResponse {
            version: response.omc_version.clone(),
            output: response.output.clone(),
        })?;
    let variables: Vec<_> = response
        .output
        .lines()
        .filter_map(|line| line.trim().strip_prefix("OMOLA_VAR:"))
        .map(str::to_owned)
        .collect();
    if variables.is_empty() {
        return Err(BackendError::InvalidCompilerResponse {
            version: response.omc_version,
            output: response.output,
        });
    }
    let metadata = metadata_sidecar(path);
    Ok(format!(
        "Format: OpenModelica MAT result\nFile: {}\nOMC: {}\nRows: {size}\nVariables: {}\nRun metadata: {}\nUnits: not exposed by the current result inspection path\n",
        path.display(),
        response.omc_version,
        variables.join(", "),
        if metadata.is_file() { metadata.display().to_string() } else { "missing".into() }
    ))
}

fn export_csv(args: &[String]) -> Result<String, BackendError> {
    if args.len() != 5 || args[1] != "--variables" || args[3] != "--output" {
        return Err(usage());
    }
    let source = PathBuf::from(&args[0]);
    if extension(&source).as_deref() != Some("mat") {
        return Err(BackendError::InvalidInput(
            "CSV export currently requires an OpenModelica `.mat` result; DSRES and CSV re-export are not supported".into(),
        ));
    }
    if !source.is_file() {
        return Err(BackendError::InvalidInput(format!(
            "result file does not exist: {}",
            source.display()
        )));
    }
    let variables: Vec<String> = args[2]
        .split(',')
        .map(str::trim)
        .map(str::to_owned)
        .collect();
    if variables.is_empty()
        || variables
            .iter()
            .any(|variable| variable.is_empty() || variable == "time")
        || variables.iter().collect::<BTreeSet<_>>().len() != variables.len()
    {
        return Err(BackendError::InvalidInput(
            "--variables requires unique, non-empty variable names; time is added automatically"
                .into(),
        ));
    }
    for variable in &variables {
        if !valid_variable(variable) {
            return Err(BackendError::InvalidInput(format!(
                "invalid Modelica result variable `{variable}`"
            )));
        }
    }
    let destination = PathBuf::from(&args[4]);
    if extension(&destination).as_deref() != Some("csv") {
        return Err(BackendError::InvalidInput(
            "CSV export output path must use the .csv extension".into(),
        ));
    }
    if destination.exists() {
        return Err(BackendError::InvalidInput(format!(
            "refusing to overwrite existing CSV: {}",
            destination.display()
        )));
    }

    let omc = Omc::discover()?;
    let source_path = modelica_string(&source.canonicalize()?.to_string_lossy());
    let available = read_mat_variables(&omc, &source_path)?;
    for variable in &variables {
        if !available.iter().any(|found| found == variable) {
            return Err(BackendError::InvalidInput(format!(
                "result variable `{variable}` is missing from {}",
                source.display()
            )));
        }
    }
    let row_count = read_mat_rows(&omc, &source_path, &variables)?;
    let mut csv = String::new();
    let mut headers = vec!["time".to_owned()];
    headers.extend(variables.iter().cloned());
    write_csv_record(&mut csv, &headers);
    for row in &row_count {
        write_csv_record(&mut csv, row);
    }
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| {
        BackendError::InvalidInput(format!(
            "could not create CSV output directory {}: {error}",
            parent.display()
        ))
    })?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .map_err(|error| {
            BackendError::InvalidInput(format!(
                "could not create CSV output {} without overwriting: {error}",
                destination.display()
            ))
        })?;
    file.write_all(csv.as_bytes()).map_err(BackendError::Io)?;
    Ok(format!(
        "Exported {} rows and {} variables to {}\nUnits: not available from this OMC export path; variable names and time are preserved\n",
        row_count.len(),
        variables.len(),
        destination.display()
    ))
}

fn read_mat_variables(omc: &Omc, source: &str) -> Result<Vec<String>, BackendError> {
    let script = format!(
        "clearMessages();\nvars := readSimulationResultVars(\"{source}\", false, true);\nfor i in 1:size(vars, 1) loop print(\"OMOLA_VAR:\" + vars[i] + \"\\n\"); end for;\n"
    );
    let response = omc.run_script(&script)?;
    let variables: Vec<String> = response
        .output
        .lines()
        .filter_map(|line| line.trim().strip_prefix("OMOLA_VAR:"))
        .map(str::to_owned)
        .collect();
    if variables.is_empty() {
        return Err(BackendError::InvalidCompilerResponse {
            version: response.omc_version,
            output: response.output,
        });
    }
    Ok(variables)
}

fn read_mat_rows(
    omc: &Omc,
    source: &str,
    variables: &[String],
) -> Result<Vec<Vec<String>>, BackendError> {
    let mut requested = vec!["time".to_owned()];
    requested.extend(variables.iter().cloned());
    let variable_list = requested.join(",");
    let script = format!(
        // OMC 1.27.1's String formatter has a 20-character buffer; 16-digit
        // output still overflows for some event values. Thirteen digits stays
        // within the buffer for the tested eventful model; CSV's own parser
        // still preserves full f64 precision for values already in CSV.
        "clearMessages();\nfile := \"{source}\";\nn := readSimulationResultSize(file);\ndata := OpenModelica.Scripting.readSimulationResult(file, {{{variable_list}}}, n);\nfor i in 1:n loop print(\"OMOLA_ROW:\"); for j in 1:size(data, 1) loop if j > 1 then print(\",\"); end if; print(String(data[j,i], significantDigits=13)); end for; print(\"\\n\"); end for;\n"
    );
    let response = omc.run_script(&script)?;
    let rows: Vec<Vec<String>> = response
        .output
        .lines()
        .filter_map(|line| line.trim().strip_prefix("OMOLA_ROW:"))
        .map(|line| line.split(',').map(str::to_owned).collect())
        .collect();
    if rows.is_empty() || rows.iter().any(|row| row.len() != requested.len()) {
        return Err(BackendError::InvalidCompilerResponse {
            version: response.omc_version,
            output: response.output,
        });
    }
    let parsed: Result<Vec<Vec<f64>>, _> = rows
        .iter()
        .map(|row| row.iter().map(|value| value.parse::<f64>()).collect())
        .collect();
    let parsed = parsed.map_err(|_| BackendError::InvalidCompilerResponse {
        version: response.omc_version.clone(),
        output: response.output.clone(),
    })?;
    validate_time_values(parsed.iter().map(|row| row[0])).map_err(BackendError::InvalidInput)?;
    Ok(parsed
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|value| format!("{value:.17e}"))
                .collect()
        })
        .collect())
}

fn parse_csv(text: &str) -> Result<Vec<Vec<String>>, String> {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut field = String::new();
    let mut chars = text.chars().peekable();
    let mut quoted = false;
    let mut closed_quote = false;
    while let Some(ch) = chars.next() {
        if quoted {
            match ch {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => {
                    quoted = false;
                    closed_quote = true;
                }
                value => field.push(value),
            }
        } else {
            match ch {
                '"' if field.is_empty() && !closed_quote => quoted = true,
                '"' => return Err("quote inside an unquoted field".into()),
                ',' => {
                    record.push(std::mem::take(&mut field));
                    closed_quote = false;
                }
                '\n' => {
                    record.push(std::mem::take(&mut field));
                    records.push(std::mem::take(&mut record));
                    closed_quote = false;
                }
                '\r' if chars.peek() == Some(&'\n') => {
                    chars.next();
                    record.push(std::mem::take(&mut field));
                    records.push(std::mem::take(&mut record));
                    closed_quote = false;
                }
                value if closed_quote && !value.is_whitespace() => {
                    return Err("unexpected data after a closing quote".into())
                }
                _value if closed_quote => {}
                value => field.push(value),
            }
        }
    }
    if quoted {
        return Err("unterminated quoted field".into());
    }
    if !field.is_empty() || !record.is_empty() || closed_quote {
        record.push(field);
        records.push(record);
    }
    if records
        .iter()
        .any(|record| record.len() == 1 && record[0].is_empty())
    {
        return Err("blank records are not allowed".into());
    }
    Ok(records)
}

fn validate_csv_records(records: &[Vec<String>]) -> Result<(Vec<String>, usize), String> {
    let headers = records.first().ok_or("CSV is empty")?;
    if headers.len() < 2 || headers.first().map(String::as_str) != Some("time") {
        return Err("the first column must be `time` and at least one variable is required".into());
    }
    if headers.iter().any(String::is_empty) {
        return Err("headers must not be empty".into());
    }
    if headers.iter().collect::<BTreeSet<_>>().len() != headers.len() {
        return Err("headers must be unique".into());
    }
    if records.len() < 2 {
        return Err("CSV has no data rows".into());
    }
    let mut times = Vec::with_capacity(records.len() - 1);
    for (index, row) in records.iter().skip(1).enumerate() {
        if row.len() != headers.len() {
            return Err(format!(
                "row {} has {} fields; expected {}",
                index + 2,
                row.len(),
                headers.len()
            ));
        }
        if row.iter().any(|field| field.trim().is_empty()) {
            return Err(format!("row {} contains a missing value", index + 2));
        }
        let time = row[0]
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
            .ok_or_else(|| format!("row {} has an invalid time value", index + 2))?;
        for (column, value) in row.iter().enumerate().skip(1) {
            if value
                .parse::<f64>()
                .ok()
                .is_none_or(|number| !number.is_finite())
            {
                return Err(format!(
                    "row {} variable `{}` is not a finite number",
                    index + 2,
                    headers[column]
                ));
            }
        }
        times.push(time);
    }
    validate_time_values(times.iter().copied())?;
    Ok((headers.clone(), times.len()))
}

fn validate_time_values(values: impl IntoIterator<Item = f64>) -> Result<(), String> {
    let mut previous = f64::NEG_INFINITY;
    for time in values {
        if !time.is_finite() || time < previous {
            return Err("time values must be finite and monotonic non-decreasing".into());
        }
        previous = time;
    }
    Ok(())
}

fn write_csv_record(output: &mut String, fields: &[String]) {
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        if field.contains([',', '"', '\n', '\r']) {
            output.push('"');
            output.push_str(&field.replace('"', "\"\""));
            output.push('"');
        } else {
            output.push_str(field);
        }
    }
    output.push('\n');
}

fn extract_simulation_options(output: &str) -> Option<String> {
    let marker = "simulationOptions = \"";
    let start = output.find(marker)? + marker.len();
    let end = output[start..].find('"')? + start;
    Some(output[start..end].to_owned())
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
}

fn metadata_sidecar(path: &Path) -> PathBuf {
    path.with_extension("json")
}

fn modelica_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn valid_variable(value: &str) -> bool {
    value.split('.').all(|part| {
        let mut chars = part.chars();
        chars
            .next()
            .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
            && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
    })
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn usage() -> BackendError {
    BackendError::InvalidInput(
        "usage: results inspect FILE.csv|FILE.mat | results export-csv FILE.mat --variables x,y --output FILE.csv".into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_round_trip_handles_quoted_headers_and_full_precision() {
        let mut text = String::new();
        write_csv_record(
            &mut text,
            &["time".into(), "plant, output".into(), "a\"b".into()],
        );
        write_csv_record(
            &mut text,
            &[
                "0.00000000000000000e0".into(),
                "1.23456789012345670e0".into(),
                "2".into(),
            ],
        );
        let records = parse_csv(&text).unwrap();
        let (headers, rows) = validate_csv_records(&records).unwrap();
        assert_eq!(headers, ["time", "plant, output", "a\"b"]);
        assert_eq!(rows, 1);
        assert_eq!(records[1][1].parse::<f64>().unwrap(), 1.2345678901234567);
    }

    #[test]
    fn csv_missing_values_and_non_monotonic_time_are_rejected() {
        for invalid in ["time,x\n0,\n", "time,x\n0,1\n2,3\n1,4\n"] {
            let error = validate_csv_records(&parse_csv(invalid).unwrap()).unwrap_err();
            assert!(error.contains("missing") || error.contains("monotonic"));
        }
    }

    #[test]
    fn quoted_names_and_crlf_are_parsed() {
        let parsed = parse_csv("time,\"long, name\"\r\n0,1\r\n").unwrap();
        assert_eq!(parsed[0], ["time", "long, name"]);
    }

    #[test]
    fn run_sidecar_records_requested_parameter_overrides() {
        let dir = std::env::temp_dir().join(format!(
            "omola-metadata-test-{}-{}",
            std::process::id(),
            now_unix_ms()
        ));
        fs::create_dir(&dir).unwrap();
        let result_file = dir.join("sample_res.mat");
        let result = CommandResult {
            omc_version: "OpenModelica 1.27.1".into(),
            output: "simulationOptions = \"stopTime = 1.0\"".into(),
            result_file: Some(result_file),
        };
        let path = write_run_metadata(
            &result,
            Path::new("model.mo"),
            "Demo",
            now_unix_ms(),
            &[("gravity".into(), "9.81".into())],
        )
        .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(json["parameters"][0], "gravity=9.81");
        fs::remove_dir_all(dir).unwrap();
    }
}
