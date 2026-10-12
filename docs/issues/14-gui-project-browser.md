# Issue #14: Launch official OMEdit reproducibly in the development environment

GitHub: https://github.com/tetsuji16/Omola/issues/14

Depends on #13. Provide a supported way to run the official, version-matched OMEdit GUI. Prefer the official distribution package; source build only if required. Keep OMC and OMEdit aligned.

## Acceptance

- A documented Docker/devcontainer profile installs a pinned official OMEdit package and runtime components, or a reproducible upstream source build if no package is suitable.
- OMEdit actually launches under documented host display forwarding or an optional Xvfb/VNC profile; package installation alone is insufficient.
- If VNC/noVNC is provided, publish only on host 127.0.0.1 and document lifecycle/shutdown.
- Image size delta, package origin, versions, and licenses are recorded; no unlicensed vendored assets.
- Linux setup is tested; Windows/macOS support is marked only when separately verified.

## Verification

Record package version, actual OMEdit process launch, startup logs/exit status, and git diff --check.
