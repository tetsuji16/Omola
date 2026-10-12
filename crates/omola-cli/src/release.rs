use omola_core::BackendError;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
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
    omc_version_prefix: Option<String>,
}

#[derive(Deserialize)]
struct Scorecard {
    schema_version: u32,
    manifest: String,
    omc_version: Option<String>,
    compatibility_claim: String,
    totals: Counts,
    fixtures: Vec<ScorecardFixture>,
}

#[derive(Deserialize, Default, PartialEq, Eq)]
struct Counts {
    pass: usize,
    fail: usize,
    skip: usize,
}

#[derive(Deserialize)]
struct ScorecardFixture {
    id: String,
    expected: String,
    outcome: String,
    omc_version: Option<String>,
}

pub fn run(args: &[String]) -> Result<String, BackendError> {
    if args.len() != 5 || args[0] != "verify" || args[1] != "--scorecard" || args[3] != "--manifest"
    {
        return Err(BackendError::InvalidInput(
            "usage: release verify --scorecard FILE.json --manifest FILE.toml".into(),
        ));
    }
    let scorecard_path = PathBuf::from(&args[2]);
    let manifest_path = PathBuf::from(&args[4]);
    let scorecard: Scorecard = read_json(&scorecard_path)?;
    let manifest: Manifest = read_toml(&manifest_path)?;

    if scorecard.schema_version != 2 || manifest.schema_version != 2 {
        return Err(invalid(
            "scorecard and manifest schema_version must both be 2",
        ));
    }
    if scorecard.manifest != manifest.name {
        return Err(invalid(format!(
            "stale scorecard manifest name: expected '{}', found '{}'",
            manifest.name, scorecard.manifest
        )));
    }
    if !scorecard
        .compatibility_claim
        .contains("does not establish Dymola compatibility")
    {
        return Err(invalid("scorecard must explicitly state that its evidence does not establish Dymola compatibility"));
    }
    let omc_version = scorecard
        .omc_version
        .as_deref()
        .filter(|version| !version.trim().is_empty())
        .ok_or_else(|| invalid("release scorecard must contain a tested OMC version"))?;

    let expected_ids: BTreeSet<_> = manifest
        .fixtures
        .iter()
        .map(|fixture| fixture.id.as_str())
        .collect();
    let actual_ids: BTreeSet<_> = scorecard
        .fixtures
        .iter()
        .map(|fixture| fixture.id.as_str())
        .collect();
    if expected_ids.len() != manifest.fixtures.len() || actual_ids.len() != scorecard.fixtures.len()
    {
        return Err(invalid("manifest and scorecard fixture IDs must be unique"));
    }
    if expected_ids != actual_ids {
        return Err(invalid(format!(
            "stale or incomplete scorecard fixture set (manifest: {}; report: {})",
            expected_ids.iter().copied().collect::<Vec<_>>().join(", "),
            actual_ids.iter().copied().collect::<Vec<_>>().join(", ")
        )));
    }

    let expected_by_id: BTreeMap<_, _> = manifest
        .fixtures
        .iter()
        .map(|fixture| (fixture.id.as_str(), fixture))
        .collect();
    let mut counts = Counts::default();
    for result in &scorecard.fixtures {
        let fixture = expected_by_id[result.id.as_str()];
        if fixture.expected != result.expected {
            return Err(invalid(format!(
                "stale expected outcome for fixture '{}': manifest='{}', report='{}'",
                result.id, fixture.expected, result.expected
            )));
        }
        match fixture.expected.as_str() {
            "skip" if result.outcome == "skip" => counts.skip += 1,
            "pass" | "fail" if result.outcome == "pass" => {
                let tested_version = result.omc_version.as_deref().unwrap_or(omc_version);
                if fixture
                    .omc_version_prefix
                    .as_deref()
                    .is_some_and(|prefix| !tested_version.starts_with(prefix))
                {
                    return Err(invalid(format!(
                        "fixture '{}' was measured with unsupported OMC version {}",
                        result.id, tested_version
                    )));
                }
                counts.pass += 1;
            }
            "pass" | "fail" => {
                return Err(invalid(format!(
                    "required fixture '{}' did not pass (outcome: {})",
                    result.id, result.outcome
                )))
            }
            _ => {
                return Err(invalid(format!(
                    "fixture '{}' has invalid expected outcome",
                    result.id
                )))
            }
        }
    }
    if counts != scorecard.totals {
        return Err(invalid(
            "reported pass/fail/skip totals do not match fixture outcomes",
        ));
    }

    Ok(format!(
        "Release scorecard gate: PASS\nManifest: {}\nOpenModelica: {}\nResults: {} pass, {} fail, {} skip\n",
        manifest.name, omc_version, counts.pass, counts.fail, counts.skip
    ))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &PathBuf) -> Result<T, BackendError> {
    let text = fs::read_to_string(path)
        .map_err(|error| invalid(format!("could not read {}: {error}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|error| invalid(format!("invalid JSON {}: {error}", path.display())))
}

fn read_toml<T: for<'de> Deserialize<'de>>(path: &PathBuf) -> Result<T, BackendError> {
    let text = fs::read_to_string(path)
        .map_err(|error| invalid(format!("could not read {}: {error}", path.display())))?;
    toml::from_str(&text)
        .map_err(|error| invalid(format!("invalid TOML {}: {error}", path.display())))
}

fn invalid(message: impl Into<String>) -> BackendError {
    BackendError::InvalidInput(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn release_gate_rejects_missing_or_stale_fixture_evidence() {
        let root = std::env::temp_dir().join(format!(
            "omola-release-gate-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let manifest_path = root.join("manifest.toml");
        let scorecard_path = root.join("scorecard.json");
        fs::write(&manifest_path, "schema_version=2\nname=\"small corpus\"\n[[fixtures]]\nid=\"required\"\nexpected=\"pass\"\nomc_version_prefix=\"OpenModelica 1.27.\"\n[[fixtures]]\nid=\"excluded\"\nexpected=\"skip\"\n").unwrap();
        fs::write(&scorecard_path, r#"{"schema_version":2,"manifest":"small corpus","omc_version":"OpenModelica 1.27.1","compatibility_claim":"subset does not establish Dymola compatibility","totals":{"pass":1,"fail":0,"skip":1},"fixtures":[{"id":"required","expected":"pass","outcome":"pass","omc_version":"OpenModelica 1.27.1"},{"id":"excluded","expected":"skip","outcome":"skip"}]}"#).unwrap();
        let args = vec![
            "verify".into(),
            "--scorecard".into(),
            scorecard_path.display().to_string(),
            "--manifest".into(),
            manifest_path.display().to_string(),
        ];
        assert!(run(&args).unwrap().contains("Release scorecard gate: PASS"));
        fs::write(&scorecard_path, r#"{"schema_version":2,"manifest":"small corpus","omc_version":"OpenModelica 1.27.1","compatibility_claim":"subset does not establish Dymola compatibility","totals":{"pass":0,"fail":0,"skip":1},"fixtures":[{"id":"excluded","expected":"skip","outcome":"skip"}]}"#).unwrap();
        assert!(run(&args)
            .unwrap_err()
            .to_string()
            .contains("stale or incomplete scorecard fixture set"));
        fs::remove_dir_all(root).unwrap();
    }
}
