# 05 — Inventory Dymola scripting and implement a small compatibility bridge

**Goal:** Identify a useful, safe subset of Dymola automation workflows and expose explicitly named Omola equivalents.

**Depends on:** 03. **Suggested owner:** one Luna implementation agent.

**Work:** Build an inventory from published documentation and user-authorized scripts. For each function, record inputs, outputs, state effects, dependencies on GUI/licensing/vendor libraries, and an equivalent OMC operation if one exists. Implement only a small approved subset as Omola commands or a versioned scripting API. Do not copy proprietary Dymola code, documentation text, or undocumented behavior.

**Done when:**
- The inventory distinguishes exact semantic mapping, partial mapping, unsupported behavior, and proprietary dependency.
- Every implemented mapping has original fixtures and paired observable assertions (loaded model, generated artifact, diagnostics, or result values).
- Unsupported calls fail with a clear compatibility diagnostic naming the missing capability.
- A compatibility report counts implemented/partial/unsupported functions; API names are not presented as Dymola's own implementation.

**Validation commands:** `cargo test --workspace`; `cargo run -p omola -- compat run --manifest tests/compat/manifest.toml --format markdown`.
