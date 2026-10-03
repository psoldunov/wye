#!/usr/bin/env bash
# Build the Wye .deb in Docker: dist/wye_<version>_amd64.deb and
# dist/lintian.txt, owned by the calling user.
#
#   packaging/deb/build.sh
#
# Builds the image packaging/deb/Dockerfile (Debian testing), copies the
# source into a container (tracked files plus untracked files Git does not
# ignore) and runs dpkg-buildpackage there with packaging/deb/debian as
# debian/, then lintian. Nothing in the worktree is written except dist/:
# the cargo registry and the target directory live in the Docker volumes
# wye-deb-cargo and wye-deb-target, so a rebuild is incremental. The version
# is the workspace version in Cargo.toml.
#
# Environment: DOCKER (default docker; podman works too), DOCKER_CONFIG
# (read by the docker CLI itself), WYE_DEB_IMAGE (default wye-deb-build).
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
docker=${DOCKER:-docker}
image=${WYE_DEB_IMAGE:-wye-deb-build}
dist=$root/dist

version=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' "$root/Cargo.toml")
if [ -z "$version" ]; then
  echo "build.sh: no version in [workspace.package] of Cargo.toml" >&2
  exit 1
fi
# The changelog date is the commit's, so the same commit gives the same
# package.
date=$(date -R -d "@$(git -C "$root" log -1 --format=%ct)")

echo "build.sh: building image $image" >&2
"$docker" build -t "$image" "$root/packaging/deb" >&2

# Tracked files that still exist and untracked files that are not ignored,
# NUL-separated.
sources() {
  git -C "$root" ls-files -z --cached --others --exclude-standard --deduplicate |
    while IFS= read -r -d '' file; do
      if [ -e "$root/$file" ] || [ -L "$root/$file" ]; then
        printf '%s\0' "$file"
      fi
    done
}

# Runs in the container with the version and the changelog date: the
# source arrives as a tar on stdin, the .deb and the lintian report leave as
# a tar on stdout; everything else goes to stderr.
build_in_container() {
  set -euo pipefail
  local version=$1 date=$2 status=0
  mkdir -p /build/wye
  cd /build/wye
  tar -x -f -
  cp -r packaging/deb/debian debian
  sed -e "s|@VERSION@|$version|g" -e "s|@DATE@|$date|g" \
    debian/changelog.in >debian/changelog
  rm debian/changelog.in
  dpkg-buildpackage -b -uc -us >&2
  cd /build
  # lintian warns when it runs as root.
  HOME=/tmp setpriv --reuid=nobody --regid=nogroup --clear-groups \
    lintian --display-info --fail-on error "wye_${version}_"*.deb \
    >lintian.txt 2>&1 || status=$?
  cat lintian.txt >&2
  tar -c -f - "wye_${version}_"*.deb lintian.txt
  if [ "$status" -ne 0 ]; then
    echo "build.sh: lintian reported errors" >&2
    exit "$status"
  fi
}

mkdir -p "$dist"
staging=$(mktemp -d "$dist/.deb-build.XXXXXX")
trap 'rm -rf "$staging"' EXIT

echo "build.sh: building wye $version" >&2
start=$(date +%s)
sources | tar -C "$root" --null -T - -c -f - |
  "$docker" run --rm -i \
    -v wye-deb-cargo:/cargo -v wye-deb-target:/target \
    "$image" bash -c "$(declare -f build_in_container); build_in_container $(printf '%q %q' "$version" "$date")" |
  tar -C "$staging" -x -f - || status=$?

# The package and the report reach dist/ even when lintian failed the build.
for file in "$staging"/wye_*.deb "$staging"/lintian.txt; do
  if [ -e "$file" ]; then mv -f "$file" "$dist/"; fi
done
if [ "${status:-0}" -ne 0 ]; then
  echo "build.sh: the build failed (exit $status)" >&2
  exit "$status"
fi
echo "build.sh: wrote $(cd "$dist" && ls wye_"${version}"_*.deb) and lintian.txt to $dist in $(($(date +%s) - start)) s" >&2
