# 07 — Add an FMI workflow through OpenModelica

**Goal:** Support a measured FMI export/import workflow through installed OpenModelica capabilities, with Omola-owned artifact tracking.

**Depends on:** 03. **Suggested owner:** one Luna implementation agent.

**Work:** Inventory OMC's documented FMU export/import commands and supported FMI versions/types for the tested OMC build. Add explicit `fmi export` and validation commands that delegate to OMC. Record FMI standard version, platform, model, tool version, and build options. Do not implement an FMI runtime until a separately scoped requirement exists.

**Done when:**
- CLI reports supported FMI types/versions for the installed OMC or explains that FMI capability probing is unavailable.
- A minimal original model exports an FMU for the selected version/type, and the result is checked for required archive structure and `modelDescription.xml` metadata.
- Where OMC supports import, a fixture imports the FMU and records limitations; export and import claims are reported separately.
- Tests without OpenModelica validate command argument handling and capability diagnostics; integration tests run on a pinned OMC build.
- No claim of Dymola parity follows from FMI conformance alone.

**Validation commands:** `cargo test --workspace`; `cargo run -p omola -- fmi capabilities`; `cargo run -p omola -- fmi export --model examples/BouncingBall.mo --class BouncingBall --version 2.0 --type cs --output out/BouncingBall.fmu`; `cargo run -p omola -- fmi validate out/BouncingBall.fmu`.

## Implementation status (2026-10-12)

Implemented in the follow-up PR commit for issue #7: `fmi capabilities`, `fmi export`, `fmi validate`, and the documented OMC-supported experimental FMI 2.0 Model Exchange `fmi import` path. Export delegates to OMC, requires a real generated file, copies without overwriting, and validates ZIP structure, modelDescription.xml, model name/version/interface, and a binaries directory. Export/import record JSON metadata and import limitations. FMI import is not an Omola runtime and does not assert numerical equivalence.

Verified in the pinned OpenModelica 1.27.1 Docker image: BouncingBall FMI 2.0 Co-Simulation and Model Exchange exports, validation of their metadata, and Model Exchange import into a new output directory. The command output directory included spaces. A hyphenated `fileNamePrefix` triggered an OMC generated C/header mismatch and no FMU; Omola now uses identifier-safe prefixes and rejects missing artifacts instead of treating an exit-0/empty result as success.

Capability claims use the official [OpenModelica 1.27.1 FMI guide](https://openmodelica.org/doc/OpenModelicaUsersGuide/1.27/fmitlm.html) and [scripting API](https://openmodelica.org/doc/OpenModelicaUsersGuide/1.27/scripting_api.html), not a live feature probe. Other OMC versions are explicitly labeled unprobed. OMC describes FMU import as experimental and its Modelica wrapper uses a non-standard flag. FMI alone establishes no Dymola parity.
