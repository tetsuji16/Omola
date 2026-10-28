# Omola

**Omola** is an independent, Rust-first orchestration layer for Modelica workflows, with OpenModelica Compiler (OMC) as an external backend. “Open Dymola” is the project aspiration: compatible workflows are added and measured incrementally. Omola is not affiliated with or endorsed by Dassault Systèmes, and this repository does not claim Dymola compatibility today.

The first slice delegates Modelica parsing, checking, translation, and solving to an installed `omc`. Omola is intended to build on the upstream OMEdit C++/Qt application and add Omola project workflows to its existing modeling UI; a native OMEdit source build and in-application project integration are in progress. Until those changes are built and tested, the existing `omola project open` command remains only an external launcher. Omola's backend behavior remains limited by the selected OMC version, its library support, and external tools used by each model.

## Current skeleton

- `omola doctor` locates OMC through `OMOLA_OMC` or `PATH` and reports its version. Missing OMC yields a setup diagnostic.
- `omola check --model FILE.mo --class Qualified.Name` loads the file and calls OMC `checkModel`.
- `omola simulate --model FILE.mo --class Qualified.Name [--stop-time SECONDS] [--parameters name=value,name=value] [--output PREFIX]` calls OMC `simulate`; numeric parameter overrides are validated and recorded in the run sidecar.
- `omola project validate|simulate examples/project.toml` opens the versioned schema-1 manifest, validates relative package/source paths and exact dependency pins, then delegates checking/simulation to OMC. Modelica files remain user-owned and unchanged.
- `omola project inspect MANIFEST.toml --format json` emits validated, canonical project paths and simulation settings in bridge format version 1 for the native OMEdit integration. It does not invoke OMC, launch a GUI, or change project files.
- `omola results inspect FILE.csv|FILE.mat` inspects strict CSV or OpenModelica-generated MAT results; `omola results export-csv FILE.mat --variables x,y --output FILE.csv` uses OMC's scripting API to export selected scalar variables.
- `omola fmi capabilities`, `omola fmi export`, `omola fmi import`, and `omola fmi validate` report the documented OMC profile, delegate FMU operations to OMC, and check basic archive/XML metadata.
- `omola compat run --manifest tests/compat/manifest.toml --format json|markdown` runs the versioned sample corpus and reports measured pass/fail/skip counts.
- `omola release verify --scorecard target/compatibility.json --manifest tests/compat/manifest.toml --gates docs/release-gates.toml` rejects stale, incomplete, failing, or below-threshold scorecards.
- `omola bridge inventory --format markdown|json` reports the small documented Dymola scripting subset; `omola bridge call --function Dymola.simulateModel ...` runs only its declared partial mapping. Unsupported Dymola functions and unmapped simulation parameters return a capability-specific error.
- `omola licenses check [--output REPORT.md]` compares the locked Cargo dependency closure, CI action references, and development/OMEdit container images and apt packages against the provenance inventory; it exits unsuccessfully on unrecorded or changed dependencies.
- The core crate uses only the Rust standard library; the CLI uses Serde/TOML/JSON for versioned compatibility manifests and reports. OMC remains a separate process; no compiler code is embedded or copied.

## Install prerequisites and run the smoke model

The checked-in `examples/BouncingBall.mo` uses only built-in Modelica syntax and has no third-party library dependency. The expected simulation variables are `height` (m) and `velocity` (m/s); `gravity`, `restitution`, and `initialHeight` are parameters. Running it requires an installed OpenModelica Compiler (OMC). This is a backend smoke test, not a Dymola parity test.

Install Rust with the official [rustup instructions](https://www.rust-lang.org/tools/install): on Windows use the linked `rustup-init.exe`; on macOS/Linux the standard command is:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Install OpenModelica using the [official Windows download](https://openmodelica.org/download/download-windows/) and ensure `omc.exe` is on `PATH`. On Debian/Ubuntu, follow the repository setup for your release in the [official Linux instructions](https://openmodelica.org/download/download-linux/), then run `sudo apt update && sudo apt install openmodelica`. OpenModelica's [official macOS page](https://openmodelica.org/download/download-mac/) notes that native binaries were discontinued after 1.16; use its Linux Docker alternative. From the repository root, the included development image provides both OMC and Rust:

```sh
docker build -f .devcontainer/Dockerfile -t omola-dev .
docker run --rm -v "$PWD:/workspaces/Omola" -w /workspaces/Omola omola-dev cargo run -p omola -- doctor
docker run --rm -v "$PWD:/workspaces/Omola" -w /workspaces/Omola omola-dev sh -lc 'cargo run -p omola -- check --model examples/BouncingBall.mo --class BouncingBall && cargo run -p omola -- simulate --model examples/BouncingBall.mo --class BouncingBall --stop-time 3 --output "target/omola smoke spaces/BouncingBall"'
```

On Windows PowerShell with Rust and OMC installed, run:

```powershell
cargo run -p omola -- doctor
cargo run -p omola -- check --model examples\BouncingBall.mo --class BouncingBall
cargo run -p omola -- simulate --model examples\BouncingBall.mo --class BouncingBall --stop-time 3 --output "out\smoke results\BouncingBall"
cargo run -p omola -- project validate examples\project.toml
cargo run -p omola -- project simulate examples\project.toml --output "out\project results\BouncingBall"
```

On macOS/Linux with native OMC installed, the equivalent commands are:

```sh
cargo run -p omola -- doctor
cargo run -p omola -- check --model examples/BouncingBall.mo --class BouncingBall
cargo run -p omola -- simulate --model examples/BouncingBall.mo --class BouncingBall --stop-time 3 --output "out/smoke results/BouncingBall"
cargo run -p omola -- project validate examples/project.toml
cargo run -p omola -- project simulate examples/project.toml --output "out/project results/BouncingBall"
```

The project schema, path/dependency policy, migration boundary, and unsupported editor workflows are described in [the project workflow proposal](docs/project-workflow.md). The sample manifest points to the existing `BouncingBall.mo`; it does not copy or modify the Modelica source. OMC integration jobs run on pull requests and can also be requested manually; Windows runs install the pinned OpenModelica version through its setup action.

The graphical workflow reuses the official OpenModelica OMEdit application. `omola project open` validates a manifest and launches its entry model in OMEdit; the Docker desktop image includes the Rust CLI for this handoff. See the [project workflow](docs/project-workflow.md), [GUI roadmap](docs/gui-roadmap.md), and [OMEdit runtime record](docs/omedit-support.md) for setup, package/license findings, staged issues, and the measured Dymola compatibility boundary.

### OMEdit desktop preview in Docker (experimental)

On Windows, from PowerShell at the repository root, build and start the upstream OMEdit desktop in a Linux container:

```powershell
docker compose -f .devcontainer/docker-compose.omedit.yml up --build
```

Open [http://127.0.0.1:6080/vnc.html?autoconnect=true](http://127.0.0.1:6080/vnc.html?autoconnect=true). The container starts the official OMEdit 1.27.1 process with the checked-in `examples/BouncingBall.mo` path as its launch argument; this smoke test has not confirmed the model appears in the visible editor. Stop it with Ctrl-C or `docker compose -f .devcontainer/docker-compose.omedit.yml down`.

The repository is mounted read-only at `/workspace/Omola`; OMEdit cannot save changes to the checked-in examples. Writable work/results go to the ignored `.omedit-workspace/` folder mounted at `/workspace/data`. The container runs OMEdit as non-root UID/GID 1000 by default. On Linux, set `OMOLA_UID` and `OMOLA_GID` before building to match your account, so files in `.omedit-workspace/` remain editable by you. On Windows Docker Desktop, the bind mount is mediated by Docker Desktop. The Qt WebEngine sandbox stays enabled because the desktop process is non-root. The VNC server is reachable only through noVNC on host loopback port 6080; do not change the published address to a wildcard interface. This preview reuses upstream OMEdit and does not claim Dymola compatibility. Container/process and HTTP readiness passed; visual browser and model-open behavior are not yet verified.

The output prefix may contain spaces. To validate Rust without OMC, run `cargo check --workspace`; this is a build-only check and does not exercise the compiler backend. The OMC commands above are backend integration checks.

Each successful simulation writes a `<result>_res.json` run metadata sidecar containing model/backend versions, timestamps, numeric parameter overrides, output path, and the solver options reported by OMC. The sidecar records requested overrides, not a readback of effective values after model initialization. CSV inspection rejects missing/non-numeric cells, duplicate headers, and non-monotonic time. CSV export preserves selected variable names and writes round-trippable `f64` text for values it receives; the OMC 1.27.1 scripting formatter is limited to 13 significant digits in this path to avoid its fixed-buffer truncation on event values. Units are not available from this OMC export API and are not inferred.

These commands do **not** establish Dymola result compatibility. `.mat` inspection/export is delegated to the selected OpenModelica version, `.dsres` is explicitly unsupported, and no Dymola MAT/DSRES decoder or proprietary result-format guarantee is included. See [result artifact behavior](docs/result-artifacts.md).

FMI export and import are delegated to OMC. `omola fmi capabilities` reports the pinned OMC 1.27.1 documentation profile and states when a different installed version is being used because live capability probing is unavailable. The default artifact export is platform-specific. `fmi validate` checks the archive, model description, FMI version/name, interface declaration, and binary directory; it does not certify FMI conformance or interoperability with Dymola. FMI import is limited to the documented experimental FMI 2.0 Model Exchange wrapper path and records that limitation in a JSON sidecar; see [FMI workflow scope](docs/fmi-workflow.md).

The compiler is resolved in this order: a non-empty `OMOLA_OMC` executable path, then `omc.exe` from `PATH` on Windows (or `omc` on Unix). A configured path that does not exist is reported directly; it does not silently fall back to PATH. The adapter reports the OMC version with each successful command result, rejects invalid Modelica class names and stop times before creating a script, and removes its temporary `.mos` file and compiler workspace after success or failure. Simulation passes OMC a unique prefix in that isolated workspace, then copies the resulting MAT file to `<output-prefix>_res.mat`; existing result files are not overwritten. This allows output prefixes containing spaces despite OMC's generated makefile path handling.

## Linux development container

The development image starts from the official `openmodelica/openmodelica:v1.27.1-minimal` image and installs the stable Rust toolchain. It is a development environment, not a redistributable Omola runtime image. The OpenModelica minimal image does not include Modelica libraries; no MSL or Dymola files are copied into the repository or image. Install only libraries whose licenses and provenance are compatible with your use.

With Docker Desktop using Linux containers, build and run the checks from PowerShell:

```powershell
docker build -f .devcontainer/Dockerfile -t omola-dev .
docker run --rm -v "${PWD}:/workspaces/Omola" -w /workspaces/Omola omola-dev cargo fmt --all -- --check
docker run --rm -v "${PWD}:/workspaces/Omola" -w /workspaces/Omola omola-dev cargo test --workspace
docker run --rm -v "${PWD}:/workspaces/Omola" -w /workspaces/Omola omola-dev cargo clippy --workspace --all-targets -- -D warnings
docker run --rm -v "${PWD}:/workspaces/Omola" -w /workspaces/Omola omola-dev cargo run -p omola -- doctor
```

The same image can be opened as a development container in VS Code using `.devcontainer/devcontainer.json`. CI runs formatting, tests, and Clippy on both Linux and Windows without requiring OMC for unit tests. Pull requests also run the OMC backend integration: Linux builds this image; Windows installs the pinned OMC through the upstream setup action. Both check/simulate the sample and upload command logs. The Rust-only checks remain independent of OMC; merge CI and backend integration coverage are reported separately.

## Scope and truth in compatibility claims

The initial compatibility corpus is under `tests/compat`. Run it with OpenModelica using:

```sh
cargo run -p omola -- compat run --manifest tests/compat/manifest.toml --format markdown
cargo run -p omola -- compat run --manifest tests/compat/manifest.toml --format json
cargo run -p omola -- release verify --scorecard target/compatibility.json --manifest tests/compat/manifest.toml --gates docs/release-gates.toml
```

The pull-request OMC job installs the pinned Modelica Standard Library 4.1.0, emits JSON and Markdown compatibility scorecards, and runs the release gate against the exact current manifest. The scorecards are uploaded as CI artifacts. Omola currently has no declared minimum supported OMC version; the pinned release evidence is OpenModelica 1.27.1 and the fixture-level version prefixes in the manifest. Before 1.0, CLI and project-schema changes may break; every project format change must increment its schema version and document migration. After 1.0, CLI/project schemas follow SemVer: breaking changes require a major version, additive compatible changes a minor version, and fixes a patch version.

The bounded corpus pins Modelica Specification 3.6 (the baseline used by MSL 4.1.0) and includes original Omola examples, a Modelica Standard Library 4.1.0 Blocks composition, a first-order ODE with numerical CSV golden samples, and a negative syntax diagnostic. It maps the Modelica Association Modelica-Compliance `Connections.Declarations` category at commit `9734eb101c1adf3bc59ee2ec999e53fffbc13f4b`; that upstream suite entry is explicitly skipped because its package/test harness is not yet integrated. An excluded vendor-library case is also reported as skipped. The report labels execution as “Omola via OpenModelica” and includes source/license/version metadata and pass/fail/skip counts by feature and library. Golden values are sampled through OMC `val()` at declared times and compared using absolute/relative tolerances; the report records variable names, time interpolation, and units. These are measured results for the listed fixtures only and never imply Dymola compatibility.

The MSL model uses the official public library under its BSD 3-Clause license. The repository and development image do not redistribute MSL. With OMC installed and network access available, install the pinned release using OpenModelica's package manager, then run the corpus:

```powershell
omc tests/compat/install-msl.mos
cargo run -p omola -- compat run --manifest tests/compat/manifest.toml --format markdown
cargo run -p omola -- compat run --manifest tests/compat/manifest.toml --format json
```

The MSL fixture requires the exact 4.1.0 release and is not treated as passed if that library cannot be loaded. The negative syntax fixture demonstrates diagnostic behavior; it does not imply conformance coverage of the complete Modelica language or public conformance suite.

The scripting bridge inventory, official documentation sources, mapped parameters, and unsupported capability diagnostics are documented in [docs/dymola-scripting-bridge.md](docs/dymola-scripting-bridge.md). `simulateModel` is only partially mapped to Omola's file-backed simulation command; translation, interactive plotting, and Dymola's persistent last-error state remain unsupported.

The dependency provenance ledger and CI check are documented in [docs/provenance-licenses.md](docs/provenance-licenses.md) and [licenses/inventory.toml](licenses/inventory.toml). This repository does not bundle OMC, MSL, Dymola libraries/runtime, or an FMI runtime. The devcontainer is a development environment; review the exact upstream distribution and linked-component terms before redistribution. Generated FMUs need a separate package-content audit. Naming/trademark review remains a release gate.

“Dymola compatible” is not a single property. We track separately: (1) standard Modelica language semantics, (2) Modelica Standard Library and third-party library behavior, (3) Dymola-specific annotations and extensions, (4) Dymola scripting functions and automation, (5) result file import/export and numerical equivalence, (6) graphical authoring and project workflows, and (7) vendor libraries, integrations, licensing, and hosted product features. A green result in one row never implies compatibility in the others.

Dymola is a commercial product with a broad environment: editing, simulation, post-processing, FMI/Python/Simulink interoperability, symbolic methods, and domain libraries. A compiler wrapper cannot recreate these by itself. Dymola-only libraries and services need lawful, independently developed substitutes or licensed interoperation; source, tests, traces, and proprietary assets must not be copied. Dymola names and marks are used only to describe compatibility targets, not as a product affiliation claim.

## Architecture

```text
CLI / future desktop and editor
          |
     Rust workflow API
          |
 backend interface (versioned, capability-reporting)
          |
 OpenModelica adapter -- temporary .mos scripts/stdout/stderr --> external `omc`
          |
 normalized diagnostics, artifacts, run metadata, compatibility reports
```

Near-term architecture uses OMC's documented scripting interface through an external process, not a Rust binding to internal OMC implementation details. Each run records OMC version, command, model/library inputs, settings, outputs, and diagnostic stream. Later adapters can import/export FMI and interoperable result formats, compare runs, and offer a UI without turning OMC-specific behavior into the public API. Independent Rust implementations are appropriate for orchestration, project metadata, diagnostics, artifact indexing, compatibility tests, and UI. Parsing/flattening/equation transformation/DAE generation/solver should remain delegated unless measured gaps and a separately scoped compiler project justify replacement.

## Compatibility scorecard

Status is `Not measured` until a versioned test corpus and reports exist. Do not publish “full compatible” based on smoke tests.

| Area | Baseline and measurable gate | Current |
| --- | --- | --- |
| Standard language | Versioned Modelica Specification subset, positive and negative conformance cases; report pass count by feature and OMC version | Not measured; delegated to OMC |
| MSL and public libraries | Pin library versions; load/check/simulate named examples; publish pass/skip/fail counts and reasons | Not measured; depends on OMC |
| Dymola extensions | Inventory constructs/annotations from consented user examples; each implemented behavior has paired fixtures and documented fallback | No adapter |
| Automation API | Curated scripts with observable operation/result equivalence; percentage pass by named function, unsupported functions explicit | No compatibility API |
| Results | Import/export format, variable/time coverage, units/metadata preservation and numerical tolerance tested separately | No Omola result reader |
| Numerical behavior | Fixed benchmark set, tolerances and solver settings; report pass rate and error, never infer exact equivalence from format compatibility | No benchmark |
| GUI/editor | Workflow checklist for package browsing, text/diagram edits, parameter editing, plotting and project reopen | No GUI |
| Vendor ecosystem | Per-library license/access/redistribution review and lawful replacement status | Out of scope for initial backend |

Release gates progress from a reproducible OMC-backed CLI, through a published open-library corpus, through measured interoperability subsets, then GUI and expanded workflows. “Full compatibility” is a long-term aspiration and remains unclaimed until every declared target has a published, reproducible result and lawful delivery path.

## Plan and issue queue

Issues are ordered by dependency. Start with issue 01 before expanding command semantics; issue 02 supplies the sample used in README. Compatibility and feature issues require their corpus and version reporting first. The concrete implementation briefs are in [`docs/issues`](docs/issues/README.md).

## Licensing and sources

This project uses MIT for original Rust code. OpenModelica is a separate dependency; its distribution license and linked third-party components must be reviewed for the selected installation and redistribution model. Its current official license page describes OSMC Public License and AGPLv3 options. Modelica Standard Library is distributed under its own 3-Clause BSD terms. Dymola program terms are commercial and must be reviewed before any integration or distribution involving Dymola. This note is project planning, not legal advice.

- [OpenModelica Compiler guide](https://openmodelica.org/doc/OpenModelicaUsersGuide/latest/compiler.html)
- [OpenModelica scripting API documentation](https://openmodelica.org/doc/OpenModelicaUsersGuide/latest/scripting_api.html)
- [OpenModelica license](https://openmodelica.org/useresresources/license/)
- [Modelica Standard Library documentation and license](https://build.openmodelica.org/Documentation/Modelica)
- [Dassault Systèmes Dymola product description](https://www.3ds.com/products/catia/dymola)
- [Dymola latest release notes](https://www.3ds.com/products/catia/dymola/latest-release/)
- [Dymola program terms](https://www.3ds.com/terms/ost/licensed-programs/)
