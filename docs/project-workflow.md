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
cargo run -p omola -- project simulate examples/project.toml
cargo run -p omola -- project simulate examples/project.toml --output "out/results with spaces/BouncingBall"
```

Simulation writes `<prefix>_res.mat` and an adjacent JSON run sidecar, and refuses to overwrite existing results. Numeric overrides are passed to OMC using its documented `simulate` runtime flags and recorded as requested values in that sidecar. The default is `examples/out/bouncing-ball_res.mat`; pass `--output` to choose another prefix. The manifest is currently a workflow input, not an editor database or complete package-manager lockfile. It does not rewrite, normalize, or copy Modelica source.

`tests/project-root/project.toml` is an integration fixture with a self-authored `FixtureLib 1.0` package. The optional OMC workflow validates and simulates it to check package-root precedence, exact pin loading, and paths containing spaces. A deliberately mismatched pin must fail validation even though OMC may still emit a later automatic-load diagnostic; Omola treats the OMC error/false response as failure. These commands use OpenModelica's documented [`getModelicaPath`, `setModelicaPath`, and `loadModel`](https://openmodelica.org/doc/OpenModelicaUsersGuide/1.27/scripting_api.html) API.

## Editor and GUI roadmap

The first editing workflow is text-first: preserve user-owned `.mo` files, use a project manifest for navigation/configuration, validate with OMC, and keep result artifacts separate. Diagram editing, graphical annotations editing, parameter dialogs, plotting/interactive post-processing, model search, project dependency download/update, and Dymola `.dymx`/project import are unsupported.

A future GUI should be accepted only when it can (1) reopen a project without changing source bytes, (2) browse package roots and identify missing pinned dependencies, (3) edit and save a Modelica file with an explicit diff, (4) run OMC validation/simulation and display diagnostics, and (5) open a result and plot named variables. Diagram authoring and Dymola project import require separate fixture-backed issues and may not be inferred from text editing or OMC support.
