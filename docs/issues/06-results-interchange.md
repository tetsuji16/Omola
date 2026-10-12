# 06 — Add result artifact discovery and CSV interchange

**Goal:** Make simulation outputs discoverable and support a small, explicitly documented interchange path.

**Depends on:** 03. **Suggested owner:** one Luna implementation agent.

**Work:** Record result paths and metadata from OMC runs. Start with CSV import/export for named variables and time columns; evaluate MAT/DSRES formats separately against public format documentation and test fixtures. Never describe CSV support as Dymola result-file compatibility. Preserve units and variable names when available and report missing metadata.

**Done when:**
- Run metadata records output location, tool/version, model, parameters, solver settings, and timestamps.
- CSV round-trip tests cover headers, quoted names, missing values policy, monotonic time, and numeric precision.
- Format detection rejects unsupported or ambiguous formats with a useful message.
- Any claimed result interoperability has fixtures, variable/time coverage, unit checks, and stated numeric tolerances.

**Validation commands:** `cargo test --workspace`; `cargo run -p omola -- results inspect PATH`; `cargo run -p omola -- results export-csv PATH --variables x,y --output result.csv`.
