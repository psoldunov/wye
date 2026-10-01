# The `channel` and `package` options both modules share: which build of Wye
# `programs.wye` installs. `git` is the flake's own source (latest master when
# the flake input tracks master); `release` is the latest published release
# recorded in nix/release.json, built by that release's own flake.
#
# Layout contract: the modules come from this flake but may install an older
# release's package, so they rely only on what every release's packaging
# ships: bin/wye, bin/wye-ui, bin/wye-native-host,
# share/dbus-1/services/dev.soldunov.wye{,.Ui}.service, and
# share/systemd/user/wye{,-ui}.service with lib/systemd/user links. Moving or
# renaming any of these breaks the `release` channel until the next release.
# Newer packages additionally ship bin/wye-gtk,
# share/dbus-1/services/dev.soldunov.wye.Gtk.service,
# share/systemd/user/wye-gtk.service, and
# share/gnome-shell/extensions/wye@dev.soldunov; modules do not require those
# paths until a release containing them is recorded.
#
# share/plasma/plasmoids/dev.soldunov.wye (the Plasma applet) is no longer
# part of the contract: this flake's package has no applet, and the modules
# rely on it nowhere. An older release's package still carries it, which is
# harmless: that release's service and applet belong together.
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
      description = "Package providing the Wye hosts, GNOME Shell extension, and D-Bus and systemd files.";
    };
  };

  assertions = [
    {
      assertion = cfg.channel != "release" || released;
      message = "programs.wye.channel = \"release\", but no Wye release is published yet; use channel = \"git\".";
    }
  ];
}
