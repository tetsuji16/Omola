# 07 — Specify a basic model project and editor workflow

**Goal:** Define a Rust-first user workflow for opening, organizing, editing, and simulating a Modelica project before selecting a desktop UI toolkit.

**Depends on:** 03. **Suggested owner:** one Luna implementation agent.

**Work:** Write a project format proposal and a small CLI prototype for package roots, dependencies, and model entry points. Compare a text-first editor workflow with a diagram editor requirement. Use OMC for validation. Do not promise diagram authoring or Dymola file/project compatibility without fixtures and an implementation.

**Done when:**
- The project format is versioned, portable, and preserves original Modelica files.
- A sample project can be opened, validated, and simulated on Windows and Linux.
- The proposal documents dependency resolution, library pinning, path handling, and migration policy.
- The GUI roadmap states which editing/plotting workflows remain unsupported and defines acceptance steps for a later UI.

**Validation commands:** `cargo test --workspace`; `cargo run -p omola -- project validate examples/project.toml`; `cargo run -p omola -- project simulate examples/project.toml`.
