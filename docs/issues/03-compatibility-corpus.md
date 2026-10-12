# 03 — Define a versioned compatibility corpus and report format

**Goal:** Establish repeatable evidence before adding compatibility claims.

**Depends on:** 01, 02. **Suggested owner:** one Luna implementation agent.

**Work:** Define a manifest schema for fixtures, expected outcome, feature tags, source/license, compiler/version constraints, numerical tolerances, and skip reason. Add a runner that invokes Omola and emits stable JSON plus human-readable Markdown. Keep fixture provenance explicit; accept only original, permissively licensed, or user-authorized models.

**Done when:**
- The schema is versioned and rejects malformed manifests with a file/field diagnostic.
- A small set of positive and negative fixtures produces deterministic pass/fail/skip counts and records Omola, OMC, OS, library, and solver versions.
- Golden result comparisons declare variables, time alignment, units, and absolute/relative tolerances; file byte equality is not treated as numerical equivalence.
- Reports label each feature as measured and never derive full compatibility from a subset.

**Validation commands:** `cargo test --workspace`; `cargo run -p omola -- compat run --manifest tests/compat/manifest.toml --format json`; rerun and compare normalized JSON output.
