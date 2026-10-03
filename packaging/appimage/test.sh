#!/usr/bin/env bash
# Smoke-test the Wye AppImage in fresh containers of the supported Debian
# and Fedora releases.
#
# For each image, starts a clean container, installs only the test tools of
# packaging/smoke-test.sh (Xvfb, xauth, dbus, desktop-file-utils,
# ImageMagick, a font, `file`) and the CA certificates every desktop has,
# removes the Mesa packages Xvfb pulls in, and
# checks that no Qt, GTK, KDE or Mesa library is left: everything Wye needs
# must come from the AppImage. Then it copies the AppImage to a directory
# whose name has a space (the integration has to quote it) and runs
# packaging/smoke-test.sh in AppImage mode. Docker has no FUSE, so the
# AppImage runs with APPIMAGE_EXTRACT_AND_RUN=1 (uruntime extracts it to
# TMPDIR); a desktop mounts it with FUSE instead.
#
#   packaging/appimage/test.sh [APPIMAGE]
#
# APPIMAGE defaults to dist/Wye-VERSION-x86_64.AppImage (build.sh writes
# it). Screenshots land in dist/screenshots/appimage-DISTRO-*.png and the
# logs in dist/smoke-logs/appimage-DISTRO/ (DISTRO is the image tag with `:`
# as `-`, e.g. debian-12), owned by the calling user. Exits 1 when any
# distribution fails.
#
# Environment: DOCKER (default docker; podman works too), DOCKER_CONFIG
# (read by the docker CLI itself), WYE_APPIMAGE_TEST_IMAGES (default
# "debian:12 debian:13 fedora:43 fedora:44").
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
docker=${DOCKER:-docker}
read -r -a images <<<"${WYE_APPIMAGE_TEST_IMAGES:-debian:12 debian:13 fedora:43 fedora:44}"
dist=$root/dist

version=$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' "$root/Cargo.toml")
appimage=${1:-$dist/Wye-$version-x86_64.AppImage}
if [ ! -f "$appimage" ]; then
  echo "test.sh: no $appimage; run packaging/appimage/build.sh first or pass one" >&2
  exit 1
fi

# Runs in the container: the AppImage and the smoke test arrive as a tar on
# stdin, the screenshots and logs leave as a tar on stdout; everything else
# goes to stderr.
test_in_container() {
  set -uo pipefail
  local status=0 mesa toolkit installed="/opt/Wye AppImage"
  mkdir -p /test /out "$installed"
  tar -C /test -x -f -
  {
    if command -v apt-get >/dev/null; then
      export DEBIAN_FRONTEND=noninteractive
      apt-get update &&
        apt-get install -y --no-install-recommends \
          xvfb xauth dbus-daemon desktop-file-utils imagemagick fonts-dejavu-core file \
          ca-certificates &&
        mesa=$(dpkg-query -W -f '${Package}\n' |
          grep -E '^(libgl1-mesa-dri|libglx-mesa0|libglapi-mesa|libegl-mesa0|libgbm1|mesa-.*|libllvm[0-9]+)$')
      if [ -n "$mesa" ]; then
        # shellcheck disable=SC2086 # one package per word
        dpkg --purge --force-depends $mesa
      fi
    else
      dnf install -y --setopt=install_weak_deps=False \
        xorg-x11-server-Xvfb xorg-x11-xauth dbus-daemon dbus-tools desktop-file-utils \
        ImageMagick dejavu-sans-fonts file ca-certificates &&
        mesa=$(rpm -qa --qf '%{NAME}\n' | grep -E '^(mesa-.*|llvm[0-9]*-libs)$')
      if [ -n "$mesa" ]; then
        # shellcheck disable=SC2086 # one package per word
        rpm -e --nodeps $mesa
      fi
    fi
  } >&2 || {
    echo "test.sh: installing the test tools failed" >&2
    exit 1
  }
  toolkit=$( {
    ldconfig -p | grep -E 'libQt[56]Core|libgtk-[34]|libadwaita|libKF[56]|libEGL_mesa|libGLX_mesa|libgallium|libgbm\.'
    find /usr/lib /usr/lib64 -maxdepth 3 -type d -name dri 2>/dev/null
  })
  if [ -n "$toolkit" ]; then
    echo "test.sh: the container has toolkit libraries: $(echo "$toolkit" | head -n 3 | tr -s ' \t\n' ' ')" >&2
    exit 1
  fi
  echo "test.sh: no Qt, GTK, KDE or Mesa library in the container" >&2
  mv /test/*.AppImage "$installed/"
  WYE_APPIMAGE=$(echo "$installed"/*.AppImage) APPIMAGE_EXTRACT_AND_RUN=1 \
    bash /test/smoke-test.sh /out >&2 || status=$?
  tar -C /out -c -f - .
  exit "$status"
}

failed=()
for image in "${images[@]}"; do
  distro=${image//:/-}
  distro=${distro//\//-}
  out=$(mktemp -d "$dist/.appimage-test.XXXXXX")
  echo "test.sh: testing $(basename "$appimage") in $image" >&2
  status=0
  tar -c -f - -C "$(dirname "$appimage")" "$(basename "$appimage")" -C "$root/packaging" smoke-test.sh |
    "$docker" run --rm -i "$image" \
      bash -c "$(declare -f test_in_container); test_in_container" |
    tar -C "$out" -x -f - || status=$?

  mkdir -p "$dist/screenshots" "$dist/smoke-logs"
  for shot in "$out"/screenshots/*.png; do
    if [ -e "$shot" ]; then mv -f "$shot" "$dist/screenshots/appimage-$distro-$(basename "$shot")"; fi
  done
  rm -rf "$dist/smoke-logs/appimage-$distro"
  if [ -d "$out/logs" ]; then mv "$out/logs" "$dist/smoke-logs/appimage-$distro"; fi
  rm -rf "$out"
  if [ "$status" -ne 0 ]; then
    echo "test.sh: $image failed (exit $status); logs in $dist/smoke-logs/appimage-$distro" >&2
    failed+=("$image")
  else
    echo "test.sh: $image passed" >&2
  fi
done

if [ "${#failed[@]}" -ne 0 ]; then
  echo "test.sh: failed on ${failed[*]}" >&2
  exit 1
fi
echo "test.sh: every check passed on ${images[*]}; screenshots in $dist/screenshots" >&2
