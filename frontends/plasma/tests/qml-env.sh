#!/usr/bin/env bash
# Shared QML import path for the lint, smoke and render scripts.
#
# Inside `nix develop` the dev shell exports WYE_PLASMA_QML_PATH with every
# module the applet imports (nix/frontends.nix). Outside it, a NixOS Plasma
# system profile carries them all.
#
# Deliberately sets no shell options: this file is sourced.

if [[ -z "${WYE_PLASMA_QML_PATH:-}" && -d /run/current-system/sw/lib/qt-6/qml ]]; then
    WYE_PLASMA_QML_PATH=/run/current-system/sw/lib/qt-6/qml
fi
if [[ -z "${WYE_PLASMA_QML_PATH:-}" ]]; then
    echo "no Plasma QML modules found; run this inside 'nix develop'." >&2
    exit 1
fi
export WYE_PLASMA_QML_PATH
export QML_IMPORT_PATH="${WYE_PLASMA_QML_PATH}"
export QML2_IMPORT_PATH="${WYE_PLASMA_QML_PATH}"
