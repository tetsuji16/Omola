# Omola

**Omola** is an independent, Rust-first orchestration layer for Modelica workflows, with OpenModelica Compiler (OMC) as an external backend. “Open Dymola” is the project aspiration: compatible workflows are added and measured incrementally. Omola is not affiliated with or endorsed by Dassault Systèmes, and this repository does not claim Dymola compatibility today.

The first slice deliberately delegates Modelica parsing, checking, translation, and solving to an installed `omc`. Omola owns the process boundary, stable user-facing commands, diagnostics, later workflow adapters, and eventually its own UI. This avoids duplicating a mature compiler before there is a conformance corpus. It also means Omola's behavior is limited by the selected OMC version, its library support, and external tools used by the model.

## Current skeleton

- `omola doctor` locates OMC through `OMOLA_OMC` or `PATH` and reports its version. Missing OMC yields a setup diagnostic.
- `omola check --model FILE.mo --class Qualified.Name` loads the file and calls OMC `checkModel`.
- `omola simulate --model FILE.mo --class Qualified.Name [--stop-time SECONDS] [--output PREFIX]` calls OMC `simulate`.
- `omola compat run --manifest tests/compat/manifest.toml --format json|markdown` runs the versioned sample corpus and reports measured pass/fail/skip counts.
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
```

On macOS/Linux with native OMC installed, the equivalent commands are:

```sh
cargo run -p omola -- doctor
cargo run -p omola -- check --model examples/BouncingBall.mo --class BouncingBall
cargo run -p omola -- simulate --model examples/BouncingBall.mo --class BouncingBall --stop-time 3 --output "out/smoke results/BouncingBall"
```

The output prefix may contain spaces. To validate Rust without OMC, run `cargo check --workspace`; this is a build-only check and does not exercise the compiler backend. The OMC commands above are backend integration checks.

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

The same image can be opened as a development container in VS Code using `.devcontainer/devcontainer.json`. CI runs formatting, tests, and Clippy on both Linux and Windows without requiring OMC for unit tests. To run the optional backend integration job, manually dispatch the Rust workflow with `run_omc` enabled; it builds this image, checks and simulates `BouncingBall`, and uploads a concise command log. Normal CI is build/unit validation only and does not claim backend integration coverage.

## Scope and truth in compatibility claims

The initial compatibility corpus is under `tests/compat`. Run it with OpenModelica using:

```sh
cargo run -p omola -- compat run --manifest tests/compat/manifest.toml --format markdown
cargo run -p omola -- compat run --manifest tests/compat/manifest.toml --format json
```

The bounded corpus pins Modelica Specification 3.6 (the baseline used by MSL 4.1.0) and includes original Omola examples, a Modelica Standard Library 4.1.0 Blocks composition, a first-order ODE with numerical CSV golden samples, and a negative syntax diagnostic. It maps the Modelica Association Modelica-Compliance `Connections.Declarations` category at commit `9734eb101c1adf3bc59ee2ec999e53fffbc13f4b`; that upstream suite entry is explicitly skipped because its package/test harness is not yet integrated. An excluded vendor-library case is also reported as skipped. The report labels execution as “Omola via OpenModelica” and includes source/license/version metadata and pass/fail/skip counts by feature and library. Golden values are sampled through OMC `val()` at declared times and compared using absolute/relative tolerances; the report records variable names, time interpolation, and units. These are measured results for the listed fixtures only and never imply Dymola compatibility.

The MSL model uses the official public library under its BSD 3-Clause license. The repository and development image do not redistribute MSL. With OMC installed and network access available, install the pinned release using OpenModelica's package manager, then run the corpus:

```powershell
omc tests/compat/install-msl.mos
cargo run -p omola -- compat run --manifest tests/compat/manifest.toml --format markdown
cargo run -p omola -- compat run --manifest tests/compat/manifest.toml --format json
```

The MSL fixture requires the exact 4.1.0 release and is not treated as passed if that library cannot be loaded. The negative syntax fixture demonstrates diagnostic behavior; it does not imply conformance coverage of the complete Modelica language or public conformance suite.

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
