# Omola project workflow, schema 1

`examples/project.toml` is a portable project manifest. It points to the unchanged `BouncingBall.mo` source, declares package roots relative to the manifest, identifies the entry class, optionally pins dependencies by exact library version, and records the default stop time and numeric scalar parameter overrides. Paths are relative, cannot traverse above the project root, and must resolve to existing files/directories. The CLI canonicalizes paths before handing them to OMC. Manifest versions other than 1 and unknown fields fail explicitly; migration to a future schema must be an explicit versioned conversion that preserves source files.

```toml
schema_version = 1
name = "bouncing-ball"
model_file = "BouncingBall.mo"
entry_model = "BouncingBall"
package_roots = ["."]
dependencies = []

[simulation]
stop_time = 3.0
parameters = { restitution = 0.65, initialHeight = 2.0 }
```

`[[dependencies]]` entries use a Modelica package name and an exact version, for example `{ name = "Modelica", version = "4.1.0" }`. OMC resolves versioned package directories/files such as `Modelica 4.1.0/package.mo` under each package root. Omola does not download or redistribute dependencies. It prepends manifest package roots to the OMC Modelica path, keeps the installation/user paths, and asks OMC to load each pinned version before the entry model. A missing pin makes validation fail even if the model could otherwise auto-load a different version. Vendor packages retain their own access and licensing requirements.

From the repository root, with Rust and OpenModelica installed:

```sh
cargo run -p omola -- project validate examples/project.toml
cargo run -p omola -- project inspect examples/project.toml --format json
cargo run -p omola -- project simulate examples/project.toml
cargo run -p omola -- project simulate examples/project.toml --output "out/results with spaces/BouncingBall"
cargo run -p omola -- project open examples/project.toml --omedit "/path/to/OMEdit"
```

### Windows native launch

In PowerShell from the repository root, set the compiler and GUI paths to the matching `bin` directory of one OpenModelica installation, then build the CLI and open a writable copy of the sample:

```powershell
$omBin = Join-Path $env:ProgramFiles 'OpenModelica1.27.1-64bit\bin' # adjust to the installed folder
$env:OMOLA_OMC = Join-Path $omBin 'omc.exe'
$env:OMOLA_OMEDIT = Join-Path $omBin 'OMEdit.exe'
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH" # rustup's default Windows cargo location
# Run from the Omola repository root.
cargo build --locked --release -p omola
& .\target\release\omola.exe doctor

$projectDir = Join-Path (Get-Location) ('.omedit-workspace\bouncing-ball-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
if (Test-Path $projectDir) { throw "Project copy already exists: $projectDir. Choose another directory to preserve its contents." }
New-Item -ItemType Directory -Force $projectDir | Out-Null
Copy-Item -LiteralPath @('examples\BouncingBall.mo', 'examples\project.toml') -Destination $projectDir
& .\target\release\omola.exe project validate (Join-Path $projectDir 'project.toml')
& .\target\release\omola.exe project open (Join-Path $projectDir 'project.toml') --omedit "$env:OMOLA_OMEDIT"
```

Change `$omBin` to the actual versioned OpenModelica installation folder. `OMOLA_OMC` and `OMOLA_OMEDIT` are read from the current process environment; explicit `--omedit PATH` overrides the latter, and each falls back to `PATH` when unset. Keep OMC and OMEdit from the same installation. `project open` validates the manifest with OMC and launches the model in OMEdit, passing project package roots to that child process. It waits until the GUI exits. On Windows, the launcher reports the OMEdit version as unverified; check that the selected GUI matches the compiler.

This is a file-open and process-launch handoff only. Manifest stop-time and parameter defaults are used by `project simulate`; `project open` does not populate OMEdit's simulation controls. Use the GUI to set its simulation options, or run `project simulate` to apply the manifest settings through the CLI. The source-built Windows OMEdit smoke check passed model checking but failed to run a generated simulator; a complete source install is not yet verified. The official packaged OpenModelica installation is the supported starting point for this workflow.

`project open` validates the manifest and exact library pins in a separate OMC preflight before launching the entry `.mo` file as one argument to OMEdit. The child process receives project package roots prepended to the current OMC Modelica path through `OPENMODELICALIBRARY`; the CLI process environment is unchanged. This path handoff does not prove which library versions OMEdit's own OMC session actually selected; when pins are declared, the CLI says so and asks the operator to check OMEdit's messages and library browser. On Debian, a known OMEdit package version mismatch is an error. On other hosts or non-package installs, the version is reported as unverified and must be checked by the operator. The command waits for OMEdit and reports a non-zero exit as failure. OMEdit remains the editor; this command does not expose or emulate a plugin API.

`project inspect --format json` is the machine-readable boundary planned for the native OMEdit extension. It runs the same strict manifest and path validation as other project commands, then writes JSON format version 1 with absolute canonical manifest/model/package-root paths, the entry class, exact dependency pins, stop time, and numeric parameter overrides. The contract is defined in [`omedit-project-bridge.schema.json`](omedit-project-bridge.schema.json). It does not contact OMC or start a process. The modified OMEdit must launch this command with a process API and argument list (never shell text), reject unsupported bridge versions, and show errors in its own message pane. The bridge does not prove which library versions OMEdit's OMC session actually loads; that remains tracked by issue #22.

The Docker desktop image builds and includes the Rust CLI. Copy a project into the writable workspace before editing so the checked-in sample stays unchanged:

```powershell
New-Item -ItemType Directory -Force .omedit-workspace/bouncing-ball | Out-Null
Copy-Item examples/BouncingBall.mo,examples/project.toml .omedit-workspace/bouncing-ball/
docker compose -f .devcontainer/docker-compose.omedit.yml exec omedit omola project open /workspace/data/bouncing-ball/project.toml --omedit /usr/bin/OMEdit
```

The Linux container's source mount remains read-only and `/workspace/data` maps to ignored `.omedit-workspace/`. This path exercises the actual OMC preflight and OMEdit executable; it does not establish visual GUI quality or Dymola parity.

Simulation writes `<prefix>_res.mat` and an adjacent JSON run sidecar, and refuses to overwrite existing results. Numeric overrides are passed to OMC using its documented `simulate` runtime flags and recorded as requested values in that sidecar. The default is `examples/out/bouncing-ball_res.mat`; pass `--output` to choose another prefix. The manifest is currently a workflow input, not an editor database or complete package-manager lockfile. It does not rewrite, normalize, or copy Modelica source.

`tests/project-root/project.toml` is an integration fixture with a self-authored `FixtureLib 1.0` package. The optional OMC workflow validates and simulates it to check package-root precedence, exact pin loading, and paths containing spaces. A deliberately mismatched pin must fail validation even though OMC may still emit a later automatic-load diagnostic; Omola treats the OMC error/false response as failure. These commands use OpenModelica's documented [`getModelicaPath`, `setModelicaPath`, and `loadModel`](https://openmodelica.org/doc/OpenModelicaUsersGuide/1.27/scripting_api.html) API.

## Editor and GUI roadmap

The current launcher has no manifest-aware GUI. The implementation target is a pinned upstream OMEdit source build with a native “Open Omola Project” action, manifest-derived simulation settings, and results shown in OMEdit's existing editors and plots. Until this patched build passes the GUI workflow checks, model views, simulation defaults, and results are capabilities of standalone upstream OMEdit only; no Omola GUI completion or Dymola parity is claimed. Diagram/annotation gaps, project dependency download/update, and Dymola `.dymx`/project import remain separately scoped.

A future GUI should be accepted only when it can (1) reopen a project without changing source bytes, (2) browse package roots and identify missing pinned dependencies, (3) edit and save a Modelica file with an explicit diff, (4) run OMC validation/simulation and display diagnostics, and (5) open a result and plot named variables. Diagram authoring and Dymola project import require separate fixture-backed issues and may not be inferred from text editing or OMC support.
