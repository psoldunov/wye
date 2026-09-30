#!/usr/bin/env bash
# Render the applet's tray icon in every state, offscreen, in the light and
# the dark Breeze colour scheme.
#
#   nix develop -c frontends/plasma/tests/render.sh [output-dir]
#
# Nothing touches the running Plasma session: the QML runtime gets a
# throw-away XDG_CONFIG_HOME, and Wye's icons come from a throw-away
# XDG_DATA_HOME.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "${here}/qml-env.sh"

repo=$(cd "${here}/../../.." && pwd)
out=${1:-/tmp/wye-plasma-shots}
scratch=$(mktemp -d)
trap 'rm -r "${scratch}"' EXIT
mkdir -p "${out}" "${scratch}/data/icons"
cp -r "${repo}/data/icons/hicolor" "${scratch}/data/icons/"

schemes_dir=""
for candidate in /run/current-system/sw/share/color-schemes /usr/share/color-schemes; do
    [[ -d "${candidate}" ]] && schemes_dir="${candidate}" && break
done

render_one() {
    local variant=$1 scheme=$2
    local config="${scratch}/config-${variant}"
    mkdir -p "${config}"
    [[ -n "${schemes_dir}" && -f "${schemes_dir}/${scheme}.colors" ]] &&
        cp "${schemes_dir}/${scheme}.colors" "${config}/kdeglobals"
    XDG_CONFIG_HOME="${config}" \
    XDG_DATA_HOME="${scratch}/data" \
    XDG_DATA_DIRS="${scratch}/data:${XDG_DATA_DIRS:-/run/current-system/sw/share}" \
    QT_QPA_PLATFORM=offscreen \
    QT_QUICK_BACKEND=software \
    QT_SCALE_FACTOR="${WYE_SHOT_SCALE:-2}" \
        qml "${here}/render.qml" -- "${out}/tray-icon-${variant}.png"
}

render_one light BreezeLight
render_one dark BreezeDark
ls -1 "${out}"/tray-icon-*.png
