# Omola GUI roadmap: reuse OMEdit

**Research checked:** 2026-10-22. Omola's GUI direction is to reuse the official OpenModelica GUI, OMEdit, wherever it already provides the workflow. Omola should add a small Rust companion/compatibility bridge around the existing CLI and OMC process boundary, not recreate an editor, annotation renderer, library browser, message pane, or plotting UI. No implementation of the previously prototyped browser GUI is adopted by this plan.

## Product boundary

OMEdit already provides a graphical Modelica editor, library and model browsing, text/diagram views, check/simulation controls, messages, and result plotting. Its documented architecture is a GUI frontend communicating with OMC. The initial Omola experience should therefore launch the distribution-matched OMEdit executable against the same OMC installation and project files. The Rust side remains responsible for versioned Omola project manifests, Dymola-compatibility inventory and explicitly mapped operations, safe process invocation, result metadata/compatibility scorecards, and actionable diagnostics where OMEdit or OMC does not expose the required behavior.

The project has **not** established an OMEdit plugin or stable embedding API. Issues must not promise that the Qt application can be embedded or extended in-process. Prefer documented command-line/session interfaces and normal Modelica files/annotations; where those do not meet an Omola workflow, first prove a small out-of-process bridge. A fork or patch to upstream OMEdit requires a separate scope, upstream/license review, and a concrete gap that cannot be addressed by configuration or the Rust companion.

## License and distribution findings

The official OpenModelica source checkout (`OpenModelica/OpenModelica`, commit `d446672047a2a1cfcdfe1e07081c696ba9d37da4`, inspected 2026-10-13) contains OMEdit source headers stating that the program is provided under **AGPL version 3 or OSMC Public License (OSMC-PL) version 1.8, at the recipient's choice**. The repository includes `OMCompiler/COPYING` and OMEdit source headers; the `common/LICENSE` file applies to that common component and is not OMEdit's license. OMEdit also links Qt and OpenModelica components with their own terms. Any redistribution or modified build must inventory the actual packaged components and comply with the chosen applicable terms; this plan does not redistribute or copy OMEdit source or assets.

The Linux GUI development image uses the digest-pinned `openmodelica/openmodelica:v1.27.1-minimal` Ubuntu 22.04 base and installs the official `omedit 1.27.1-1` package. A multi-stage builder compiles the Rust CLI with Rust 1.88.0 on the same base and copies only the executable into the GUI image. OMEdit and `omola project open` were launched in the container against a copied project whose path contains spaces; its process argument was observed. noVNC HTTP readiness and loopback binding were checked. **Visible editor contents and visual rendering have not been reviewed in a browser.**

## Staged issues

| Issue | Stage | Observable completion |
|---|---|---|
| [#13](https://github.com/tetsuji16/Omola/issues/13) | OMEdit reuse decision and license/runtime inventory | Versioned support matrix, pinned official package/source and license inventory, no copied assets, documented no-plugin boundary |
| [#14](https://github.com/tetsuji16/Omola/issues/14) | Reproducible OMEdit development container and launch | Matching OMC/OMEdit installation, reproducible build, executable launch under X/Wayland or documented host display, smoke evidence |
| [#15](https://github.com/tetsuji16/Omola/issues/15) | Open Omola projects in OMEdit | Resolve manifest paths and preflight pins, launch the entry model through OMEdit, preserve project files; GUI-visible model and selected library versions still require verification |
| [#16](https://github.com/tetsuji16/Omola/issues/16) | OMEdit/OMC workflow and Rust companion | Verified check/simulate and diagnostics through OMEdit or supported OMC CLI; Rust adds only safe, tested bridge behaviors |
| [#17](https://github.com/tetsuji16/Omola/issues/17) | Reuse OMEdit result browser and plots | Run a real fixture and verify variable browsing/plots/result paths; document unsupported formats and units |
| [#18](https://github.com/tetsuji16/Omola/issues/18) | Dymola workflow gaps and diagram/annotation compatibility | Build a gap matrix, preserve Modelica annotations, add only evidenced bridges; graphical editing parity is separately measured |
| [#19](https://github.com/tetsuji16/Omola/issues/19) | GUI compatibility and release regression gates | Versioned test report for launched OMEdit, project-open/check/simulate/plot and explicit unsupported Dymola behaviors |
| [#22](https://github.com/tetsuji16/Omola/issues/22) | Verify library versions selected by OMEdit against project pins | Reconcile manifest pins with `uses(...)` annotations and observe the version selected by OMEdit itself; a separate OMC preflight is insufficient |

Dependency order: #13 → #14 → #15 → #16 → #17 → #19. Issue #18 depends on #13, #15, and #16 and may proceed independently of result plotting. Issue #22 depends on #15 and #16 and tracks verification of library versions actually selected in OMEdit. An issue stays open until its own acceptance criteria are demonstrated; planning or package installation alone does not count as a functional GUI. No stage asserts complete Dymola compatibility.

## Compatibility target matrix

| Area | Reuse/plan | Current boundary |
|---|---|---|
| Modelica editor and diagram annotations | OMEdit text and diagram editors | OMEdit capabilities are OpenModelica behavior; no Omola UI or Dymola-equivalence claim |
| Library/model browsing | OMEdit library browser plus manifest-to-model launch | `project open` hands off canonical model paths and package roots; OMEdit-selected versions and visual browsing are unverified |
| Check, simulation, messages | OMEdit with distribution-matched OMC; Rust adapter only for measured gaps | CLI workflows and OMEdit launch exist; GUI diagnostics and operation results need verification |
| Results and plotting | Reuse OMEdit variable browser/plotting | Existing CLI result support is limited; Dymola `.mat`/`.dsres` compatibility is not established |
| Dymola scripting/API | Rust bridge with explicit function-by-function mapping | Small documented subset only; no arbitrary script execution |
| FMI, optimization, calibration, Monte Carlo and vendor services | Separate requirement and lawful test evidence per capability | Dymola-specific or commercial functions are unsupported unless measured individually |
| Packaging, licensing and display | Use official matched distribution; document local desktop/container display modes | OMEdit runs in a non-root Linux container; noVNC readiness is checked, but browser-rendered quality remains unverified |

Keep Modelica language/library support, OMC semantics, OMEdit behavior, Omola bridge behavior, Dymola-specific behavior, and commercial/vendor services as distinct scorecard dimensions. Passing OMC/OMEdit tests never implies Dymola parity.

## Primary sources and local evidence

- OpenModelica's [OMEdit overview](https://openmodelica.org/free-and-open-source-software/omconnectioneditoromedit/) describes it as a GUI frontend communicating with OMC and lists graphical modeling, simulation, and plotting. The official [OMEdit User's Guide](https://openmodelica.org/doc/OpenModelicaUsersGuide/latest/omedit.html) documents library, messages and variables browsers, text/diagram perspectives, simulation, and plots.
- The official [OpenModelica source repository](https://github.com/OpenModelica/OpenModelica/tree/master/OMEdit) was inspected locally. [`OMEdit/README.md`](https://github.com/OpenModelica/OpenModelica/blob/master/OMEdit/README.md) lists OpenModelica, OMPlot, and OMSimulator dependencies and directs builds through OpenModelica's OS-specific build instructions. [`OMEdit/OMEditLIB/OMEditApplication.cpp`](https://github.com/OpenModelica/OpenModelica/blob/master/OMEdit/OMEditLIB/OMEditApplication.cpp) contains the dual-license header; [`OMCompiler/COPYING`](https://github.com/OpenModelica/OpenModelica/blob/master/OMCompiler/COPYING) is present in the source tree. The full component/license inventory remains required before redistributing an image.
- Dassault Systèmes' official [Dymola latest release page](https://www.3ds.com/products/catia/dymola/latest-release) is a changing public feature summary. It signals areas such as calibration UI, parameter sweeps, FMU optimization, and 3DEXPERIENCE services but is not a full compatibility specification. Licensed Dymola manuals, software, models, and UI assets are not copied.
- Local package probe: the `omola-dev` Docker image had no `OMEdit` binary. The official apt repository supplied `omedit 1.27.1-1`; an ephemeral container installed that package and Xvfb/xauth/x11vnc successfully. No GUI launch or remote desktop access was verified.

## Development and verification rules

Keep the existing Rust core/CLI as the companion and do not add a parallel GUI unless a measured OMEdit gap requires it. Use the official matching OpenModelica package before considering a source fork. Pin distribution versions, record package origins/licenses, keep OMEdit and OMC versions aligned, and avoid vendoring upstream UI assets. For Linux-container GUI access, first test local X forwarding/headless Xvfb; only then decide whether a loopback-only VNC/noVNC development profile is warranted. Never publish a remote desktop port beyond host loopback by default.

Each completed stage needs a reproducible command and evidence from a real OMEdit/OMC model workflow. Tests must state OS and versions, keep generated output in a temporary copy or `target/`, and avoid modifying example source files. Record unsupported Dymola semantics and differences explicitly. The earlier uncommitted browser-UI prototype is excluded from this plan and must not be shipped as part of OMEdit reuse work.
