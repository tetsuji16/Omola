#!/bin/sh
# Development stand-in for `omc`, for exercising omola-gui WITHOUT OpenModelica.
# It never compiles or simulates anything: it copies a prepared MAT file.
# Results produced through it are NOT evidence of Modelica or Dymola compatibility.
#
# Usage:
#   cargo run -p omola-gui --example demo_mat -- target/demo_res.mat
#   OMOLA_FAKE_MAT="$PWD/target/demo_res.mat" OMOLA_OMC="$PWD/scripts/fake-omc.sh" \
#     cargo run -p omola-gui -- examples/BouncingBall.mo
#
# Optional: OMOLA_FAKE_FAIL=1 makes every check/simulation report an OMC error.
set -eu
if [ "${1:-}" = "--version" ]; then
  echo "OpenModelica v1.27.1 (FAKE omc for GUI development)"
  exit 0
fi
script="${1:?expected a .mos script path}"
if [ "${OMOLA_FAKE_FAIL:-}" = "1" ]; then
  echo "Error: injected failure from scripts/fake-omc.sh"
  exit 0
fi
if grep -q "checkModel" "$script"; then
  echo "Check of model completed successfully (fake)."
  exit 0
fi
: "${OMOLA_FAKE_MAT:?set OMOLA_FAKE_MAT to a MAT file (see header)}"
cp "$OMOLA_FAKE_MAT" ./fake_res.mat
echo "OMOLA_RESULT_FILE:fake_res.mat"
