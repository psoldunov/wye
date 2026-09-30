#!/usr/bin/env bash
# qmllint over every QML file of the applet and its tests; the flake check
# `plasmoid-lint` runs the same.
#
#   nix develop -c frontends/plasma/tests/lint.sh
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "${here}/qml-env.sh"

applet="${here}/../dev.soldunov.wye"
import_args=()
while IFS= read -r path; do
    import_args+=(-I "${path}")
done < <(tr ':' '\n' <<< "${WYE_PLASMA_QML_PATH}")
import_args+=(-I "${applet}/contents/ui")

mapfile -t files < <(find "${applet}" "${here}" -name '*.qml' | sort)

qmllint "${import_args[@]}" "${files[@]}"
