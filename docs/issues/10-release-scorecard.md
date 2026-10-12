# 09 — Publish compatibility scorecard and release gates

**Goal:** Make each release's supported behavior independently reviewable and prevent unmeasured “full compatible” claims.

**Depends on:** 04–08. **Suggested owner:** one Luna implementation agent.

**Work:** Generate the README/release scorecard from compatibility reports, document minimum and tested OMC versions, and define semantic versioning rules for the Omola API/project format. Publish failures and exclusions alongside passes.

**Done when:**
- Release artifacts include machine-readable and human-readable scorecards with versions, pass/fail/skip by scope, and known limits.
- CI prevents release when required corpus results are missing, stale, or below declared gates.
- “Full Dymola compatibility” remains absent unless every declared target area has reproducible evidence and a lawful delivery path.
- Reproducing a release report requires only documented commands and pinned inputs.

**Validation commands:** `cargo test --workspace`; `cargo run -p omola -- compat run --manifest tests/compat/manifest.toml --format json`; `cargo run -p omola -- release verify --scorecard target/compatibility.json`.
