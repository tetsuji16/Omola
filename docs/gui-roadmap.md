# Omola GUI roadmap: extend upstream OMEdit

**Direction changed:** 2026-10-28. Omola's GUI product should build on and modify OpenModelica's OMEdit application. Launching stock OMEdit from the Rust CLI is a useful existing handoff, but it is not the intended Omola desktop product. No GUI parity or Dymola compatibility is claimed until the modified application runs measurable workflows.

## Product and implementation boundary

OMEdit already provides a C++/Qt Modelica editor, diagram canvas, library/model browser, simulation setup, diagnostics, and plots. Omola should add project-aware actions and settings to those existing views instead of implementing a parallel editor or graph canvas. The initial native slice is:

1. OMEdit provides **Open Omola Project…** and launches the installed Omola CLI with `QProcess` and an argument list.
2. The Rust `project inspect --format json` command validates schema, path confinement, dependency syntax, and numeric simulation settings, then returns canonical paths and project data in JSON bridge format v1.
3. OMEdit loads the model into its existing Library/Model widgets and initializes simulation controls from the project. Project parameter overrides must appear in the GUI and be applied to the actual OMC run.
4. The run finishes through OMEdit's normal diagnostics/result widgets and plot browser. BouncingBall results must distinguish the manifest overrides from model defaults.

The upstream source does not establish a stable in-process plugin/embedding API. The product path is therefore a small reviewed patch series against a pinned upstream source tag, plus a versioned Rust process bridge. Do not call the patch a plugin. Do not execute arbitrary Modelica scripts or interpolate shell text. Keep the modified OMEdit/OMC installation isolated from the user's existing installation.

The schema-1 project manifest remains the owner of model/source paths, dependencies, and numeric simulation defaults. `omola project inspect` is a read-only command; its v1 contract is [`omedit-project-bridge.schema.json`](omedit-project-bridge.schema.json). OMEdit must reject unknown bridge versions and show missing CLI, invalid project, and backend errors in its message pane. Preflight in a separate process does not prove which library versions OMEdit's session selected; issue #22 covers that gap.

## Upstream source and licensing

The starting point is official [`OpenModelica/OpenModelica` v1.27.1](https://github.com/OpenModelica/OpenModelica/tree/v1.27.1), commit `4a7248c1ab64f4205eb4222b6b009bbf2b0df383`, with recursive submodules pinned by commit. The OMEdit source header offers GNU AGPL-3.0 or OSMC-PL 1.8 at the recipient's choice. The tagged source contains `OSMC-License.txt` and `OMCompiler/COPYING`; Qt, OMCompiler, OMPlot, OMSimulator, and every runtime dependency retain their own license/notice obligations. Before redistributing a modified build, record the selected license path, complete package inventory, corresponding source/notices, and build provenance. See [`omedit-upstream.md`](omedit-upstream.md) for the measured source and host record.

No Dymola code, manuals, models, icons, or proprietary UI assets may be copied. Omola remains a separate product and makes no Dassault Systèmes affiliation claim. A current public Dymola feature summary can guide a gap matrix but is not a full product specification.

## Dymola compatibility dimensions

| Area | OMEdit/OMC starting point | Omola and Dymola status |
|---|---|---|
| Modelica text/diagram editing | Existing OMEdit text and diagram editors | Reuse and test; proprietary Dymola-specific extensions remain unknown |
| Libraries and project files | OMEdit browser plus Omola schema-1 project manifest | Native manifest integration is in progress; exact library version selected by GUI is unverified until #22 passes |
| Check and simulation | OMEdit actions and OMC backend | Project stop time and numeric overrides must be visibly applied and verified through the GUI |
| Results and plotting | OMEdit variable browser and plot windows | Reuse; Dymola MAT/DSRES formats are unsupported unless directly measured |
| FMI and cross-tool integration | Existing OMC workflows | Version/type/platform-specific coverage only; no inference of Dymola equivalence |
| Dymola scripting and project semantics | No general compatibility layer | Explicit function/file mapping only; unknown behavior stays unsupported |
| Parameter sweep, calibration, optimization, vendor services | Dymola public pages describe these as product features | Omola has no claim to them; define lawful fixtures and separate completion criteria before implementation |

Keep Modelica language and library coverage, OMC semantics, OMEdit behavior, Omola project features, Dymola-specific APIs/formats, and vendor/commercial services as separate scorecard dimensions. Passing an OpenModelica test is not evidence of Dymola parity.

## Measured GUI status command

`omola gui status [--format json|markdown]` reports the current measured state of the GUI/editor dimension against the Dymola compatibility matrix. It probes the installed OMC (presence and version) and OMEdit (presence, resolved path, and version-match status where the host supports it), then prints the eight GUI/editor dimensions with per-row status (`reused`, `partial`, `not-measured`, `unsupported`) and the known blockers. The command degrades gracefully when OMC or OMEdit is absent instead of failing, and it never claims Dymola parity. This advances the scorecard's "GUI/editor" row from "No GUI" toward a reproducible, runnable status check; the underlying integration gaps (#15/#16/#18/#19/#22/#25) remain open until each has measured evidence.

## Staged GitHub work

| Issue | Scope | Completion evidence |
|---|---|---|
| [#25](https://github.com/tetsuji16/Omola/issues/25) | Pinned native Windows OMEdit source build and license/dependency record | Fresh source/toolchain setup; isolated OMEdit+OMC build and visible Windows launch; fixture check/simulate |
| [#15](https://github.com/tetsuji16/Omola/issues/15) | Native OMEdit **Open Omola Project…** and model/library loading | GUI action consumes validated JSON, opens the canonical model, preserves source, reports missing inputs |
| [#16](https://github.com/tetsuji16/Omola/issues/16) | Manifest simulation settings inside OMEdit | Stop time and parameter values visible and applied to real OMC simulation; GUI diagnostics and results agree |
| [#17](https://github.com/tetsuji16/Omola/issues/17) | Result browsing and plotting | OMEdit renders fixture variables with the reported result path/backend; no synthetic plot evidence |
| [#18](https://github.com/tetsuji16/Omola/issues/18) | Diagram annotations and measured Dymola gaps | Lawful fixtures, annotation-preserving saves, explicit supported/unsupported matrix |
| [#22](https://github.com/tetsuji16/Omola/issues/22) | Verify versions actually selected in OMEdit's own OMC session | Conflicting pins/annotations fail visibly; selected version evidence comes from the GUI session |
| [#19](https://github.com/tetsuji16/Omola/issues/19) | Native Windows GUI regression gate | Versioned report for source/toolchain, model/manifest, open/check/simulate/plot and explicit unsupported cases |

Dependencies: #25 → #15 → #16 → #17 → #19. #18 depends on #25/#15/#16 and can proceed independently of plots. #22 depends on #15/#16. Existing closed #13/#14 record the earlier OMEdit reuse/license inventory and Docker launch experiment; they do not satisfy source-patch/build acceptance in #25. An issue remains open until its own acceptance criteria have measured evidence.

## Current Windows evidence and blockers

- Stock OpenModelica 1.27.1 is installed at `C:\Program Files\OpenModelica1.27.1-64bit`. A previous native session launched stock OMEdit, opened a temporary BouncingBall project, and completed a GUI check/simulation/plot. Its manifest overrides did **not** propagate into that stock OMEdit simulation; this is the concrete gap #16 is intended to close.
- The pinned upstream source and submodules have been fetched outside this repository; checkout size is about 1.06 GB. The unmodified installation's bundled MSYS2/UCRT64 is a runtime image with Qt development files stripped. CMake source configuration has not completed and no modified C++ has compiled.
- The official Windows instructions recommend a fixed-package OMDev/UCRT64 toolchain, but fetching the OMDevUCRT GitLab pack stalled at zero bytes in this session. A clean user-owned MSYS2 development setup remains the next environment attempt. Do not update the existing OpenModelica installation in place.
- The exact configure failures, local unshipped SUNDIALS guard experiment, package missing-file evidence, versioned source/license findings, and unmeasured build/download footprint are recorded in [`omedit-upstream.md`](omedit-upstream.md). A configure pass is not a build or GUI verification.

## Primary sources

- OpenModelica [OMEdit overview](https://openmodelica.org/free-and-open-source-software/omconnectioneditoromedit/) and [v1.27.1 OMEdit README](https://github.com/OpenModelica/OpenModelica/blob/v1.27.1/OMEdit/README.md).
- OpenModelica [v1.27.1 Windows build instructions](https://github.com/OpenModelica/OpenModelica/blob/v1.27.1/OMCompiler/README.Windows.md) and [CMake build instructions](https://github.com/OpenModelica/OpenModelica/blob/v1.27.1/README.cmake.md).
- OpenModelica [v1.27.1 OMEdit User's Guide](https://openmodelica.org/doc/OpenModelicaUsersGuide/v1.27.1/omedit.html).
- Dassault Systèmes [Dymola design tools](https://www.3ds.com/products/catia/dymola/model-design-tools), [export/interfacing](https://www.3ds.com/products/catia/dymola/export-capabilities-interfacing-other-software), and [latest release](https://www.3ds.com/products/catia/dymola/latest-release). These are changing product summaries, not a full compatibility specification.
