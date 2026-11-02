#!/usr/bin/env bash
# Run OpenModelica's `omc` inside Docker, for systems without a native OpenModelica package
# (OpenModelica no longer ships native macOS builds). Point Omola at it with
#   export OMOLA_OMC="$PWD/scripts/omc-docker.sh"
#
# The home directory, the temporary directory and the current directory are mounted at the
# same paths, so model files, Omola's scratch folders and result files keep their paths.
# Installed Modelica libraries persist in the Docker volume $OMOLA_OMC_HOME_VOLUME.
#
#   OMOLA_OMC_IMAGE        image with omc (default: omola-dev, built by scripts/setup-docker.sh)
#   OMOLA_OMC_HOME_VOLUME  volume used as HOME inside the container (default: omola-omc-home)
#   OMOLA_OMC_DOCKER_USER  set to 1 to run as your uid:gid (recommended on Linux hosts)
set -euo pipefail

IMAGE="${OMOLA_OMC_IMAGE:-omola-dev}"
VOLUME="${OMOLA_OMC_HOME_VOLUME:-omola-omc-home}"
TMP="${TMPDIR:-/tmp}"
TMP="${TMP%/}"

args=(run --rm -i -v "$VOLUME:/omola-home" -e HOME=/omola-home)
if [ "${OMOLA_OMC_DOCKER_USER:-0}" = "1" ]; then
  args+=(--user "$(id -u):$(id -g)")
fi
mounted=""
for dir in "$HOME" "$TMP" /tmp "$PWD"; do
  [ -d "$dir" ] || continue
  dir="$(cd "$dir" && pwd -P)"
  case " $mounted " in *" $dir "*) continue ;; esac
  mounted="$mounted $dir"
  args+=(-v "$dir:$dir")
done
args+=(-w "$(pwd -P)" "$IMAGE" omc)
exec docker "${args[@]}" "$@"
