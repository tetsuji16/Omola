use omola_core::BackendError;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
struct Manifest {
    schema_version: u32,
    name: String,
    fixtures: Vec<ManifestFixture>,
}

#[derive(Deserialize)]
struct ManifestFixture {
    id: String,
    expected: String,
    model: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    schema_version: u32,
    manifest: String,
    manifest_sha256: String,
    environment: Environment,
    fixtures: Vec<EvidenceFixture>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Environment {
    os: String,
    omc_version: String,
    omc_origin: String,
    omedit_version: String,
    omedit_origin: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceFixture {
    id: String,
    status: String,
    model: Option<String>,
    model_sha256: Option<String>,
    evidence_kind: String,
    note: String,
}

pub fn run(args: &[String]) -> Result<String, BackendError> {
    let (evidence_path, manifest_path, require_verified) = parse_args(args)?;
    let evidence_text = fs::read_to_string(&evidence_path).map_err(|error| {
        invalid(format!(
            "could not read {}: {error}",
            evidence_path.display()
        ))
    })?;
    let evidence: Evidence = serde_json::from_str(&evidence_text).map_err(|error| {
        invalid(format!(
            "invalid evidence JSON {}: {error}",
            evidence_path.display()
        ))
    })?;
    let manifest_text = fs::read_to_string(&manifest_path).map_err(|error| {
        invalid(format!(
            "could not read {}: {error}",
            manifest_path.display()
        ))
    })?;
    let manifest: Manifest = toml::from_str(&manifest_text).map_err(|error| {
        invalid(format!(
            "invalid manifest TOML {}: {error}",
            manifest_path.display()
        ))
    })?;

    if evidence.schema_version != 1 || manifest.schema_version != 2 {
        return Err(invalid(
            "evidence schema_version must be 1 and manifest schema_version must be 2",
        ));
    }
    if evidence.manifest != manifest.name {
        return Err(invalid(format!(
            "evidence manifest name does not match: expected '{}', found '{}'",
            manifest.name, evidence.manifest
        )));
    }
    let current_manifest_hash = sha256(manifest_text.as_bytes());
    if evidence.manifest_sha256 != current_manifest_hash {
        return Err(invalid(
            "stale OMEdit evidence: manifest SHA-256 does not match",
        ));
    }
    validate_environment(&evidence.environment)?;
    if require_verified && environment_is_unverified(&evidence.environment) {
        return Err(invalid(
            "release evidence requires measured OS, OMC, and OMEdit versions and package origins",
        ));
    }

    let root = find_workspace_root(&manifest_path)?;
    if manifest.fixtures.iter().any(|fixture| {
        fixture.id.trim().is_empty()
            || !matches!(fixture.expected.as_str(), "pass" | "fail" | "skip")
    }) {
        return Err(invalid(
            "manifest fixtures require non-empty IDs and expected values pass, fail, or skip",
        ));
    }
    let fixtures_by_id: BTreeMap<_, _> = manifest
        .fixtures
        .iter()
        .map(|fixture| (fixture.id.as_str(), fixture))
        .collect();
    if fixtures_by_id.len() != manifest.fixtures.len() {
        return Err(invalid("manifest fixture IDs must be unique"));
    }
    let mut seen = BTreeSet::new();
    let (mut pass, mut fail, mut unverified) = (0usize, 0usize, 0usize);
    for result in &evidence.fixtures {
        if !seen.insert(result.id.as_str()) {
            return Err(invalid(format!(
                "duplicate evidence fixture id '{}'",
                result.id
            )));
        }
        let fixture = fixtures_by_id
            .get(result.id.as_str())
            .ok_or_else(|| invalid(format!("unknown evidence fixture id '{}'", result.id)))?;
        if result.evidence_kind != "manual" && result.evidence_kind != "automated" {
            return Err(invalid(format!(
                "fixture '{}' evidence_kind must be manual or automated",
                result.id
            )));
        }
        if result.note.trim().is_empty() {
            return Err(invalid(format!(
                "fixture '{}' must include a note",
                result.id
            )));
        }
        match result.status.as_str() {
            "PASS" => pass += 1,
            "FAIL" => fail += 1,
            "UNVERIFIED" => unverified += 1,
            _ => {
                return Err(invalid(format!(
                    "fixture '{}' has invalid status '{}'",
                    result.id, result.status
                )))
            }
        }
        match fixture.model.as_deref() {
            Some(model) => {
                if result.model.as_deref() != Some(model) {
                    return Err(invalid(format!(
                        "fixture '{}' model path is stale or mismatched",
                        result.id
                    )));
                }
                let model_path = confined_model_path(&root, &manifest_path, model)?;
                let model_hash = sha256(&fs::read(&model_path).map_err(|error| {
                    invalid(format!(
                        "could not read fixture model {}: {error}",
                        model_path.display()
                    ))
                })?);
                if result.model_sha256.as_deref() != Some(model_hash.as_str()) {
                    return Err(invalid(format!(
                        "stale OMEdit evidence: fixture '{}' model SHA-256 does not match",
                        result.id
                    )));
                }
            }
            None if result.model.is_none() && result.model_sha256.is_none() => {}
            None => {
                return Err(invalid(format!(
                    "fixture '{}' has model hash fields but the manifest has no model",
                    result.id
                )))
            }
        }
        if require_verified && fixture.expected != "skip" && result.status != "PASS" {
            return Err(invalid(format!(
                "release evidence requires fixture '{}' to PASS; found {}",
                result.id, result.status
            )));
        }
    }
    let expected_ids: BTreeSet<_> = fixtures_by_id.keys().copied().collect();
    if seen != expected_ids {
        let missing = expected_ids
            .difference(&seen)
            .copied()
            .collect::<Vec<_>>()
            .join(", ");
        return Err(invalid(format!(
            "evidence fixture set is incomplete; missing: {missing}"
        )));
    }
    Ok(format!(
        "OMEdit evidence record verification: PASS\nManifest: {}\nManifest SHA-256: {}\nEnvironment: {} / OMC {} ({}) / OMEdit {} ({})\nFixture results: {pass} PASS, {fail} FAIL, {unverified} UNVERIFIED\n",
        manifest.name, current_manifest_hash, evidence.environment.os,
        evidence.environment.omc_version, evidence.environment.omc_origin,
        evidence.environment.omedit_version, evidence.environment.omedit_origin
    ))
}

fn parse_args(args: &[String]) -> Result<(PathBuf, PathBuf, bool), BackendError> {
    if args.first().map(String::as_str) != Some("evidence")
        || args.get(1).map(String::as_str) != Some("verify")
    {
        return Err(usage_error());
    }
    let (mut evidence, mut manifest, mut require_verified) = (None, None, false);
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--evidence" | "--manifest" => {
                let option = args[index].as_str();
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| invalid(format!("missing value for {option}")))?;
                let slot = if option == "--evidence" {
                    &mut evidence
                } else {
                    &mut manifest
                };
                if slot.replace(PathBuf::from(value)).is_some() {
                    return Err(invalid(format!("repeated option {option}")));
                }
                index += 2;
            }
            "--require-verified" if !require_verified => {
                require_verified = true;
                index += 1;
            }
            other => return Err(invalid(format!("unknown or repeated option {other}"))),
        }
    }
    Ok((
        evidence.ok_or_else(usage_error)?,
        manifest.ok_or_else(usage_error)?,
        require_verified,
    ))
}

fn validate_environment(environment: &Environment) -> Result<(), BackendError> {
    for (name, value) in [
        ("os", &environment.os),
        ("omc_version", &environment.omc_version),
        ("omc_origin", &environment.omc_origin),
        ("omedit_version", &environment.omedit_version),
        ("omedit_origin", &environment.omedit_origin),
    ] {
        if value.trim().is_empty() {
            return Err(invalid(format!("evidence environment field '{name}' must be recorded (use 'unverified' when not measured)")));
        }
    }
    Ok(())
}

fn environment_is_unverified(environment: &Environment) -> bool {
    [
        &environment.os,
        &environment.omc_version,
        &environment.omc_origin,
        &environment.omedit_version,
        &environment.omedit_origin,
    ]
    .into_iter()
    .any(|value| value.to_ascii_lowercase().contains("unverified"))
}

fn find_workspace_root(manifest: &Path) -> Result<PathBuf, BackendError> {
    let manifest = fs::canonicalize(manifest).map_err(BackendError::Io)?;
    manifest
        .parent()
        .and_then(|start| {
            start
                .ancestors()
                .find(|path| path.join("Cargo.toml").is_file())
        })
        .map(fs::canonicalize)
        .transpose()
        .map_err(BackendError::Io)?
        .ok_or_else(|| invalid("manifest is not inside a workspace containing Cargo.toml"))
}

fn confined_model_path(root: &Path, manifest: &Path, model: &str) -> Result<PathBuf, BackendError> {
    let relative = Path::new(model);
    if relative.is_absolute() {
        return Err(invalid(format!(
            "fixture model path must be relative to manifest: {model}"
        )));
    }
    let manifest_dir = fs::canonicalize(manifest)
        .map_err(BackendError::Io)?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| invalid("manifest has no parent directory"))?;
    let resolved = fs::canonicalize(manifest_dir.join(relative)).map_err(|error| {
        invalid(format!(
            "could not resolve fixture model '{model}': {error}"
        ))
    })?;
    if !resolved.starts_with(root) {
        return Err(invalid(format!(
            "fixture model escapes workspace root: {model}"
        )));
    }
    Ok(resolved)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn usage_error() -> BackendError {
    invalid("usage: omola omedit evidence verify --evidence FILE.json --manifest FILE.toml [--require-verified]")
}

fn invalid(message: impl Into<String>) -> BackendError {
    BackendError::InvalidInput(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_files() -> (PathBuf, PathBuf, String, String) {
        let root = std::env::temp_dir().join(format!(
            "omola-omedit-evidence-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
        fs::write(root.join("tests/model.mo"), "model A end A;\n").unwrap();
        let manifest = root.join("tests/manifest.toml");
        fs::write(&manifest, "schema_version=2\nname=\"test corpus\"\n[[fixtures]]\nid=\"modeled\"\nexpected=\"pass\"\nmodel=\"model.mo\"\n[[fixtures]]\nid=\"excluded\"\nexpected=\"skip\"\n").unwrap();
        let manifest_text = fs::read_to_string(&manifest).unwrap();
        let manifest_hash = sha256(manifest_text.as_bytes());
        let model_hash = sha256(&fs::read(root.join("tests/model.mo")).unwrap());
        (root, manifest, manifest_hash, model_hash)
    }

    fn write_evidence(root: &Path, manifest_hash: &str, model_hash: &str) -> PathBuf {
        let evidence = root.join("evidence.json");
        fs::write(&evidence, format!(r#"{{"schema_version":1,"manifest":"test corpus","manifest_sha256":"{manifest_hash}","environment":{{"os":"unverified","omc_version":"unverified","omc_origin":"unverified","omedit_version":"unverified","omedit_origin":"unverified"}},"fixtures":[{{"id":"modeled","status":"UNVERIFIED","model":"model.mo","model_sha256":"{model_hash}","evidence_kind":"manual","note":"GUI display interaction not yet measured"}},{{"id":"excluded","status":"UNVERIFIED","evidence_kind":"manual","note":"excluded vendor fixture"}}]}}"#)).unwrap();
        evidence
    }

    fn args(evidence: &Path, manifest: &Path, strict: bool) -> Vec<String> {
        let mut args = vec![
            "evidence".into(),
            "verify".into(),
            "--evidence".into(),
            evidence.display().to_string(),
            "--manifest".into(),
            manifest.display().to_string(),
        ];
        if strict {
            args.push("--require-verified".into());
        }
        args
    }

    #[test]
    fn normal_mode_accepts_unverified_but_release_mode_rejects_it() {
        let (root, manifest, manifest_hash, model_hash) = fixture_files();
        let evidence = write_evidence(&root, &manifest_hash, &model_hash);
        let report = run(&args(&evidence, &manifest, false)).unwrap();
        assert!(report.contains("0 PASS, 0 FAIL, 2 UNVERIFIED"));
        assert!(run(&args(&evidence, &manifest, true))
            .unwrap_err()
            .to_string()
            .contains("requires measured OS"));
        let verified_environment = fs::read_to_string(&evidence)
            .unwrap()
            .replace("\"unverified\"", "\"measured test environment\"");
        fs::write(&evidence, verified_environment).unwrap();
        assert!(run(&args(&evidence, &manifest, true))
            .unwrap_err()
            .to_string()
            .contains("requires fixture 'modeled' to PASS"));
        let passing_fixture = fs::read_to_string(&evidence).unwrap().replace(
            "\"id\":\"modeled\",\"status\":\"UNVERIFIED\"",
            "\"id\":\"modeled\",\"status\":\"PASS\"",
        );
        fs::write(&evidence, passing_fixture).unwrap();
        assert!(run(&args(&evidence, &manifest, true)).is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_stale_manifest_and_model_hashes() {
        let (root, manifest, manifest_hash, model_hash) = fixture_files();
        let evidence = write_evidence(&root, &manifest_hash, &model_hash);
        let original_manifest = fs::read_to_string(&manifest).unwrap();
        fs::write(
            &manifest,
            original_manifest.replace("test corpus", "changed corpus"),
        )
        .unwrap();
        assert!(run(&args(&evidence, &manifest, false))
            .unwrap_err()
            .to_string()
            .contains("manifest name"));
        fs::write(&manifest, &original_manifest).unwrap();
        fs::write(root.join("tests/model.mo"), "model Changed end Changed;\n").unwrap();
        let changed_hash = sha256(original_manifest.as_bytes());
        let evidence = write_evidence(&root, &changed_hash, &model_hash);
        assert!(run(&args(&evidence, &manifest, false))
            .unwrap_err()
            .to_string()
            .contains("model SHA-256"));
        fs::write(&manifest, format!("{original_manifest}\n# modified\n")).unwrap();
        assert!(run(&args(&evidence, &manifest, false))
            .unwrap_err()
            .to_string()
            .contains("manifest SHA-256"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_duplicate_unknown_and_incomplete_fixture_sets() {
        let (root, manifest, manifest_hash, model_hash) = fixture_files();
        let evidence = write_evidence(&root, &manifest_hash, &model_hash);
        let original = fs::read_to_string(&evidence).unwrap();
        fs::write(
            &evidence,
            original.replace("{\"id\":\"excluded\"", "{\"id\":\"modeled\""),
        )
        .unwrap();
        assert!(run(&args(&evidence, &manifest, false))
            .unwrap_err()
            .to_string()
            .contains("duplicate"));
        fs::write(
            &evidence,
            original.replace("\"id\":\"excluded\"", "\"id\":\"unknown\""),
        )
        .unwrap();
        assert!(run(&args(&evidence, &manifest, false))
            .unwrap_err()
            .to_string()
            .contains("unknown evidence"));
        fs::write(&evidence, original.replace(",{\"id\":\"excluded\",\"status\":\"UNVERIFIED\",\"evidence_kind\":\"manual\",\"note\":\"excluded vendor fixture\"}", "")).unwrap();
        assert!(run(&args(&evidence, &manifest, false))
            .unwrap_err()
            .to_string()
            .contains("incomplete"));
        let duplicated_manifest = fs::read_to_string(&manifest).unwrap().replace(
            "[[fixtures]]\nid=\"excluded\"\nexpected=\"skip\"",
            "[[fixtures]]\nid=\"modeled\"\nexpected=\"skip\"",
        );
        fs::write(&manifest, duplicated_manifest).unwrap();
        let changed_manifest_hash = sha256(fs::read(&manifest).unwrap().as_slice());
        let evidence_text = fs::read_to_string(&evidence)
            .unwrap()
            .replace(&manifest_hash, &changed_manifest_hash);
        fs::write(&evidence, evidence_text).unwrap();
        assert!(run(&args(&evidence, &manifest, false))
            .unwrap_err()
            .to_string()
            .contains("manifest fixture IDs must be unique"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_missing_environment_provenance_and_model_escape() {
        let (root, manifest, manifest_hash, model_hash) = fixture_files();
        let evidence = write_evidence(&root, &manifest_hash, &model_hash);
        let text = fs::read_to_string(&evidence)
            .unwrap()
            .replace("\"omedit_origin\":\"unverified\"", "\"omedit_origin\":\"\"");
        fs::write(&evidence, text).unwrap();
        assert!(run(&args(&evidence, &manifest, false))
            .unwrap_err()
            .to_string()
            .contains("omedit_origin"));
        let original_manifest = fs::read_to_string(&manifest).unwrap();
        let outside_name = format!("{}-outside.mo", root.file_name().unwrap().to_string_lossy());
        let escaped_model = format!("../../{outside_name}");
        let escaped_manifest = original_manifest.replace("model.mo", &escaped_model);
        fs::write(&manifest, &escaped_manifest).unwrap();
        let outside = root.parent().unwrap().join(&outside_name);
        fs::write(&outside, "model Outside end Outside;\n").unwrap();
        let escaped_hash = sha256(escaped_manifest.as_bytes());
        let outside_hash = sha256(&fs::read(&outside).unwrap());
        let fresh_evidence = write_evidence(&root, &manifest_hash, &model_hash);
        let text = fs::read_to_string(&fresh_evidence)
            .unwrap()
            .replace(&manifest_hash, &escaped_hash)
            .replace("model.mo", &escaped_model)
            .replace(&model_hash, &outside_hash);
        fs::write(&evidence, text).unwrap();
        assert!(run(&args(&evidence, &manifest, false))
            .unwrap_err()
            .to_string()
            .contains("escapes workspace"));
        fs::remove_dir_all(root).unwrap();
        fs::remove_file(outside).unwrap();
    }
}
