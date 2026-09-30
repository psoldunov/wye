# NixOS module: `programs.wye` (system-wide install for all users).
#
# Parity gap with the home-manager module, by design: no `settings` option and
# no local-HTML association (DEF-07), because `config.toml` is per user. Users
# set those in Wye's Settings window or with home-manager.
{ self, release }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.wye;
  channel = import ./channel.nix {
    inherit
      self
      release
      lib
      pkgs
      ;
  } cfg;
  desktopId = "dev.soldunov.wye.desktop";
in
{
  options.programs.wye = channel.options // {
    enable = lib.mkEnableOption "Wye, a native browser picker that sends every link to the right browser";

    defaultBrowser = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Make Wye the default web browser for every user, as the system-wide
        handler of `x-scheme-handler/http` and `x-scheme-handler/https` in
        {file}`/etc/xdg/mimeapps.list`. A user's own {file}`mimeapps.list`
        still wins.
      '';
    };

    launchAtLogin = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Start the Wye service with every graphical session. Off, the service
        still starts on the first link. The module owns login start through
        the systemd unit: the service leaves the XDG autostart entry alone
        (`WYE_LOGIN_MANAGED` is `on` or `off`, following this option) and the
        "Launch at login" setting shows as managed by Nix.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    inherit (channel) assertions;
    environment.systemPackages = [ cfg.package ];
    # The D-Bus activation files, the systemd user units `wye` and `wye-ui`
    # (lib/systemd/user) and the Plasma applet (share/plasma/plasmoids) come
    # from the package.
    services.dbus.packages = [ cfg.package ];
    systemd.packages = [ cfg.package ];
    systemd.user.services.wye = {
      wantedBy = lib.optional cfg.launchAtLogin "graphical-session.target";
      environment = {
        # Browsers with a bare `Exec=firefox` are looked up here: the
        # per-user and system profiles first, then a user's own Nix profile
        # and the usual system directories.
        PATH = lib.mkForce (
          lib.concatStringsSep ":" [
            "/etc/profiles/per-user/%u/bin"
            "/run/wrappers/bin"
            "/run/current-system/sw/bin"
            "%h/.nix-profile/bin"
            "%h/.local/bin"
            "/usr/local/bin"
            "/usr/bin"
            "/bin"
          ]
        );
        # GEN-01: login start is this unit's wantedBy, not the XDG autostart
        # entry the service would otherwise write; the value tells Settings
        # which way it is set.
        WYE_LOGIN_MANAGED = if cfg.launchAtLogin then "on" else "off";
      };
    };
    xdg.mime.defaultApplications = lib.mkIf cfg.defaultBrowser {
      "x-scheme-handler/http" = desktopId;
      "x-scheme-handler/https" = desktopId;
    };
  };
}
