# OMEdit source and native Windows build record

**Checked:** 2026-10-28. This is a development record for a future Omola-modified OMEdit build. It does not claim that the source has compiled or that the Omola GUI feature is implemented.

## Pinned upstream inputs

| Input | Observed value |
|---|---|
| Repository | [`OpenModelica/OpenModelica`](https://github.com/OpenModelica/OpenModelica) |
| Release | [`v1.27.1`](https://github.com/OpenModelica/OpenModelica/releases/tag/v1.27.1) |
| Commit | `4a7248c1ab64f4205eb4222b6b009bbf2b0df383` |
| Recursive submodules | Pinned by the release tree; their resolved commits are in the local checkout record below |
| Source footprint | 32,197 files, 1,060,465,012 bytes including recursive submodules |
| Existing installed GUI | OpenModelica 1.27.1 at `C:\Program Files\OpenModelica1.27.1-64bit`; this installation is kept separate from source builds |

The local source checkout is outside the Omola repository at `C:\Users\tetsu\Documents\codex\Omola-dev\OpenModelica-v1.27.1`. It is pinned to the release commit and has these recursive submodule commits:

```text
OMCompiler/3rdParty                         dfc66f9ee9ab081c79a33bcb208e3b307f48f16f
OMCompiler/Compiler/boot/bomc               65b9723845221cff46bc3e1b8a7313bdd0598cb3
OMOptim                                     87f72d594906dac0597591c938f2008fc82160de
OMOptim/common                              08a01802db5ba5edb540383c46718b89ff229ef2
OMSens_Qt                                   bd321e3e09a91748cf197bdaa571f571734a30e8
OMSens_Qt/common                            6e6d4fd78c74da79ef079ee412d5325eb3b60166
OMSimulator                                 81bd489c4cd190397182b8c4892cc3f02f9353d4
OMSimulator/3rdParty                        d688271c317d4093620076e9d5a1a4030d036446
```

## Licensing and product boundary

Headers in OMEdit v1.27.1 source, including `OMEdit/OMEditLIB/Simulation/SimulationOptions.h` and `OMEdit/OMEditLIB/OMEditApplication.cpp`, offer **GNU AGPL version 3 or OSMC Public License 1.8**, at the recipient's choice. The pinned source tree contains [`OSMC-License.txt`](https://github.com/OpenModelica/OpenModelica/blob/v1.27.1/OSMC-License.txt) and [`OMCompiler/COPYING`](https://github.com/OpenModelica/OpenModelica/blob/v1.27.1/OMCompiler/COPYING). OMEdit also depends on OpenModelica Compiler, OMPlot and OMSimulator, and the final application links Qt and other libraries with their own notices and obligations.

Before distributing a modified build, choose and record the license path for the modified OMEdit source, include required source/notices, and inventory the complete runtime dependency closure. This record does not choose a legal interpretation or authorize redistributing any binary. Omola will not copy Dymola source, documentation, models, icons, or UI artwork. The product name remains Omola and will not imply Dassault Systèmes affiliation.

## OMEdit extension points confirmed in source

The upstream code is C++/Qt, with the GUI in `OMEdit/OMEditLIB` and the executable entry point in `OMEdit/OMEditGUI`. In v1.27.1, `MainWindow` owns model-open and simulate actions. `SimulationDialog::show(LibraryTreeItem*, bool, SimulationOptions)`, `initializeFields`, and `applySimulationOptions` already feed values into the standard simulation settings UI. `SimulationOptions` contains start/stop time but no dedicated manifest parameter-override member; `SimulationDialog::performSimulation` builds the simulation parameter call. This is a concrete source-level extension point, not a supported plugin API.

The first vertical feature is an OMEdit-native **Open Omola Project** action that calls the Rust `omola project inspect --format json` command through `QProcess` (arguments passed separately), opens the canonical model in OMEdit's existing model browser, and initializes existing simulation controls from manifest settings. The actual run must use the displayed parameter overrides and stop time, then leave results in the existing OMEdit result/plot UI. Invalid bridge versions, malformed JSON, unavailable `omola`, missing paths, and unsupported parameter forms must produce visible diagnostics. It must not rewrite Modelica source. The Rust bridge contract is versioned in [`omedit-project-bridge.schema.json`](omedit-project-bridge.schema.json).

## Windows build feasibility: measured state

The pinned [`OMEdit README`](https://github.com/OpenModelica/OpenModelica/blob/v1.27.1/OMEdit/README.md) says OMEdit depends on OMC, OMPlot, and OMSimulator and directs builds through the full OpenModelica build instructions. The pinned [`Windows instructions`](https://github.com/OpenModelica/OpenModelica/blob/v1.27.1/OMCompiler/README.Windows.md) recommend OMDev (MSYS2 UCRT64 with fixed package versions), and list Git, a Java JDK, TortoiseSVN, and CMake >=3.21. The pinned CMake instructions use the `MSYS Makefiles` generator and build the install target. A successful modified GUI therefore needs an isolated OpenModelica source/build/install tree, not just a rebuild of `OMEdit.exe` against the existing system install.

Measured host state:

- Windows has the official OpenModelica 1.27.1 distribution and its bundled MSYS2/UCRT64 runtime. The bundled compiler reports GCC 16.1.0, CMake 4.3.2, Make 4.4.1, and Qt 6.11.0 package records.
- The bundled MSYS2 package database records Qt development packages, but their development files are stripped from this runtime installation: `pacman -Qk mingw-w64-ucrt-x86_64-qt6-base` reported 3,233 missing package files, including `Qt6Config.cmake` and headers. The unmodified installation is not a complete OMEdit build environment.
- CMake configuration of the pinned source was attempted in a separate build directory. With default optimization/MOO settings, the bundled CMake 4.3.2 hits Ipopt CMake regex errors. Turning off optional MOO/optimization/Fortran removes those errors. A local test-only guard for the unavailable SUNDIALS LAPACK target lets configuration proceed further; no OMEdit code or shipped source was patched by this test. Configuration then stops because the stripped runtime lacks the Qt6 development configuration/headers. No C++ compile or modified OMEdit launch has occurred.
- The official OMDevUCRT GitLab remote advertises a ref, but a shallow clone produced zero files/bytes for about a minute and was stopped. The partial clone is outside the repository. It has not been installed and the existing OpenModelica installation was not altered.
- A separate official MSYS2 installer (`2026-09-27`, SHA-256 `416d506c839a2da4bc184eeec8cbaaf1055ee698618228c62dfc4232a6970f0e`) was installed under `C:\Users\tetsu\Documents\codex\Omola-dev\msys64` and updated with `pacman -Syu`. The Qt6 development packages are present in its package repository, but a search of the UCRT64 repository found no `mingw-w64-ucrt-x86_64-qt6-webengine` package. OMEdit v1.27.1 requires `Qt6::WebEngineWidgets` in `OMEdit/CMakeLists.txt`, so the official MSYS2 package set cannot currently satisfy the GUI build. Its available Qt package metadata is 6.12.0, while the installed OpenModelica runtime records Qt 6.11.0; mixing those binaries is not a supported workaround. No Qt development packages were installed and no source build was attempted against this incomplete set.
- The installer initialized the MSYS2 home skeleton and created `%USERPROFILE%\.bashrc`, `.bash_profile`, and `.profile`; these files were left in place. The installation is isolated under `Omola-dev` and is separate from the existing OpenModelica runtime.
- C: had about 600 GB free during this check. Space is available for a clean toolchain, but the eventual Qt/OMC/OMPlot/OMSimulator dependency download and full build size/time are still unmeasured.

The next build attempt needs a toolchain that supplies the pinned Qt WebEngine development files together with the compatible OpenModelica dependencies, such as a verified OMDev package set or a compatible Qt/MSVC profile. Keep it in a dedicated user-owned directory and keep the existing OpenModelica install untouched. If the pinned source requires local portability patches, keep them as a narrow, reviewable Omola patch series and state any disabled OpenModelica capabilities. Do not describe a configure pass as a build or GUI verification.

## Compatibility target and Dymola limits

OMEdit already supplies Modelica text/diagram editing, library browsing, checking, simulation settings, messages, and result plotting through OpenModelica. The first Omola contribution should join those existing workflows to the project manifest. It should not replace OMEdit's editor, diagram canvas, model browser, or plotting widgets.

Dassault Systèmes public Dymola material describes graphical Modelica authoring, simulation/post-processing, Python and FMI/other-software integration, and separate parameter-sweep, calibration, and optimization features. Those summaries are feature leads, not a full compatibility specification. Each gap is measured separately across Modelica language/libraries, OMC behavior, OMEdit behavior, Omola project integration, Dymola-specific APIs/formats, and vendor services. Dymola `.dymx`, `.dsres`, Dymola-specific scripting semantics, proprietary libraries, encrypted models, Simulink and 3DEXPERIENCE functions remain unsupported or unknown until a lawful fixture and measurable acceptance test exist. OpenModelica or OMEdit success never counts as Dymola parity.

Primary Dymola references: [official model design tools](https://www.3ds.com/products/catia/dymola/model-design-tools), [official integration/export capabilities](https://www.3ds.com/products/catia/dymola/export-capabilities-interfacing-other-software), and [official release features](https://www.3ds.com/products/catia/dymola/latest-release). The pages change over time and are not a complete Dymola specification.
