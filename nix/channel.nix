# The `channel` and `package` options both modules share: which build of Wye
# `programs.wye` installs. `git` is the flake's own source (latest master when
# the flake input tracks master); `release` is the latest published release
# recorded in nix/release.json, built by that release's own flake.
{
  self,
  release,
  lib,
  pkgs,
}:
cfg:
let
  released = (import ./release.nix).isPublished release;
  packages = self.packages.${pkgs.stdenv.hostPlatform.system};
in
{
  options = {
    channel = lib.mkOption {
      type = lib.types.enum [
        "release"
        "git"
      ];
      default = if released then "release" else "git";
      defaultText = lib.literalMD ''
        `"release"` when the flake records a published release in
        {file}`nix/release.json`, else `"git"`
      '';
      description = ''
        Which build to install. `"release"` is the latest tagged release, built
        with that release's own Nix files. `"git"` compiles the flake's own
        source, which is the latest master commit when the flake input tracks
        master. Ignored when {option}`package` is set.
      '';
    };

    package = lib.mkOption {
      type = lib.types.package;
      default =
        if cfg.channel == "release" then packages.wye-release or packages.wye-git else packages.wye-git;
      defaultText = lib.literalExpression "wye.packages.\${system}.wye-release or wye.packages.\${system}.wye-git, by channel";
      description = "Package providing the `wye` and `wye-ui` binaries, the D-Bus and systemd files and the Plasma applet.";
    };
  };

  assertions = [
    {
      assertion = cfg.channel != "release" || released;
      message = "programs.wye.channel = \"release\", but no Wye release is published yet; use channel = \"git\".";
    }
  ];
}
