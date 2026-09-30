#!/bin/sh
# Assemble the unpacked extension for one browser family: the shared files
# plus that family's manifest as manifest.json.
#
#   build.sh chromium OUT_DIR
#   build.sh firefox OUT_DIR
set -eu

family=${1:?usage: build.sh chromium|firefox OUT_DIR}
out=${2:?usage: build.sh chromium|firefox OUT_DIR}
here=$(cd "$(dirname "$0")" && pwd)

case "$family" in
chromium | firefox) ;;
*)
	echo "build.sh: unknown browser family '$family' (chromium or firefox)" >&2
	exit 2
	;;
esac

mkdir -p "$out/icons"
for file in background.js options.html options.js popup.html popup.js pages.css; do
	cp "$here/$file" "$out/$file"
done
cp "$here"/icons/*.png "$out/icons/"
cp "$here/manifest.$family.json" "$out/manifest.json"
