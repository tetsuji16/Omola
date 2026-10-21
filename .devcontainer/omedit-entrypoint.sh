#!/bin/sh
set -eu

display="${DISPLAY:-:0}"
start_file="${OMOLA_START_FILE:-}"
data_dir="${OMOLA_DATA_DIR:-/workspace/data}"
children=""
omedit_pid=""

fail() {
    printf 'omedit-container: %s\n' "$*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

for command_name in Xvfb xdpyinfo xprop xwininfo openbox-session x11vnc websockify python3; do
    require_command "$command_name"
done
[ -x /usr/bin/OMEdit ] || fail "required executable not found: /usr/bin/OMEdit"
[ -d /usr/share/novnc ] || fail "noVNC web files not found at /usr/share/novnc"
if [ -n "$start_file" ] && [ ! -f "$start_file" ]; then
    fail "OMOLA_START_FILE does not exist: $start_file"
fi

mkdir -p "$data_dir" "$HOME/.config"

cleanup() {
    trap - INT TERM EXIT
    if [ -n "$omedit_pid" ]; then
        kill "$omedit_pid" 2>/dev/null || true
    fi
    for pid in $children; do
        kill "$pid" 2>/dev/null || true
    done
    if [ -n "$omedit_pid" ]; then
        wait "$omedit_pid" 2>/dev/null || true
    fi
    for pid in $children; do
        wait "$pid" 2>/dev/null || true
    done
}
trap cleanup INT TERM EXIT

wait_for_x() {
    attempts=0
    while [ "$attempts" -lt 40 ]; do
        kill -0 "$xvfb_pid" 2>/dev/null || fail "Xvfb exited before display $display became ready"
        if xdpyinfo -display "$display" >/dev/null 2>&1; then
            return 0
        fi
        sleep 0.25
        attempts=$((attempts + 1))
    done
    fail "Xvfb display $display did not become ready after 40 attempts"
}

wait_for_wm() {
    attempts=0
    while [ "$attempts" -lt 40 ]; do
        kill -0 "$wm_pid" 2>/dev/null || fail "Openbox exited during startup"
        if xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null | grep -q 'window id'; then
            return 0
        fi
        sleep 0.25
        attempts=$((attempts + 1))
    done
    fail "Openbox did not register as the X11 window manager after 40 attempts"
}

wait_for_tcp() {
    pid="$1"
    name="$2"
    port="$3"
    attempts=0
    while [ "$attempts" -lt 40 ]; do
        kill -0 "$pid" 2>/dev/null || fail "$name exited during startup"
        if python3 -c 'import socket, sys; s=socket.socket(); s.settimeout(0.5); result=s.connect_ex(("127.0.0.1", int(sys.argv[1]))); s.close(); sys.exit(0 if result == 0 else 1)' "$port"; then
            return 0
        fi
        sleep 0.25
        attempts=$((attempts + 1))
    done
    fail "$name did not listen on localhost port $port after 40 attempts"
}

wait_for_http() {
    pid="$1"
    attempts=0
    while [ "$attempts" -lt 40 ]; do
        kill -0 "$pid" 2>/dev/null || fail "websockify/noVNC exited during startup"
        if python3 -c 'import sys, urllib.request; sys.exit(0 if urllib.request.urlopen(sys.argv[1], timeout=1).status == 200 else 1)' "http://127.0.0.1:6080/vnc.html" 2>/dev/null; then
            return 0
        fi
        sleep 0.25
        attempts=$((attempts + 1))
    done
    fail "websockify did not serve noVNC on localhost port 6080 after 40 attempts"
}

Xvfb "$display" -screen 0 "${OMOLA_SCREEN:-1600x1000x24}" -nolisten tcp -ac &
xvfb_pid=$!
children="$children $xvfb_pid"
wait_for_x
openbox-session >/tmp/omola-openbox.log 2>&1 &
wm_pid=$!
children="$children $wm_pid"
wait_for_wm
x11vnc -display "$display" -localhost -nopw -forever -shared -rfbport 5900 >/tmp/omola-x11vnc.log 2>&1 &
x11vnc_pid=$!
children="$children $x11vnc_pid"
wait_for_tcp "$x11vnc_pid" "x11vnc" 5900
websockify --web=/usr/share/novnc 0.0.0.0:6080 127.0.0.1:5900 >/tmp/omola-websockify.log 2>&1 &
websockify_pid=$!
children="$children $websockify_pid"
wait_for_http "$websockify_pid"

if [ -n "$start_file" ]; then
    printf 'Launching upstream OMEdit with Modelica file: %s\n' "$start_file"
    /usr/bin/OMEdit "$start_file" &
else
    printf 'Launching upstream OMEdit without a starting file.\n'
    /usr/bin/OMEdit &
fi
omedit_pid=$!

sleep 2
kill -0 "$omedit_pid" 2>/dev/null || fail "OMEdit exited during startup; see the container logs"
printf 'Window list:\n'
xwininfo -display "$display" -root -tree 2>/dev/null || true

set +e
wait "$omedit_pid"
status=$?
set -e
exit "$status"
