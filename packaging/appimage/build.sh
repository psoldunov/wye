#!/usr/bin/env bash
# Build the Wye AppImage in Docker.
#
# Builds the image of packaging/appimage/Dockerfile, copies the source tree
# into a container (tracked and untracked-but-not-ignored files, as a
# tarball), builds the four binaries there, assembles the AppDir with
# quick-sharun (every library, glibc and its loader included, Mesa, the QML
# modules and Qt plugins wye-ui loads, GTK's modules and data, the Adwaita
# icon theme; no media codecs, no /tmp path mapping) and packs it with the
# uruntime AppImage runtime:
#
#   dist/Wye-VERSION-ARCH.AppImage
#
#   packaging/appimage/build.sh
#
# ARCH is the machine's own architecture (`uname -m`), x86_64 or aarch64:
# Docker builds natively, without emulation. On aarch64 the image starts
# from Arch Linux ARM (see the Dockerfile).
#
# Run from anywhere; paths are resolved against the repository root. Nothing
# in the worktree is written by the container, and the worktree's target/ is
# not used: the cargo registry and the target directory live on the Docker
# volumes wye-appimage-cargo and wye-appimage-target, so a rebuild is
# incremental (`docker volume rm wye-appimage-cargo wye-appimage-target`
# starts clean). The AppImage's own AppRun.sh is packaging/appimage/AppRun.
#
# Environment:
#   DOCKER              the container CLI (default: docker; podman works too)
#   DOCKER_CONFIG       read by the docker CLI itself, e.g. an empty config
#                       when stale Docker Hub credentials refuse pulls
#   WYE_APPIMAGE_IMAGE  the build image tag (default: wye-appimage-build)
#   WYE_APPIMAGE_BASE   the image's base (default: archlinux:latest on
#                       x86_64, ghcr.io/pkgforge-dev/archlinux:aarch64 on
#                       aarch64)
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
docker=${DOCKER:-docker}
image=${WYE_APPIMAGE_IMAGE:-wye-appimage-build}
dist=$root/dist

# The workspace version, from [workspace.package] in Cargo.toml.
version=$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' "$root/Cargo.toml")
if [ -z "$version" ]; then
  echo "build.sh: no version in [workspace.package] of $root/Cargo.toml" >&2
  exit 1
fi
arch=$(uname -m)
case $arch in
  x86_64) base=${WYE_APPIMAGE_BASE:-archlinux:latest} ;;
  aarch64) base=${WYE_APPIMAGE_BASE:-ghcr.io/pkgforge-dev/archlinux:aarch64} ;;
  *)
    echo "build.sh: no AppImage build for $arch; x86_64 and aarch64 only" >&2
    exit 1
    ;;
esac
appimage=Wye-$version-$arch.AppImage

work=$(mktemp -d)
container=
cleanup() {
  if [ -n "$container" ]; then "$docker" rm -f "$container" >/dev/null 2>&1 || true; fi
  rm -rf "$work"
}
trap cleanup EXIT

echo "build.sh: building the image $image from $base"
"$docker" build -t "$image" --build-arg "BASE=$base" "$root/packaging/appimage"

# Source tarball: what git would see in a commit of the working tree now
# (tracked files that still exist plus untracked files that are not ignored),
# under wye-VERSION/. Their parent directories go in as entries of their own
# so their mtimes survive extraction: wye-ui's build script watches
# directories, and a directory that looks new reruns it (and rebuilds wye-ui
# and wye-gtk) on every build.
tarball=$work/wye-$version.tar.gz
(
  cd "$root"
  git ls-files -z --cached --others --exclude-standard |
    while IFS= read -r -d '' file; do
      if [ -f "$file" ] || [ -L "$file" ]; then printf '%s\0' "$file"; fi
    done >"$work/files"
  declare -A seen=()
  while IFS= read -r -d '' dir; do
    while [ "${dir%/*}" != "$dir" ]; do
      dir=${dir%/*}
      if [ -n "${seen[$dir]:-}" ]; then break; fi
      seen[$dir]=1
      printf '%s\0' "$dir"
    done
  done <"$work/files" >"$work/dirs"
  # Depth-first order (`/` sorts before every other byte), each directory
  # right before its contents: tar sets a directory's mtime once it has left
  # it, so a directory revisited later would get the extraction time. The S
  # flag leaves symlink targets alone: wye-gtk compiles some of wye-ui's
  # sources through relative symlinks.
  cat "$work/dirs" "$work/files" | tr '/' '\001' | LC_ALL=C sort -z | tr '\001' '/' |
    tar --null --no-recursion --files-from=- --transform "s|^|wye-$version/|S" \
      --owner=0 --group=0 --numeric-owner -czf "$tarball"
  rm "$work/files" "$work/dirs"
)

# Runs in the container as bash, from /build: $1 is the version, $2 the
# AppImage's file name.
build_in_container() {
  set -euo pipefail
  local version=$1 appimage=$2
  local src=/build/wye-$version stage=/build/stage appdir=/build/AppDir
  tar -C /build -xzf "/build/src/wye-$version.tar.gz"

  echo "build.sh: cargo build" >&2
  (cd "$src" && cargo build --release --locked \
    --package wye --package wye-native-host --package wye-ui --package wye-gtk)
  "$src/packaging/install.sh" --destdir "$stage" --prefix /usr \
    --target-dir "$CARGO_TARGET_DIR/release"
  # quick-sharun deploys programs from /usr best (it looks for their data
  # next to them), so the binaries go there.
  install -m755 "$stage"/usr/bin/* /usr/bin/

  # What wye-ui loads at run time beyond its ELF dependencies, named here so
  # quick-sharun deploys their libraries too: every QML module's plugin, and
  # the KDE Frameworks' Qt plugins (Kirigami's org.kde.desktop platform
  # plugin, which gives the desktop style its colours and icons,
  # KWindowSystem's X11 and Wayland back ends, Sonnet's spell checkers).
  # Qt's own platform, image format and Wayland plugins and GTK's modules
  # come with quick-sharun's DEPLOY_QT and DEPLOY_GTK. A plugin whose
  # libraries are not installed (Arch ships some QtQuick3D ones without
  # QtQuick3D, Sonnet's aspell back end without aspell) belongs to nothing
  # Wye uses.
  local extra=() plugin
  while IFS= read -r plugin; do
    if ! ldd "$plugin" | grep -q 'not found'; then extra+=("$plugin"); fi
  done < <(find /usr/lib/qt6/qml /usr/lib/qt6/plugins/kf6 -name '*.so' -type f)

  echo "build.sh: quick-sharun" >&2
  export APPDIR=$appdir OUTPATH=/build/dist OUTNAME=$appimage VERSION=$version
  export DESKTOP=$stage/usr/share/applications/dev.soldunov.wye.desktop
  export ICON=$stage/usr/share/icons/hicolor/scalable/apps/dev.soldunov.wye.svg
  export DEPLOY_QT=1 DEPLOY_QML=1 DEPLOY_GTK=1 DEPLOY_OPENGL=1
  export TMPDIR=/build/tmp
  mkdir -p "$TMPDIR"
  cp "/opt/appimage-tools/sharun+helper-libs-$(uname -m).tar" "$TMPDIR/"
  quick-sharun /usr/bin/wye /usr/bin/wye-native-host /usr/bin/wye-ui /usr/bin/wye-gtk \
    "${extra[@]}"

  # The four programs move to libexec/: sharun puts AppDir/bin first on the
  # PATH of everything it starts, and the service and `wye extension install`
  # look `wye` and `wye-native-host` up on PATH to write paths that outlive
  # this mount (the ~/.local/bin links AppRun makes). Each is a hard link to
  # sharun, which finds the AppDir through SHARUN_DIR (set by the AppRun).
  mkdir -p "$appdir/libexec"
  local program
  for program in wye wye-native-host wye-ui wye-gtk; do
    if [ ! "$appdir/bin/$program" -ef "$appdir/sharun" ]; then
      echo "build.sh: $appdir/bin/$program is not a hard link to sharun" >&2
      exit 1
    fi
    mv "$appdir/bin/$program" "$appdir/libexec/$program"
  done
  install -m755 "$src/packaging/appimage/AppRun" "$appdir/AppRun.sh"
  # quick-sharun copies the package's D-Bus service files, which name
  # /usr/bin; the AppRun writes the AppImage's own.
  rm -rf "$appdir/share/dbus-1"
  # What the AppImage leaves out, with every library only these pulled in:
  # - the plugins quick-sharun copied with Qt's plugin and QML trees whose
  #   libraries this system does not have either (Qt and Sonnet warn when
  #   one fails to load);
  # - Qt's GTK 3 platform theme, which quick-sharun keeps without GTK 3 to
  #   load the host's: host GTK 3 against the bundled glib and glibc. Qt
  #   falls back to its portal and generic themes;
  # - Qt Multimedia's FFmpeg plugin and glycin's HEIF loader: Wye plays no
  #   media and shows no HEIF images, and they bring FFmpeg, x264/x265, AV1
  #   and VPX.
  local copy drop=()
  while IFS= read -r copy; do
    plugin=/usr/lib/${copy#"$appdir/lib/"}
    if [ -f "$plugin" ] && ldd "$plugin" | grep -q 'not found'; then
      echo "build.sh: dropping ${copy#"$appdir/"}: $(ldd "$plugin" | grep -m 1 'not found' | tr -s ' \t' ' ')" >&2
      drop+=("$copy")
    fi
  done < <(find "$appdir/lib/qt6" -name '*.so' -type f)
  drop+=(
    "$appdir/lib/qt6/plugins/platformthemes/libqgtk3.so"
    "$appdir/lib/qt6/plugins/multimedia/libffmpegmediaplugin.so"
    "$appdir/shared/bin/glycin-heif"
  )
  drop_with_libraries "$appdir" "${drop[@]}"
  rm -f "$appdir/bin/glycin-heif" "$appdir"/share/glycin-loaders/*/conf.d/glycin-heif.conf
  rmdir "$appdir/lib/qt6/plugins/multimedia" 2>/dev/null || true
  # p11-kit, unpatched, loads its modules from the host's /usr/lib/pkcs11 as
  # set in its build, so the bundled trust module and the hook that feeds it
  # the host's CA bundle through ~/.config would only be dead weight.
  rm -rf "$appdir/lib/pkcs11" "$appdir/bin/01-check-ca-certs.hook"

  # Icon themes the windows name: Adwaita for wye-gtk. Kirigami's icons need
  # no copy of /usr/share/icons/breeze: the desktop style loads them through
  # KIconThemes, which links libKF6BreezeIcons, Breeze compiled in. Fonts
  # stay the host's.
  cp -a /usr/share/icons/Adwaita "$appdir/share/icons/"
  gtk-update-icon-cache -q -f -t "$appdir/share/icons/Adwaita" || true

  # The templates of the AppRun's session integration: the desktop entry
  # and D-Bus services as data/ has them, the icons and the GNOME Shell
  # extension as install.sh stages them. The extension's stamp changes with
  # its contents, so the AppRun replaces an older copy.
  local templates=$appdir/share/wye-appimage
  install -Dm644 "$src/data/applications/dev.soldunov.wye.desktop" \
    "$templates/applications/dev.soldunov.wye.desktop"
  mkdir -p "$templates/dbus-1/services" "$templates/icons" "$templates/gnome-shell/extensions"
  install -m644 "$src"/data/dbus/*.service.in "$templates/dbus-1/services/"
  cp -a "$stage/usr/share/icons/hicolor" "$templates/icons/"
  local extension=$templates/gnome-shell/extensions/wye@dev.soldunov
  cp -a "$stage/usr/share/gnome-shell/extensions/wye@dev.soldunov" "$extension"
  local stamp
  stamp=$(cd "$extension" && find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum |
    sha256sum | cut -d' ' -f1)
  echo "$stamp" >"$extension/.x-wye-appimage"

  check_appdir "$appdir"
  echo "build.sh: making the AppImage" >&2
  quick-sharun --make-appimage
}

# The ELF files under directory $1, one per line.
elf_files() {
  find "$1" -type f -size +4c -exec sh -c '
    for file; do
      if head -c 4 "$file" | grep -qa "^.ELF"; then echo "$file"; fi
    done' sh {} +
}

# "FILE<TAB>SONAME" for every NEEDED entry of the ELF files on stdin (FILE is
# empty when there is only one). readelf fails on the few files without a
# dynamic section, static helpers that need nothing.
needed_entries() {
  { xargs -r -d '\n' readelf -d -W 2>/dev/null || true; } | awk '
    /^File: / { file = substr($0, 7); next }
    /\(NEEDED\)/ { sub(/.*\[/, ""); sub(/\].*/, ""); print file "\t" $0 }'
}

# Delete files $2... from AppDir $1, then every library they pulled in that
# nothing left in the AppDir needs.
drop_with_libraries() {
  local appdir=$1 soname file real changed needed
  shift
  local drop=("$@") queue=() candidates=() visited=' '
  mapfile -t queue < <(printf '%s\n' "${drop[@]}" | needed_entries | cut -f2)
  while [ "${#queue[@]}" -gt 0 ]; do
    soname=${queue[0]}
    queue=("${queue[@]:1}")
    case $visited in *" $soname "*) continue ;; esac
    visited+="$soname "
    file=$appdir/lib/$soname
    [ -e "$file" ] || continue
    candidates+=("$soname")
    mapfile -t -O "${#queue[@]}" queue < <(readlink -f "$file" | needed_entries | cut -f2)
  done
  rm -f "${drop[@]}"
  changed=1
  while [ "$changed" = 1 ]; do
    changed=0
    needed=$(elf_files "$appdir" | needed_entries | cut -f2 | sort -u)
    for soname in "${candidates[@]}"; do
      file=$appdir/lib/$soname
      if [ ! -e "$file" ] || grep -qxF "$soname" <<<"$needed"; then continue; fi
      real=$(readlink -f "$file")
      echo "build.sh: dropping lib/$soname (only dropped files needed it)" >&2
      find "$appdir/lib" -maxdepth 1 -lname "${real##*/}" -delete
      rm -f "$file" "$real"
      changed=1
    done
  done
}

# Fail when AppDir $1 still has a /tmp path mapping, or a library one of its
# ELF files needs is not in it.
check_appdir() {
  local appdir=$1 have missing
  if compgen -G "$appdir/bin/*path-mapping*.hook" >/dev/null; then
    echo "build.sh: quick-sharun added a /tmp path mapping hook" >&2
    exit 1
  fi
  have=$(find "$appdir" \( -type f -o -type l \) -name '*.so*' -printf '%f\n')
  missing=$(elf_files "$appdir" | needed_entries |
    awk -F'\t' 'NR == FNR { have[$0]; next } !($2 in have) { print $1 " needs " $2 }' \
      <(echo "$have") -)
  if [ -n "$missing" ]; then
    echo "build.sh: libraries missing from the AppDir:" >&2
    echo "$missing" >&2
    exit 1
  fi
}

container=$("$docker" create \
  -v wye-appimage-cargo:/build/cargo-home \
  -v wye-appimage-target:/build/target \
  "$image" bash -c "$(declare -f build_in_container elf_files needed_entries drop_with_libraries check_appdir); build_in_container $(printf '%q %q' "$version" "$appimage")")
"$docker" cp "$work/." "$container:/build/src"

echo "build.sh: building wye $version"
"$docker" start -a "$container"

# `docker cp` out writes files owned by the user running it, never root.
mkdir -p "$dist"
"$docker" cp "$container:/build/dist/$appimage" "$work/$appimage"
mv -f "$work/$appimage" "$dist/$appimage"
chmod 755 "$dist/$appimage"
echo "build.sh: $dist/$appimage ($(du -h "$dist/$appimage" | cut -f1))"
