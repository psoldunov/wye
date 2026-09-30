# NixOS module: `programs.wye` (system-wide install for all users).
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
        still starts on the first link.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    inherit (channel) assertions;
    environment.systemPackages = [ cfg.package ];
    # The D-Bus activation files, the systemd user unit (lib/systemd/user) and
    # the Plasma applet (share/plasma/plasmoids) come from the package.
    services.dbus.packages = [ cfg.package ];
    systemd.packages = [ cfg.package ];
    systemd.user.services.wye = {
      wantedBy = lib.optional cfg.launchAtLogin "graphical-session.target";
      # Browsers with a bare `Exec=firefox` need the per-user profile.
      environment.PATH = lib.mkForce "/etc/profiles/per-user/%u/bin:/run/wrappers/bin:/run/current-system/sw/bin";
    };
    xdg.mime.defaultApplications = lib.mkIf cfg.defaultBrowser {
      "x-scheme-handler/http" = desktopId;
      "x-scheme-handler/https" = desktopId;
    };
  };
}
