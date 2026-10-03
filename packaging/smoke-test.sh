#!/usr/bin/env bash
# Smoke-test an installed Wye package (.deb, .rpm) inside a clean container.
#
# Distribution-agnostic: the wrapper (packaging/deb/test.sh, the .rpm one)
# installs the package plus the test-only tools this script needs, then
# runs it as root in the container:
#
#   packaging/smoke-test.sh [OUT_DIR]
#
# Test-only tools: Xvfb, desktop-file-validate, dbus-run-session, ldd and
# one of ImageMagick's `import`, `scrot` or `xwd` + `convert` for the
# screenshots. OUT_DIR (default $WYE_SMOKE_OUT, else /out) receives
# screenshots/wye-ui-settings.png, screenshots/wye-gtk-settings.png and the
# logs. PREFIX (default /usr) is where the package installed Wye.
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

check_libraries
check_cli
check_session_files
check_self_tests
check_live

if [ "$failures" -gt 0 ]; then
  echo "smoke-test: $failures check(s) failed; logs in $logs"
  exit 1
fi
echo "smoke-test: every check passed; screenshots in $shots"
