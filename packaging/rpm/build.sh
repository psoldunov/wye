#!/usr/bin/env bash
# Build the Wye .rpm for Fedora 44 in Docker.
#
# Builds the image of packaging/rpm/Dockerfile, copies the source tree into a
# container (tracked and untracked-but-not-ignored files, as a tarball), runs
# rpmbuild there and copies the packages out:
#
#   dist/wye-VERSION-1.fc44.x86_64.rpm   (and the -debuginfo/-debugsource rpms)
#   dist/wye-VERSION-1.fc44.src.rpm
#
#   packaging/rpm/build.sh
#
# Run from anywhere; paths are resolved against the repository root. Nothing
# in the worktree is written by the container, and the worktree's target/ is
# not used: the cargo registry and the target directory live on the Docker
# volumes wye-rpm-cargo and wye-rpm-target, so a rebuild is incremental
# (`docker volume rm wye-rpm-cargo wye-rpm-target` starts clean).
#
# Environment:
#   DOCKER         the container CLI (default: docker; podman works too)
#   DOCKER_CONFIG  read by the docker CLI itself, e.g. for other registry
#                  credentials
#   WYE_RPM_IMAGE  the build image tag (default: wye-rpm-build:fc44)
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
docker=${DOCKER:-docker}
image=${WYE_RPM_IMAGE:-wye-rpm-build:fc44}
dist=$root/dist

# The workspace version, from [workspace.package] in Cargo.toml.
version=$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' "$root/Cargo.toml")
if [ -z "$version" ]; then
  echo "build.sh: no version in [workspace.package] of $root/Cargo.toml" >&2
  exit 1
fi

work=$(mktemp -d)
container=
cleanup() {
  if [ -n "$container" ]; then "$docker" rm -f "$container" >/dev/null 2>&1 || true; fi
  rm -rf "$work"
}
trap cleanup EXIT

echo "build.sh: building the image $image"
"$docker" build -t "$image" "$root/packaging/rpm"

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
# The spec with the version written in, so the src.rpm needs no --define.
{
  echo "%global wye_version $version"
  cat "$root/packaging/rpm/wye.spec"
} >"$work/wye.spec"

# rpmbuild in a container, then rpmlint over the packages and the spec
# (advisory: its findings go to dist/rpmlint.txt and never fail the build).
# The build directory is fresh each run; the cargo home and target directory
# are volumes (paths from the Dockerfile's ENV).
# shellcheck disable=SC2016 # expanded by the container's shell
in_container='
set -e
rpmbuild -ba --define "_topdir /build/rpmbuild" --define "_sourcedir /build/src" /build/src/wye.spec
rpmlint /build/src/wye.spec /build/rpmbuild/RPMS/*/*.rpm /build/rpmbuild/SRPMS/*.rpm >/build/rpmlint.txt 2>&1 || true
'
container=$("$docker" create \
  -v wye-rpm-cargo:/build/cargo-home \
  -v wye-rpm-target:/build/target \
  "$image" bash -c "$in_container")
"$docker" cp "$work/." "$container:/build/src"

echo "build.sh: running rpmbuild for wye $version"
"$docker" start -a "$container"

# `docker cp` out writes files owned by the user running it, never root.
"$docker" cp "$container:/build/rpmbuild/RPMS/." "$work/RPMS"
"$docker" cp "$container:/build/rpmbuild/SRPMS/." "$work/SRPMS"
mkdir -p "$dist"
find "$work/RPMS" "$work/SRPMS" -name '*.rpm' -exec cp {} "$dist/" \;
"$docker" cp "$container:/build/rpmlint.txt" "$dist/rpmlint.txt"

echo "build.sh: packages in $dist:"
find "$work/RPMS" "$work/SRPMS" -name '*.rpm' -printf '  %f\n'
echo "build.sh: rpmlint: $(tail -n 1 "$dist/rpmlint.txt")"
