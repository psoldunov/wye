# Shared by capture.sh and demo.sh: driving a running demo stage (stage.sh up).
# shellcheck shell=bash disable=SC2034 # the variables are for the scripts that source this

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../../../.." && pwd)
stage=${WYE_STAGE:-/tmp/wye-stage}
[[ -s $stage/scheme ]] || {
    echo "$(basename "$0"): no stage in $stage; run: stage.sh up" >&2
    exit 1
}
scheme=$(<"$stage/scheme")
py=$stage/tools/python/bin/python3

# The stage's layout in logical pixels (stage.sh): the screen, the top of the
# panel, and the scale of every capture.
screen_width=1280
screen_height=800
panel_top=738
scale=2

run() { "$here/stage.sh" run "$@"; }
drive() { run "$py" "$here/drive.py" "$@"; }
shot() { run "$py" "$here/shot.py" "$@"; }
show() { run qdbus dev.soldunov.wye /dev/soldunov/wye dev.soldunov.wye1.ShowWindow "$1" "${2:-}" >/dev/null; }

# kwin JS: run a KWin script once.
kwin() {
    local script=$stage/script.js id
    printf '%s\n' "$1" >"$script"
    id=$(run qdbus org.kde.KWin /Scripting org.kde.kwin.Scripting.loadScript "$script" "stage-$RANDOM$RANDOM")
    run qdbus org.kde.KWin "/Scripting/Script$id" org.kde.kwin.Script.run >/dev/null
    sleep 0.3
    run qdbus org.kde.KWin "/Scripting/Script$id" org.kde.kwin.Script.stop >/dev/null
}

# each_window CAPTION-SUFFIX JS: run JS with `w` set to every window whose
# caption ends with CAPTION-SUFFIX.
each_window() {
    kwin "for (const w of workspace.stackingOrder) { if (w.caption.endsWith($("$py" -c 'import json, sys; print(json.dumps(sys.argv[1]))' "$1"))) { $2 } }"
}
activate() {
    each_window "$1" 'workspace.activeWindow = w; workspace.raiseWindow(w);'
    sleep 0.6
}
close_window() {
    each_window "$1" 'w.closeWindow();'
    sleep 0.6
}
# place CAPTION W H: give a window this size, centred above the panel.
place() {
    each_window "$1" "w.frameGeometry = {x: $(((screen_width - $2) / 2)), y: $(((panel_top - $3) / 2)), width: $2, height: $3};"
    sleep 0.6
}

# opaque_box IMAGE: "x y w h" of the visible part of a transparent capture.
opaque_box() {
    magick "$1" -alpha extract -trim -format '%X %Y %w %h' info: | tr -d +
}
