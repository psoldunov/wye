#!/usr/bin/env bash
# The GNOME demo stage behind Wye's GNOME screenshots
# (docs/media/gnome/stage/README.md): a headless GNOME Shell with its own
# D-Bus bus, runtime directory and made-up home directory, running this
# checkout's Wye as `nix build` makes it. Every process runs with a clean
# environment; nothing reads the real home directory or touches the running
# session, except `app`, which asks your user manager (systemd-run --user)
# for a transient app-gnome-*.scope to run its command in.
#
#   stage.sh up [dark|light]       start it
#   stage.sh run CMD...            run a program inside the stage
#   stage.sh app DESKTOP-ID CMD... run CMD in an app-gnome-DESKTOP-ID scope,
#                                  as the Shell starts apps, so Wye can tell
#                                  where a link came from
#   stage.sh down                  stop every stage process
#
# WYE_GNOME_STAGE (default /tmp/wye-gnome-stage) is where the stage lives.
# WYE_PACKAGE uses a built Wye instead of `nix build .#default`;
# WYE_GTK_BIN runs another GTK host (a cargo build of wye-gtk);
# WYE_SHELL_EXTENSION installs the Shell extension from another directory
# than frontends/gnome-shell; WYE_GNOME_STAGE_BLOCK lists extra D-Bus names
# to keep from starting.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../../../.." && pwd)
stage=${WYE_GNOME_STAGE:-/tmp/wye-gnome-stage}
display=wye-gnome-stage
# 1280×800 logical pixels at scale 2: every capture is 2560×1600 device pixels.
width=1280
height=800
scale=2
wye_extension=wye@dev.soldunov
stage_extension=stage@wye.soldunov.dev
# The Qt UI host: on GNOME, Wye falls back to it when the GTK host or the
# Shell extension is missing, and a KDE window has no place in these shots.
# org.a11y.Bus would start a second accessibility bus beside the stage's.
block="dev.soldunov.wye.Ui org.a11y.Bus ${WYE_GNOME_STAGE_BLOCK:-}"
# The browsers the demo configuration shows; their desktop entries and icons
# come from the host (see prepare_browsers).
browsers="firefox google-chrome zen brave-browser"
# The apps the demo's links came from (history.json, the rules), copied the
# same way: Wye names a source app ("from Slack") only when it is installed.
# GNOME Terminal comes from tools.nix.
source_apps="slack org.telegram.desktop org.gnome.Fractal"

die() {
    echo "stage.sh: $*" >&2
    exit 1
}

# The whole environment of a stage process, in `stage_env`. WYE_GNOME_STAGE_MARK
# is how `down` finds them.
stage_env() {
    local tools=$stage/tools mesa=$stage/tools/mesa path=$PATH
    # Inside the stage already (stage.sh run stage.sh ...): the host's PATH
    # follows the stage's tools.
    case $path in
        "$stage/wye/bin:"*) path=${path#*":$tools/oxipng/bin:"} ;;
    esac
    stage_env=(
        "WYE_GNOME_STAGE=$stage"
        "WYE_GNOME_STAGE_MARK=$stage"
        # share/bin holds gnome-terminal: its desktop entry's TryExec must
        # resolve, or Wye does not count Terminal as installed and the
        # picker shows no "from Terminal".
        "PATH=$stage/wye/bin:$tools/shell/bin:$tools/dbus/bin:$tools/glib/bin:$tools/python/bin:$tools/magick/bin:$tools/share/bin:$tools/oxipng/bin:$path"
        "HOME=$stage/home"
        "XDG_CONFIG_HOME=$stage/home/.config"
        "XDG_DATA_HOME=$stage/home/.local/share"
        "XDG_CACHE_HOME=$stage/home/.cache"
        "XDG_STATE_HOME=$stage/home/.local/state"
        "XDG_RUNTIME_DIR=$stage/runtime"
        # The made-up browsers, the checkout's Wye, the GNOME data; nothing
        # from the host, so no host service starts on the stage's bus.
        "XDG_DATA_DIRS=$stage/browsers/share:$stage/wye/share:$tools/share/share:$tools/shell/share:$tools/portal/share"
        # Only the GTK backend's portals, and of them only Settings
        # (portals.conf in the home directory): nothing can ask for
        # permission in a session nobody watches.
        "XDG_DESKTOP_PORTAL_DIR=$tools/portal/share/xdg-desktop-portal/portals"
        "XDG_CONFIG_DIRS=$stage/xdg"
        "XDG_SESSION_TYPE=wayland"
        "XDG_SESSION_DESKTOP=gnome"
        "XDG_CURRENT_DESKTOP=GNOME"
        "DESKTOP_SESSION=gnome"
        "GSETTINGS_BACKEND=keyfile"
        "GSETTINGS_SCHEMA_DIR=$tools/schemas"
        "FONTCONFIG_FILE=$tools/fonts"
        "LANG=C.UTF-8"
        # The stage's system bus is an empty one of its own: the panel shows no
        # host network, battery or Bluetooth, and nothing reaches the host's
        # system services.
        "DBUS_SYSTEM_BUS_ADDRESS=unix:path=$stage/runtime/system-bus"
        # The accessibility bus, a plain dbus-daemon of the stage's own: the
        # one org.a11y.Bus starts uses dbus-broker, which needs systemd to
        # start the registry. ui.py finds GTK's widgets through it.
        "AT_SPI_BUS_ADDRESS=unix:path=$stage/runtime/a11y-bus"
        # The Mesa of the repository's nixpkgs: the host's may not load.
        "__EGL_VENDOR_LIBRARY_FILENAMES=$mesa/share/glvnd/egl_vendor.d/50_mesa.json"
        "LIBGL_DRIVERS_PATH=$mesa/lib/dri"
        "GBM_BACKENDS_PATH=$mesa/lib/gbm"
        "LD_LIBRARY_PATH=$mesa/lib"
    )
    # The stage's own time zone (see stage_clock), else the caller's TZ.
    if [[ -s $stage/tz ]]; then
        stage_env+=("TZ=$(<"$stage/tz")")
    elif [[ -n ${TZ:-} ]]; then
        stage_env+=("TZ=$TZ")
    fi
    if [[ -s $stage/bus ]]; then
        stage_env+=("DBUS_SESSION_BUS_ADDRESS=$(<"$stage/bus")" "WAYLAND_DISPLAY=$display")
    fi
}

in_stage() {
    stage_env
    env -i "${stage_env[@]}" "$@"
}

# PIDs of the stage's processes, never this script or its parents.
stage_pids() {
    local pid
    for pid in $(pgrep -u "$(id -u)" .); do
        [[ $pid == "$$" || $pid == "$PPID" ]] && continue
        if tr '\0' '\n' 2>/dev/null <"/proc/$pid/environ" | grep -qxF "WYE_GNOME_STAGE_MARK=$stage"; then
            echo "$pid"
        fi
    done
}

# start NAME CMD...: run CMD in the stage, detached, its output in log/NAME.log.
start() {
    local name=$1
    shift
    stage_env
    setsid -f env -i "${stage_env[@]}" "$@" >"$stage/log/$name.log" 2>&1 </dev/null
}

# wait_for SECONDS CMD...: retry CMD in the stage until it succeeds.
wait_for() {
    local seconds=$1
    shift
    for _ in $(seq $((seconds * 5))); do
        in_stage "$@" >/dev/null 2>&1 && return 0
        sleep 0.2
    done
    return 1
}

build() {
    local expr tool
    if [[ -n ${WYE_PACKAGE:-} ]]; then
        [[ -x $WYE_PACKAGE/bin/wye ]] || die "WYE_PACKAGE=$WYE_PACKAGE has no bin/wye"
        ln -s "$WYE_PACKAGE" "$stage/wye"
    else
        nix build "$repo#default" --out-link "$stage/wye"
    fi
    expr="(import $here/tools.nix {
      pkgs = (builtins.getFlake \"git+file://$repo\").inputs.nixpkgs.legacyPackages.\${builtins.currentSystem};
    })"
    for tool in shell dbus glib compile mesa oxipng magick python atspi schemas share portal fonts wallpaper; do
        nix build --impure --out-link "$stage/tools/$tool" --expr "$expr.$tool"
    done
}

# The demo's browsers and source apps, as the host has them installed: their
# desktop entries and hicolor icons, copied. A browser the host lacks is left
# out of the picker; a source app it lacks shows by no name.
prepare_browsers() {
    local share=$stage/browsers/share dirs dir name entry icon file found
    mkdir -p "$share/applications" "$share/icons"
    IFS=: read -ra dirs <<<"${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
    for name in $browsers $source_apps; do
        entry=
        for dir in "$HOME/.local/share" "${dirs[@]}"; do
            if [[ -f $dir/applications/$name.desktop ]]; then
                entry=$dir/applications/$name.desktop
                break
            fi
        done
        if [[ -z $entry ]]; then
            echo "stage.sh: no $name.desktop on this host; the stage goes without it" >&2
            continue
        fi
        cp -L "$entry" "$share/applications/"
        chmod u+w "$share/applications/$name.desktop"
        icon=$(sed -n 's/^Icon=//p' "$entry" | head -n 1)
        [[ -z $icon || $icon == /* ]] && continue
        found=
        for dir in "$HOME/.local/share" "${dirs[@]}"; do
            [[ -d $dir/icons/hicolor ]] || continue
            for file in "$dir"/icons/hicolor/*/apps/"$icon".{png,svg}; do
                [[ -e $file ]] || continue
                mkdir -p "$(dirname "$share/${file#"$dir"/}")"
                cp -L "$file" "$share/${file#"$dir"/}"
                found=1
            done
            [[ -n $found ]] && break
        done
        [[ -n $found ]] || echo "stage.sh: no icon $icon for $name on this host" >&2
    done
    chmod -R u+w "$share"
}

prepare_home() {
    local home=$stage/home services name gtk_bin source variant dir
    cp -a "$here/home" "$home"
    chmod -R u+w "$home"
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

    # D-Bus activation: the service and the GTK host from the package (or
    # WYE_GTK_BIN), and nothing on the block list. The home directory's
    # files win over the package's.
    services=$home/.local/share/dbus-1/services
    gtk_bin=${WYE_GTK_BIN:-$stage/wye/bin/wye-gtk}
    [[ -x $gtk_bin ]] || echo "stage.sh: no GTK host at $gtk_bin; Wye's windows will not open" >&2
    mkdir -p "$services"
    printf '[D-BUS Service]\nName=dev.soldunov.wye\nExec=%s service\n' "$stage/wye/bin/wye" \
        >"$services/dev.soldunov.wye.service"
    printf '[D-BUS Service]\nName=dev.soldunov.wye.Gtk\nExec=%s\n' "$gtk_bin" \
        >"$services/dev.soldunov.wye.Gtk.service"
    for name in $block; do
        printf '[D-BUS Service]\nName=%s\nExec=%s\n' "$name" "$(type -P false)" >"$services/$name.service"
    done

    # The Shell extensions: Wye's from the working tree (or
    # WYE_SHELL_EXTENSION), and the stage's own.
    source=${WYE_SHELL_EXTENSION:-$repo/frontends/gnome-shell}
    [[ -f $source/metadata.json ]] || die "no Shell extension in $source"
    mkdir -p "$home/.local/share/gnome-shell/extensions"
    cp -rL "$source" "$home/.local/share/gnome-shell/extensions/$wye_extension"
    cp -rL "$here/extension" "$home/.local/share/gnome-shell/extensions/$stage_extension"
    chmod -R u+w "$home/.local/share/gnome-shell"
    for dir in "$home/.local/share/gnome-shell/extensions"/*/schemas; do
        [[ -d $dir ]] || continue
        "$stage/tools/compile/bin/glib-compile-schemas" "$dir"
    done

    mkdir -p "$home/.config/xdg-desktop-portal"
    printf '[preferred]\ndefault=none\norg.freedesktop.impl.portal.Settings=gtk\n' \
        >"$home/.config/xdg-desktop-portal/portals.conf"

    # GSettings, as the keyfile backend reads them.
    mkdir -p "$home/.config/glib-2.0/settings"
    {
        echo "[org/gnome/shell]"
        echo "enabled-extensions=['$stage_extension', '$wye_extension']"
        echo "disable-user-extensions=false"
        echo "disable-extension-version-validation=true"
        echo "welcome-dialog-last-shown-version='999'"
        echo
        echo "[org/gnome/desktop/interface]"
        if [[ $scheme == dark ]]; then
            echo "color-scheme='prefer-dark'"
        else
            echo "color-scheme='prefer-light'"
        fi
        # Menus and windows appear at once, so a screenshot never catches one
        # half drawn.
        echo "enable-animations=false"
        # The stage's pointer starts in the corner: no overview from there.
        echo "enable-hot-corners=false"
        echo "clock-show-weekday=true"
        echo
        echo "[org/gnome/desktop/background]"
        for variant in l d; do
            name=picture-uri
            [[ $variant == d ]] && name=picture-uri-dark
            echo "$name='file://$stage/tools/wallpaper/adwaita-$variant.png'"
        done
        echo "picture-options='zoom'"
        echo
        echo "[org/gnome/desktop/screensaver]"
        echo "lock-enabled=false"
        echo
        echo "[org/gnome/desktop/session]"
        echo "idle-delay=uint32 0"
    } >"$home/.config/glib-2.0/settings/keyfile"
    mkdir -p "$stage/xdg"
}

# bus_config FILE [SERVICEDIRS]: a session bus configuration of the stage's
# own. The one dbus ships includes /etc/dbus-1, and with it the host's
# services (on NixOS, xdg-desktop-portal: its empty answers hide the colour
# scheme from libadwaita).
bus_config() {
    {
        echo '<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"'
        echo ' "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">'
        echo '<busconfig>'
        echo '  <type>session</type>'
        echo '  <keep_umask/>'
        echo "  <listen>unix:dir=$stage/runtime</listen>"
        echo '  <auth>EXTERNAL</auth>'
        [[ -n ${2:-} ]] && echo '  <standard_session_servicedirs/>'
        echo '  <policy context="default">'
        echo '    <allow send_destination="*" eavesdrop="true"/>'
        echo '    <allow eavesdrop="true"/>'
        echo '    <allow own="*"/>'
        echo '  </policy>'
        echo '</busconfig>'
    } >"$1"
}

# The demo history spans 21 hours before the stage starts. A fixed-offset time
# zone in which the stage starts at about 22:00 keeps all of it on one day,
# "Today", as the KDE gallery shows it, whatever the hour on the host.
# WYE_GNOME_STAGE_TZ overrides it.
stage_clock() {
    local utc offset
    if [[ -n ${WYE_GNOME_STAGE_TZ:-} ]]; then
        echo "$WYE_GNOME_STAGE_TZ" >"$stage/tz"
        return
    fi
    utc=$(date -u +%-H)
    # Hours east of UTC, in -12..11.
    offset=$(((22 - utc + 36) % 24 - 12))
    # POSIX TZ counts hours west of UTC.
    printf 'WYE%+d\n' $((-offset)) >"$stage/tz"
}

up() {
    scheme=${1:-dark}
    [[ $scheme == dark || $scheme == light ]] || die "unknown scheme $scheme (dark or light)"
    [[ -n $(stage_pids) ]] && die "a stage is already running in $stage; run: stage.sh down"
    if [[ -e $stage ]]; then
        [[ -f $stage/scheme || -d $stage/log ]] || die "$stage exists and is not a stage; set WYE_GNOME_STAGE"
        unmount
        rm -r "$stage"
    fi
    mkdir -p "$stage/log" "$stage/runtime" "$stage/tools"
    chmod 700 "$stage/runtime"
    stage_clock
    build
    prepare_browsers
    prepare_home

    bus_config "$stage/session-bus.conf" servicedirs
    bus_config "$stage/system-bus.conf"
    start system-bus dbus-daemon --config-file "$stage/system-bus.conf" --nofork --nopidfile \
        --address "unix:path=$stage/runtime/system-bus"
    start a11y-bus dbus-daemon --nofork --nopidfile \
        --config-file "$stage/tools/atspi/share/defaults/at-spi2/accessibility.conf" \
        --address "unix:path=$stage/runtime/a11y-bus"
    # shellcheck disable=SC2016 # expanded by the inner shell
    start shell dbus-run-session --config-file "$stage/session-bus.conf" -- bash -c \
        'printf %s "$DBUS_SESSION_BUS_ADDRESS" >"$1.part" && mv "$1.part" "$1" &&
         exec gnome-shell --wayland --headless --no-x11 --wayland-display "$2" --virtual-monitor "$3"' \
        _ "$stage/bus" "$display" "$((width * scale))x$((height * scale))"
    for _ in $(seq 150); do
        [[ -s $stage/bus ]] && break
        sleep 0.2
    done
    [[ -s $stage/bus ]] || die "the stage's D-Bus bus did not come up; see $stage/log/shell.log"
    wait_for 60 python3 "$here/shell.py" ready ||
        die "the Shell did not come up with the stage extension; see $stage/log/shell.log"
    in_stage python3 "$here/shell.py" scale "$scale"
    sleep 1
    in_stage python3 "$here/shell.py" clean
    echo "$scheme" >"$stage/scheme"
    wait_for 10 python3 "$here/shell.py" owner dev.soldunov.wye.Gnome ||
        echo "stage.sh: Wye's Shell extension owns no dev.soldunov.wye.Gnome; see $stage/log/shell.log" >&2
    start service wye service
    wait_for 20 python3 "$here/shell.py" owner dev.soldunov.wye ||
        die "the Wye service did not start; see $stage/log/service.log"
    sleep 2
    in_stage python3 "$here/shell.py" clean
    echo "stage is up ($scheme): WAYLAND_DISPLAY=$display in $stage"
}

# The document portal (which the portal frontend needs) mounts a FUSE file
# system in the stage's runtime directory; a killed one leaves it behind.
unmount() {
    if mountpoint -q "$stage/runtime/doc" 2>/dev/null; then
        fusermount3 -u "$stage/runtime/doc" 2>/dev/null || fusermount -u "$stage/runtime/doc" 2>/dev/null || true
    fi
}

down() {
    local pids
    pids=$(stage_pids)
    [[ -z $pids ]] && { unmount; return 0; }
    # shellcheck disable=SC2086 # one PID per word
    kill $pids 2>/dev/null || true
    for _ in $(seq 10); do
        sleep 0.3
        pids=$(stage_pids)
        [[ -z $pids ]] && break
    done
    # shellcheck disable=SC2086
    [[ -n $pids ]] && kill -KILL $pids 2>/dev/null
    unmount
    return 0
}

# The Shell starts an app in a systemd scope named after its desktop ID, and
# Wye reads the scope of the process that hands it a link. The scope lives in
# the host's user manager; the process inside it lives in the stage.
app() {
    local id=$1 runtime
    runtime=/run/user/$(id -u)
    shift
    stage_env
    setsid -f env -i PATH="$PATH" XDG_RUNTIME_DIR="$runtime" DBUS_SESSION_BUS_ADDRESS="unix:path=$runtime/bus" \
        systemd-run --user --scope --quiet --collect --unit "app-gnome-${id//-/\\x2d}-$RANDOM.scope" \
        env -i "${stage_env[@]}" "$@" >"$stage/log/$id.log" 2>&1 </dev/null
}

case ${1:-} in
    up) up "${2:-dark}" ;;
    down) down ;;
    run)
        shift
        stage_env
        exec env -i "${stage_env[@]}" "$@"
        ;;
    app)
        shift
        [[ $# -ge 2 ]] || die "usage: stage.sh app DESKTOP-ID CMD..."
        app "$@"
        ;;
    *)
        sed -n '2,21p' "$0" | sed 's/^# \{0,1\}//' >&2
        exit 2
        ;;
esac
