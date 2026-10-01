#!/usr/bin/env bash
# Render real GTK windows in the Fedora 42 GNOME image, with Qt's fixture corpus.
set -euo pipefail
cd "$(dirname "$0")/../.."
output="${1:-docs/media/gnome/screenshots/gtk}"
mkdir -p "$output/light" "$output/dark"
for scheme in light dark; do
    docker run --rm --init \
        -v "$PWD:/workspace" -w /workspace -e GDK_BACKEND=x11 -e GDK_SCALE=2 \
        -e GSK_RENDERER=cairo -e PYTHONPATH=/workspace/frontends/gtk \
        wye-gnome:42 sh -ec '
            # Browser apps are absent in this image; use the same illustrative
            # icon fixtures as the Shell capture for legible history rows.
            cp /workspace/tests/gnome/icons/*.svg /usr/share/icons/hicolor/scalable/apps/
            gtk-update-icon-cache -q -f /usr/share/icons/hicolor
            Xvfb :99 -screen 0 2000x1600x24 -nolisten tcp >/tmp/wye-xvfb.log 2>&1 &
            export DISPLAY=:99
            sleep 1
            dbus-run-session -- python3 -m wye_gtk --self-test --scheme "$1" --snapshots "$2/$1"
        ' sh "$scheme" "/workspace/$output"
done
printf 'GTK light and dark captures: %s/{light,dark}/\n' "$output"
