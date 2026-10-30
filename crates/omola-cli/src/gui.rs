//! Measured status of the OMEdit-based GUI integration toward Dymola-compatible
//! graphical workflows. This command reports where the GUI/editor dimension
//! stands today; it never claims Dymola parity.

use crate::omedit;
use omola_core::{BackendError, Omc};
use serde::Serialize;
use std::path::{Path, PathBuf};

const LABEL: &str = "Omola via OpenModelica";
const COMPATIBILITY_CLAIM: &str =
    "This GUI status reports measured integration points only and does not claim Dymola parity.";

/// One row of the GUI/editor compatibility dimension matrix.
#[derive(Serialize)]
struct Dimension {
    area: &'static str,
    status: &'static str,
    detail: &'static str,
}

const DIMENSIONS: &[Dimension] = &[
    Dimension {
        area: "Text and diagram editing",
        status: "reused",
        detail: "OMEdit's existing Modelica text and diagram editors are reused; Dymola-specific extensions are unknown and unmeasured.",
    },
    Dimension {
        area: "Package and project browsing",
        status: "partial",
        detail: "OMEdit library/model browser plus the Omola schema-1 project manifest; native 'Open Omola Project...' action is in progress (#15) and the library version the GUI session selects is unverified (#22).",
    },
    Dimension {
        area: "Check and simulation",
        status: "partial",
        detail: "OMEdit actions delegate to the OMC backend; the source-built OMEdit simulation was not verified to produce a runnable model, and manifest settings are not yet propagated into the GUI session (#16).",
    },
    Dimension {
        area: "Simulation settings handoff",
        status: "not-measured",
        detail: "Manifest stop time and numeric parameter overrides must appear in the GUI and apply to the real OMC run (#16); stock OMEdit does not receive them, so this requires the patched OMEdit build.",
    },
    Dimension {
        area: "Results and plotting",
        status: "reused",
        detail: "OMEdit variable browser and plot windows are reused; Dymola MAT/DSRES result formats are unsupported unless directly measured.",
    },
    Dimension {
        area: "Diagram annotations and connections",
        status: "not-measured",
        detail: "OMEdit icon/diagram annotation and connection round-tripping needs a lawful fixture matrix distinguishing Modelica, OMEdit, and Dymola semantics (#18).",
    },
    Dimension {
        area: "Native GUI regression gate",
        status: "not-measured",
        detail: "A versioned open/check/simulate/plot regression report for the source/toolchain build is not yet produced (#19).",
    },
    Dimension {
        area: "Dymola-specific GUI semantics",
        status: "unsupported",
        detail: "Proprietary Dymola graphical features have no lawful, measured Omola mapping and are out of scope until one exists.",
    },
];

const BLOCKERS: &[&str] = &[
    "Native OMEdit source build (#25) is not completed: CMake source configuration has not finished and the OMDev/UCRT64 toolchain download stalled, so no modified OMEdit C++ has compiled.",
    "Stock OMEdit does not receive manifest simulation settings on launch (#16); 'omola project open' validates and launches but the simulation dialog keeps its own defaults.",
];

#[derive(Serialize)]
struct EditorStatus {
    installed: bool,
    path: Option<PathBuf>,
    version_display: String,
    version_warning: Option<String>,
    mismatch: bool,
}

#[derive(Serialize)]
struct BackendStatus {
    found: bool,
    version: Option<String>,
}

#[derive(Serialize)]
struct GuiStatus {
    format_version: u32,
    label: &'static str,
    compatibility_claim: &'static str,
    project_bridge: &'static str,
    backend: BackendStatus,
    editor: EditorStatus,
    dimensions: &'static [Dimension],
    known_blockers: &'static [&'static str],
}

pub fn run(args: &[String]) -> Result<String, BackendError> {
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    let format = match argv.as_slice() {
        [] | ["status"] => "markdown",
        ["status", flag, value] if *flag == "--format" => *value,
        _ => return Err(usage()),
    };
    let status = collect_status();
    render(&status, format)
}

/// Collect the live backend/editor probes. Probe failures degrade to
/// `not found`/`unavailable` instead of failing the status report.
fn collect_status() -> GuiStatus {
    let backend = match Omc::discover() {
        Ok(omc) => BackendStatus {
            found: true,
            version: omc.version().ok(),
        },
        Err(_) => BackendStatus {
            found: false,
            version: None,
        },
    };
    let backend_version = backend.version.clone().unwrap_or_else(|| "unknown".into());
    let editor_path = omedit::probe();
    let editor = match &editor_path {
        Some(path) => {
            let version_status = omedit::verify_version(path, &backend_version);
            EditorStatus {
                installed: true,
                path: Some(path.clone()),
                version_display: version_status.display,
                version_warning: version_status.warning,
                mismatch: version_status.mismatch,
            }
        }
        None => EditorStatus {
            installed: false,
            path: None,
            version_display: "OMEdit was not found on PATH or OMOLA_OMEDIT".into(),
            version_warning: Some(
                "install the OMEdit build matching OpenModelica or set OMOLA_OMEDIT".into(),
            ),
            mismatch: false,
        },
    };
    GuiStatus {
        format_version: 1,
        label: LABEL,
        compatibility_claim: COMPATIBILITY_CLAIM,
        project_bridge:
            "bridge format version 1 via `omola project inspect MANIFEST.toml --format json`",
        backend,
        editor,
        dimensions: DIMENSIONS,
        known_blockers: BLOCKERS,
    }
}

fn render(status: &GuiStatus, format: &str) -> Result<String, BackendError> {
    match format {
        "json" => serde_json::to_string_pretty(status).map_err(|error| {
            BackendError::InvalidInput(format!("could not encode GUI status: {error}"))
        }),
        "markdown" => Ok(render_markdown(status)),
        _ => Err(BackendError::InvalidInput(
            "gui status format must be json or markdown".into(),
        )),
    }
}

fn render_markdown(status: &GuiStatus) -> String {
    let mut text = String::new();
    text.push_str("# OMEdit GUI integration status\n\n");
    text.push_str(&format!("Label: {LABEL}  \n"));
    text.push_str(&format!("Project bridge: {}  \n", status.project_bridge));
    text.push_str(&format!("{COMPATIBILITY_CLAIM}\n\n"));
    text.push_str("## Backend and editor\n\n");
    text.push_str(&format!(
        "- OpenModelica compiler (`omc`): {}\n",
        backend_line(&status.backend)
    ));
    text.push_str(&format!("- OMEdit editor: {}\n", editor_line(status)));
    text.push_str("\n## Dymola GUI compatibility dimensions\n\n");
    text.push_str("| Area | Status | Detail |\n| --- | --- | --- |\n");
    for dimension in status.dimensions {
        text.push_str(&format!(
            "| {} | {} | {} |\n",
            dimension.area, dimension.status, dimension.detail
        ));
    }
    text.push_str("\n## Known blockers\n\n");
    for blocker in status.known_blockers {
        text.push_str(&format!("- {blocker}\n"));
    }
    text
}

fn backend_line(backend: &BackendStatus) -> String {
    if !backend.found {
        return "not found (checked OMOLA_OMC and PATH)".into();
    }
    match &backend.version {
        Some(version) => format!("found — {version}"),
        None => "found — version unavailable".into(),
    }
}

fn editor_line(status: &GuiStatus) -> String {
    let editor = &status.editor;
    if !editor.installed {
        return "not installed".into();
    }
    let path = editor
        .path
        .as_ref()
        .map(|path| normalize_display(path))
        .unwrap_or_else(|| "<unknown>".into());
    match editor.version_warning {
        Some(_) if editor.mismatch => format!("{path} — version mismatch"),
        Some(_) => format!("{path} — version unverified"),
        None => format!("{path} — {}", editor.version_display),
    }
}

/// Strip the Windows extended-length path prefix so the reported editor path
/// reads naturally in the status report.
fn normalize_display(path: &Path) -> String {
    path.display().to_string().replace(r"\\?\", "")
}

fn usage() -> BackendError {
    BackendError::InvalidInput("usage: omola gui status [--format json|markdown]".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_status() -> GuiStatus {
        GuiStatus {
            format_version: 1,
            label: LABEL,
            compatibility_claim: COMPATIBILITY_CLAIM,
            project_bridge: "bridge format version 1",
            backend: BackendStatus {
                found: true,
                version: Some("OpenModelica v1.27.1".into()),
            },
            editor: EditorStatus {
                installed: true,
                path: Some(PathBuf::from("OMEdit.exe")),
                version_display: "OMEdit.exe (unverified)".into(),
                version_warning: Some("confirm that it matches the OMC version".into()),
                mismatch: false,
            },
            dimensions: DIMENSIONS,
            known_blockers: BLOCKERS,
        }
    }

    #[test]
    fn json_reports_every_dimension_without_claiming_parity() {
        let report = render(&sample_status(), "json").unwrap();
        assert!(report.contains("does not claim Dymola parity"));
        assert!(report.contains("\"format_version\": 1"));
        assert!(report.contains("\"partial\""));
        assert!(report.contains("\"unsupported\""));
        // Every dimension carries an area, status, and detail field.
        let value: serde_json::Value = serde_json::from_str(&report).unwrap();
        let dimensions = value["dimensions"].as_array().unwrap();
        assert_eq!(dimensions.len(), DIMENSIONS.len());
        assert_eq!(dimensions.len(), 8);
        for dimension in dimensions {
            assert!(dimension["area"].is_string());
            assert!(dimension["status"].is_string());
            assert!(dimension["detail"].is_string());
        }
    }

    #[test]
    fn markdown_lists_dimensions_and_blockers() {
        let report = render(&sample_status(), "markdown").unwrap();
        assert!(report.contains("| Area | Status | Detail |"));
        assert!(report.contains("Simulation settings handoff"));
        assert!(report.contains("## Known blockers"));
        assert!(report.contains("source build (#25)"));
    }

    #[test]
    fn missing_backend_and_editor_degrade_gracefully() {
        let status = GuiStatus {
            format_version: 1,
            label: LABEL,
            compatibility_claim: COMPATIBILITY_CLAIM,
            project_bridge: "bridge format version 1",
            backend: BackendStatus {
                found: false,
                version: None,
            },
            editor: EditorStatus {
                installed: false,
                path: None,
                version_display: "OMEdit was not found on PATH or OMOLA_OMEDIT".into(),
                version_warning: Some("install the OMEdit build matching OpenModelica".into()),
                mismatch: false,
            },
            dimensions: DIMENSIONS,
            known_blockers: BLOCKERS,
        };
        let json = render(&status, "json").unwrap();
        assert!(json.contains("\"found\": false"));
        assert!(json.contains("\"installed\": false"));
    }

    #[test]
    fn unknown_format_is_rejected() {
        let error = render(&sample_status(), "yaml").unwrap_err();
        assert!(error.to_string().contains("json or markdown"));
    }

    #[test]
    fn collected_status_carries_the_static_matrix() {
        // Independent of whether OMC/OMEdit exist on the host, the static
        // matrix and blockers must always be present.
        let status = collect_status();
        assert_eq!(status.dimensions.len(), DIMENSIONS.len());
        assert_eq!(status.known_blockers.len(), BLOCKERS.len());
        assert_eq!(status.compatibility_claim, COMPATIBILITY_CLAIM);
    }
}
