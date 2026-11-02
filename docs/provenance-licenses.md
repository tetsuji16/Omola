# Dependency provenance and license inventory

Run `cargo run -p omola -- licenses check --output target/license-inventory.md` from the workspace root. The command reads `Cargo.lock` through `cargo metadata --locked`, compares every reachable registry crate's exact name/version/license/source to [`licenses/inventory.toml`](../licenses/inventory.toml), and checks workflow action references plus `FROM` images and apt-installed package inputs in both development Dockerfiles. It writes a Markdown inventory and returns a failing exit status for missing, stale, or changed records. The required CI job uploads this report.

The inventory separates Rust dependencies from tools and assets. Omola does not vendor OMC, MSL, Dymola software/libraries, or an FMI runtime. OMC is an external process and the devcontainer is not a redistributable product image. OpenModelica's official license page lists OSMC-PL/AGPL terms and its current terms page says third-party components retain their own terms; review the exact selected distribution and its notices before redistribution. The MSL 4.1.0 compatibility target is optional and BSD-3-Clause licensed; it is not installed by Omola. The self-authored FixtureLib test package uses this repository's MIT license.

Dymola is a commercial Dassault Systèmes product. No vendor binaries, vendor libraries, or Dymola runtime are included. Dymola-compatible naming in documentation describes an interoperability goal only and does not imply affiliation or endorsement. Public release requires a naming/trademark review. Any generated FMU is a separate artifact: inspect its source, binaries, and notices before redistribution; Omola's inventory is not an FMU license determination.

CI action references are recorded with their upstream repository and declared license. Their tags are mutable; they are downloaded by GitHub-hosted CI and are not part of Omola runtime distribution. Pinning actions to verified commit SHAs remains a security-hardening follow-up.

Update policy: Rust dependencies must be updated with Cargo so `Cargo.lock` and this exact-version/license/source ledger change together; CI checks the resolved graph, not just direct manifest declarations. Changes to OMC image tags, installed OS packages, rustup channel, optional MSL pins, or workflow actions require reviewing the upstream source and current license, updating this inventory, and rerunning the license check. Container OS packages currently inherit versions from the upstream image repositories and are not individually pinned; the image is development-only. Action SHA pinning and a fully version-pinned container toolchain remain explicit hardening follow-ups.

### Native GUI (`omola-gui`, egui)

The `omola-gui` crate (added 2026-11-02) depends on `eframe`/`egui` 0.37 and `egui_plot` 0.38 and brings 268 additional crates into the locked graph, all recorded in `licenses/inventory.toml`. Every recorded license is permissive or offers a permissive choice (MIT, Apache-2.0, Zlib, BSD, ISC, BSL-1.0, Unicode-3.0). Two crates are dual-licensed with a copyleft alternative (`Apache-2.0 OR GPL-2.0-only`, `MIT OR Apache-2.0 OR LGPL-2.1-or-later`); Omola uses the permissive option. `epaint_default_fonts` embeds fonts under OFL-1.1 and the Ubuntu Font Licence 1.0: a distributed `omola-gui` binary must ship those font notices. The GUI copies no Dymola or OMEdit code, icons, or assets; Dymola screenshots are referenced by URL only (`docs/reference/dymola-gui.md`).

Since 2026-11-02 the GUI also uses `egui_extras` with the `image` (PNG/JPEG/GIF) and `resvg` (SVG) loaders to show images in Modelica documentation, and `egui_material_icons` (crate MIT; it embeds the Material Symbols font under Apache-2.0, whose notice a binary distribution must retain). `egui_plot` was removed; the plot is drawn by Omola. User-interface fonts (Segoe UI, Courier New, Liberation, DejaVu, Noto CJK) are read from the operating system at run time and are not bundled. Documentation images are read from the installed library (for example MSL under its BSD-3-Clause license) and are never copied into this repository.

Since 2026-11-02 the GUI enables eframe's `persistence` feature, which adds `enumn` (MIT OR Apache-2.0), `ron` (MIT OR Apache-2.0), and `typeid` (MIT OR Apache-2.0) to serialize window state across restarts. `serde` (MIT OR Apache-2.0) is also added directly for the persisted-state struct. All three are recorded in `licenses/inventory.toml`.

Since 2026-11-02 the GUI uses `rfd` 0.15 (MIT) with the `xdg-portal` feature for native file dialogs on Windows, macOS, and Linux (via the XDG Desktop Portal; no GTK system dependency). This pulls in `ashpd` (MIT), `zbus`/`zvariant` (MIT), `url` (MIT OR Apache-2.0), ICU normalization crates (Unicode-3.0), `rand` (MIT OR Apache-2.0), and `tempfile` (MIT OR Apache-2.0) — 62 additional crates, all permissively licensed (MIT, Apache-2.0, Unicode-3.0). All are recorded in `licenses/inventory.toml`.

### Setup scripts and release archives

`scripts/setup-windows.ps1`, `scripts/setup-linux.sh` and `scripts/setup-docker.sh` download OpenModelica directly from `build.openmodelica.org` (official Windows installer, official apt repository, or the pinned `openmodelica/openmodelica:v1.27.1-minimal` image) and install the Modelica Standard Library 4.1.0 through OpenModelica's package manager. The user accepts those upstream licenses (OSMC-PL/AGPLv3 for OpenModelica, BSD-3-Clause for MSL) on their own machine; nothing from OpenModelica or MSL is committed or redistributed. Rust is installed from `rustup.rs` only when missing. The release workflow packages Omola binaries, scripts, examples, `licenses/` and a generated `THIRD_PARTY_LICENSES.md`; it does not include OpenModelica or MSL.

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
