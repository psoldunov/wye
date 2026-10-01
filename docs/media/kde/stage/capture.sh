#!/usr/bin/env bash
# Takes Wye's screenshots from a running demo stage (stage.sh up) into
# docs/media/kde/screenshots/<scheme>/: one PNG per surface, in device pixels
# (twice the logical size). Windows keep their frame and shadow on a
# transparent background. The picker and the tray menu blur what is behind
# them, so they are cut out of the screen with some wallpaper around them.
#
#   capture.sh [STEP...]
#
# The steps are picker, tray, settings_pages, rule_editor, rule_tester and
# windows; all of them by default. WYE_CAPTURE_OUT overrides the directory.
set -euo pipefail

# shellcheck source-path=SCRIPTDIR source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
out=${WYE_CAPTURE_OUT:-$repo/docs/media/kde/screenshots/$scheme}
work=$stage/capture
mkdir -p "$out" "$work"

# Wallpaper kept around a cut-out, in logical pixels.
pad=48
settings="Wye"

saved() { echo "  $out/$1.png"; }

# window_shot NAME CAPTION: the window with its frame and shadow.
window_shot() {
    activate "$2"
    shot window "$out/$1.png"
    saved "$1"
}

# cut NAME SCREEN X Y W H: cut a box (device pixels, grown by the padding and
# kept on the screen) out of a screenshot.
cut() {
    local name=$1 screen=$2 p=$((pad * scale)) width height left top right bottom
    width=$(magick identify -format '%w' "$screen")
    height=$(magick identify -format '%h' "$screen")
    left=$(($3 - p < 0 ? 0 : $3 - p))
    top=$(($4 - p < 0 ? 0 : $4 - p))
    right=$(($3 + $5 + p > width ? width : $3 + $5 + p))
    bottom=$(($4 + $6 + p > height ? height : $4 + $6 + p))
    magick "$screen" -crop "$((right - left))x$((bottom - top))+$left+$top" +repage "$out/$name.png"
    saved "$name"
}

# union X Y W H X Y W H: the box around two boxes.
union() {
    local left=$(($1 < $5 ? $1 : $5)) top=$(($2 < $6 ? $2 : $6))
    local right=$(($1 + $3 > $5 + $7 ? $1 + $3 : $5 + $7)) bottom=$(($2 + $4 > $6 + $8 ? $2 + $4 : $6 + $8))
    echo "$left $top $((right - left)) $((bottom - top))"
}

picker() {
    local box more tile
    # The link as Konsole would hand it over (the picker shows where it came
    # from), with the pointer over the wallpaper so the blur has something to show.
    drive move:900,700 glide:640,330,400 sleep:300
    "$here/stage.sh" app org.kde.konsole wye open https://github.com/psoldunov/wye/pull/7
    sleep 2
    shot screen "$work/picker.png"
    shot window "$work/picker-surface.png"
    # shellcheck disable=SC2046 # four numbers
    set -- $(opaque_box "$work/picker-surface.png")
    box="$*"
    # shellcheck disable=SC2086
    cut picker "$work/picker.png" $box

    # A right click on the first tile: what that browser can do with the link.
    tile="move:$(($1 / scale + 70)),$(($2 / scale + 60))"
    drive "$tile" sleep:300 rclick sleep:1200
    shot screen "$work/picker-tile.png"
    # shellcheck disable=SC2046,SC2086
    cut picker-tile-menu "$work/picker-tile.png" $(union $box $(shot changed "$work/picker.png" "$work/picker-tile.png" 0 $((panel_top * scale))))
    drive key:esc sleep:600

    # The overflow menu and its Open In: everything else Wye can open the link in.
    drive move:640,700 key:menu sleep:1000 key:down sleep:200 key:right sleep:1200
    shot screen "$work/picker-more.png"
    # shellcheck disable=SC2046,SC2086
    more=$(union $box $(shot changed "$work/picker.png" "$work/picker-more.png" 0 $((panel_top * scale))))
    # shellcheck disable=SC2086
    cut picker-more "$work/picker-more.png" $more
    drive key:esc sleep:400 key:esc sleep:400 key:esc sleep:800
}

tray() {
    # The Wye icon is the first in the system tray once the clipboard holds a
    # link (Plasma's clipboard icon then joins it).
    local tray_icon=1017,769 x y
    run bash -c 'wl-copy "https://www.youtube.com/watch?v=dQw4w9WgXcQ&utm_source=newsletter"'
    drive move:640,400 sleep:800
    shot screen "$work/tray-before.png"
    drive "glide:$tray_icon,400" sleep:150 click sleep:1200
    shot screen "$work/tray.png"
    # shellcheck disable=SC2046 # four numbers
    set -- $(shot changed "$work/tray-before.png" "$work/tray.png")
    x=$1 y=$2
    # From the menu to the bottom-right corner, so the tray icon shows too.
    cut tray-menu "$work/tray.png" "$x" "$y" $((screen_width * scale - x)) $((screen_height * scale - y))
    # Hover More: its submenu opens to the left.
    drive "glide:$((x / scale + 60)),$((y / scale + 317)),400" sleep:300 \
        "glide:$((x / scale + 90)),$((y / scale + 317)),200" sleep:1500
    shot screen "$work/tray-more.png"
    # shellcheck disable=SC2046
    set -- $(shot changed "$work/tray-before.png" "$work/tray-more.png")
    cut tray-more "$work/tray-more.png" "$1" "$2" $((screen_width * scale - $1)) $((screen_height * scale - $2))
    drive key:esc sleep:200 key:esc sleep:200 key:esc sleep:600
}

settings_pages() {
    local page
    for page in general browsers apps picker rules extras advanced; do
        run wye settings "$page"
        sleep 2
        place "— $settings" 780 720
        window_shot "settings-$page" "— $settings"
    done
}

rule_editor() {
    run wye settings rules
    sleep 1.5
    place "— $settings" 780 720
    activate "— $settings"
    # The first rule's row: Work links from Slack.
    drive glide:330,140,300 sleep:100 click sleep:1500
    window_shot rule-editor "— $settings"
    drive key:esc sleep:800
}

rule_tester() {
    show test-rules
    sleep 1.5
    activate "— $settings"
    drive "type:https://www.youtube.com/watch?v=dQw4w9WgXcQ&utm_source=newsletter" sleep:3000
    window_shot rule-tester "— $settings"
    drive key:esc sleep:800
    close_window "— $settings"
}

windows() {
    show script-editor global
    sleep 2
    place "Transform Script — Global — Wye" 680 548
    activate "Transform Script — Global — Wye"
    # A link the script changes, typed into the Test field.
    drive glide:680,512,300 sleep:300 click sleep:400 down:ctrl key:a up:ctrl sleep:200 \
        "type:https://www.reddit.com/r/kde/comments/1fx2k9q/plasma_67_is_out/" sleep:2000
    window_shot script-editor "Transform Script — Global — Wye"
    close_window "Transform Script — Global — Wye"

    show history
    sleep 2
    window_shot history "History — Wye"
    close_window "History — Wye"

    show about
    sleep 2
    window_shot about "About Wye"
    close_window "About Wye"

    show first-run
    sleep 2
    window_shot first-run "Welcome to Wye"
    # Get Started, Continue: the page that chooses the browsers.
    place "Welcome to Wye" 540 496
    drive glide:850,593,300 click sleep:1000 click sleep:1200
    window_shot first-run-browsers "Welcome to Wye"
    close_window "Welcome to Wye"
}

steps=("$@")
[[ ${#steps[@]} -gt 0 ]] || steps=(picker tray settings_pages rule_editor rule_tester windows)
echo "capturing the $scheme scheme into $out"
for step in "${steps[@]}"; do
    "$step"
done
# Lossless: the screenshots stay pixel for pixel what the stage drew.
"$stage/tools/oxipng/bin/oxipng" -o 4 --strip safe -q "$out"/*.png
