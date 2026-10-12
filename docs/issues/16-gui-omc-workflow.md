# Issue #16: Connect Omola project actions to OMEdit and OMC diagnostics

GitHub: https://github.com/tetsuji16/Omola/issues/16

Depends on #14 and #15. Reuse OMEdit check/simulation controls and messages wherever they satisfy the workflow. Rust companion adds only manifest validation, explicit Dymola mappings, safe process control, and measured diagnostic gaps.

## Acceptance

- Real model check and simulation complete using the matched OMC/OMEdit pair.
- Diagnostics include model, backend version, action outcome, and result path/reason; exit-zero error text or empty result path is failure.
- Rust invokes typed documented operations only, never arbitrary scripts or interpolated shell text.
- Concurrent job, cancel/timeout, missing OMC, invalid model, and cleanup behavior are deterministic and tested.
- OMC-free unit tests and one Linux/Windows backend integration result are recorded; unsupported automation platforms are stated.

## Verification

Run cargo test --workspace, cargo clippy --workspace --all-targets -- -D warnings, project validate/simulate for examples/project.toml, and documented OMEdit open/check/simulate smoke.
