{
  description = "Wye: native Linux browser picker";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    # Only the module evaluation check (nix/tests/modules.nix) imports it.
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      home-manager,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      releaseInfo = import ./nix/release.nix;
      release = releaseInfo.read;
      released = releaseInfo.isPublished release;
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});

      mkBuild =
        pkgs:
        let
          craneLib = crane.mkLib pkgs;
          wye = pkgs.callPackage ./nix/package.nix { inherit craneLib; };
          # Imported only by the packages and checks that need it, never by
          # the dev shell.
          frontends = pkgs.callPackage ./nix/frontends.nix {
            inherit (wye.package) version;
          };
          # The binaries, data files and the browser extension in one tree.
          # The tray is the service's own StatusNotifierItem; no Plasma applet
          # ships any more.
          installed =
            pkgs.runCommand "wye-${wye.package.version}"
              {
                inherit (wye.package) meta;
                passthru = {
                  unwrapped = wye.package;
                  inherit (frontends) extension;
                  # ADV-12: ships the GTK host (nix/channel.nix).
                  inherit (wye.package) hasGtk;
                };
              }
              ''
                mkdir -p $out
                cp -rs ${wye.package}/. $out/
                chmod -R u+w $out
                cp -r ${frontends.extension}/share/wye $out/share/
              '';
        in
        {
          inherit
            craneLib
            wye
            frontends
            installed
            ;
        };
    in
    {
      packages = forAllSystems (
        pkgs:
        let
          inherit (mkBuild pkgs) installed frontends;
        in
        {
          # The build of this flake's own source: what a user tracking master
          # gets.
          default = installed;
          wye = installed;
          wye-git = installed;
          inherit (frontends) extension;
        }
        # The latest release, built by that release's own flake so an old
        # release never meets newer packaging. Absent until one is recorded in
        # nix/release.json.
        // nixpkgs.lib.optionalAttrs released {
          wye-release =
            (builtins.getFlake (releaseInfo.flakeRef release))
            .packages.${pkgs.stdenv.hostPlatform.system}.default;
        }
      );

      checks = forAllSystems (
        pkgs:
        let
          inherit (mkBuild pkgs) craneLib wye frontends;
          # nixpkgs' `dbus-daemon --session` reads /etc/dbus-1/session.conf,
          # which the build sandbox does not have. The private-bus tests run
          # this one instead: `--session` becomes the package's own session
          # configuration, listening in $TMPDIR.
          sandboxDbusDaemon = pkgs.writeShellScriptBin "dbus-daemon" ''
            tmp=''${TMPDIR:-/tmp}
            conf=$(mktemp "$tmp/wye-session-XXXXXX.conf")
            sed "s|<listen>.*</listen>|<listen>unix:tmpdir=$tmp</listen>|" \
              ${pkgs.dbus}/share/dbus-1/session.conf > "$conf"
            args=()
            for arg in "$@"; do
              if [ "$arg" = --session ]; then args+=("--config-file=$conf"); else args+=("$arg"); fi
            done
            exec ${pkgs.dbus}/bin/dbus-daemon "''${args[@]}"
          '';
        in
        {
          build = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
          # clippy::pedantic is on for the whole workspace (Cargo.toml); CI
          # turns every warning into an error.
          clippy = craneLib.cargoClippy (
            wye.commonArgs
            // {
              cargoArtifacts = wye.cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );
          test = craneLib.cargoTest (
            wye.commonArgs
            // {
              inherit (wye) cargoArtifacts;
              # dbus-daemon for the private-bus tests (wye, wye-service,
              # wye-ui); without it they skip.
              nativeBuildInputs = wye.commonArgs.nativeBuildInputs ++ [ sandboxDbusDaemon ];
            }
          );
          fmt = craneLib.cargoFmt { inherit (wye.commonArgs) src pname version; };
          # Licences, bans and sources per deny.toml. Advisories need the
          # network, so CI runs them as a separate job.
          deny = craneLib.cargoDeny {
            inherit (wye.commonArgs) pname version strictDeps;
            src = pkgs.lib.fileset.toSource {
              root = ./.;
              fileset = pkgs.lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./crates
                ./deny.toml
              ];
            };
          };
          # Dependencies declared in a Cargo.toml but never used in the source.
          machete = pkgs.runCommand "cargo-machete-check" { nativeBuildInputs = [ pkgs.cargo-machete ]; } ''
            cargo-machete ${wye.commonArgs.src}
            touch $out
          '';
          # The shipped desktop entry must pass the freedesktop validator.
          desktop-entry =
            pkgs.runCommand "wye-desktop-entry-valid" { nativeBuildInputs = [ pkgs.desktop-file-utils ]; }
              ''
                desktop-file-validate ${./data/applications/dev.soldunov.wye.desktop}
                touch $out
              '';
          # The entry the package installs: D-Bus activatable (DEF-04), with
          # absolute Execs for the main group and each action, and TryExec.
          installed-desktop-entry =
            pkgs.runCommand "wye-installed-desktop-entry-valid"
              { nativeBuildInputs = [ pkgs.desktop-file-utils ]; }
              ''
                entry=${wye.package}/share/applications/dev.soldunov.wye.desktop
                desktop-file-validate $entry
                grep -qxF 'DBusActivatable=true' $entry
                grep -qxF 'Exec=${wye.package}/bin/wye open %U' $entry
                grep -qxF 'Exec=${wye.package}/bin/wye settings' $entry
                grep -qxF 'Exec=${wye.package}/bin/wye clipboard' $entry
                # No Exec left that relies on $PATH.
                if grep -E '^Exec=wye( |$)' $entry; then exit 1; fi
                grep -qxF 'TryExec=${wye.package}/bin/wye' $entry
                # Exactly one TryExec, in the [Desktop Entry] group.
                awk '/^\[Desktop Entry\]$/ { group = 1; next } /^\[/ { group = 0 } group && /^TryExec=/ { n++ } END { exit n != 1 }' $entry
                # Every icon the package installs.
                for icon in scalable 16x16 24x24 32x32; do test -f ${wye.package}/share/icons/hicolor/$icon/apps/dev.soldunov.wye.svg; done
                test -f ${wye.package}/share/icons/hicolor/symbolic/apps/dev.soldunov.wye-symbolic.svg
                # The tray icon while the primary browser is the Picker (TRAY-02).
                test -f ${wye.package}/share/icons/hicolor/symbolic/apps/dev.soldunov.wye-picker-symbolic.svg
                touch $out
              '';
          # The D-Bus activation files and the systemd user unit the package
          # installs (DEF-04): absolute paths to this package's binaries, and
          # bus activation of the service handed to systemd.
          installed-dbus-files =
            pkgs.runCommand "wye-installed-dbus-files"
              {
                nativeBuildInputs = [
                  pkgs.jq
                  pkgs.nodejs
                ];
              }
              ''
                services=${wye.package}/share/dbus-1/services
                grep -qxF 'Name=dev.soldunov.wye' $services/dev.soldunov.wye.service
                grep -qxF 'Exec=${wye.package}/bin/wye service' $services/dev.soldunov.wye.service
                grep -qxF 'SystemdService=wye.service' $services/dev.soldunov.wye.service
                grep -qxF 'Name=dev.soldunov.wye.Ui' $services/dev.soldunov.wye.Ui.service
                grep -qxF 'Exec=${wye.package}/bin/wye-ui' $services/dev.soldunov.wye.Ui.service
                grep -qxF 'SystemdService=wye-ui.service' $services/dev.soldunov.wye.Ui.service
                grep -qxF 'Name=dev.soldunov.wye.Gtk' $services/dev.soldunov.wye.Gtk.service
                grep -qxF 'Exec=${wye.package}/bin/wye-gtk' $services/dev.soldunov.wye.Gtk.service
                grep -qxF 'SystemdService=wye-gtk.service' $services/dev.soldunov.wye.Gtk.service
                unit=${wye.package}/share/systemd/user/wye.service
                grep -qxF 'Type=dbus' $unit
                grep -qxF 'BusName=dev.soldunov.wye' $unit
                grep -qxF 'ExecStart=${wye.package}/bin/wye service' $unit
                grep -qxF 'KillMode=process' $unit
                # The UI host: bus-activated through systemd, never at login.
                ui=${wye.package}/share/systemd/user/wye-ui.service
                grep -qxF 'Type=dbus' $ui
                grep -qxF 'BusName=dev.soldunov.wye.Ui' $ui
                grep -qxF 'ExecStart=${wye.package}/bin/wye-ui' $ui
                if grep -q '^\[Install\]' $ui; then exit 1; fi
                # The GTK host is bus-activated, never enabled at login.
                gtk=${wye.package}/share/systemd/user/wye-gtk.service
                grep -qxF 'Type=dbus' $gtk
                grep -qxF 'BusName=dev.soldunov.wye.Gtk' $gtk
                grep -qxF 'ExecStart=${wye.package}/bin/wye-gtk' $gtk
                if grep -q '^\[Install\]' $gtk; then exit 1; fi
                # NixOS' `systemd.packages` finds every unit under lib/.
                test -f ${wye.package}/lib/systemd/user/wye.service
                test -f ${wye.package}/lib/systemd/user/wye-ui.service
                test -f ${wye.package}/lib/systemd/user/wye-gtk.service
                # The GTK host is the wrapped Rust binary, with GTK's run-time
                # environment (wrapGAppsHook4) and no Python left.
                test -x ${wye.package}/bin/wye-gtk
                ${wye.package}/bin/wye-gtk --help > /dev/null
                grep -q 'XDG_DATA_DIRS' ${wye.package}/bin/wye-gtk
                # The Adwaita icons its windows name, whatever the desktop has.
                grep -qF '${pkgs.adwaita-icon-theme}/share' ${wye.package}/bin/wye-gtk
                if [ -e ${wye.package}/lib/wye-gtk ]; then exit 1; fi
                # GNOME Shell finds the extension but package installation does not enable it.
                extension=${wye.package}/share/gnome-shell/extensions/wye@dev.soldunov
                jq -e '.uuid == "wye@dev.soldunov" and (."shell-version" | index("48"))' $extension/metadata.json > /dev/null
                for file in extension.js picker.js model.mjs stylesheet.css; do test -f $extension/$file; done
                # The whole directory ships, except its README and tests.
                # (find, not a glob: stdenv sets nullglob, so an unmatched
                # glob would leave a bare `ls` that succeeds.)
                if [ -e $extension/README.md ] || [ -n "$(find $extension -name 'test-*')" ]; then exit 1; fi
                node --check $extension/extension.js
                node --check $extension/picker.js
                node --check $extension/model.mjs
                # No template placeholder left anywhere.
                if grep -rF '@bindir@' ${wye.package}/share; then exit 1; fi
                test -x ${wye.package}/bin/wye
                test -x ${wye.package}/bin/wye-ui
                # The extension's native-messaging host (BEXT-04) ships with it.
                test -x ${wye.package}/bin/wye-native-host
                touch $out
              '';
          # Every QML file of wye-ui against the types its imports and its own
          # cxx-qt bridges declare; any warning fails.
          qmllint = craneLib.mkCargoDerivation (
            wye.commonArgs
            // {
              inherit (wye) cargoArtifacts;
              pname = "wye-ui-qmllint";
              nativeBuildInputs = wye.commonArgs.nativeBuildInputs ++ [ wye.qt.qmllint ];
              # The build writes the module's qmldir and qmltypes to
              # target/cxxqt/qml_modules, which qmllint reads.
              buildPhaseCargoCommand = ''
                cargoWithProfile build --locked --package wye-ui
                wye-qmllint
              '';
              doInstallCargoArtifacts = false;
              installPhaseCommand = "touch $out";
            }
          );
          # Every surface of the installed wye-ui loads offscreen with its
          # fixtures and logs no unexpected warning (crates/wye-ui/src/selftest).
          ui-selftest =
            pkgs.runCommand "wye-ui-selftest"
              {
                # What a desktop session provides and the sandbox lacks: fonts,
                # a UTF-8 locale, a desktop name (without one Kirigami picks
                # its Android-only "breeze-internal" icon theme) and an icon
                # theme.
                FONTCONFIG_FILE = pkgs.makeFontsConf { fontDirectories = [ pkgs.dejavu_fonts ]; };
                LANG = "C.UTF-8";
                XDG_CURRENT_DESKTOP = "KDE";
                XDG_DATA_DIRS = "${pkgs.kdePackages.breeze-icons}/share";
                QT_FORCE_STDERR_LOGGING = "1";
              }
              ''
                export HOME=$TMPDIR XDG_RUNTIME_DIR=$TMPDIR/runtime
                mkdir -m 700 $XDG_RUNTIME_DIR
                ${wye.package}/bin/wye-ui --self-test
                touch $out
              '';
          # Every surface of the installed wye-gtk shows its fixtures on a
          # private Xvfb (cairo renderer) and logs no unexpected GTK, libadwaita
          # or GLib warning (crates/wye-gtk/src/selftest).
          gtk-selftest =
            let
              fonts = pkgs.makeFontsConf {
                fontDirectories = [
                  pkgs.adwaita-fonts
                  pkgs.dejavu_fonts
                ];
              };
            in
            pkgs.runCommand "wye-gtk-selftest"
              {
                nativeBuildInputs = [ pkgs.xvfb ];
                # What a GNOME session provides and the sandbox lacks: fonts
                # and a UTF-8 locale. The icon theme is the wrapper's own, as
                # on a desktop without one.
                FONTCONFIG_FILE = fonts;
                WYE_GTK_FONTCONFIG_FILE = fonts;
                LANG = "C.UTF-8";
              }
              ''
                export HOME=$TMPDIR XDG_RUNTIME_DIR=$TMPDIR/runtime
                mkdir -m 700 $XDG_RUNTIME_DIR
                ${wye.package}/bin/wye-gtk --self-test
                touch $out
              '';
          # Both browser families' extensions, unpacked and zipped, with the
          # manifest the family needs (BEXT-01, BEXT-02).
          extension-manifests =
            pkgs.runCommand "wye-extension-manifests"
              {
                nativeBuildInputs = [
                  pkgs.jq
                  pkgs.unzip
                ];
              }
              ''
                dir=${frontends.extension}/share/wye/extension
                jq -e '.manifest_version == 3 and .background.scripts != null and .browser_specific_settings.gecko.id == "wye@soldunov.dev" and .browser_specific_settings.gecko.strict_min_version == "140.0"' $dir/firefox/manifest.json > /dev/null
                jq -e '.manifest_version == 3 and .background.service_worker != null and .key != null' $dir/chromium/manifest.json > /dev/null
                for family in firefox chromium; do
                  jq -e '.version == "${wye.package.version}"' $dir/$family/manifest.json > /dev/null
                  test -f $dir/$family/background.js
                  test -f $dir/$family/icons/wye-128.png
                  unzip -Z1 $dir/wye-extension-$family.zip | grep -qx manifest.json
                done
                # The Chrome Web Store refuses a `key`; the rest of the manifest
                # is the Chromium build's.
                unzip -Z1 $dir/wye-extension-chromium-webstore.zip | grep -qx manifest.json
                unzip -p $dir/wye-extension-chromium-webstore.zip manifest.json | jq -e 'has("key") | not' > /dev/null
                diff <(unzip -p $dir/wye-extension-chromium-webstore.zip manifest.json | jq -S .) <(jq -S 'del(.key)' $dir/chromium/manifest.json)
                diff <(unzip -Z1 $dir/wye-extension-chromium-webstore.zip | sort) <(unzip -Z1 $dir/wye-extension-chromium.zip | sort)
                # The ID the `key` gives the extension is the one the host
                # manifest allows (BEXT-04).
                id=$(jq -r .key $dir/chromium/manifest.json | base64 -d | sha256sum | cut -c1-32 | tr 0-9a-f a-p)
                grep -qxF "pub const CHROMIUM_EXTENSION_ID: &str = \"$id\";" ${./crates/wye-desktop/src/native_messaging.rs}
                touch $out
              '';
          # Both modules evaluated as a user's configuration would, with the
          # config file, unit, D-Bus files and associations they produce.
          modules-eval = import ./nix/tests/modules.nix {
            inherit
              pkgs
              self
              home-manager
              nixpkgs
              ;
          };
          nix-fmt = pkgs.runCommand "nix-fmt-check" { nativeBuildInputs = [ pkgs.nixfmt ]; } ''
            nixfmt --check ${./flake.nix} ${./nix}/*.nix ${./nix/tests}/*.nix
            touch $out
          '';
        }
      );

      devShells = forAllSystems (
        pkgs:
        let
          inherit ((mkBuild pkgs).wye) qt gtk;
        in
        {
          default = pkgs.mkShell {
            packages =
              (with pkgs; [
                cargo
                rustc
                clippy
                rustfmt
                rust-analyzer
                cargo-deny
                cargo-machete
                desktop-file-utils
                nixfmt
                cachix
                # dbus-daemon for the private-bus tests (crates/wye-service/tests).
                dbus
              ])
              # qmllint and the other Qt tools come with qt.env.
              ++ qt.nativeBuildInputs
              ++ [ qt.qmllint ]
              # glib-compile-resources for wye-gtk's build script, and Xvfb:
              # `wye-gtk --self-test` runs its windows on a private headless
              # X server when one is on PATH.
              ++ gtk.nativeBuildInputs
              ++ [ pkgs.xvfb ];
            buildInputs = qt.buildInputs ++ gtk.buildInputs;
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
            # `wye-gtk --self-test` children draw with GNOME's fonts (Adwaita
            # Sans), whatever the host's fontconfig has.
            WYE_GTK_FONTCONFIG_FILE = pkgs.makeFontsConf {
              fontDirectories = [
                pkgs.adwaita-fonts
                pkgs.dejavu_fonts
              ];
            };
            # `wye-ui --self-test --snapshots` loads the `kde` platform theme
            # from here: the desktop's copy is built against another Qt.
            WYE_SNAPSHOT_QT_PLUGIN_PATH = "${pkgs.kdePackages.plasma-integration}/${pkgs.qt6.qtbase.qtPluginPrefix}";
            # Debug builds compile the cxx-qt C++ without -O, where
            # _FORTIFY_SOURCE only prints a warning per file.
            hardeningDisable = [ "fortify" ];
            # `target/debug/wye-ui` finds the QML modules and plugins the
            # installed one gets from its wrapper.
            shellHook = qt.exportQmake + "\n" + qt.exportRunEnv;
          };
        }
      );

      homeManagerModules.default = import ./nix/hm-module.nix { inherit self release; };
      nixosModules.default = import ./nix/nixos-module.nix { inherit self release; };

      formatter = forAllSystems (pkgs: pkgs.nixfmt);

      overlays.default = final: _prev: {
        wye = self.packages.${final.stdenv.hostPlatform.system}.default;
      };
    };
}
