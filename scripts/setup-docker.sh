#!/usr/bin/env bash
# Set up Omola with OpenModelica running in Docker (macOS, or any system with Docker but
# without a native OpenModelica package): builds the pinned OpenModelica development image,
# installs the Modelica Standard Library 4.1.0 into a Docker volume, builds omola +
# omola-gui natively, and runs a smoke simulation through scripts/omc-docker.sh.
#
# Usage: scripts/setup-docker.sh [--skip-build] [--no-rust]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SKIP_BUILD=0 NO_RUST=0
for arg in "$@"; do
  case "$arg" in
    --skip-build) SKIP_BUILD=1 ;;
    --no-rust) NO_RUST=1 ;;
    -h|--help) sed -n '2,8p' "$0"; exit 0 ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done
step() { printf '\n==> %s\n' "$*"; }

command -v docker >/dev/null || { echo "Docker is required (Docker Desktop on macOS). See docs/install.md." >&2; exit 1; }
docker info >/dev/null 2>&1 || { echo "Docker is installed but not running. Start Docker Desktop and retry." >&2; exit 1; }

IMAGE="${OMOLA_OMC_IMAGE:-omola-dev}"
step "Building the OpenModelica image '$IMAGE' from .devcontainer/Dockerfile (OpenModelica 1.27.1)"
docker build -f "$ROOT/.devcontainer/Dockerfile" -t "$IMAGE" "$ROOT"

export OMOLA_OMC="$ROOT/scripts/omc-docker.sh"
export OMOLA_OMC_IMAGE="$IMAGE"
step "Using $("$OMOLA_OMC" --version) through $OMOLA_OMC"

step "Installing Modelica Standard Library 4.1.0 (and ModelicaReference) into the Docker volume"
OUT="$(cd "${TMPDIR:-/tmp}" && "$OMOLA_OMC" "$ROOT/scripts/install-libraries.mos")"
echo "$OUT" | grep -E "^OMOLA_" || true
echo "$OUT" | grep -q "OMOLA_INSTALLED Modelica 4.1.0" || { echo "$OUT" >&2; exit 1; }

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
  step "Building omola and omola-gui (release)"
  (cd "$ROOT" && cargo build --locked --release -p omola -p omola-gui)
  step "Smoke test: simulate examples/BouncingBall.mo"
  OUTDIR="$(mktemp -d)"
  "$ROOT/target/release/omola" doctor
  "$ROOT/target/release/omola" simulate --model "$ROOT/examples/BouncingBall.mo" --class BouncingBall \
    --stop-time 1 --output "$OUTDIR/BouncingBall"
  test -s "$OUTDIR/BouncingBall_res.mat"
  rm -rf "$OUTDIR" 2>/dev/null || true
fi

cat <<EOF

==> Done. Add this to your shell profile (~/.zshrc on macOS) so Omola finds the compiler:
  export OMOLA_OMC="$ROOT/scripts/omc-docker.sh"
Then start the workbench with:
  $ROOT/target/release/omola-gui
EOF
