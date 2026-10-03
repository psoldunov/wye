#!/usr/bin/env bash
# Smoke-test an installed Wye package (.deb, .rpm) or the Wye AppImage inside
# a clean container.
#
# Distribution-agnostic: the wrapper (packaging/deb/test.sh, the .rpm one,
# packaging/appimage/test.sh) installs the package plus the test-only tools
# this script needs, then runs it as root in the container:
#
#   packaging/smoke-test.sh [OUT_DIR]
#   WYE_APPIMAGE=/path/to/Wye.AppImage packaging/smoke-test.sh [OUT_DIR]
#
# Test-only tools: Xvfb, desktop-file-validate, dbus-run-session, ldd and
# one of ImageMagick's `import`, `scrot` or `xwd` + `convert` for the
# screenshots. OUT_DIR (default $WYE_SMOKE_OUT, else /out) receives
# screenshots/wye-ui-settings.png, screenshots/wye-gtk-settings.png and the
# logs. PREFIX (default /usr) is where the package installed Wye.
#
# With WYE_APPIMAGE set, the programs run through the AppImage's multicall
# (links named wye, wye-ui… pointing at it, its first argument, its own
# name), the session files checked are the ones the AppImage writes under
# this test's $XDG_DATA_HOME and ~/.local/bin on its first launch, and the
# AppImage-only checks run too: files it did not write survive, a second
# launch rewrites nothing, the live session activates the UI hosts through
# the integrated D-Bus files, and --remove-integration removes exactly what
# it wrote. The container needs no FUSE when APPIMAGE_EXTRACT_AND_RUN=1 is
# exported.
#
# Every check prints PASS, FAIL or SKIP; the script exits 1 when any check
# fails.
set -uo pipefail

prefix=${PREFIX:-/usr}
bindir=$prefix/bin
out=${1:-${WYE_SMOKE_OUT:-/out}}
shots=$out/screenshots
logs=$out/logs
mkdir -p "$shots" "$logs"

failures=0
pass() { echo "PASS: $*"; }
fail() { echo "FAIL: $*"; failures=$((failures + 1)); }
skip() { echo "SKIP: $*"; }

# A private home, so nothing reads or writes the container's root home and
# every run starts from Wye's defaults.
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
export HOME=$scratch/home
export XDG_CONFIG_HOME=$HOME/.config XDG_DATA_HOME=$HOME/.local/share
export XDG_STATE_HOME=$HOME/.local/state XDG_CACHE_HOME=$HOME/.cache
export XDG_RUNTIME_DIR=$scratch/runtime
mkdir -p "$HOME" "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
unset WAYLAND_DISPLAY DBUS_SESSION_BUS_ADDRESS LC_ALL
# Container images default to the C locale, which Qt warns about (and the
# self-test fails on); desktop sessions run a UTF-8 one.
export LANG=C.UTF-8

binaries=(wye wye-native-host wye-ui wye-gtk)

# AppImage mode: the programs are links named after them, as the AppImage
# makes in ~/.local/bin, so each one runs through its multicall.
appimage=${WYE_APPIMAGE:-}
if [ -n "$appimage" ]; then
  bindir=$scratch/programs
  mkdir -p "$bindir"
  for binary in "${binaries[@]}"; do ln -s "$appimage" "$bindir/$binary"; done
fi

# a. Every shared library of every binary resolves.
check_libraries() {
  local binary missing
  for binary in "${binaries[@]}"; do
    if [ ! -x "$bindir/$binary" ]; then
      fail "ldd $binary: $bindir/$binary is not installed"
      continue
    fi
    missing=$(ldd "$bindir/$binary" 2>&1 | grep 'not found')
    if [ -n "$missing" ]; then
      fail "ldd $binary: $(echo "$missing" | tr -s ' \t\n' ' ')"
    else
      pass "ldd $binary: every library found"
    fi
  done
}

# b. The command-line tools start and stop on their own.
check_cli() {
  local version status
  if version=$("$bindir/wye" --version 2>&1); then
    pass "wye --version: $version"
  else
    fail "wye --version: $version"
  fi
  if "$bindir/wye" --help >"$logs/wye-help.txt" 2>&1; then
    pass "wye --help"
  else
    fail "wye --help: $(head -n 1 "$logs/wye-help.txt")"
  fi
  # With stdin closed the host must see end of input and exit, not hang.
  timeout 10 "$bindir/wye-native-host" chrome-extension://smoke-test/ \
    </dev/null >"$logs/native-host.txt" 2>&1
  status=$?
  if [ "$status" -eq 0 ]; then
    pass "wye-native-host exits on closed stdin"
  else
    fail "wye-native-host on closed stdin: exit $status, $(head -n 1 "$logs/native-host.txt")"
  fi
  if "$bindir/wye-native-host" --remove >"$logs/native-host-remove.txt" 2>&1; then
    pass "wye-native-host --remove"
  else
    fail "wye-native-host --remove: $(head -n 1 "$logs/native-host-remove.txt")"
  fi
}

# c. The desktop entry validates; the D-Bus services and the systemd user
# units name installed binaries.
check_session_files() {
  local entry=$prefix/share/applications/dev.soldunov.wye.desktop file key exe
  if [ ! -f "$entry" ]; then
    fail "desktop entry: $entry is not installed"
  elif desktop-file-validate "$entry" >"$logs/desktop-file-validate.txt" 2>&1; then
    pass "desktop-file-validate $entry"
  else
    fail "desktop-file-validate: $(head -n 1 "$logs/desktop-file-validate.txt")"
  fi
  for file in \
    "$prefix/share/dbus-1/services/dev.soldunov.wye.service:Exec" \
    "$prefix/share/dbus-1/services/dev.soldunov.wye.Ui.service:Exec" \
    "$prefix/share/dbus-1/services/dev.soldunov.wye.Gtk.service:Exec" \
    "$prefix/lib/systemd/user/wye.service:ExecStart" \
    "$prefix/lib/systemd/user/wye-ui.service:ExecStart" \
    "$prefix/lib/systemd/user/wye-gtk.service:ExecStart"; do
    key=${file##*:}
    file=${file%:*}
    if [ ! -f "$file" ]; then
      fail "$file is not installed"
      continue
    fi
    exe=$(sed -n "s/^$key=\([^ ]*\).*/\1/p" "$file")
    case $exe in
      "$bindir"/*)
        if [ -x "$exe" ]; then
          pass "$file: $key=$exe"
        else
          fail "$file: $key=$exe is not an installed binary"
        fi
        ;;
      *) fail "$file: $key=${exe:-<none>} is not in $bindir" ;;
    esac
  done
  # Launch at login is each user's switch (GEN-01, an XDG autostart entry):
  # the package must not enable a user unit for everyone.
  local enabled
  enabled=$(find /etc/systemd/user -name 'wye*.service' 2>/dev/null)
  if [ -n "$enabled" ]; then
    fail "user unit enabled for every user: $(echo "$enabled" | head -n 1)"
  else
    pass "no user unit enabled for every user"
  fi
}

# d. The self-tests compiled into the UI hosts: wye-ui loads every surface
# offscreen, wye-gtk on its own Xvfb; either fails on any warning.
check_self_tests() {
  local host
  for host in wye-ui wye-gtk; do
    if timeout 600 "$bindir/$host" --self-test >"$logs/$host-self-test.txt" 2>&1; then
      pass "$host --self-test"
    else
      fail "$host --self-test: $(grep -m 1 '^FAIL' "$logs/$host-self-test.txt" ||
        tail -n 1 "$logs/$host-self-test.txt")"
      # The first message behind it, for the summary line.
      grep -m 1 -E '^\s+(Warning|Critical|Fatal)' "$logs/$host-self-test.txt" | sed 's/^\s*/      /'
    fi
  done
}

# Whether a process named $1 runs.
running() {
  local comm
  for comm in /proc/[0-9]*/comm; do
    [ "$(cat "$comm" 2>/dev/null)" = "$1" ] && return 0
  done
  return 1
}

# Kill every process named one of $@ (activated processes outlive their
# private bus).
stop() {
  local comm name
  for comm in /proc/[0-9]*/comm; do
    name=$(cat "$comm" 2>/dev/null) || continue
    for wanted in "$@"; do
      if [ "$name" = "$wanted" ]; then
        comm=${comm%/comm}
        kill "${comm#/proc/}" 2>/dev/null
      fi
    done
  done
}

# Save the X root window as PNG $1.
screenshot() {
  if command -v import >/dev/null; then
    import -window root "$1"
  elif command -v scrot >/dev/null; then
    scrot --overwrite "$1"
  elif command -v xwd >/dev/null && command -v convert >/dev/null; then
    xwd -root -silent | convert xwd:- "$1"
  else
    echo "no screenshot tool (import, scrot, xwd + convert)" >&2
    return 1
  fi
}

# Lines of a host's output that mean a QML module, a platform plugin or a
# window failed to load.
load_errors() {
  grep -E 'is not installed|module ".*" .*not|QQmlApplicationEngine failed|Could not load the Qt platform plugin|could not be loaded|Failed to load|Cannot open display|cannot open display|CRITICAL' "$1"
}

# The steps of a live session, run on its private bus (as their own bash):
# host $1 must be up 8 s after `wye settings`, screenshot to $2.
live_steps() {
  "$bindir/wye" service --activate || { echo "LIVE: service activation failed"; return 1; }
  echo "LIVE: service activated"
  "$bindir/wye" config check || echo "LIVE: config check failed"
  "$bindir/wye" browsers || echo "LIVE: browsers failed"
  "$bindir/wye" settings || { echo "LIVE: wye settings failed"; return 1; }
  sleep 8
  if running "$1"; then echo "LIVE: $1 running"; fi
  if screenshot "$2"; then echo "LIVE: screenshot saved"; fi
}

# e. Live launch in a private session bus on an Xvfb display: the bus
# activates the installed service, `wye settings` makes the service activate
# the frontend $1 (`kde`: wye-ui, `gnome`: wye-gtk), and the window must
# still be up a few seconds later, with no load errors in the output.
live_session() {
  local frontend=$1 host=$2 log=$logs/live-$2.txt shot=$shots/$2-settings.png display=:${3}
  mkdir -p "$XDG_CONFIG_HOME/wye"
  printf '[advanced]\nfrontend = "%s"\n' "$frontend" >"$XDG_CONFIG_HOME/wye/config.toml"

  Xvfb "$display" -screen 0 1600x1000x24 -nolisten tcp >"$logs/xvfb-$host.txt" 2>&1 &
  local xvfb=$!
  local tries=0
  while [ ! -S "/tmp/.X11-unix/X${display#:}" ] && [ "$tries" -lt 50 ]; do
    sleep 0.2
    tries=$((tries + 1))
  done
  if [ ! -S "/tmp/.X11-unix/X${display#:}" ]; then
    fail "live $host: Xvfb $display did not start ($(tail -n 1 "$logs/xvfb-$host.txt"))"
    kill "$xvfb" 2>/dev/null
    return
  fi

  # The inner script runs on the private bus; every activated process (the
  # service, the UI host) inherits the bus daemon's stderr, so all of it
  # lands in $log.
  DISPLAY=$display bindir=$bindir \
    dbus-run-session -- bash -c "$(declare -f running screenshot live_steps); live_steps $(printf '%q %q' "$host" "$shot")" \
    >"$log" 2>&1
  local status=$?
  stop wye wye-ui wye-gtk
  kill "$xvfb" 2>/dev/null
  wait "$xvfb" 2>/dev/null

  local errors
  errors=$(load_errors "$log")
  if [ "$status" -ne 0 ]; then
    fail "live $host: $(grep -m 1 '^LIVE: .*failed' "$log" || tail -n 1 "$log")"
  elif ! grep -q "^LIVE: $host running" "$log"; then
    fail "live $host: not running 8 s after wye settings ($(grep -m 1 -iE 'error|panic' "$log" || tail -n 1 "$log"))"
  elif [ -n "$errors" ]; then
    fail "live $host: $(echo "$errors" | head -n 1)"
  else
    pass "live $host: service activated, Settings window up ($frontend frontend)"
  fi
  if ! grep -q '^LIVE: screenshot saved' "$log"; then
    fail "live $host: no screenshot ($(tail -n 1 "$log"))"
  elif blank "$shot"; then
    # A host can stay up and draw nothing (GTK without libGLESv2 on Xvfb).
    fail "live $host: screenshot $shot is one colour, nothing was drawn"
  else
    pass "live $host: screenshot $shot"
  fi
}

# Whether PNG $1 has a single colour, so nothing was drawn. Without
# ImageMagick's `identify` it cannot tell and answers no.
blank() {
  local colours
  command -v identify >/dev/null || return 1
  colours=$(identify -format %k "$1" 2>/dev/null) || return 1
  [ "$colours" -le 1 ]
}

check_live() {
  live_session kde wye-ui 91
  live_session gnome wye-gtk 92
}

# AppImage a. The files the AppImage writes on its first launch.
integrated_files() {
  printf '%s\n' \
    "$XDG_DATA_HOME/applications/dev.soldunov.wye.desktop" \
    "$XDG_DATA_HOME/dbus-1/services/dev.soldunov.wye.service" \
    "$XDG_DATA_HOME/dbus-1/services/dev.soldunov.wye.Ui.service" \
    "$XDG_DATA_HOME/dbus-1/services/dev.soldunov.wye.Gtk.service" \
    "$XDG_DATA_HOME/icons/hicolor/24x24/apps/dev.soldunov.wye.svg" \
    "$XDG_DATA_HOME/icons/hicolor/32x32/apps/dev.soldunov.wye.svg" \
    "$XDG_DATA_HOME/icons/hicolor/symbolic/apps/dev.soldunov.wye-symbolic.svg" \
    "$XDG_DATA_HOME/icons/hicolor/symbolic/apps/dev.soldunov.wye-picker-symbolic.svg" \
    "$XDG_DATA_HOME/gnome-shell/extensions/wye@dev.soldunov/metadata.json" \
    "$HOME/.local/bin/wye" \
    "$HOME/.local/bin/wye-native-host"
}

# Files planted where the AppImage would write: a symlink into a store, as
# home-manager makes, and a plain file.
foreign_link=$XDG_DATA_HOME/icons/hicolor/scalable/apps/dev.soldunov.wye.svg
foreign_link_target=$scratch/nix/store/00000000000000000000000000000000-home-manager-files/dev.soldunov.wye.svg
foreign_file=$XDG_DATA_HOME/icons/hicolor/16x16/apps/dev.soldunov.wye.svg
foreign_content='<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"/>'

plant_foreign() {
  mkdir -p "${foreign_link%/*}" "${foreign_file%/*}" "${foreign_link_target%/*}"
  echo "$foreign_content" >"$foreign_link_target"
  rm -f "$foreign_link" "$foreign_file"
  ln -s "$foreign_link_target" "$foreign_link"
  echo "$foreign_content" >"$foreign_file"
}

# Whether both planted files are as they were planted.
foreign_intact() {
  [ "$(readlink "$foreign_link")" = "$foreign_link_target" ] &&
    [ "$(cat "$foreign_file" 2>/dev/null)" = "$foreign_content" ]
}

# AppImage b. The first launch integrates the AppImage and leaves the planted
# files alone; a second one rewrites nothing. The planted files go away
# afterwards (the AppImage then writes its own icons there on its next
# launch) so the other checks see a clean session, and come back for
# check_appimage_removal.
check_appimage_launch() {
  local version file missing=() changed
  plant_foreign

  if version=$("$appimage" --version 2>"$logs/first-launch.txt"); then
    pass "first launch, $appimage --version: $version"
  else
    fail "first launch, $appimage --version: $(tail -n 1 "$logs/first-launch.txt")"
  fi
  while IFS= read -r file; do
    if [ ! -e "$file" ]; then missing+=("$file"); fi
  done < <(integrated_files)
  if [ "${#missing[@]}" -eq 0 ]; then
    pass "first launch integrates the AppImage ($(integrated_files | wc -l) files and links)"
  else
    fail "first launch did not write ${missing[*]}"
  fi
  if foreign_intact && grep -qF "$foreign_link" "$logs/first-launch.txt" &&
    grep -qF "$foreign_file" "$logs/first-launch.txt"; then
    pass "files the AppImage did not write are left alone, with a warning: $(grep -m 1 'left alone' "$logs/first-launch.txt")"
  else
    fail "a planted file was changed or not reported ($(tail -n 1 "$logs/first-launch.txt"))"
  fi

  touch "$scratch/before-second-launch"
  sleep 1.1
  "$appimage" --version >/dev/null 2>"$logs/second-launch.txt"
  changed=$(find "$XDG_DATA_HOME" "$HOME/.local/bin" \( -type f -o -type l \) \
    -newer "$scratch/before-second-launch" 2>/dev/null)
  if [ -z "$changed" ]; then
    pass "second launch rewrites nothing"
  else
    fail "second launch rewrote $(echo "$changed" | tr '\n' ' ')"
  fi
  rm -f "$foreign_link" "$foreign_file"
}

# AppImage c. Every library of every program comes from the AppImage: its
# own dynamic loader lists them, with the library path sharun gives it and
# without the host's ld.so.cache.
check_appimage_libraries() {
  local extract=$scratch/extract appdir loader libpath binary listing outside
  mkdir -p "$extract"
  file -L "$appimage" >"$logs/appimage-file.txt" 2>&1
  if ! (cd "$extract" && "$appimage" --appimage-extract >"$logs/appimage-extract.txt" 2>&1); then
    fail "--appimage-extract: $(tail -n 1 "$logs/appimage-extract.txt")"
    return
  fi
  appdir=$(dirname "$(find "$extract" -maxdepth 2 -name AppRun.sh -print -quit)")
  # ld-linux-x86-64.so.2 or ld-linux-aarch64.so.1.
  loader=$(find "$appdir/lib" "$appdir/shared/lib" -maxdepth 1 -name 'ld-linux-*.so.[0-9]' -print -quit 2>/dev/null)
  if [ -z "$loader" ]; then
    fail "the AppImage has no dynamic loader ld-linux-*.so ($(head -n 1 "$logs/appimage-file.txt"))"
    return
  fi
  libpath=$(dirname "$loader")
  # lib.path has no newline after its last entry.
  while IFS= read -r dir || [ -n "$dir" ]; do
    libpath=$libpath:$(dirname "$loader")/${dir#+}
  done <"$(dirname "$loader")/lib.path"
  for binary in "${binaries[@]}"; do
    listing=$("$loader" --inhibit-cache --library-path "$libpath" --list "$appdir/shared/bin/$binary" 2>&1)
    echo "$listing" >"$logs/libraries-$binary.txt"
    outside=$(echo "$listing" | grep -E 'not found|=> /' | grep -vF "=> $appdir/")
    if [ -n "$outside" ]; then
      fail "libraries of $binary: $(echo "$outside" | head -n 1 | tr -s ' \t' ' ')"
    else
      pass "libraries of $binary: all $(echo "$listing" | grep -c '=>') from the AppImage"
    fi
  done
}

# AppImage d. The integrated desktop entry validates and runs the AppImage;
# the D-Bus services run it without SystemdService=; the links point at it;
# no other Wye D-Bus service file exists, so the live session can only
# activate through these.
check_appimage_session_files() {
  local entry=$XDG_DATA_HOME/applications/dev.soldunov.wye.desktop file program exec link
  if desktop-file-validate "$entry" >"$logs/desktop-file-validate.txt" 2>&1; then
    pass "desktop-file-validate $entry"
  else
    fail "desktop-file-validate: $(head -n 1 "$logs/desktop-file-validate.txt")"
  fi
  local wrong=
  for exec in "TryExec=$appimage" "Exec=\"$appimage\" open %U" \
    "Exec=\"$appimage\" settings" "Exec=\"$appimage\" clipboard"; do
    grep -qxF "$exec" "$entry" || wrong="$wrong${wrong:+, }no '$exec'"
  done
  if [ -z "$wrong" ]; then
    pass "desktop entry: TryExec and the three Exec lines run \"$appimage\""
  else
    fail "desktop entry: $wrong"
  fi
  for file in dev.soldunov.wye.service:service dev.soldunov.wye.Ui.service:wye-ui dev.soldunov.wye.Gtk.service:wye-gtk; do
    program=${file#*:}
    file=$XDG_DATA_HOME/dbus-1/services/${file%:*}
    exec=$(grep '^Exec=' "$file" 2>/dev/null)
    if [ "$exec" != "Exec='$appimage' $program" ]; then
      fail "$file: ${exec:-no Exec}"
    elif grep -q '^SystemdService=' "$file"; then
      fail "$file names a systemd unit: $(grep '^SystemdService=' "$file")"
    else
      pass "$file: $exec, no SystemdService="
    fi
  done
  for link in "$HOME/.local/bin/wye" "$HOME/.local/bin/wye-native-host"; do
    if [ "$(readlink "$link")" = "$appimage" ]; then
      pass "$link -> $appimage"
    else
      fail "$link -> $(readlink "$link" || echo nothing)"
    fi
  done
  local others
  others=$(find /usr/share/dbus-1/services /usr/local/share/dbus-1/services \
    -name 'dev.soldunov.wye*' 2>/dev/null)
  if [ -z "$others" ]; then
    pass "no Wye D-Bus service file outside \$XDG_DATA_HOME"
  else
    fail "Wye D-Bus service file outside \$XDG_DATA_HOME: $(echo "$others" | head -n 1)"
  fi
}

# AppImage e. --remove-integration removes everything the AppImage wrote and
# nothing else: the planted files are back where it wrote its own icons
# (a user who installs Wye with home-manager later), and must survive.
check_appimage_removal() {
  local file left=()
  plant_foreign
  if ! "$appimage" --remove-integration >"$logs/remove-integration.txt" 2>&1; then
    fail "--remove-integration: $(tail -n 1 "$logs/remove-integration.txt")"
    return
  fi
  while IFS= read -r file; do
    if [ -e "$file" ] || [ -L "$file" ]; then left+=("$file"); fi
  done < <(integrated_files)
  if [ "${#left[@]}" -ne 0 ]; then
    fail "--remove-integration left ${left[*]}"
  elif ! foreign_intact; then
    fail "--remove-integration touched a file it did not write"
  else
    pass "--remove-integration removed the $(grep -c '^removed ' "$logs/remove-integration.txt") files it wrote, not the planted ones"
  fi
}

if [ -n "$appimage" ]; then
  check_appimage_launch
  check_appimage_libraries
  check_cli
  check_appimage_session_files
  check_self_tests
  check_live
  check_appimage_removal
else
  check_libraries
  check_cli
  check_session_files
  check_self_tests
  check_live
fi

if [ "$failures" -gt 0 ]; then
  echo "smoke-test: $failures check(s) failed; logs in $logs"
  exit 1
fi
echo "smoke-test: every check passed; screenshots in $shots"
