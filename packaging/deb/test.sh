#!/usr/bin/env bash
# Install dist/wye_*_ARCH.deb in a fresh Debian testing container and run
# packaging/smoke-test.sh against it.
#
#   packaging/deb/test.sh [DEB]
#
# DEB defaults to the newest dist/wye_*_ARCH.deb (packaging/deb/build.sh
# writes it), ARCH being the machine's own Debian architecture (amd64 or
# arm64), which the container runs too. apt-get installs it with its Recommends, so its dependencies
# resolve from the archive exactly as on a user's machine; the test-only
# tools (Xvfb, desktop-file-utils, dbus, ImageMagick) are installed next to
# it, not by the package. Screenshots land in dist/screenshots/debian-*.png and the
# smoke test's logs in dist/smoke-logs/debian/, owned by the calling user.
#
# Environment: DOCKER (default docker; podman works too), DOCKER_CONFIG
# (read by the docker CLI itself), WYE_DEB_TEST_IMAGE (default debian:testing).
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
docker=${DOCKER:-docker}
image=${WYE_DEB_TEST_IMAGE:-debian:testing}
dist=$root/dist

case $(uname -m) in
  x86_64) arch=amd64 ;;
  aarch64) arch=arm64 ;;
  *) arch=$(uname -m) ;;
esac

deb=${1:-}
if [ -z "$deb" ]; then
  deb=$(find "$dist" -maxdepth 1 -name "wye_*_$arch.deb" -printf '%T@ %p\n' 2>/dev/null |
    sort -n | tail -n 1 | cut -d' ' -f2-)
fi
if [ -z "$deb" ] || [ ! -f "$deb" ]; then
  echo "test.sh: no .deb; run packaging/deb/build.sh first or pass one" >&2
  exit 1
fi

# Runs in the container: the .deb and the smoke test arrive as a tar on
# stdin, the screenshots and logs leave as a tar on stdout; everything else
# goes to stderr.
test_in_container() {
  set -uo pipefail
  local status=0
  mkdir -p /test /out
  tar -C /test -x -f -
  {
    export DEBIAN_FRONTEND=noninteractive
    apt-get update &&
      apt-get install -y /test/*.deb &&
      apt-get install -y --no-install-recommends \
        xvfb xauth desktop-file-utils dbus-daemon imagemagick
  } >&2 || {
    echo "test.sh: installing the package failed" >&2
    exit 1
  }
  bash /test/smoke-test.sh /out >&2 || status=$?
  tar -C /out -c -f - .
  exit "$status"
}

out=$(mktemp -d "$dist/.deb-test.XXXXXX")
trap 'rm -rf "$out"' EXIT

echo "test.sh: testing $(basename "$deb") in $image" >&2
status=0
tar -c -f - -C "$(dirname "$deb")" "$(basename "$deb")" -C "$root/packaging" smoke-test.sh |
  "$docker" run --rm -i "$image" \
    bash -c "$(declare -f test_in_container); test_in_container" |
  tar -C "$out" -x -f - || status=$?

mkdir -p "$dist/screenshots" "$dist/smoke-logs"
for shot in "$out"/screenshots/*.png; do
  if [ -e "$shot" ]; then mv -f "$shot" "$dist/screenshots/debian-$(basename "$shot")"; fi
done
if [ -d "$out/logs" ]; then
  rm -rf "$dist/smoke-logs/debian"
  mv "$out/logs" "$dist/smoke-logs/debian"
fi
if [ "$status" -ne 0 ]; then
  echo "test.sh: the smoke test failed (exit $status); logs in $dist/smoke-logs/debian" >&2
  exit "$status"
fi
echo "test.sh: every check passed; screenshots in $dist/screenshots" >&2
