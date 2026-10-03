#!/usr/bin/env bash
# Install the Wye .rpm in a fresh Fedora 44 container and smoke-test it.
#
# Starts a clean fedora:44 container (not the build image), installs
# dist/wye-VERSION-1.fc44.x86_64.rpm with dnf so its dependencies resolve
# from the Fedora repositories, adds the test-only tools, then runs
# packaging/smoke-test.sh from the repository mounted read-only. The
# screenshots land in dist/screenshots/fedora-*.png and the logs in
# dist/smoke-logs/fedora/, owned by the user running this script.
#
#   packaging/rpm/build.sh && packaging/rpm/test.sh
#
# Exits with the smoke test's status. Environment: DOCKER (the container CLI,
# default docker), DOCKER_CONFIG (read by the docker CLI itself) and
# WYE_RPM_TEST_IMAGE (default fedora:44).
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
docker=${DOCKER:-docker}
image=${WYE_RPM_TEST_IMAGE:-fedora:44}
dist=$root/dist

version=$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' "$root/Cargo.toml")
rpm=$(find "$dist" -maxdepth 1 -name "wye-$version-1.fc*.x86_64.rpm" -print -quit 2>/dev/null)
if [ -z "$rpm" ]; then
  echo "test.sh: no dist/wye-$version-1.fc*.x86_64.rpm; run packaging/rpm/build.sh first" >&2
  exit 1
fi
if [ ! -x "$root/packaging/smoke-test.sh" ]; then
  echo "test.sh: packaging/smoke-test.sh is missing" >&2
  exit 1
fi

work=$(mktemp -d)
container=
# shellcheck disable=SC2329 # run by the EXIT trap; shellcheck 0.11 misses that when the script ends in `exit`
cleanup() {
  if [ -n "$container" ]; then "$docker" rm -f "$container" >/dev/null 2>&1 || true; fi
  rm -rf "$work"
}
trap cleanup EXIT

container=$("$docker" run -d -v "$root:/src:ro" "$image" sleep infinity)
"$docker" cp "$rpm" "$container:/tmp/"

# Test-only tools of smoke-test.sh: Xvfb (also started by wye-gtk
# --self-test), desktop-file-validate, dbus-run-session, ImageMagick's
# `import` for the screenshots.
echo "test.sh: installing $(basename "$rpm") in $image"
"$docker" exec "$container" dnf install -y \
  "/tmp/$(basename "$rpm")" \
  xorg-x11-server-Xvfb \
  desktop-file-utils \
  dbus-daemon \
  dbus-tools \
  ImageMagick

status=0
"$docker" exec -e WYE_SMOKE_OUT=/out "$container" \
  /src/packaging/smoke-test.sh /out || status=$?

# `docker cp` out writes files owned by the user running it, never root.
"$docker" cp "$container:/out/." "$work/out"
rm -rf "$dist/smoke-logs/fedora"
mkdir -p "$dist/screenshots" "$dist/smoke-logs/fedora"
for shot in "$work"/out/screenshots/*.png; do
  if [ -f "$shot" ]; then cp "$shot" "$dist/screenshots/fedora-$(basename "$shot")"; fi
done
cp -r "$work"/out/logs/. "$dist/smoke-logs/fedora/"

echo "test.sh: screenshots in $dist/screenshots, logs in $dist/smoke-logs/fedora"
exit "$status"
