#!/usr/bin/env bash
# Run the applet's DaemonClient against a real `wye service` on a private
# session bus, offscreen; the flake check `plasmoid-dbus-smoke` runs the same.
#
#   nix develop -c frontends/plasma/tests/dbus-smoke.sh [path/to/wye]
#
# Nothing touches the running session: the bus, HOME and every XDG
# directory are temporary.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "${here}/qml-env.sh"

wye=${1:-$(command -v wye || true)}
if [[ -z "${wye}" || ! -x "${wye}" ]]; then
    echo "usage: $0 path/to/wye (none on PATH)" >&2
    exit 2
fi

scratch=$(mktemp -d)
# Remove the scratch directory. The private bus may have started the
# document portal, which mounts a FUSE file system at
# $XDG_RUNTIME_DIR/doc and lets go of it only once it sees the bus is gone.
remove_scratch() {
    for _ in $(seq 30); do
        fusermount3 -uz "${scratch}/runtime/doc" 2>/dev/null || fusermount -uz "${scratch}/runtime/doc" 2>/dev/null || true
        rm -r "${scratch}" 2>/dev/null && return 0
        sleep 0.1
    done
    echo "warning: cannot remove ${scratch}" >&2
}
bus_pid=""
service_pid=""
cleanup() {
    [[ -n "${service_pid}" ]] && kill "${service_pid}" 2>/dev/null || true
    [[ -n "${bus_pid}" ]] && kill "${bus_pid}" 2>/dev/null || true
    remove_scratch
}
trap cleanup EXIT

export HOME="${scratch}/home"
export XDG_CONFIG_HOME="${HOME}/.config"
export XDG_DATA_HOME="${HOME}/.local/share"
export XDG_STATE_HOME="${HOME}/.local/state"
export XDG_RUNTIME_DIR="${scratch}/runtime"
export XDG_DATA_DIRS="${scratch}/data"
mkdir -p "${HOME}" "${XDG_DATA_DIRS}"
mkdir -m 700 "${XDG_RUNTIME_DIR}"
# No desktop: no KDE grace, no Wayland or X11 probes.
unset XDG_CURRENT_DESKTOP WAYLAND_DISPLAY DISPLAY

address_file="${scratch}/bus-address"
dbus-daemon --session --nofork --print-address=3 3> "${address_file}" &
bus_pid=$!
for _ in $(seq 50); do
    [[ -s "${address_file}" ]] && break
    sleep 0.1
done
DBUS_SESSION_BUS_ADDRESS=$(head -n 1 "${address_file}")
export DBUS_SESSION_BUS_ADDRESS

"${wye}" service &
service_pid=$!

QT_QPA_PLATFORM=offscreen \
QT_FORCE_STDERR_LOGGING=1 \
    qml "${here}/dbus-smoke.qml"
