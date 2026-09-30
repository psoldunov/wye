# home-manager module: `programs.wye`.
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
  tomlFormat = pkgs.formats.toml { };
  desktopId = "dev.soldunov.wye.desktop";
  plasmoidId = "dev.soldunov.wye";
  serviceFiles = [
    "dev.soldunov.wye.service"
    "dev.soldunov.wye.Ui.service"
  ];

  # DEF-01, DEF-07: the schemes always, HTML files only when the settings ask.
  localHtml = cfg.settings.general.open-local-html or false;
  mimeTypes = [
    "x-scheme-handler/http"
    "x-scheme-handler/https"
  ]
  ++ lib.optionals localHtml [
    "text/html"
    "application/xhtml+xml"
  ];

  # GEN-01: with a managed config the file has to carry the choice, or Wye
  # would default to launching at login and write an autostart entry itself.
  managedSettings = lib.recursiveUpdate (lib.optionalAttrs (!cfg.launchAtLogin) {
    general.launch-at-login = false;
  }) cfg.settings;
  managedConfig = cfg.settings != { } || !cfg.launchAtLogin;

  # systemd user services start with a minimal PATH; browsers launched from
  # desktop entries with a bare `Exec=firefox` need the profile directories.
  searchPath = lib.concatStringsSep ":" [
    "${config.home.profileDirectory}/bin"
    "/etc/profiles/per-user/${config.home.username}/bin"
    "/run/current-system/sw/bin"
    "${config.home.homeDirectory}/.local/bin"
  ];
in
{
  options.programs.wye = channel.options // {
    enable = lib.mkEnableOption "Wye, a native browser picker that sends every link to the right browser";

    settings = lib.mkOption {
      inherit (tomlFormat) type;
      default = { };
      example = lib.literalExpression ''
        {
          browsers.primary.picker = true;
          general.show-tray-icon = false;
        }
      '';
      description = ''
        Contents of {file}`$XDG_CONFIG_HOME/wye/config.toml`. When set, the file
        is a read-only link into the Nix store: Wye reports it as read-only and
        settings changed in its window are not saved. Leave it empty to keep
        the file writable and edit it from Wye.
      '';
    };

    defaultBrowser = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Make Wye the default web browser: it becomes the handler of
        `x-scheme-handler/http` and `x-scheme-handler/https` in
        {file}`mimeapps.list` (through {option}`xdg.mimeApps`, which makes that
        file read-only), and of `text/html` and `application/xhtml+xml` when
        `settings.general.open-local-html` is on. On Plasma, Wye also sets
        `BrowserApplication` in {file}`kdeglobals`.
      '';
    };

    launchAtLogin = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Start the Wye service with the graphical session, so the tray icon is
        there from the start (the "Launch at login" setting). Off, the service
        still starts on the first link. Combined with a managed
        {option}`settings`, this also writes `general.launch-at-login` to the
        config file unless {option}`settings` sets it.
      '';
    };
  };

  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      {
        inherit (channel) assertions;

        # The Plasma applet ships in the package's share/plasma/plasmoids.
        home.packages = [ cfg.package ];

        xdg.configFile."wye/config.toml" = lib.mkIf managedConfig {
          source = tomlFormat.generate "wye-config.toml" managedSettings;
        };

        # Route D-Bus activation through the package's own files, so the
        # session bus finds them even when the profile is not on
        # XDG_DATA_DIRS. `dev.soldunov.wye` starts the systemd unit.
        xdg.dataFile = lib.genAttrs (map (name: "dbus-1/services/${name}") serviceFiles) (path: {
          source = "${cfg.package}/share/${path}";
        });

        systemd.user.services.wye = {
          Unit = {
            Description = "Wye browser picker service";
            Documentation = [ "https://github.com/psoldunov/wye" ];
            PartOf = [ "graphical-session.target" ];
            After = [ "graphical-session.target" ];
          };
          Service = {
            Type = "dbus";
            BusName = "dev.soldunov.wye";
            ExecStart = "${lib.getExe cfg.package} service";
            Restart = "on-failure";
            RestartSec = 2;
            # 75 (EX_TEMPFAIL): another service owns the bus name.
            RestartPreventExitStatus = 75;
            # Browsers Wye launches outlive a restart of the service (LAUNCH-06).
            KillMode = "process";
            Environment = [ "PATH=${searchPath}" ];
          };
          Install.WantedBy = lib.optional cfg.launchAtLogin "graphical-session.target";
        };

        # Plasma's system tray looks for applets when plasmashell starts, and
        # after that only when KPackage announces an install over D-Bus. A
        # switch installs the applet without that announcement, so a running
        # session would not show it until the next login. Announce it whenever
        # this generation adds or changes it. Nothing listens outside Plasma.
        home.activation.wyePlasmoid = lib.hm.dag.entryAfter [ "installPackages" "linkGeneration" ] ''
          wyeAnnouncePlasmoid() {
            local rel=home-path/share/plasma/plasmoids/${plasmoidId}
            local new old="" bus socket
            new=$(readlink -e "$newGenPath/$rel") || return 0
            if [[ -v oldGenPath && -e "$oldGenPath/$rel" ]]; then
              old=$(readlink -e "$oldGenPath/$rel")
            fi
            [[ $new != "$old" ]] || return 0

            bus=''${DBUS_SESSION_BUS_ADDRESS:-}
            if [[ -z $bus ]]; then
              socket=''${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/bus
              [[ -S $socket ]] || return 0
              bus=unix:path=$socket
            fi

            if ! run ${lib.getExe' pkgs.dbus "dbus-send"} --bus="$bus" --type=signal \
              /KPackage/Plasma/Applet org.kde.plasma.kpackage.packageInstalled \
              string:${plasmoidId}; then
              warnEcho "Could not announce the Wye applet to Plasma; it appears after the next login."
            fi
          }
          wyeAnnouncePlasmoid
        '';
      }

      (lib.mkIf cfg.defaultBrowser {
        xdg.mimeApps.enable = true;
        # mkBefore: Wye leads the list when another module names a browser.
        xdg.mimeApps.defaultApplications = lib.genAttrs mimeTypes (_: lib.mkBefore [ desktopId ]);

        # Plasma keeps its own default browser in kdeglobals, a file Plasma
        # writes too, so it is set in place rather than linked. Only where
        # the file exists (Plasma has run) and is not already a managed link.
        home.activation.wyeKdeBrowser = lib.hm.dag.entryAfter [ "linkGeneration" ] ''
          kdeglobals=''${XDG_CONFIG_HOME:-$HOME/.config}/kdeglobals
          if [[ -f $kdeglobals && ! -L $kdeglobals ]]; then
            run ${lib.getExe' pkgs.kdePackages.kconfig "kwriteconfig6"} \
              --file "$kdeglobals" --group General --key BrowserApplication ${desktopId}
          fi
        '';
      })
    ]
  );
}
