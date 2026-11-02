#!/usr/bin/env bash
# Set up Omola on Debian/Ubuntu: OpenModelica compiler (omc) from the official OpenModelica
# apt repository, the Modelica Standard Library 4.1.0, Rust (if missing), a release build of
# omola + omola-gui, and a smoke simulation.
#
# Usage: scripts/setup-linux.sh [--skip-omc] [--skip-libraries] [--skip-build] [--no-rust]
#   OMOLA_OM_VERSION=1.27.1   OpenModelica version to prefer (falls back to the newest
#                             "stable" package if that exact version is not published for
#                             your distribution).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OM_VERSION="${OMOLA_OM_VERSION:-1.27.1}"
SKIP_OMC=0 SKIP_LIBS=0 SKIP_BUILD=0 NO_RUST=0
for arg in "$@"; do
  case "$arg" in
    --skip-omc) SKIP_OMC=1 ;;
    --skip-libraries) SKIP_LIBS=1 ;;
    --skip-build) SKIP_BUILD=1 ;;
    --no-rust) NO_RUST=1 ;;
    -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done

step() { printf '\n==> %s\n' "$*"; }
SUDO=""
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo"; fi

# 1. OpenModelica compiler -----------------------------------------------------------------
if [ "$SKIP_OMC" -eq 0 ] && ! command -v omc >/dev/null 2>&1; then
  step "Installing OpenModelica $OM_VERSION (omc) from https://build.openmodelica.org/apt"
  command -v apt-get >/dev/null || { echo "This script supports Debian/Ubuntu (apt). See docs/install.md." >&2; exit 1; }
  $SUDO apt-get update
  $SUDO apt-get install -y --no-install-recommends ca-certificates curl gnupg build-essential
  curl -fsSL https://build.openmodelica.org/apt/openmodelica.asc \
    | $SUDO gpg --dearmor --yes -o /usr/share/keyrings/openmodelica-keyring.gpg
  # shellcheck disable=SC1091
  CODENAME="$(. /etc/os-release && echo "${UBUNTU_CODENAME:-${VERSION_CODENAME:-}}")"
  [ -n "$CODENAME" ] || { echo "Could not determine the distribution codename." >&2; exit 1; }
  echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/openmodelica-keyring.gpg] https://build.openmodelica.org/apt $CODENAME stable" \
    | $SUDO tee /etc/apt/sources.list.d/openmodelica.list >/dev/null
  $SUDO apt-get update
  PINNED="$(apt-cache madison omc | awk -v v="$OM_VERSION" '$3 ~ "^"v"-" {print $3; exit}')"
  if [ -n "$PINNED" ]; then
    $SUDO apt-get install -y --no-install-recommends "omc=$PINNED"
  else
    echo "OpenModelica $OM_VERSION is not published for '$CODENAME'; installing the current stable omc." >&2
    $SUDO apt-get install -y --no-install-recommends omc
  fi
fi
if command -v omc >/dev/null 2>&1; then
  step "Using $(command -v omc): $(omc --version)"
else
  echo "omc is not available; install OpenModelica or set OMOLA_OMC (see docs/install.md)." >&2
  [ "$SKIP_OMC" -eq 1 ] || exit 1
fi

# 2. Modelica libraries --------------------------------------------------------------------
if [ "$SKIP_LIBS" -eq 0 ] && command -v omc >/dev/null 2>&1; then
  step "Installing Modelica Standard Library 4.1.0 (and ModelicaReference) for $(id -un)"
  OUT="$(cd "${TMPDIR:-/tmp}" && omc "$ROOT/scripts/install-libraries.mos")"
  echo "$OUT" | grep -E "^OMOLA_" || true
  echo "$OUT" | grep -q "OMOLA_INSTALLED Modelica 4.1.0" || { echo "$OUT" >&2; exit 1; }
fi

# 3. Build Omola ---------------------------------------------------------------------------
if [ "$SKIP_BUILD" -eq 0 ]; then
  if ! command -v cargo >/dev/null 2>&1 && [ -x "$HOME/.cargo/bin/cargo" ]; then
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
  if ! command -v cargo >/dev/null 2>&1; then
    [ "$NO_RUST" -eq 0 ] || { echo "cargo not found and --no-rust given." >&2; exit 1; }
    step "Installing Rust with rustup (https://rustup.rs)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
  step "Installing runtime libraries for the desktop window (OpenGL, X11/Wayland keyboard)"
  $SUDO apt-get install -y --no-install-recommends libgl1 libegl1 libxkbcommon0 libxkbcommon-x11-0 libx11-6 libxcursor1 libxrandr2 libxi6 libwayland-client0 fonts-liberation \
    || echo "Could not install some GUI runtime packages; the CLI still works." >&2
  step "Building omola and omola-gui (release)"
  (cd "$ROOT" && cargo build --locked --release -p omola -p omola-gui)

  # 4. Smoke test --------------------------------------------------------------------------
  if command -v omc >/dev/null 2>&1; then
    step "Smoke test: simulate examples/BouncingBall.mo"
    OUTDIR="$(mktemp -d)"
    "$ROOT/target/release/omola" doctor
    "$ROOT/target/release/omola" simulate --model "$ROOT/examples/BouncingBall.mo" --class BouncingBall \
      --stop-time 1 --output "$OUTDIR/BouncingBall"
    test -s "$OUTDIR/BouncingBall_res.mat"
    rm -rf "$OUTDIR"
  fi
  step "Done. Start the workbench with:  $ROOT/target/release/omola-gui"
else
  step "Done (build skipped)."
fi
