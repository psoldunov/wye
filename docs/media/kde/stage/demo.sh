#!/usr/bin/env bash
# Records the demo GIF at the top of the README from a running stage
# (stage.sh up dark): a link clicked in Konsole, the picker, and the page
# opening in a Firefox profile.
#
#   demo.sh [OUT.gif]
set -euo pipefail

# shellcheck source-path=SCRIPTDIR source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
out=${1:-$repo/docs/media/kde/demo.gif}
work=$stage/demo-$(date +%s)
mkdir -p "$work"
export WYE_DRIVE_LOG=$work/pointer.log

url=https://kde.org/announcements/plasma/6/6.7.0/
profile=Profiles/p7m4r8tc.research
# The part of the screen the GIF shows until the browser opens, and its width,
# in logical pixels.
area=(120 40 1040 650)
width=1040
# Konsole's size, and where its link is: the fifth line of the terminal, from
# the third column to the 48th.
konsole_w=892 konsole_h=342
konsole_x=$(((screen_width - konsole_w) / 2)) konsole_y=$(((panel_top - konsole_h) / 2))
link_y=$((konsole_y + 159))
link=($((konsole_x + 24)) $((link_y - 9)) $((konsole_x + 474)) $((link_y + 9)))

firefox_running() {
    local pid
    for pid in $(pgrep -f "firefox.*$profile" || true); do
        tr '\0' '\n' 2>/dev/null <"/proc/$pid/environ" | grep -qxF "WYE_STAGE_MARK=$stage" && return 0
    done
    return 1
}

# Open the page once beforehand, as in a browser that has been used before:
# the recording then shows it load, not a fresh profile download it.
echo "warming up Firefox"
run bash -c 'setsid -f firefox --profile "$HOME/.mozilla/firefox/$1" "$2" >/dev/null 2>&1 </dev/null' _ "$profile" "$url"
sleep 20
close_window "Mozilla Firefox"
for _ in $(seq 50); do
    firefox_running || break
    sleep 0.2
done
# Without the closed session, Firefox does not offer to restore it.
rm -f "$stage/home/.mozilla/firefox/$profile/sessionstore.jsonlz4"
rm -rf "$stage/home/.mozilla/firefox/$profile/sessionstore-backups"

"$here/stage.sh" app org.kde.konsole konsole
# Konsole takes its D-Bus name once its window is up.
for _ in $(seq 150); do
    run qdbus | grep -q org.kde.konsole && break
    sleep 0.2
done
sleep 1.5
place "— Konsole" "$konsole_w" "$konsole_h"
activate "— Konsole"
# Konsole shows its new size for a moment after a resize.
drive move:960,640 sleep:2000

echo "recording"
run "$py" "$here/shot.py" record "$work/frames" 17 20 &
recorder=$!
sleep 0.8
drive "glide:$((konsole_x + 440)),$link_y,1000" sleep:500 click sleep:1500
# Where the picker opened: hover its tiles from left to right, then pick the last.
shot window "$work/picker.png"
# shellcheck disable=SC2046 # four numbers
set -- $(opaque_box "$work/picker.png")
left=$(($1 / scale)) top=$(($2 / scale)) panel_w=$(($3 / scale))
# Six tiles and the overflow button; measured on the stage's large tiles.
pitch=$(((panel_w - 70) / 6))
steps=()
for tile in 0 1 2 3 4 5; do
    steps+=("glide:$((left + 74 + tile * pitch)),$((top + 56)),260" sleep:340)
done
drive "${steps[@]}" sleep:300 click sleep:1000 glide:1000,600,600
wait "$recorder"

echo "making the GIF"
for cursor in default pointer; do
    svg=$(run bash -c 'for dir in ${XDG_DATA_DIRS//:/ } /run/current-system/sw/share /usr/share; do
        ls "$dir/icons/breeze_cursors/cursors_scalable/$1/"*.svg 2>/dev/null && break; done | head -1' _ "$cursor")
    [[ -n $svg ]] || {
        echo "demo.sh: no Breeze cursor theme found" >&2
        exit 1
    }
    magick -background none -density 192 "$svg" "$work/cursor-$cursor.png"
done
"$py" "$here/demo.py" "$work/frames" "$WYE_DRIVE_LOG" "$out" \
    --area "${area[@]}" --link "${link[@]}" \
    --cursors "$work/cursor-default.png" "$work/cursor-pointer.png" \
    --width "$width" --gifski "$stage/tools/gifski/bin/gifski"
close_window "Mozilla Firefox"
close_window "— Konsole"
ls -l "$out"
