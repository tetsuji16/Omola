use omola_core::{BackendError, CommandResult, Omc, SimulationConfig};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Project {
    schema_version: u32,
    name: String,
    model_file: PathBuf,
    entry_model: String,
    #[serde(default)]
    package_roots: Vec<PathBuf>,
    #[serde(default)]
    dependencies: Vec<LibraryPin>,
    #[serde(default)]
    simulation: Simulation,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LibraryPin {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Simulation {
    stop_time: f64,
    #[serde(default)]
    parameters: BTreeMap<String, f64>,
}

impl Default for Simulation {
    fn default() -> Self {
        Self {
            stop_time: 1.0,
            parameters: BTreeMap::new(),
        }
    }
}

struct LoadedProject {
    manifest: Project,
    root: PathBuf,
    model_file: PathBuf,
    libraries: Vec<(String, String)>,
    package_roots: Vec<PathBuf>,
}

pub(crate) fn run(args: &[String]) -> Result<String, BackendError> {
    let (command, path, output) = match args {
        [command, path] if command == "validate" => (command.as_str(), path.as_str(), None),
        [command, path] if command == "simulate" => (command.as_str(), path.as_str(), None),
        [command, path, flag, output] if command == "simulate" && flag == "--output" => {
            (command.as_str(), path.as_str(), Some(PathBuf::from(output)))
        }
        _ => return Err(BackendError::InvalidInput("usage: omola project validate MANIFEST.toml | omola project simulate MANIFEST.toml [--output PREFIX]".into())),
    };
    let project = load_project(Path::new(path))?;
    if command == "validate" {
        let result = Omc::discover()?.check_project_model(
            &project.model_file,
            &project.manifest.entry_model,
            &project.libraries,
            &project.package_roots,
        )?;
        return Ok(format_project_result("validated", &project, &result));
    }
    let backend = Omc::discover()?;
    backend.check_project_model(
        &project.model_file,
        &project.manifest.entry_model,
        &project.libraries,
        &project.package_roots,
    )?;
    let output = output.unwrap_or_else(|| project.root.join("out").join(&project.manifest.name));
    let parameters: Vec<(String, String)> = project
        .manifest
        .simulation
        .parameters
        .iter()
        .map(|(name, value)| (name.clone(), value.to_string()))
        .collect();
    let started_at_unix_ms = now_unix_ms();
    let result = backend.simulate_project_model_with_parameters(
        &project.model_file,
        &project.manifest.entry_model,
        SimulationConfig {
            stop_time: project.manifest.simulation.stop_time,
            result: Some(&output),
            libraries: &project.libraries,
            package_roots: &project.package_roots,
            parameters: &parameters,
        },
    )?;
    let metadata = crate::results::write_run_metadata(
        &result,
        &project.model_file,
        &project.manifest.entry_model,
        started_at_unix_ms,
        &parameters,
    )?;
    Ok(format!(
        "{}\nRun metadata: {}",
        format_project_result("simulated", &project, &result),
        metadata.display()
    ))
}

fn format_project_result(action: &str, project: &LoadedProject, result: &CommandResult) -> String {
    format!(
        "Project: {} (schema {})\nModel: {} ({})\nOpenModelica: {}\nAction: {action}\nResult: {}\n{}",
        project.manifest.name,
        project.manifest.schema_version,
        project.manifest.entry_model,
        project.model_file.display(),
        result.omc_version,
        result.result_file.as_ref().map_or_else(|| "<none>".into(), |path| path.display().to_string()),
        result.output
    )
}

fn load_project(path: &Path) -> Result<LoadedProject, BackendError> {
    let manifest_path = fs::canonicalize(path).map_err(|error| {
        BackendError::InvalidInput(format!(
            "cannot open project manifest {}: {error}",
            path.display()
        ))
    })?;
    let text = fs::read_to_string(&manifest_path)?;
    let manifest: Project = toml::from_str(&text).map_err(|error| {
        BackendError::InvalidInput(format!("invalid project manifest: {error}"))
    })?;
    if manifest.schema_version != 1 {
        return Err(BackendError::InvalidInput(format!(
            "unsupported project schema_version {}; supported version is 1",
            manifest.schema_version
        )));
    }
    if manifest.name.is_empty()
        || !manifest
            .name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(BackendError::InvalidInput(
            "project name must contain only ASCII letters, digits, '-' and '_'".into(),
        ));
    }
    validate_stop_time(manifest.simulation.stop_time)?;
    let parameters: Vec<(String, String)> = manifest
        .simulation
        .parameters
        .iter()
        .map(|(name, value)| (name.clone(), value.to_string()))
        .collect();
    omola_core::validate_parameter_overrides(&parameters)?;
    let root = manifest_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let model_file = resolve_project_path(&root, &manifest.model_file, "model_file", true)?;
    if manifest.package_roots.is_empty() {
        return Err(BackendError::InvalidInput(
            "at least one package_roots entry is required".into(),
        ));
    }
    let package_roots = manifest
        .package_roots
        .iter()
        .map(|package_root| resolve_project_path(&root, package_root, "package_roots entry", false))
        .collect::<Result<Vec<_>, _>>()?;
    let mut libraries = Vec::new();
    for pin in &manifest.dependencies {
        libraries.push((pin.name.clone(), pin.version.clone()));
    }
    let pins = libraries
        .iter()
        .map(|(name, _)| name)
        .collect::<std::collections::HashSet<_>>();
    if pins.len() != libraries.len() {
        return Err(BackendError::InvalidInput(
            "each dependency package may be pinned only once".into(),
        ));
    }
    // Validate package names and exact version syntax before invoking external tooling.
    omola_core::validate_library_pins(&libraries)?;
    Ok(LoadedProject {
        manifest,
        root,
        model_file,
        libraries,
        package_roots,
    })
}

fn now_unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn validate_stop_time(stop_time: f64) -> Result<(), BackendError> {
    if stop_time.is_finite() && stop_time > 0.0 {
        Ok(())
    } else {
        Err(BackendError::InvalidInput(
            "simulation.stop_time must be finite and greater than zero".into(),
        ))
    }
}

fn resolve_project_path(
    root: &Path,
    relative: &Path,
    field: &str,
    file: bool,
) -> Result<PathBuf, BackendError> {
    if relative.as_os_str().is_empty()
        || !relative
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Err(BackendError::InvalidInput(format!(
            "{field} must be a relative path without parent traversal: {}",
            relative.display()
        )));
    }
    let path = fs::canonicalize(root.join(relative)).map_err(|error| {
        BackendError::InvalidInput(format!(
            "{field} path {} does not exist: {error}",
            relative.display()
        ))
    })?;
    if !path.starts_with(root) {
        return Err(BackendError::InvalidInput(format!(
            "{field} resolves outside project root: {}",
            relative.display()
        )));
    }
    if file && !path.is_file() {
        return Err(BackendError::InvalidInput(format!(
            "model_file is not a file: {}",
            path.display()
        )));
    }
    if !file && !path.is_dir() {
        return Err(BackendError::InvalidInput(format!(
            "package root is not a directory: {}",
            path.display()
        )));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_schema_is_versioned_and_rejects_parent_paths() {
        let toml = r#"
schema_version = 1
name = "demo"
model_file = "../outside.mo"
entry_model = "Demo"
package_roots = ["."]
"#;
        let project: Project = toml::from_str(toml).unwrap();
        assert_eq!(project.schema_version, 1);
        assert!(
            resolve_project_path(Path::new("."), &project.model_file, "model_file", true).is_err()
        );
    }

    #[test]
    fn simulation_requires_positive_finite_stop_time() {
        assert!(validate_stop_time(0.0).is_err());
        assert!(validate_stop_time(-1.0).is_err());
        assert!(validate_stop_time(f64::INFINITY).is_err());
        assert!(validate_stop_time(0.1).is_ok());
    }

    #[test]
    fn simulation_manifest_accepts_numeric_parameter_overrides_and_rejects_unknown_fields() {
        let parsed: Project = toml::from_str(
            r#"
schema_version = 1
name = "demo"
model_file = "model.mo"
entry_model = "Demo"
package_roots = ["."]
[simulation]
stop_time = 1.0
parameters = { gain = 2.5, "Demo.offset" = -1 }
"#,
        )
        .unwrap();
        assert_eq!(parsed.simulation.parameters["gain"], 2.5);
        assert_eq!(parsed.simulation.parameters["Demo.offset"], -1.0);
        let invalid: Project = toml::from_str(
            r#"
schema_version = 1
name = "demo"
model_file = "model.mo"
entry_model = "Demo"
package_roots = ["."]
[simulation]
stop_time = 1.0
parameters = { "gain;print(1)" = 2.5 }
"#,
        )
        .unwrap();
        let overrides: Vec<_> = invalid
            .simulation
            .parameters
            .iter()
            .map(|(name, value)| (name.clone(), value.to_string()))
            .collect();
        assert!(omola_core::validate_parameter_overrides(&overrides).is_err());
    }
}
