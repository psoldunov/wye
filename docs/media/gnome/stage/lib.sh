# Shared by the GNOME stage's scripts: driving a running stage (stage.sh up).
# shellcheck shell=bash disable=SC2034 # the variables are for the scripts that source this

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../../../.." && pwd)
stage=${WYE_GNOME_STAGE:-/tmp/wye-gnome-stage}
[[ -s $stage/scheme ]] || {
    echo "$(basename "$0"): no stage in $stage; run: stage.sh up" >&2
    exit 1
}
scheme=$(<"$stage/scheme")

# The stage's layout in logical pixels (stage.sh) and the scale of every
# capture.
screen_width=1280
screen_height=800
scale=2

# The GTK host's application ID (`adw::Application` in crates/wye-gtk, the
# same ID as Wye's desktop entry): its windows are found by it.
gtk_app=${WYE_GTK_APP_ID:-dev.soldunov.wye}

run() { "$here/stage.sh" run "$@"; }
magick() { "$stage/tools/magick/bin/magick" "$@"; }
py() { run python3 "$@"; }
drive() { py "$here/drive.py" "$@"; }
shot() { py "$here/shot.py" "$@"; }
shell() { py "$here/shell.py" "$@"; }
ui() { run env GI_TYPELIB_PATH="$stage/tools/atspi/lib/girepository-1.0" python3 "$here/ui.py" "$@"; }

# show WINDOW [ARGUMENT]: Windows1.ShowWindow through the service, as the
# tray and the CLI do.
show() {
    run gdbus call --session --dest dev.soldunov.wye --object-path /dev/soldunov/wye \
        --method dev.soldunov.wye1.ShowWindow "$1" "${2:-}" >/dev/null
}

# window TITLE [SECONDS]: wait for the newest window of the GTK host whose
# title matches the regular expression TITLE ("" for any); print its ID.
window() { shell wait "$gtk_app" "$1" "${2:-10}"; }
activate() {
    shell activate "$1"
    sleep 0.6
}
close_window() {
    shell close "$1"
    sleep 0.6
}
# place ID W H: give a window's frame this size, centred in the work area.
place() {
    shell place "$1" "$2" "$3"
    sleep 0.6
}
# clean: no overview, no notification, before a screenshot.
clean() { shell clean; }

# widget_click ID NAME [ROLE]: click a widget of window ID, found by its
# accessible name through AT-SPI: its action when it has one (a button),
# else the pointer on the middle of its box (a list row). NAME may list
# alternatives, "A|B": the first that shows wins.
widget_click() {
    local id=$1 names name frame box
    shift
    IFS='|' read -ra names <<<"$1"
    shift
    for name in "${names[@]}"; do
        ui press "$name" "$@" 2>/dev/null && return 0
    done
    for name in "${names[@]}"; do
        box=$(ui box "$name" "$@" 2>/dev/null) && break
    done
    if ! frame=$(shell frame "$id") || [[ -z $box ]]; then
        echo "$(basename "$0"): no widget '${names[*]}' in window $id" >&2
        return 1
    fi
    # shellcheck disable=SC2086 # four numbers each
    set -- $frame $box
    drive "glide:$(($1 + $5 + $7 / 2)),$(($2 + $6 + $8 / 2)),300" sleep:150 click
}

# shown_box: the box (device pixels) around the Shell actors that show now
# and did not at the last `shell mark`: the picker, a menu.
shown_box() {
    # shellcheck disable=SC2046 # four numbers
    set -- $(shell shown)
    echo $(($1 * scale)) $(($2 * scale)) $(($3 * scale)) $(($4 * scale))
}

# wait_shown [SECONDS]: wait until something new shows in the Shell.
wait_shown() {
    for _ in $(seq $((${1:-10} * 5))); do
        shell shown >/dev/null 2>&1 && return 0
        sleep 0.2
    done
    echo "$(basename "$0"): nothing new shows in the Shell" >&2
    return 1
}
