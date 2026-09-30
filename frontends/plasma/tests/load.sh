#!/usr/bin/env bash
# Load the whole applet in plasmawindowed, offscreen, against a real
# `wye service` on a private bus, and fail on any QML warning or error from
# the applet's files.
#
#   frontends/plasma/tests/load.sh path/to/wye
#
# Needs plasmawindowed (a Plasma session has it). Nothing touches the running
# session: the bus, HOME and every XDG directory are temporary, and the
# window is offscreen.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
wye=${1:?usage: $0 path/to/wye}
command -v plasmawindowed > /dev/null || { echo "plasmawindowed not found" >&2; exit 2; }

scratch=$(mktemp -d)
pids=()
cleanup() {
    for pid in "${pids[@]}"; do kill "${pid}" 2>/dev/null || true; done
    rm -r "${scratch}"
}
trap cleanup EXIT

export HOME="${scratch}/home"
export XDG_CONFIG_HOME="${HOME}/.config"
export XDG_DATA_HOME="${HOME}/.local/share"
export XDG_STATE_HOME="${HOME}/.local/state"
export XDG_CACHE_HOME="${HOME}/.cache"
export XDG_RUNTIME_DIR="${scratch}/runtime"
mkdir -p "${XDG_DATA_HOME}/plasma/plasmoids"
mkdir -m 700 "${XDG_RUNTIME_DIR}"
cp -r "${here}/../dev.soldunov.wye" "${XDG_DATA_HOME}/plasma/plasmoids/"
unset XDG_CURRENT_DESKTOP WAYLAND_DISPLAY DISPLAY

address_file="${scratch}/bus-address"
dbus-daemon --session --nofork --print-address=3 3> "${address_file}" &
pids+=($!)
for _ in $(seq 50); do
    [[ -s "${address_file}" ]] && break
    sleep 0.1
done
DBUS_SESSION_BUS_ADDRESS=$(head -n 1 "${address_file}")
export DBUS_SESSION_BUS_ADDRESS

"${wye}" service 2> "${scratch}/service.log" &
pids+=($!)

log="${scratch}/applet.log"
QT_QPA_PLATFORM=offscreen QT_FORCE_STDERR_LOGGING=1 \
    timeout 8 plasmawindowed dev.soldunov.wye > "${log}" 2>&1 || true

if grep -q "tray host registered" "${scratch}/service.log"; then
    echo "ok   the applet registered as the tray host"
else
    echo "FAIL the applet did not register"; cat "${scratch}/service.log"; exit 1
fi
if grep -E "dev.soldunov.wye/contents/ui/.*(Error|Warning|error|warning|TypeError|ReferenceError)" "${log}"; then
    echo "FAIL the applet logged QML problems (above)"
    exit 1
fi
echo "ok   the applet loaded without QML warnings"
