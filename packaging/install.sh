#!/usr/bin/env bash
# Stage an installed Wye tree for the distribution packages (.deb, .rpm).
#
# Mirrors the postInstall of nix/package.nix: the same files, with the
# binaries' absolute path written into the desktop entry, the D-Bus service
# files and the systemd user units. Keep the two in step.
#
#   packaging/install.sh [--destdir DIR] [--prefix /usr] [--target-dir target/release]
#
# Run from anywhere; paths are resolved against the repository root. The four
# binaries must already be built (cargo build --release --locked --package wye
# --package wye-native-host --package wye-ui --package wye-gtk).
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
destdir=
prefix=/usr
target_dir=$root/target/release

while [ "$#" -gt 0 ]; do
  case $1 in
    --destdir) destdir=$2; shift 2 ;;
    --prefix) prefix=$2; shift 2 ;;
    --target-dir) target_dir=$2; shift 2 ;;
    *) echo "install.sh: unknown argument: $1" >&2; exit 2 ;;
  esac
done

case $prefix in
  /*) ;;
  *) echo "install.sh: --prefix must be absolute: $prefix" >&2; exit 2 ;;
esac

bindir=$prefix/bin
out=$destdir$prefix

# Binaries.
for binary in wye wye-native-host wye-ui wye-gtk; do
  if [ ! -x "$target_dir/$binary" ]; then
    echo "install.sh: $target_dir/$binary is missing; build it first" >&2
    exit 1
  fi
  install -Dm755 "$target_dir/$binary" "$out/bin/$binary"
done

# Desktop entry: the source entry runs `wye` from $PATH; the installed one
# names the installed binary (main Exec and every action's), and TryExec in
# the main group alone hides the entry once the binary is gone.
entry=$out/share/applications/dev.soldunov.wye.desktop
install -Dm644 "$root/data/applications/dev.soldunov.wye.desktop" "$entry"
sed -i \
  -e "s|^Exec=wye open %U\$|Exec=$bindir/wye open %U|" \
  -e "s|^Exec=wye settings\$|Exec=$bindir/wye settings|" \
  -e "s|^Exec=wye clipboard\$|Exec=$bindir/wye clipboard|" \
  "$entry"
for exec in "open %U" settings clipboard; do
  grep -qxF "Exec=$bindir/wye $exec" "$entry" || {
    echo "install.sh: desktop entry has no 'Exec=wye $exec' line to rewrite" >&2
    exit 1
  }
done
sed -i "/^\[Desktop Entry\]\$/a TryExec=$bindir/wye" "$entry"

# Icons, one line each, so data/icons/src (the generators) is never installed.
icons=$root/data/icons/hicolor
for icon in \
  scalable/apps/dev.soldunov.wye.svg \
  16x16/apps/dev.soldunov.wye.svg \
  24x24/apps/dev.soldunov.wye.svg \
  32x32/apps/dev.soldunov.wye.svg \
  symbolic/apps/dev.soldunov.wye-symbolic.svg \
  symbolic/apps/dev.soldunov.wye-picker-symbolic.svg; do
  install -Dm644 "$icons/$icon" "$out/share/icons/hicolor/$icon"
done

# GNOME Shell extension. Installing it does not enable it: each user chooses
# whether Shell owns the picker and the tray. Everything ships except its
# README and its tests (test-*.mjs).
extension=$out/share/gnome-shell/extensions/wye@dev.soldunov
mkdir -p "$extension"
cp -r "$root/frontends/gnome-shell/." "$extension/"
rm -f "$extension/README.md" "$extension"/test-*.mjs
find "$extension" -type d -exec chmod 755 {} +
find "$extension" -type f -exec chmod 644 {} +

# D-Bus activation (DEF-04) and the systemd user units of the service and the
# two UI hosts, with the absolute path of the installed binaries. Distribution
# packages put user units in lib/systemd/user.
for template in \
  dbus/dev.soldunov.wye.service.in:share/dbus-1/services/dev.soldunov.wye.service \
  dbus/dev.soldunov.wye.Ui.service.in:share/dbus-1/services/dev.soldunov.wye.Ui.service \
  dbus/dev.soldunov.wye.Gtk.service.in:share/dbus-1/services/dev.soldunov.wye.Gtk.service \
  systemd/wye.service.in:lib/systemd/user/wye.service \
  systemd/wye-ui.service.in:lib/systemd/user/wye-ui.service \
  systemd/wye-gtk.service.in:lib/systemd/user/wye-gtk.service; do
  target=$out/${template#*:}
  install -Dm644 "$root/data/${template%%:*}" "$target"
  sed -i "s|@bindir@|$bindir|g" "$target"
done

# The licence and README are the package format's business (debian/copyright,
# %license), so they are not staged here.
echo "install.sh: staged Wye in ${destdir:-/} with prefix $prefix"
