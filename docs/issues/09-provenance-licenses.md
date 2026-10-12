# 09 — Audit library provenance, license constraints, and extensions

**Goal:** Define what Omola may redistribute, invoke, or replace while working with OpenModelica and Dymola-related ecosystems.

**Depends on:** 03. **Suggested owner:** one Luna implementation agent; legal questions should be escalated to qualified counsel.

**Work:** Maintain a third-party inventory for OMC, MSL, test libraries, FMI runtimes, and any vendor-specific dependencies. Record license/version/source and distribution plan. Review OSMC-PL/AGPL options for the chosen OpenModelica distribution and the applicable Dymola program terms before packaging or interoperability claims. This work is a technical inventory, not legal advice.

**Implementation status:** The locked Cargo dependency closure, CI action references, and development container base image are checked against `licenses/inventory.toml`. CI emits `dependency-license-inventory` and fails when records drift. The inventory is technical metadata, not a legal opinion; it records current upstream license declarations and does not grant redistribution rights. Dymola binaries/libraries are excluded. MSL is optional and not bundled. Generated FMUs require a separate content/license review before redistribution. Trademark/naming review remains a pre-release action.

**Done when:**
- Every shipped or downloaded dependency has a provenance/license record and redistribution decision.
- Vendor libraries and services are either excluded, separately licensed by the user, or replaced by independently authored code with clean provenance.
- Documentation states that Omola is independent and disclaims Dassault affiliation; naming/trademark review is tracked before public release.
- CI produces an inventory and fails on unrecorded runtime dependencies.

**Validation commands:** `cargo test --workspace`; run the dependency/license inventory tool and inspect its report; verify a clean install with no Dymola files present.

```sh
cargo run -p omola -- licenses check --output target/license-inventory.md
```

Official references: [OpenModelica license](https://openmodelica.org/useresresources/license/), [OpenModelica terms](https://openmodelica.org/terms-of-service/), [MSL 4.1.0 documentation/license](https://build.openmodelica.org/Documentation/Modelica.html), and [Dassault licensed-program terms](https://www.3ds.com/terms/ost/licensed-programs/). Check applicable terms for the exact version and distribution; this inventory is not legal advice.
