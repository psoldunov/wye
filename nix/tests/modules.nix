# Evaluates the home-manager and NixOS modules the way a user's configuration
# would, and asserts what they produce: the config file, the systemd unit, the
# D-Bus activation files, the default-browser associations, the frontend
# (GEN-01, DEF-01, DEF-02, DEF-04, DEF-07, ADV-12). Nothing here boots
# anything; the frontend script runs on a scratch config directory.
{
  pkgs,
  self,
  home-manager,
  nixpkgs,
}:
let
  inherit (pkgs) lib;
  system = pkgs.stdenv.hostPlatform.system;
  package = self.packages.${system}.default;

  # Neither case reads nix/release.json, so the checks mean the same before
  # and after a release is recorded there: the release channel is exercised
  # with a stand-in package and a made-up release, everything else with no
  # release at all (the `git` channel, the flake's own package).
  noRelease = {
    version = null;
    rev = null;
    narHash = null;
  };
  unreleasedHomeModule = import ../hm-module.nix {
    inherit self;
    release = noRelease;
  };
  unreleasedNixosModule = import ../nixos-module.nix {
    inherit self;
    release = noRelease;
  };
  fakeRelease = {
    version = "0.1.0";
    rev = "0000000000000000000000000000000000000000";
    narHash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
  fakeReleasePackage = pkgs.runCommand "wye-release-stand-in" { meta.mainProgram = "wye"; } ''
    mkdir -p $out/bin
    ln -s ${package}/bin/wye $out/bin/wye
  '';
  releasedSelf = self // {
    packages = self.packages // {
      ${system} = self.packages.${system} // {
        wye-release = fakeReleasePackage;
      };
    };
  };
  releasedHomeModule = import ../hm-module.nix {
    self = releasedSelf;
    release = fakeRelease;
  };
  releasedNixosModule = import ../nixos-module.nix {
    self = releasedSelf;
    release = fakeRelease;
  };

  homeConfigWith =
    module: programs:
    home-manager.lib.homeManagerConfiguration {
      inherit pkgs;
      modules = [
        module
        {
          home = {
            username = "alice";
            homeDirectory = "/home/alice";
            stateVersion = "24.11";
          };
          inherit programs;
        }
      ];
    };
  homeConfig = homeConfigWith unreleasedHomeModule;

  nixosConfigWith =
    module: programs:
    nixpkgs.lib.nixosSystem {
      inherit system;
      modules = [
        module
        {
          boot.loader.grub.enable = false;
          fileSystems."/" = {
            device = "none";
            fsType = "tmpfs";
          };
          system.stateVersion = "24.11";
          inherit programs;
        }
      ];
    };
  nixosConfig = nixosConfigWith unreleasedNixosModule;

  failedAssertions = config: map (a: a.message) (lib.filter (a: !a.assertion) config.assertions);

  # Which package each channel picks, per module (channel.nix).
  channels =
    let
      home =
        programs:
        (homeConfigWith releasedHomeModule {
          wye = {
            enable = true;
          }
          // programs;
        }).config;
      nixos =
        programs:
        (nixosConfigWith releasedNixosModule {
          wye = {
            enable = true;
          }
          // programs;
        }).config;
      pick = config: {
        channel = config.programs.wye.channel;
        package = toString config.programs.wye.package;
        failed = failedAssertions config;
        frontend = config.programs.wye.frontend;
        # ADV-12: whether the GTK host's unit is declared (home-manager).
        gtkUnit = config.systemd.user.services ? wye-gtk;
      };
    in
    {
      released = {
        home = pick (home { });
        homeGit = pick (home {
          channel = "git";
        });
        nixos = pick (nixos { });
        nixosGit = pick (nixos {
          channel = "git";
        });
        # ADV-12: the frontend option on both channels of both modules.
        homeGnome = pick (home {
          frontend = "gnome";
        });
        homeGitKde = pick (home {
          channel = "git";
          frontend = "kde";
        });
        nixosGnome = pick (nixos {
          frontend = "gnome";
        });
        nixosGitKde = pick (nixos {
          channel = "git";
          frontend = "kde";
        });
        homeOverride = pick (home {
          package = pkgs.hello;
        });
      };
      unreleased = {
        home = pick (homeConfig { wye.enable = true; }).config;
        # home-manager throws on any access to a configuration whose
        # assertions fail; the message is read from the shared channel.nix.
        homeReleaseThrows =
          !(builtins.tryEval
            (homeConfig {
              wye = {
                enable = true;
                channel = "release";
              };
            }).config.programs.wye.channel
          ).success;
        releaseMessages = map (a: a.message) (
          lib.filter (a: !a.assertion) (
            (import ../channel.nix {
              inherit self lib pkgs;
              release = noRelease;
            } { channel = "release"; }).assertions
          )
        );
        nixosRelease =
          pick
            (nixosConfig {
              wye = {
                enable = true;
                channel = "release";
              };
            }).config;
      };
    };

  # Everything that is not the module's output is left out of the JSON.
  homeFacts =
    name: programs:
    let
      config = (homeConfig programs).config;
      file = path: config.xdg.configFile.${path} or null;
    in
    {
      inherit name;
      unit = config.systemd.user.services.wye or null;
      packages = map toString config.home.packages;
      dataFiles = lib.mapAttrs (_: f: toString f.source) (
        lib.filterAttrs (n: _: lib.hasPrefix "dbus-1/" n) config.xdg.dataFile
      );
      configToml =
        if file "wye/config.toml" == null then null else toString (file "wye/config.toml").source;
      mimeApps = config.xdg.mimeApps.defaultApplications;
      mimeAdded = config.xdg.mimeApps.associations.added;
      unitFile = toString (file "systemd/user/wye.service").source;
      uiUnit = config.systemd.user.services.wye-ui or null;
      activations = lib.attrNames (lib.filterAttrs (n: _: lib.hasPrefix "wye" n) config.home.activation);
      retirePlasmoid = config.home.activation.wyeRetirePlasmoid.data or null;
      frontend = config.programs.wye.frontend;
      gtkUnit = config.systemd.user.services.wye-gtk or null;
      gtkDbus = config.xdg.dataFile."dbus-1/services/dev.soldunov.wye.Gtk.service".text or null;
    };

  enabled = homeFacts "enabled" {
    wye = {
      enable = true;
      defaultBrowser = true;
      settings = {
        browsers.primary.picker = true;
        general.open-local-html = true;
      };
    };
  };
  minimal = homeFacts "minimal" { wye.enable = true; };
  quiet = homeFacts "quiet" {
    wye = {
      enable = true;
      launchAtLogin = false;
    };
  };
  # ADV-12: a writable file gets the frontend from the unit, a managed one
  # from `settings`; every frontend but "kde" needs the GTK host's activation
  # files.
  gnome = homeFacts "gnome" {
    wye = {
      enable = true;
      frontend = "gnome";
    };
  };
  gnomeManaged = homeFacts "gnomeManaged" {
    wye = {
      enable = true;
      frontend = "gnome";
      settings.browsers.primary.picker = true;
    };
  };
  kde = homeFacts "kde" {
    wye = {
      enable = true;
      frontend = "kde";
    };
  };
  # An unknown frontend, or one `settings` contradicts, fails evaluation.
  homeThrows =
    programs: !(builtins.tryEval (homeConfig programs).config.programs.wye.frontend).success;
  frontendChecks = {
    unknownThrows = homeThrows {
      wye = {
        enable = true;
        frontend = "xfce";
      };
    };
    disagreeingThrows = homeThrows {
      wye = {
        enable = true;
        frontend = "kde";
        settings.advanced.frontend = "gnome";
      };
    };
    agreeingEvaluates =
      !(homeThrows {
        wye = {
          enable = true;
          frontend = "kde";
          settings.advanced.frontend = "kde";
        };
      });
  };

  nixos =
    (nixosConfig {
      wye = {
        enable = true;
        defaultBrowser = true;
      };
    }).config;
  nixosQuiet =
    (nixosConfig {
      wye = {
        enable = true;
        launchAtLogin = false;
      };
    }).config;
  nixosOff = (nixosConfig { }).config;
  nixosGnome =
    (nixosConfig {
      wye = {
        enable = true;
        frontend = "gnome";
      };
    }).config;

  nixosFacts = {
    systemPackages = map toString nixos.environment.systemPackages;
    dbusPackages = map toString nixos.services.dbus.packages;
    systemdPackages = map toString nixos.systemd.packages;
    wantedBy = nixos.systemd.user.services.wye.wantedBy;
    environment = nixos.systemd.user.services.wye.environment;
    quietWantedBy = nixosQuiet.systemd.user.services.wye.wantedBy;
    quietEnvironment = nixosQuiet.systemd.user.services.wye.environment;
    mime = nixos.xdg.mime.defaultApplications;
    offHasUnit = nixosOff.systemd.user.services ? wye;
    frontend = nixos.programs.wye.frontend;
    execStartPre = nixos.systemd.user.services.wye.serviceConfig.ExecStartPre or null;
    gnomeExecStartPre = nixosGnome.systemd.user.services.wye.serviceConfig.ExecStartPre or null;
    offPackages = map toString nixosOff.environment.systemPackages;
  };

  facts = pkgs.writeText "wye-module-facts.json" (
    builtins.toJSON {
      home = {
        inherit
          enabled
          minimal
          quiet
          gnome
          gnomeManaged
          kde
          ;
      };
      inherit frontendChecks;
      nixos = nixosFacts;
      inherit channels;
      package = toString package;
      releasePackage = toString fakeReleasePackage;
      helloPackage = toString pkgs.hello;
    }
  );
in
pkgs.runCommand "wye-modules-eval" { nativeBuildInputs = [ pkgs.jq ]; } ''
  facts=${facts}
  check() { jq -e "$1" "$facts" > /dev/null || { echo "failed: $1" >&2; jq -c ".package, .home.enabled.unit.Service" "$facts" >&2; exit 1; }; }

  # home-manager: the package, the unit (DEF-04, LAUNCH-06), the D-Bus files.
  check '.package as $p | .home.enabled.packages | index($p) != null'
  for name in enabled minimal quiet; do
    check ".home.$name.unit.Service | .Type == \"dbus\" and .BusName == \"dev.soldunov.wye\" and .KillMode == \"process\" and .RestartPreventExitStatus == 75"
    check ".package as \$p | .home.$name.unit.Service.ExecStart == [\"\(\$p)/bin/wye service\"]"
    # The GTK host's D-Bus file is written by the module (ADV-12, checked below).
    check ".package as \$p | .home.$name.dataFiles | del(.\"dbus-1/services/dev.soldunov.wye.Gtk.service\") == {
      \"dbus-1/services/dev.soldunov.wye.service\": \"\(\$p)/share/dbus-1/services/dev.soldunov.wye.service\",
      \"dbus-1/services/dev.soldunov.wye.Ui.service\": \"\(\$p)/share/dbus-1/services/dev.soldunov.wye.Ui.service\"
    }"
    grep -qxF 'KillMode=process' "$(jq -r ".home.$name.unitFile" "$facts")"
    # GEN-01: the unit owns login start; the service leaves XDG autostart alone.
    login=$([ $name = quiet ] && echo off || echo on)
    check ".home.$name.unit.Service.Environment | index(\"WYE_LOGIN_MANAGED=$login\") != null"
    # Browsers with a bare Exec: Nix profiles first, then the system's directories.
    check ".home.$name.unit.Service.Environment[] | select(startswith(\"PATH=\")) | startswith(\"PATH=/home/alice/.nix-profile/bin:\") and endswith(\":/usr/local/bin:/usr/bin:/bin\")"
    # The UI host's unit: bus-activated, never at login.
    check ".package as \$p | .home.$name.uiUnit | .Service.Type == \"dbus\" and .Service.BusName == \"dev.soldunov.wye.Ui\" and .Service.ExecStart == [\"\(\$p)/bin/wye-ui\"] and ((.Install.WantedBy // []) == [])"
  done

  # The tray is the service's StatusNotifierItem: the package ships no Plasma
  # applet, and the module only tells a running Plasma that an older
  # generation's applet is gone.
  test ! -e ${package}/share/plasma
  check '.home.minimal.activations | index("wyePlasmoid") == null and index("wyeRetirePlasmoid") != null'
  check '.home.minimal.retirePlasmoid | contains("org.kde.plasma.kpackage.packageUninstalled") and contains("string:dev.soldunov.wye")'

  # GEN-01: launch at login follows the unit's WantedBy.
  check '.home.enabled.unit.Install.WantedBy == ["graphical-session.target"]'
  check '.home.quiet.unit.Install.WantedBy == []'

  # A managed config file only when settings ask; launchAtLogin alone never
  # makes the file read-only.
  check '.home.minimal.configToml == null'
  check '.home.quiet.configToml == null'
  config=$(jq -r '.home.enabled.configToml' "$facts")
  grep -qxF '[browsers.primary]' "$config"
  grep -qxF 'picker = true' "$config"
  grep -qxF 'open-local-html = true' "$config"
  if grep -q 'launch-at-login' "$config"; then exit 1; fi

  # DEF-02, DEF-07: associations only on request; HTML files only when on,
  # as the default and as an added association.
  check '.home.minimal.mimeApps == {}'
  check '.home.minimal.mimeAdded == {}'
  check '.home.enabled.mimeApps == (
    ["x-scheme-handler/http", "x-scheme-handler/https", "text/html", "application/xhtml+xml"]
    | map({(.): ["dev.soldunov.wye.desktop"]}) | add)'
  check '.home.enabled.mimeAdded == (
    ["text/html", "application/xhtml+xml"] | map({(.): ["dev.soldunov.wye.desktop"]}) | add)'

  # ADV-12: "auto" by default and leaves the file alone; another value is set
  # in a writable file before the service starts, or written into a managed
  # one; every frontend but "kde" makes the GTK host bus-activatable (GNOME
  # sessions send their windows to it under "auto").
  for name in enabled minimal quiet; do
    check ".home.$name.frontend == \"auto\" and .home.$name.gtkUnit != null and .home.$name.gtkDbus != null"
    check ".home.$name.unit.Service | has(\"ExecStartPre\") | not"
  done
  check '.home.gnome.configToml == null and .home.kde.configToml == null'
  check '.home.gnome.unit.Service.ExecStartPre | length == 1 and (.[0] | startswith("-/nix/store/") and endswith("-wye-set-frontend gnome"))'
  check '.home.kde.unit.Service.ExecStartPre | length == 1 and (.[0] | endswith("-wye-set-frontend kde"))'
  check '.home.gnomeManaged.unit.Service | has("ExecStartPre") | not'
  grep -qxF 'frontend = "gnome"' "$(jq -r '.home.gnomeManaged.configToml' "$facts")"
  check '.package as $p | .home.gnome.gtkUnit | .Service.Type == "dbus" and .Service.BusName == "dev.soldunov.wye.Gtk" and .Service.ExecStart == ["\($p)/bin/wye-gtk"] and ((.Install.WantedBy // []) == [])'
  check '.package as $p | .home.gnome.gtkDbus | contains("Name=dev.soldunov.wye.Gtk\n") and contains("Exec=\($p)/bin/wye-gtk\n") and contains("SystemdService=wye-gtk.service\n")'
  check '.home.kde.gtkUnit == null and .home.kde.gtkDbus == null and .home.gnomeManaged.gtkUnit != null'
  check '.frontendChecks | .unknownThrows and .disagreeingThrows and .agreeingEvaluates'
  check '.nixos.frontend == "auto" and .nixos.execStartPre == null'
  check '.nixos.gnomeExecStartPre | (if type == "array" then . else [.] end) | length == 1 and (.[0] | endswith("-wye-set-frontend gnome"))'
  check '.channels.released
    | (.homeGnome.frontend == "gnome" and .homeGnome.channel == "release" and .homeGnome.failed == [])
    # A release package without wye-gtk (no passthru.hasGtk) gets no GTK
    # host; the git package, which ships it, does.
    and (.homeGnome.gtkUnit | not) and .homeGit.gtkUnit
    and (.homeGitKde.frontend == "kde" and .homeGitKde.channel == "git" and .homeGitKde.failed == [])
    and (.nixosGnome.frontend == "gnome" and .nixosGnome.channel == "release" and .nixosGnome.failed == [])
    and (.nixosGitKde.frontend == "kde" and .nixosGitKde.channel == "git" and .nixosGitKde.failed == [])
    and .home.frontend == "auto" and .nixos.frontend == "auto"'

  # The script itself: sets the key and keeps the rest of the file, leaves a
  # store link and a file that is not TOML alone, and creates a missing file.
  script=$(jq -r '.home.gnome.unit.Service.ExecStartPre[0] | ltrimstr("-") | split(" ")[0]' "$facts")
  export XDG_CONFIG_HOME=$PWD/scratch
  mkdir -p scratch/wye
  printf '# mine\n[extras]\nforce-https = true\n' > scratch/wye/config.toml
  "$script" gnome
  grep -qxF '# mine' scratch/wye/config.toml
  grep -qxF 'force-https = true' scratch/wye/config.toml
  grep -qxF 'frontend = "gnome"' scratch/wye/config.toml
  "$script" kde
  grep -qxF 'frontend = "kde"' scratch/wye/config.toml
  if grep -qxF 'frontend = "gnome"' scratch/wye/config.toml; then exit 1; fi
  printf '[extras\n' > scratch/wye/config.toml
  "$script" gnome
  [ "$(cat scratch/wye/config.toml)" = '[extras' ]
  printf '[advanced]\n' > linked.toml
  rm scratch/wye/config.toml
  ln -s "$PWD/linked.toml" scratch/wye/config.toml
  "$script" gnome
  [ "$(cat linked.toml)" = '[advanced]' ]
  rm -r scratch
  "$script" gnome
  grep -qxF 'frontend = "gnome"' scratch/wye/config.toml
  # The file keeps its mode; one made read-only on purpose stays untouched.
  chmod 640 scratch/wye/config.toml
  "$script" kde
  grep -qxF 'frontend = "kde"' scratch/wye/config.toml
  [ "$(stat -c %a scratch/wye/config.toml)" = 640 ]
  chmod 444 scratch/wye/config.toml
  "$script" gnome
  grep -qxF 'frontend = "kde"' scratch/wye/config.toml
  [ "$(stat -c %a scratch/wye/config.toml)" = 444 ]

  # NixOS: the package's own files, wired in.
  check '.package as $p | .nixos.systemPackages | index($p) != null'
  check '.package as $p | .nixos.dbusPackages | index($p) != null'
  check '.package as $p | .nixos.systemdPackages | index($p) != null'
  check '.nixos.wantedBy == ["graphical-session.target"]'
  check '.nixos.quietWantedBy == []'
  check '.nixos.environment.WYE_LOGIN_MANAGED == "on"'
  check '.nixos.quietEnvironment.WYE_LOGIN_MANAGED == "off"'
  check '.nixos.environment.PATH | startswith("/etc/profiles/per-user/%u/bin:/run/wrappers/bin:") and endswith(":/usr/local/bin:/usr/bin:/bin")'
  check '.nixos.mime == {
    "x-scheme-handler/http": "dev.soldunov.wye.desktop",
    "x-scheme-handler/https": "dev.soldunov.wye.desktop"
  }'
  check '.nixos.offHasUnit == false'
  check '.package as $p | .nixos.offPackages | index($p) == null'

  # channel.nix: `release` when a release is recorded, else `git`; an explicit
  # package wins; asking for a release that does not exist fails clearly.
  check '.package as $git | .releasePackage as $rel | .channels.released
    | (.home.channel == "release" and .home.package == $rel and .home.failed == [])
    and (.homeGit.channel == "git" and .homeGit.package == $git)
    and (.nixos.channel == "release" and .nixos.package == $rel)
    and (.nixosGit.channel == "git" and .nixosGit.package == $git)'
  check '.helloPackage as $hello | .channels.released.homeOverride.package == $hello'
  check '.package as $git | .channels.unreleased
    | (.home.channel == "git" and .home.package == $git and .home.failed == [])
    and .homeReleaseThrows
    and (.releaseMessages | length == 1 and (.[0] | test("no Wye release is published yet; use channel = \"git\"")))
    and (.nixosRelease.failed | length == 1)'
  touch $out
''
