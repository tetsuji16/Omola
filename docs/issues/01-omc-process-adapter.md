# 01 — Harden and document the OMC process adapter

**Goal:** Make the existing Rust boundary predictable across Windows and Unix, with actionable failures and no OMC-specific internals leaking into the public API.

**Depends on:** none. **Suggested owner:** one Luna implementation agent.

**Work:** Review `crates/omola-core`; define structured error kinds for missing executable, launch failure, nonzero exit, and compiler-reported diagnostics; validate Modelica qualified class names and numeric options before writing `.mos`; preserve paths containing spaces/quotes; ensure temp scripts are removed on every exit path; capture OMC version in every command result; document environment resolution order. Do not implement a Modelica compiler or claim Dymola compatibility.

**Done when:**
- `OMOLA_OMC` is preferred and an invalid configured path reports that path; otherwise `omc`/`omc.exe` is resolved from PATH.
- `doctor`, `check`, and `simulate` return distinct actionable errors for missing OMC, unreadable model, invalid class/stop time, and compiler failure.
- Paths with spaces work; temporary scripts are cleaned after successful and failed invocations.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass on Windows and Linux CI.
- Add fake-executable tests for arguments, errors, and cleanup; do not require OMC for unit tests.

**Validation commands:** `cargo fmt --all -- --check`; `cargo test --workspace`; `cargo clippy --workspace --all-targets -- -D warnings`; with OMC: `cargo run -p omola -- doctor`.
