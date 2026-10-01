#!/usr/bin/env bash
# Takes Wye's GNOME screenshots from a running GNOME stage (stage.sh up) into
# docs/media/gnome/screenshots/<scheme>/: one PNG per surface, in device
# pixels (twice the logical size). GTK windows keep their own shadow and
# rounded corners on a transparent background. The picker and the tray menu
# are Shell surfaces: they are cut out of the screen with some wallpaper
# around them.
#
#   capture.sh [STEP...]
#
# The steps are picker, tray, settings_pages, rule_editor, rule_tester,
# windows and sheets; all of them by default. WYE_SHOTS overrides the
# directory.
set -euo pipefail

# shellcheck source-path=SCRIPTDIR source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
out=${WYE_SHOTS:-$repo/docs/media/gnome/screenshots/$scheme}
work=$stage/capture
mkdir -p "$out" "$work"

# Wallpaper kept around a cut-out, in logical pixels.
pad=48

# What the steps look for, by the text the surfaces show. The picker and the
# tray are the Shell extension's; the rest are the GTK host's accessible
# names (its labels; buttons are looked up with the role "button", so a row
# with the same title does not get the click). Change them here when a
# surface renames them.
link=https://github.com/psoldunov/wye/pull/7
first_tile=Firefox                  # the first browser of the demo configuration
picker_more="More targets"          # the picker's "⋯" button (its accessible name)
picker_open_in="Open In"            # the page of its menu with every other target
tray_indicator=wye@dev.soldunov     # the extension's key in Main.panel.statusArea
tray_more=More                      # the tray page with the rest
tray_recent="Recent Links"          # a page of More
first_rule="Work links from Slack"  # the first rule of the demo configuration
# A row's button is named "Label: Row title" (older hosts: by its label or by
# its row's title; "A|B": either).
shown_browsers="Choose: Shown browsers|Choose…|Shown browsers"  # the Browsers page's button to its sheet
picker_keys="Customize: Picker keys|Customize…|Picker keys"     # the Picker page's button to its sheet
url_expansion="Configure: Expand redirect and short URLs|Configure…|Expand redirect and short URLs"  # the Advanced page's button to its sheet
get_started="Get Started"           # the first-run window's next buttons
continue_button="Continue"

# Window titles (regular expressions) the GTK host gives its windows; ""
# takes the host's newest window. Settings is titled after its page; the
# rule tester is a sheet inside it. By title, not by age: a window the host
# already had is reused, so the newest one may be another.
title_settings="^(General|Browsers|Apps|Picker|Rules|Extras|Advanced)$"
title_tester=$title_settings
title_script="^Transform Script"
title_history="^History$"
title_about="^About"
title_first_run="^Welcome to Wye$"

# Window sizes (logical, frame without the shadow), as the KDE stage uses.
settings_size="780 720"
script_size="680 548"
first_run_size="540 496"

saved() { echo "  $out/$1.png"; }

# window_shot NAME ID: the window as GTK drew it, shadow included.
window_shot() {
    activate "$2"
    clean
    shot window "$2" "$out/$1.png"
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

# tray_cut NAME SCREEN X Y W H: from the menu's box up to the top of the
# screen and right to its edge, so the panel and the indicator show too.
tray_cut() {
    local name=$1 screen=$2 left=$(($3 - pad * scale)) bottom=$(($4 + $6 + pad * scale))
    left=$((left < 0 ? 0 : left))
    bottom=$((bottom > screen_height * scale ? screen_height * scale : bottom))
    magick "$screen" -crop "$((screen_width * scale - left))x$bottom+$left+0" +repage "$out/$name.png"
    saved "$name"
}

# union X Y W H X Y W H: the box around two boxes.
union() {
    local left=$(($1 < $5 ? $1 : $5)) top=$(($2 < $6 ? $2 : $6))
    local right=$(($1 + $3 > $5 + $7 ? $1 + $3 : $5 + $7)) bottom=$(($2 + $4 > $6 + $8 ? $2 + $4 : $6 + $8))
    echo "$left $top $((right - left)) $((bottom - top))"
}

# settings_window PAGE: open Settings at PAGE, sized and focused; print its ID.
settings_window() {
    local id
    run wye settings "$1" >/dev/null
    id=$(window "$title_settings")
    # Twice: a window placed while GTK still sizes its first frame keeps
    # GTK's height.
    # shellcheck disable=SC2086 # two numbers
    place "$id" $settings_size >/dev/null
    sleep 0.4
    # shellcheck disable=SC2086
    place "$id" $settings_size >/dev/null
    activate "$id"
    echo "$id"
}

# open_picker: hand the link over as GNOME Terminal would (the picker shows
# where it came from); print the picker's box in device pixels.
open_picker() {
    shell mark
    "$here/stage.sh" app org.gnome.Terminal wye open "$link"
    wait_shown
    sleep 1
    shown_box
}

picker() {
    local box
    clean
    # The pointer away from the picker's middle.
    drive move:900,700 glide:640,330,300 sleep:200
    box=$(open_picker)
    shot screen "$work/picker.png"
    # shellcheck disable=SC2086 # four numbers
    cut picker "$work/picker.png" $box

    # A right click on the first tile: what that browser can do with the link.
    drive "to:$first_tile" sleep:300 rclick sleep:1000
    shot screen "$work/picker-tile.png"
    # shellcheck disable=SC2046,SC2086
    cut picker-tile-menu "$work/picker-tile.png" $(union $box $(shown_box))
    drive key:esc sleep:600
    # Escape may close the picker with the menu: then open it again.
    if ! shell text "$picker_more" >/dev/null 2>&1; then
        drive move:640,330
        box=$(open_picker)
    fi

    # The "⋯" menu at its Open In page: everything else Wye can open the
    # link in. The pointer comes up from below the button, not across
    # the tiles, so no tile is left selected by hover.
    # shellcheck disable=SC2046 # four numbers
    set -- $(shell text "$picker_more")
    drive "move:$(($1 + $3 / 2)),$(($2 + $4 + 160))" sleep:200 "to:$picker_more" sleep:200 click sleep:800 \
        "to:$picker_open_in" sleep:200 click sleep:1000
    shot screen "$work/picker-more.png"
    # shellcheck disable=SC2046,SC2086
    cut picker-more "$work/picker-more.png" $(union $box $(shown_box))
    drive key:esc sleep:400 key:esc sleep:400 key:esc sleep:600
    clean
}

tray() {
    local box
    clean
    shell clipboard "https://www.youtube.com/watch?v=dQw4w9WgXcQ&utm_source=newsletter"
    drive move:640,400 sleep:500
    # shellcheck disable=SC2046 # four numbers
    set -- $(shell indicator "$tray_indicator")
    shell mark
    drive "glide:$(($1 + $3 / 2)),$(($2 + $4 / 2)),400" sleep:150 click
    wait_shown
    sleep 0.8
    shot screen "$work/tray.png"
    box=$(shown_box)
    # shellcheck disable=SC2086 # four numbers
    tray_cut tray-menu "$work/tray.png" $box

    # More and then Recent Links: pages that replace the menu's content, so
    # each is cut to the menu as it shows then.
    # After each click the pointer leaves the menu (the wallpaper on its
    # left), so no row is left lit by hover.
    drive "to:$tray_more,400" sleep:300 click sleep:600 glide:200,600,300 sleep:600
    shot screen "$work/tray-more.png"
    # shellcheck disable=SC2046
    tray_cut tray-more "$work/tray-more.png" $(shown_box)

    drive "to:$tray_recent,400" sleep:300 click sleep:600 glide:200,600,300 sleep:600
    shot screen "$work/tray-recent.png"
    # shellcheck disable=SC2046
    tray_cut tray-recent "$work/tray-recent.png" $(shown_box)
    drive key:esc sleep:200 key:esc sleep:200 key:esc sleep:200 key:esc sleep:600
    clean
}

settings_pages() {
    local page id
    for page in general browsers apps picker rules extras advanced; do
        id=$(settings_window "$page")
        sleep 1
        window_shot "settings-$page" "$id"
    done
}

rule_editor() {
    local id
    id=$(settings_window rules)
    sleep 1
    widget_click "$id" "$first_rule"
    sleep 1.5
    # The editor is a sheet (AdwDialog) inside Settings.
    window_shot rule-editor "$id"
    drive key:esc sleep:800
}

rule_tester() {
    local id
    show test-rules
    id=$(window "$title_tester")
    activate "$id"
    drive "type:https://www.youtube.com/watch?v=dQw4w9WgXcQ&utm_source=newsletter" sleep:3000
    window_shot rule-tester "$id"
    close_window "$id"
}

windows() {
    local id
    show script-editor global
    id=$(window "$title_script")
    # shellcheck disable=SC2086 # two numbers
    place "$id" $script_size >/dev/null
    sleep 1
    # The test link is the last opened one, a Reddit link the script changes.
    window_shot script-editor "$id"
    close_window "$id"

    show history
    id=$(window "$title_history")
    sleep 1.5
    window_shot history "$id"
    close_window "$id"

    show about
    id=$(window "$title_about")
    sleep 1.5
    window_shot about "$id"
    close_window "$id"

    # The GTK host quits once its last window closes; a window asked for
    # while it shuts down never opens, so ask again once D-Bus restarts it.
    show first-run
    if ! id=$(window "$title_first_run" 5 2>/dev/null); then
        sleep 2
        show first-run
        id=$(window "$title_first_run")
    fi
    # shellcheck disable=SC2086 # two numbers
    place "$id" $first_run_size >/dev/null
    sleep 1.5
    window_shot first-run "$id"
    # Get Started, Continue: the page that chooses the browsers.
    widget_click "$id" "$get_started" button
    sleep 1
    widget_click "$id" "$continue_button" button
    sleep 1.2
    window_shot first-run-browsers "$id"
    close_window "$id"
}

# The settings sheets: AdwDialogs inside Settings.
sheets() {
    local page button name id
    for page in browsers:shown-browsers picker:picker-keys advanced:url-expansion; do
        name=${page#*:}
        page=${page%%:*}
        case $name in
            shown-browsers) button=$shown_browsers ;;
            picker-keys) button=$picker_keys ;;
            url-expansion) button=$url_expansion ;;
        esac
        id=$(settings_window "$page")
        sleep 1
        widget_click "$id" "$button" button
        sleep 1.5
        window_shot "$name" "$id"
        drive key:esc sleep:800
    done
    close_window "$(window "$title_settings")"
}

steps=("$@")
[[ ${#steps[@]} -gt 0 ]] || steps=(picker tray settings_pages rule_editor rule_tester windows sheets)
echo "capturing the $scheme scheme into $out"
for step in "${steps[@]}"; do
    "$step"
done
# Lossless: the screenshots stay pixel for pixel what the stage drew.
"$stage/tools/oxipng/bin/oxipng" -o 4 --strip safe -q "$out"/*.png
