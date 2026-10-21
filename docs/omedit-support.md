# OMEdit runtime support record

This record separates facts checked against the packaged runtime from upstream-source facts and untested behavior. It supports the OMEdit reuse decision; it does not claim Dymola compatibility or establish permission to redistribute the complete container image.

## Versioned test matrix

| Check (2026-10-22) | Result | Evidence / boundary |
|---|---|---|
| Host | Windows 11 with Docker Desktop; browser rendering not visually inspected | Container and HTTP checks only; Computer Use could not establish the browser URL. |
| Container base | `openmodelica/openmodelica:v1.27.1-minimal`, Ubuntu 22.04 (Jammy), amd64; resolved digest `sha256:ac9a0223e69b8055d444b42f58ccb9680342b56848c68b81385bf83d7c0ba824` | Official image tag and observed Docker registry digest. Dockerfile uses the version tag. |
| OMC / OMEdit | 1.27.1 / `omedit` Debian package 1.27.1-1 | Installed from the versioned official OpenModelica apt channel `https://build.openmodelica.org/omc/builds/linux/releases/1.27.1`; exact `dpkg-query` check is part of container smoke test. |
| Launch/runtime | OMEdit process launched with `examples/BouncingBall.mo` as its argument under Xvfb; Openbox, localhost-only x11vnc and noVNC/websockify readiness checked | `docker compose -f .devcontainer/docker-compose.omedit.yml up`; startup log, process, window tree and HTTP readiness are checked. Actual model-open success and visual rendering are not confirmed and remain for #15. NoVNC host port is bound to 127.0.0.1 only. |
| OS support | Linux container tested on Windows Docker Desktop | Native Windows/macOS OMEdit setup is not established. The optional web viewer was HTTP-tested, not visually reviewed in a browser. |
| Added package footprint | Base image 1,188,083,050 bytes; final image 2,414,158,177 bytes; delta 1,226,075,127 bytes (~1.14 GiB) | Docker image inspect virtual size; apt adds the Qt/WebEngine/OSG/OMPlot/OMSimulator closure. No source or GUI assets are vendored by Omola. |

The repository bind mount is read-only at `/workspace/Omola`; `/workspace/data` maps to the ignored `.omedit-workspace/` directory. The image runs as UID/GID 1000 (`omola`) and keeps QtWebEngine's sandbox enabled. Running as root would require disabling that sandbox for QtWebEngine, so the development image instead runs non-root. On Linux, set `OMOLA_UID` and `OMOLA_GID` to match the host user to avoid root-owned workspace files. Docker Desktop mediates Windows bind-mount ownership. The container can write only to the explicit data mount and its own home/tmp areas.

## Source, package and license records

The pinned source inspected is `OpenModelica/OpenModelica` commit [`d446672047a2a1cfcdfe1e07081c696ba9d37da4`](https://github.com/OpenModelica/OpenModelica/commit/d446672047a2a1cfcdfe1e07081c696ba9d37da4). The [`OMEdit application source`](https://github.com/OpenModelica/OpenModelica/blob/d446672047a2a1cfcdfe1e07081c696ba9d37da4/OMEdit/OMEditGUI/main.cpp) and other OMEdit source headers in that checkout say **AGPL-3.0 or OSMC Public License 1.8**, at recipient choice. In the installed 1.27.1-1 package, `/usr/share/doc/omedit/copyright` instead records **GPL-3.0 or OSMC Public License 1.2**. These refer to different source/package revisions; the installed package record is the one used for this runtime. Do not treat the current source header as a substitute for the package's exact notices. The package originates in the [official OpenModelica 1.27.1 distribution channel](https://build.openmodelica.org/omc/builds/linux/releases/1.27.1/).

The directly added GUI/runtime packages and inspected Debian copyright identifiers are:

| Package | Version (Ubuntu Jammy image) | Copyright/license record |
|---|---:|---|
| `omedit` | 1.27.1-1 | GPL-3.0 OR OSMC-PL 1.2 (recipient choice), `/usr/share/doc/omedit/copyright` |
| `xvfb` | 2:21.1.4-2ubuntu1.7~22.04.16 | X.Org notices, including MIT-like/X11 terms in the installed Debian copyright file |
| `x11-utils` | 7.7+5build2 | Mixed X.Org MIT-like/X11 copyright notices |
| `openbox` | 3.6.1-10 | GPL-2-or-later; Debian packaging includes BSD-3-Clause notices |
| `x11vnc` | 0.9.16-8 | GPL-2.0 |
| `novnc` | 1:1.0.0-5 | Mixed: MPL-2.0 core, BSD-2-Clause HTML/CSS, OFL fonts, CC-BY-SA-3.0 images, and other notices |
| `websockify` | 0.10.0+dfsg1-2build1 | Mixed notices including LGPL-3.0, MPL-2.0, LGPL-2.0 and BSD |

The OMEdit closure also contains Qt, QtWebEngine, OpenSceneGraph, OMPlot, OMSimulator and many distribution packages, each with its own notices. Debian/Ubuntu copyright files remain in the image; the table above is a direct-package summary, **not a complete transitive SBOM or legal redistribution review**. Before publishing an image or binaries, generate and review a complete package/license inventory and satisfy every applicable notice/source obligation. Omola copies no OMEdit/Dymola source, icons, or UI assets.

Official source facts: the OMEdit application accepts Modelica file paths as command-line arguments, and the inspected application code checks and opens those paths. This is a command-line launch behavior, not a supported plugin or embedding API. No in-process OMEdit extension boundary is claimed. The project will use normal `.mo` files/annotations and an out-of-process Rust companion where tested CLI behavior is needed.

## Feature map and compatibility boundary

| OMEdit capability reused | Omola stage | Dymola compatibility status |
|---|---|---|
| Text editor, diagram editor, Modelica annotations | Launch official OMEdit, then verify project-open/edit/save workflows | OpenModelica behavior; Dymola parity and proprietary extensions not measured |
| Libraries and model browser | Open manifest-selected entry model and configured library roots | Manifest handoff not implemented; library differences unknown |
| Check/simulate and messages | Exercise OMEdit/OMC against fixtures; Rust bridge only for measured gaps | Backend CLI is separate; exact GUI workflow equivalence not measured |
| Result browser and plots | Run fixture and verify variables, paths and plot behavior | Proprietary Dymola result formats and metadata remain unsupported/unknown |
| Dymola-specific scripting, annotations and commercial features | Inventory per feature with paired fixtures and explicit fallback | Unsupported until a lawful, measurable implementation exists |

## Validation commands

```powershell
docker compose -f .devcontainer/docker-compose.omedit.yml config
docker compose -f .devcontainer/docker-compose.omedit.yml build
docker compose -f .devcontainer/docker-compose.omedit.yml up -d
docker compose -f .devcontainer/docker-compose.omedit.yml logs --no-color
docker compose -f .devcontainer/docker-compose.omedit.yml exec omedit id
docker compose -f .devcontainer/docker-compose.omedit.yml exec omedit dpkg-query -W omedit
docker compose -f .devcontainer/docker-compose.omedit.yml down
```

These commands establish container configuration and process/runtime health, not visual browser quality. The `up` command keeps the preview available until `down`; host access is `http://127.0.0.1:6080/vnc.html?autoconnect=true`.
