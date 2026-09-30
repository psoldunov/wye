#!/usr/bin/env bash
# Render a contact sheet of the Wye icons, as an icon theme lookup would pick them:
# the hand-tuned SVGs at 16, 24 and 32 px, the scalable master above. Top: 512 px on light
# and dark. Then, per background: every size at 1:1, and 16/24/32 plus the symbolic icon
# magnified with nearest-neighbour scaling.
#
# Usage: contact-sheet.sh [OUT_PNG] [HICOLOR_DIR]
# Needs rsvg-convert (librsvg) and magick (ImageMagick 7).
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
out=${1:-${TMPDIR:-/tmp}/wye-icon-sheet.png}
hicolor=${2:-$here/../hicolor}
light='#f6f5f4'
dark='#222226'
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

pick() {
  case $1 in
    16 | 24 | 32) echo "$hicolor/${1}x${1}/apps/dev.soldunov.wye.svg" ;;
    *) echo "$hicolor/scalable/apps/dev.soldunov.wye.svg" ;;
  esac
}

symbolic=$hicolor/symbolic/apps/dev.soldunov.wye-symbolic.svg
for size in 16 24 32 48 64 128 256 512; do
  rsvg-convert -w "$size" -h "$size" "$(pick "$size")" -o "$work/$size.png"
done
sed 's/#2e3436/#ffffff/' "$symbolic" >"$work/symbolic-white.svg"
for size in 16 128; do
  rsvg-convert -w "$size" -h "$size" "$symbolic" -o "$work/symbolic-dark-$size.png"
  rsvg-convert -w "$size" -h "$size" "$work/symbolic-white.svg" -o "$work/symbolic-light-$size.png"
done

# Every size at 1:1 on one background.
strip() {
  local bg=$1 cells=()
  for size in 16 24 32 48 64 128 256; do
    magick "$work/$size.png" -background "$bg" -gravity center \
      -extent "$((size + 32))x288" "$work/cell-$size.png"
    cells+=("$work/cell-$size.png")
  done
  magick "${cells[@]}" +append -background "$bg" -gravity center -extent 900x288 "$2"
}

# 16/24/32 and the 16 px symbolic icon magnified x6, then the symbolic icon at 128 px.
zoom() {
  local bg=$1 tone=$2 cells=()
  for size in 16 24 32; do
    magick "$work/$size.png" -background "$bg" -flatten -filter point \
      -resize "$((size * 6))x" -gravity center -extent 200x216 "$work/zoom-$size.png"
    cells+=("$work/zoom-$size.png")
  done
  magick "$work/symbolic-$tone-16.png" -background "$bg" -flatten -filter point -resize 96x \
    -gravity center -extent 150x216 "$work/zoom-symbolic.png"
  magick "$work/symbolic-$tone-128.png" -background "$bg" -flatten \
    -gravity center -extent 150x216 "$work/zoom-symbolic-large.png"
  magick "${cells[@]}" "$work/zoom-symbolic.png" "$work/zoom-symbolic-large.png" +append \
    -background "$bg" -gravity center -extent 900x216 "$3"
}

for bg in "$light" "$dark"; do
  magick "$work/512.png" -background "$bg" -flatten -resize 400x \
    -gravity center -extent 450x460 "$work/hero-${bg#\#}.png"
done
magick "$work/hero-${light#\#}.png" "$work/hero-${dark#\#}.png" +append "$work/hero.png"
strip "$light" "$work/strip-light.png"
strip "$dark" "$work/strip-dark.png"
zoom "$light" dark "$work/zoom-light.png"
zoom "$dark" light "$work/zoom-dark.png"
magick "$work/hero.png" "$work/strip-light.png" "$work/zoom-light.png" \
  "$work/strip-dark.png" "$work/zoom-dark.png" -append "$out"
echo "$out"
