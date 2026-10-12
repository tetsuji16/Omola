# Dependency provenance and license inventory

Run `cargo run -p omola -- licenses check --output target/license-inventory.md` from the workspace root. The command reads `Cargo.lock` through `cargo metadata --locked`, compares every reachable registry crate's exact name/version/license/source to [`licenses/inventory.toml`](../licenses/inventory.toml), and checks workflow action references and the development container's `FROM` image. It writes a Markdown inventory and returns a failing exit status for missing, stale, or changed records. The required CI job uploads this report.

The inventory separates Rust dependencies from tools and assets. Omola does not vendor OMC, MSL, Dymola software/libraries, or an FMI runtime. OMC is an external process and the devcontainer is not a redistributable product image. OpenModelica's official license page lists OSMC-PL/AGPL terms and its current terms page says third-party components retain their own terms; review the exact selected distribution and its notices before redistribution. The MSL 4.1.0 compatibility target is optional and BSD-3-Clause licensed; it is not installed by Omola. The self-authored FixtureLib test package uses this repository's MIT license.

Dymola is a commercial Dassault Systèmes product. No vendor binaries, vendor libraries, or Dymola runtime are included. Dymola-compatible naming in documentation describes an interoperability goal only and does not imply affiliation or endorsement. Public release requires a naming/trademark review. Any generated FMU is a separate artifact: inspect its source, binaries, and notices before redistribution; Omola's inventory is not an FMU license determination.

CI action references are recorded with their upstream repository and declared license. Their tags are mutable; they are downloaded by GitHub-hosted CI and are not part of Omola runtime distribution. Pinning actions to verified commit SHAs remains a security-hardening follow-up.

Update policy: Rust dependencies must be updated with Cargo so `Cargo.lock` and this exact-version/license/source ledger change together; CI checks the resolved graph, not just direct manifest declarations. Changes to OMC image tags, installed OS packages, rustup channel, optional MSL pins, or workflow actions require reviewing the upstream source and current license, updating this inventory, and rerunning the license check. Container OS packages currently inherit versions from the upstream image repositories and are not individually pinned; the image is development-only. Action SHA pinning and a fully version-pinned container toolchain remain explicit hardening follow-ups.

This is a technical inventory, not legal advice. Check exact versions and terms with qualified counsel where needed.

## Primary sources

- [OpenModelica license](https://openmodelica.org/useresresources/license/)
- [OpenModelica terms of service](https://openmodelica.org/terms-of-service/)
- [Modelica Standard Library documentation and license](https://build.openmodelica.org/Documentation/Modelica.html)
- [Dassault Systèmes licensed program terms](https://www.3ds.com/terms/ost/licensed-programs/)
- [OpenModelica setup action](https://github.com/OpenModelica/setup-openmodelica)
- [GitHub checkout action](https://github.com/actions/checkout)
- [GitHub upload-artifact action](https://github.com/actions/upload-artifact)
- [Rust toolchain action](https://github.com/dtolnay/rust-toolchain)
