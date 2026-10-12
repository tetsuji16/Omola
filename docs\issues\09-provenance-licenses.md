# 08 — Audit library provenance, license constraints, and extensions

**Goal:** Define what Omola may redistribute, invoke, or replace while working with OpenModelica and Dymola-related ecosystems.

**Depends on:** 03. **Suggested owner:** one Luna implementation agent; legal questions should be escalated to qualified counsel.

**Work:** Maintain a third-party inventory for OMC, MSL, test libraries, FMI runtimes, and any vendor-specific dependencies. Record license/version/source and distribution plan. Review OSMC-PL/AGPL options for the chosen OpenModelica distribution and the applicable Dymola program terms before packaging or interoperability claims. This work is a technical inventory, not legal advice.

**Done when:**
- Every shipped or downloaded dependency has a provenance/license record and redistribution decision.
- Vendor libraries and services are either excluded, separately licensed by the user, or replaced by independently authored code with clean provenance.
- Documentation states that Omola is independent and disclaims Dassault affiliation; naming/trademark review is tracked before public release.
- CI produces an inventory and fails on unrecorded runtime dependencies.

**Validation commands:** `cargo test --workspace`; run the dependency/license inventory tool and inspect its report; verify a clean install with no Dymola files present.
