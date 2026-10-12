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
