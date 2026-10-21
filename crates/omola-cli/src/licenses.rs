use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::PathBuf, process::Command};

#[derive(Debug, Deserialize)]
struct Inventory {
    schema_version: u32,
    cargo_packages: Vec<CargoRecord>,
    external_components: Vec<ExternalRecord>,
    workflow_actions: Vec<ActionRecord>,
    container_images: Vec<String>,
    container_packages: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ActionRecord {
    reference: String,
    source: String,
    license: String,
    distribution: String,
}

#[derive(Debug, Deserialize)]
struct CargoRecord {
    name: String,
    version: String,
    license: String,
    source: String,
}

#[derive(Debug, Deserialize)]
struct ExternalRecord {
    name: String,
    version: String,
    source: String,
    license: String,
    distribution: String,
}

pub fn run(args: &[String]) -> Result<String, String> {
    if args.first().map(String::as_str) != Some("check") {
        return Err("usage: omola licenses check [--output REPORT.md]".into());
    }
    let mut output = None;
    let mut index = 1;
    while index < args.len() {
        if args[index] != "--output" || index + 1 >= args.len() || output.is_some() {
            return Err("usage: omola licenses check [--output REPORT.md]".into());
        }
        output = Some(PathBuf::from(&args[index + 1]));
        index += 2;
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let inventory: Inventory = toml::from_str(
        &fs::read_to_string(root.join("licenses/inventory.toml"))
            .map_err(|e| format!("read license inventory: {e}"))?,
    )
    .map_err(|e| format!("parse license inventory: {e}"))?;
    if inventory.schema_version != 1 {
        return Err("unsupported license inventory schema".into());
    }
    let metadata = Command::new("cargo")
        .args(["metadata", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .args(["--format-version", "1", "--locked"])
        .current_dir(&root)
        .output()
        .map_err(|e| format!("run cargo metadata: {e}; install Rust/Cargo and try again"))?;
    if !metadata.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&metadata.stderr).trim()
        ));
    }
    let document: serde_json::Value = serde_json::from_slice(&metadata.stdout)
        .map_err(|e| format!("parse cargo metadata JSON: {e}"))?;
    let packages = document["packages"]
        .as_array()
        .ok_or("cargo metadata omitted packages")?;
    let nodes = document["resolve"]["nodes"]
        .as_array()
        .ok_or("cargo metadata omitted resolve graph")?;
    let members: Vec<String> = document["workspace_members"]
        .as_array()
        .ok_or("cargo metadata omitted workspace members")?
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    let graph: BTreeMap<&str, Vec<&str>> = nodes
        .iter()
        .filter_map(|node| {
            let id = node["id"].as_str()?;
            let deps = node["deps"]
                .as_array()?
                .iter()
                .filter_map(|d| d["pkg"].as_str())
                .collect();
            Some((id, deps))
        })
        .collect();
    let ids: BTreeMap<&str, &serde_json::Value> = packages
        .iter()
        .filter_map(|package| Some((package["id"].as_str()?, package)))
        .collect();
    let mut reachable = std::collections::BTreeSet::new();
    let mut pending = members.iter().map(String::as_str).collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        if reachable.insert(id) {
            pending.extend(graph.get(id).into_iter().flatten().copied());
        }
    }
    let mut actual = BTreeMap::new();
    let mut problems = Vec::new();
    for id in reachable {
        let Some(package) = ids.get(id) else { continue };
        let Some(source) = package["source"].as_str() else {
            continue;
        };
        let Some(name) = package["name"].as_str() else {
            continue;
        };
        let Some(version) = package["version"].as_str() else {
            continue;
        };
        let Some(license) = package["license"].as_str() else {
            problems.push(format!(
                "missing license metadata: {name} {version} ({source})"
            ));
            continue;
        };
        actual.insert(
            (name.to_owned(), version.to_owned()),
            (license.to_owned(), source.to_owned()),
        );
    }
    let expected: BTreeMap<_, _> = inventory
        .cargo_packages
        .iter()
        .map(|p| {
            (
                (p.name.clone(), p.version.clone()),
                (p.license.clone(), p.source.clone()),
            )
        })
        .collect();
    for (key, value) in &actual {
        match expected.get(key) {
            None => problems.push(format!(
                "unrecorded crate: {} {} ({})",
                key.0, key.1, value.1
            )),
            Some(recorded) if recorded != value => problems.push(format!(
                "crate metadata changed: {} {} (inventory {:?}, actual {:?})",
                key.0, key.1, recorded, value
            )),
            _ => {}
        }
    }
    for key in expected.keys() {
        if !actual.contains_key(key) {
            problems.push(format!("stale crate record: {} {}", key.0, key.1));
        }
    }
    let workflow =
        fs::read_dir(root.join(".github/workflows")).map_err(|e| format!("read workflows: {e}"))?;
    let mut actions = std::collections::BTreeSet::new();
    for entry in workflow {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s == "yml" || s == "yaml")
        {
            let text =
                fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
            for line in text.lines() {
                if let Some((_, action)) = line.split_once("uses:") {
                    actions.insert(action.trim().to_owned());
                }
            }
        }
    }
    let expected_actions: std::collections::BTreeSet<_> = inventory
        .workflow_actions
        .iter()
        .map(|a| a.reference.clone())
        .collect();
    for action in actions.difference(&expected_actions) {
        problems.push(format!("unrecorded workflow action: {action}"));
    }
    for action in expected_actions.difference(&actions) {
        problems.push(format!("stale workflow action record: {action}"));
    }
    let mut images = std::collections::BTreeSet::new();
    let mut installed_packages = std::collections::BTreeSet::new();
    for dockerfile in [
        ".devcontainer/Dockerfile",
        ".devcontainer/Dockerfile.omedit",
    ] {
        let path = root.join(dockerfile);
        let docker = fs::read_to_string(&path)
            .map_err(|e| format!("read devcontainer image {}: {e}", path.display()))?;
        let (file_images, file_packages) = parse_docker_inputs(&docker);
        images.extend(file_images);
        installed_packages.extend(file_packages);
    }
    let expected_images: std::collections::BTreeSet<_> =
        inventory.container_images.iter().cloned().collect();
    for image in images.difference(&expected_images) {
        problems.push(format!("unrecorded container image: {image}"));
    }
    for image in expected_images.difference(&images) {
        problems.push(format!("stale container image record: {image}"));
    }
    let expected_packages: std::collections::BTreeSet<_> =
        inventory.container_packages.iter().cloned().collect();
    for package in installed_packages.difference(&expected_packages) {
        problems.push(format!("unrecorded container package: {package}"));
    }
    for package in expected_packages.difference(&installed_packages) {
        problems.push(format!("stale container package record: {package}"));
    }
    let report = make_report(&inventory, &actual, &problems);
    if let Some(path) = output {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|e| format!("create report directory: {e}"))?;
        }
        fs::write(&path, &report)
            .map_err(|e| format!("write inventory report {}: {e}", path.display()))?;
    }
    if problems.is_empty() {
        Ok(report)
    } else {
        Err(format!(
            "license inventory mismatch ({}):\n{}",
            problems.len(),
            problems.join("\n")
        ))
    }
}

fn parse_docker_inputs(dockerfile: &str) -> (Vec<String>, Vec<String>) {
    let mut images = Vec::new();
    let mut packages = Vec::new();
    let mut reading_packages = false;
    for line in dockerfile.lines() {
        if let Some(image) = line.strip_prefix("FROM ") {
            if let Some(image) = image.split_whitespace().next() {
                images.push(image.to_owned());
            }
        }
        if let Some((_, rest)) = line.split_once("apt-get install -y --no-install-recommends") {
            reading_packages = true;
            packages.extend(package_tokens(rest));
            continue;
        }
        if reading_packages {
            if line.contains("&& rm -rf /var/lib/apt/lists") {
                packages.extend(package_tokens(line));
                reading_packages = false;
            } else {
                packages.extend(package_tokens(line));
            }
        }
    }
    (images, packages)
}

fn package_tokens(line: &str) -> impl Iterator<Item = String> + '_ {
    line.split_once("&& rm -rf /var/lib/apt/lists")
        .map_or(line, |(install, _)| install)
        .split_whitespace()
        .filter(|token| *token != "\\" && *token != "&&")
        .filter(|token| {
            token
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "+._-=".contains(character))
        })
        .map(str::to_owned)
}

fn make_report(
    inventory: &Inventory,
    actual: &BTreeMap<(String, String), (String, String)>,
    problems: &[String],
) -> String {
    let mut report = String::from("# Dependency and license inventory\n\nGenerated by `omola licenses check`. Cargo packages are resolved from the locked workspace graph.\n\n## Rust dependencies\n\n| Package | Version | Declared license | Source |\n|---|---|---|---|\n");
    for ((name, version), (license, source)) in actual {
        report.push_str(&format!("| {name} | {version} | {license} | {source} |\n"));
    }
    report.push_str("\n## External components\n\n| Component | Version | License / terms | Source | Distribution decision |\n|---|---|---|---|---|\n");
    for c in &inventory.external_components {
        report.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            c.name, c.version, c.license, c.source, c.distribution
        ));
    }
    report.push_str("\n## CI actions\n\n| Reference | Source | License | Distribution decision |\n|---|---|---|---|\n");
    for action in &inventory.workflow_actions {
        report.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            action.reference, action.source, action.license, action.distribution
        ));
    }
    report.push_str("\n## Development container inputs\n\n| Input |\n|---|\n");
    for image in &inventory.container_images {
        report.push_str(&format!("| image `{image}` |\n"));
    }
    for package in &inventory.container_packages {
        report.push_str(&format!("| package `{package}` |\n"));
    }
    report.push_str("\n## Guard result\n\n");
    if problems.is_empty() {
        report.push_str("PASS — dependency, workflow-action, container-image, and container package inventory matches the repository.\n");
    } else {
        report.push_str(&format!("FAIL — {} mismatch(es).\n", problems.len()));
        for problem in problems {
            report.push_str(&format!("- {problem}\n"));
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_is_explicit_when_no_dynamic_dependencies_exist() {
        let inv = Inventory {
            schema_version: 1,
            cargo_packages: vec![],
            external_components: vec![],
            workflow_actions: vec![],
            container_images: vec![],
            container_packages: vec![],
        };
        let report = make_report(&inv, &BTreeMap::new(), &[]);
        assert!(report.contains("PASS"));
        assert!(report.contains("External components"));
    }

    #[test]
    fn docker_inventory_parser_includes_multiline_gui_image_packages() {
        let dockerfile = r#"FROM example/base:1 AS build
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl build-essential \
        omedit=1.2-3 \
    && rm -rf /var/lib/apt/lists/*
FROM example/runtime:1
"#;
        let (images, packages) = parse_docker_inputs(dockerfile);
        assert_eq!(images, ["example/base:1", "example/runtime:1"]);
        assert_eq!(packages, ["curl", "build-essential", "omedit=1.2-3"]);
    }
}
