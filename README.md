# Omola

**Omola** is an independent, Rust-first orchestration layer for Modelica workflows, with OpenModelica Compiler (OMC) as an external backend. “Open Dymola” is the project aspiration: compatible workflows are added and measured incrementally. Omola is not affiliated with or endorsed by Dassault Systèmes, and this repository does not claim Dymola compatibility today.

The first slice deliberately delegates Modelica parsing, checking, translation, and solving to an installed `omc`. Omola owns the process boundary, stable user-facing commands, diagnostics, later workflow adapters, and eventually its own UI. This avoids duplicating a mature compiler before there is a conformance corpus. It also means Omola's behavior is limited by the selected OMC version, its library support, and external tools used by the model.

## Current skeleton

- `omola doctor` locates OMC through `OMOLA_OMC` or `PATH` and reports its version. Missing OMC yields a setup diagnostic.
- `omola check --model FILE.mo --class Qualified.Name` loads the file and calls OMC `checkModel`.
- `omola simulate --model FILE.mo --class Qualified.Name [--stop-time SECONDS] [--output PREFIX]` calls OMC `simulate`.
- Rust crates contain only standard-library dependencies. The compiler remains a separate process; no OMC code is embedded or copied.

## Windows development

1. Install Rust using the official rustup installer and ensure `cargo` is on `PATH`.
2. Install OpenModelica and ensure `omc.exe` is on `PATH`; alternatively set `OMOLA_OMC` to the full `omc.exe` path.
3. From this repository run:

```powershell
cargo run -p omola -- doctor
cargo run -p omola -- check --model .\examples\BouncingBall.mo --class BouncingBall
cargo run -p omola -- simulate --model .\examples\BouncingBall.mo --class BouncingBall --stop-time 3 --output .\out\BouncingBall
```

The final command becomes runnable when the example model is added in issue 02. To check the current Rust workspace once Rust is installed, run `cargo check --workspace` and `cargo run -p omola -- doctor`.

## Scope and truth in compatibility claims

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
