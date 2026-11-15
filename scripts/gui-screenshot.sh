#!/bin/sh
# Headless screenshot of omola-gui (Linux). Needs: Xvfb, ImageMagick `import`,
# and for clicks/keys: `pip install python-xlib` (see scripts/xdrive.py).
#
# Usage:
#   scripts/gui-screenshot.sh OUT.png [xdrive actions...]
# Example (click the "Simulation" ribbon tab, wait, then capture):
#   scripts/gui-screenshot.sh target/sim.png click:337,42 sleep:1
#
# The GUI runs with scripts/fake-omc.sh unless OMOLA_OMC is already set.
set -eu
out="${1:?output png path}"; shift
root="$(cd "$(dirname "$0")/.." && pwd)"
display_num=":${OMOLA_XVFB_DISPLAY:-77}"
cargo build -q -p omola-gui --manifest-path "$root/Cargo.toml"
if [ -z "${OMOLA_OMC:-}" ]; then
  cargo run -q -p omola-gui --example demo_mat --manifest-path "$root/Cargo.toml" -- "$root/target/demo_res.mat" >/dev/null
  export OMOLA_OMC="$root/scripts/fake-omc.sh" OMOLA_FAKE_MAT="$root/target/demo_res.mat"
fi
Xvfb "$display_num" -screen 0 1280x800x24 >/dev/null 2>&1 &
xvfb_pid=$!
trap 'kill "$gui_pid" "$xvfb_pid" 2>/dev/null || true' EXIT
sleep 1
export DISPLAY="$display_num" LIBGL_ALWAYS_SOFTWARE=1 WINIT_UNIX_BACKEND=x11
"$root/target/debug/omola-gui" "$root/examples/BouncingBall.mo" >/dev/null 2>&1 &
gui_pid=$!
sleep 4
if [ "$#" -gt 0 ]; then
  python3 -I "$root/scripts/xdrive.py" "$@"
fi
import -window root "$out"
echo "wrote $out"
