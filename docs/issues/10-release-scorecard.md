# 10 — Publish compatibility scorecard and release gates

**Goal:** Make each release's supported behavior independently reviewable and prevent unmeasured “full compatible” claims.

**Depends on:** [#4](https://github.com/tetsuji16/Omola/issues/4), [#5](https://github.com/tetsuji16/Omola/issues/5), [#6](https://github.com/tetsuji16/Omola/issues/6), [#7](https://github.com/tetsuji16/Omola/issues/7), [#8](https://github.com/tetsuji16/Omola/issues/8), [#9](https://github.com/tetsuji16/Omola/issues/9). **Suggested owner:** one Luna implementation agent.

**Work:** Generate the README/release scorecard from compatibility reports, document minimum and tested OMC versions, and define semantic versioning rules for the Omola API/project format. Publish failures and exclusions alongside passes.

**Done when:**
- Release artifacts include machine-readable and human-readable scorecards with versions, pass/fail/skip by scope, and known limits.
- CI prevents release when required corpus results are missing, stale, or below declared gates.
- “Full Dymola compatibility” remains absent unless every declared target area has reproducible evidence and a lawful delivery path.
- Reproducing a release report requires only documented commands and pinned inputs.

**Validation commands:** `cargo test --workspace`; `cargo run -p omola -- compat run --manifest tests/compat/manifest.toml --format json`; `cargo run -p omola -- release verify --scorecard target/compatibility.json --manifest tests/compat/manifest.toml --gates docs/release-gates.toml`.

## Implementation status

`release verify --scorecard FILE.json --manifest FILE.toml --gates FILE.toml` requires the raw manifest SHA-256 to match the generated report, rejects schema mismatch, changed manifest names or fixture sets/outcomes, missing required passes, incorrect counts, OMC version-prefix mismatch, and reports that omit the explicit Dymola-compatibility limitation. The versioned gate config declares four required fixtures, a four-pass minimum, zero failures, and at most two explicit skips. Linux PR OMC CI installs pinned MSL 4.1.0, generates JSON and Markdown reports, runs this gate, and uploads both reports as a downloadable workflow artifact. Docker and GitHub Actions run #26 produced 4 pass, 0 fail, 2 skip and passed the gate on OpenModelica 1.27.1; Windows OMC evidence passed on run #22. Issue #10 acceptance criteria are implemented. Repository branch protection must require the scorecard status before merge/release to make bypass impossible; this repository policy is documented but not modified by this PR.
