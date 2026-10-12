# 02 — Add a checked-in smoke model and reproducible CLI examples

**Goal:** Ensure a fresh checkout has a tiny model that demonstrates the supported OMC-backed workflow.

**Depends on:** 01. **Suggested owner:** one Luna implementation agent.

**Work:** Add a self-contained Modelica bouncing-ball or first-order model with no third-party library dependency. Keep the class name and file path stable. Document install commands for Rust and OpenModelica on Windows, macOS, and Linux, and show checking/simulating the model. State that execution requires OMC and is not a Dymola parity test.

**Done when:**
- `examples/BouncingBall.mo` contains a valid, minimal model and its expected variables are named in the docs.
- All README commands work when prerequisites are installed, including output prefix paths with spaces.
- CI without OMC builds the Rust workspace; an optional OMC job checks and simulates the example and stores a concise log/artifact.
- README clearly distinguishes build-only checks from backend integration checks.

**Validation commands:** `cargo check --workspace`; `cargo run -p omola -- doctor`; `cargo run -p omola -- check --model examples/BouncingBall.mo --class BouncingBall`; `cargo run -p omola -- simulate --model examples/BouncingBall.mo --class BouncingBall --stop-time 3 --output out/BouncingBall`.

## Implementation status

The checked-in model and OS-specific setup instructions are implemented. The OMC integration workflow now runs on pull requests (and can also be manually dispatched) on Linux and Windows and stores concise logs. It also checks that an `initialHeight=2` parameter override changes the exported initial height from 1 to 2. Keep this issue open until PR-triggered OMC checks pass on both operating systems.
