# The latest published Wye release, recorded in nix/release.json by the
# release workflow (.github/workflows/release.yml). All three fields are
# `null` until the first release exists.
rec {
  read = builtins.fromJSON (builtins.readFile ./release.json);

  # Whether `release` names a release the flake can fetch.
  isPublished = release: release.version != null && release.rev != null && release.narHash != null;

  # A locked reference (commit and content hash), so evaluating it needs no
  # network access the lock file does not cover and is pure-eval safe.
  flakeRef = release: "github:psoldunov/wye/${release.rev}?narHash=${release.narHash}";
}
