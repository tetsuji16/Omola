# 04 — Measure standard language and public-library coverage

**Goal:** Publish coverage evidence for named Modelica Specification features and public libraries through the selected OMC backend.

**Depends on:** 03. **Suggested owner:** one Luna implementation agent.

**Work:** Select a pinned release of the Modelica Specification and MSL; map the public conformance suite and a bounded set of library examples to feature tags. Record expected limitations and OMC version. Avoid asserting Dymola-specific semantics from standard-only fixtures.

**Done when:**
- Every included fixture has source/provenance, license, spec/library version, feature tag, expected outcome, and reproducible command.
- The report shows pass/fail/skip counts by feature and library, including the exact OMC build.
- At least one negative fixture proves an unsupported or invalid construct is reported rather than silently passing.
- Results are explicitly labelled “Omola via OpenModelica” and do not claim parity with Dymola.

**Validation commands:** `cargo run -p omola -- compat run --manifest tests/compat/manifest.toml --format markdown`; `cargo run -p omola -- compat run --manifest tests/compat/manifest.toml --format json`.
