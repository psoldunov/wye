#!/usr/bin/env bash
# The demo stage behind Wye's screenshots and demo GIF (docs/media/kde/stage/README.md):
# a headless KWin and Plasma session with its own D-Bus bus, runtime directory
# and made-up home directory, running this checkout's Wye as `nix build` makes
# it. It does not read the real home directory and does not touch the running
# session.
#
#   stage.sh up [dark|light]       start it
#   stage.sh run CMD...            run a program inside the stage
#   stage.sh app DESKTOP-ID CMD... start an app in its own systemd scope, as
#                                  Plasma does, so Wye can tell where a link
#                                  came from
#   stage.sh down                  stop every stage process
#
# WYE_STAGE (default /tmp/wye-stage) is where the stage lives while it runs.
# WYE_STAGE_BLOCK lists extra D-Bus names to keep from starting in it.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../../../.." && pwd)
stage=${WYE_STAGE:-/tmp/wye-stage}
socket=wye-stage
# 1280×800 logical pixels at scale 2: every capture is in 2560×1600 device pixels.
width=1280
height=800
scale=2
# Services that would reach outside the stage: KDE Connect announces itself on
# the network, OBEX registers with BlueZ, Token Station is a tray app of its own.
block="org.kde.kdeconnect org.bluez.obex dev.soldunov.TokenStation ${WYE_STAGE_BLOCK:-}"

die() {
    echo "stage.sh: $*" >&2
    exit 1
}

# The environment of every process in the stage. WYE_STAGE_MARK is how `down`
# finds them.
enter() {
    export WYE_STAGE_MARK="$stage"
    export HOME="$stage/home"
    export XDG_CONFIG_HOME="$HOME/.config" XDG_DATA_HOME="$HOME/.local/share"
    export XDG_CACHE_HOME="$HOME/.cache" XDG_STATE_HOME="$HOME/.local/state"
    export XDG_RUNTIME_DIR="$stage/runtime"
    # The checkout's package comes first: its desktop entry, icons and D-Bus
    # activation files win over an installed Wye.
    case ${XDG_DATA_DIRS:-} in
        "$stage/wye/share:"*) ;;
        *) export XDG_DATA_DIRS="$stage/wye/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}" ;;
    esac
    case $PATH in
        "$stage/wye/bin:"*) ;;
        *) export PATH="$stage/wye/bin:$PATH" ;;
    esac
    # No portal backends, so nothing asks for permission in a session nobody watches.
    export XDG_DESKTOP_PORTAL_DIR="$stage/portals"
    export KWIN_SCREENSHOT_NO_PERMISSION_CHECKS=1 KWIN_WAYLAND_NO_PERMISSION_CHECKS=1
    export WAYLAND_DISPLAY="$socket" QT_QPA_PLATFORM=wayland XDG_SESSION_TYPE=wayland
    export XDG_CURRENT_DESKTOP=KDE KDE_FULL_SESSION=true KDE_SESSION_VERSION=6
    unset DISPLAY XDG_SESSION_ID DBUS_SESSION_BUS_ADDRESS
    if [[ -s $stage/bus ]]; then
        DBUS_SESSION_BUS_ADDRESS=$(<"$stage/bus")
        export DBUS_SESSION_BUS_ADDRESS
    fi
}

# PIDs of the stage's processes, never this script or its parents. KWin runs
# with capabilities, so its environment is not readable: it leaves its PID.
stage_pids() {
    local pid
    if [[ -s $stage/kwin.pid ]] && kill -0 "$(<"$stage/kwin.pid")" 2>/dev/null; then
        cat "$stage/kwin.pid"
    fi
    for pid in $(pgrep -u "$(id -u)" .); do
        [[ $pid == "$$" || $pid == "$PPID" ]] && continue
        if tr '\0' '\n' 2>/dev/null <"/proc/$pid/environ" | grep -qxF "WYE_STAGE_MARK=$stage"; then
            echo "$pid"
        fi
    done
}

# start NAME CMD...: run CMD detached, its output in log/NAME.log.
start() {
    local name=$1
    shift
    setsid -f "$@" >"$stage/log/$name.log" 2>&1 </dev/null
}

# wait_for WHAT CMD...: retry CMD for up to 30 s.
wait_for() {
    local what=$1
    shift
    for _ in $(seq 150); do
        "$@" >/dev/null 2>&1 && return 0
        sleep 0.2
    done
    die "$what did not come up; see $stage/log"
}

build() {
    local expr tool
    nix build "$repo#default" --out-link "$stage/wye"
    expr="(import $here/tools.nix {
      pkgs = (builtins.getFlake \"git+file://$repo\").inputs.nixpkgs.legacyPackages.\${builtins.currentSystem};
    })"
    for tool in python protocols gifski oxipng mesa; do
        nix build --impure --out-link "$stage/tools/$tool" --expr "$expr.$tool"
    done
    "$stage/tools/python/bin/python3" -m pywayland.scanner \
        -i "$stage/tools/protocols/share/plasma-wayland-protocols/fake-input.xml" \
        -o "$stage/proto" 2>/dev/null
}

prepare_home() {
    local home=$stage/home name
    cp -a "$here/home" "$home"
    # The history's times are seconds before the stage starts.
    "$stage/tools/python/bin/python3" - "$home/.local/state/wye/history.json" <<'EOF'
import json, sys, time
path = sys.argv[1]
with open(path, encoding="utf-8") as file:
    history = json.load(file)
now = int(time.time())
for entry in history["entries"]:
    entry["time"] += now
with open(path, "w", encoding="utf-8") as file:
    json.dump(history, file, indent=2, ensure_ascii=False)
EOF
    # D-Bus starts the UI host with the Mesa its Qt was built against (with the
    # host's, it finds no EGL), and nothing on the block list.
    local services=$home/.local/share/dbus-1/services mesa=$stage/tools/mesa
    mkdir -p "$services" "$home/.local/share/konsole"
    printf '[D-BUS Service]\nName=dev.soldunov.wye.Ui\nExec=%s %s %s %s %s\n' "$(type -P env)" \
        "__EGL_VENDOR_LIBRARY_FILENAMES=$mesa/share/glvnd/egl_vendor.d/50_mesa.json" \
        "LIBGL_DRIVERS_PATH=$mesa/lib/dri" "GBM_BACKENDS_PATH=$mesa/lib/gbm" \
        "$stage/wye/bin/wye-ui" >"$services/dev.soldunov.wye.Ui.service"
    for name in $block; do
        printf '[D-BUS Service]\nName=%s\nExec=%s\n' "$name" "$(type -P false)" >"$services/$name.service"
    done
    # Konsole opens links with a plain click and shows the demo's pull request.
    cat >"$home/.local/share/konsole/Demo.profile" <<EOF
[Appearance]
ColorScheme=Breeze
Font=Hack,12,-1,7,400,0,0,0,0,0,0,0,0,0,0,1

[General]
Command=$(command -v bash) --rcfile $home/.bashrc
Name=Demo
Parent=FALLBACK/
TerminalColumns=88
TerminalRows=14

[Interaction Options]
OpenLinksByDirectClickEnabled=true
UnderlineLinksEnabled=true

[Scrolling]
ScrollBarPosition=2
EOF
    printf '[Desktop Entry]\nDefaultProfile=Demo.profile\n' >"$home/.config/konsolerc"
}

apply_scheme() {
    local theme
    case $1 in
        dark) theme=org.kde.breezedark.desktop ;;
        light) theme=org.kde.breeze.desktop ;;
        *) die "unknown scheme $1 (dark or light)" ;;
    esac
    plasma-apply-lookandfeel --apply "$theme" >>"$stage/log/scheme.log" 2>&1
    # Plasma's default fonts, written out: an app whose platform theme falls
    # back to fontconfig would otherwise show the host's sans-serif.
    local key font
    for key in font menuFont toolBarFont smallestReadableFont fixed; do
        case $key in
            smallestReadableFont) font="Noto Sans,8" ;;
            fixed) font="Hack,10" ;;
            *) font="Noto Sans,10" ;;
        esac
        kwriteconfig6 --file kdeglobals --group General --key "$key" "$font,-1,5,400,0,0,0,0,0,0,0,0,0,0,1"
    done
    kwriteconfig6 --file kdeglobals --group WM --key activeFont "Noto Sans,10,-1,5,700,0,0,0,0,0,0,0,0,0,0,1"
    echo "$1" >"$stage/scheme"
}

up() {
    [[ -n $(stage_pids) ]] && die "a stage is already running in $stage; run: stage.sh down"
    if [[ -e $stage ]]; then
        [[ -f $stage/scheme || -d $stage/log ]] || die "$stage exists and is not a stage; set WYE_STAGE"
        rm -r "$stage"
    fi
    mkdir -p "$stage/log" "$stage/portals" "$stage/runtime" "$stage/tools"
    chmod 700 "$stage/runtime"
    build
    prepare_home

    enter
    # shellcheck disable=SC2016 # expanded by the inner shell
    start kwin dbus-run-session -- bash -c \
        'echo $$ >"$5" && printf %s "$DBUS_SESSION_BUS_ADDRESS" >"$1.part" && mv "$1.part" "$1" &&
         exec kwin_wayland --virtual --no-lockscreen --width "$2" --height "$3" --scale "$6" --socket "$4"' \
        _ "$stage/bus" "$width" "$height" "$socket" "$stage/kwin.pid" "$scale"
    wait_for "the stage's D-Bus bus" test -s "$stage/bus"
    enter
    wait_for KWin qdbus org.kde.KWin /KWin
    kscreen-doctor "output.1.scale.$scale" >>"$stage/log/kscreen.log" 2>&1
    apply_scheme "${1:-dark}"
    start kded kded6
    start plasmashell plasmashell --no-respawn
    wait_for "the Plasma shell" qdbus org.kde.plasmashell /PlasmaShell
    wye service --activate
    sleep 5
    echo "stage is up: WAYLAND_DISPLAY=$socket in $stage"
}

down() {
    local pids
    pids=$(stage_pids)
    [[ -z $pids ]] && return 0
    # shellcheck disable=SC2086 # one PID per word
    kill $pids 2>/dev/null || true
    sleep 1
    pids=$(stage_pids)
    # shellcheck disable=SC2086
    [[ -n $pids ]] && kill -KILL $pids 2>/dev/null
    return 0
}

app() {
    local id=$1 runtime=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
    shift
    [[ $runtime == "$stage/runtime" ]] && runtime=/run/user/$(id -u)
    enter
    setsid -f env XDG_RUNTIME_DIR="$runtime" DBUS_SESSION_BUS_ADDRESS="unix:path=$runtime/bus" \
        systemd-run --user --scope --quiet --collect --unit "app-$id-$RANDOM.scope" \
        env XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" DBUS_SESSION_BUS_ADDRESS="$DBUS_SESSION_BUS_ADDRESS" "$@" \
        >"$stage/log/$id.log" 2>&1 </dev/null
}

case ${1:-} in
    up) up "${2:-dark}" ;;
    down) down ;;
    run)
        shift
        enter
        exec "$@"
        ;;
    app)
        shift
        app "$@"
        ;;
    *)
        sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//' >&2
        exit 2
        ;;
esac
