{
  description = "Wye: native Linux browser picker";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});

      mkBuild =
        pkgs:
        let
          craneLib = crane.mkLib pkgs;
        in
        {
          inherit craneLib;
          wye = pkgs.callPackage ./nix/package.nix { inherit craneLib; };
        };
    in
    {
      packages = forAllSystems (
        pkgs:
        let
          inherit (mkBuild pkgs) wye;
        in
        {
          default = wye.package;
          wye = wye.package;
        }
      );

      checks = forAllSystems (
        pkgs:
        let
          inherit (mkBuild pkgs) craneLib wye;
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
                touch $out
              '';
          # The D-Bus activation files and the systemd user unit the package
          # installs (DEF-04): absolute paths to this package's binaries, and
          # bus activation of the service handed to systemd.
          installed-dbus-files = pkgs.runCommand "wye-installed-dbus-files" { } ''
            services=${wye.package}/share/dbus-1/services
            grep -qxF 'Name=dev.soldunov.wye' $services/dev.soldunov.wye.service
            grep -qxF 'Exec=${wye.package}/bin/wye service' $services/dev.soldunov.wye.service
            grep -qxF 'SystemdService=wye.service' $services/dev.soldunov.wye.service
            grep -qxF 'Name=dev.soldunov.wye.Ui' $services/dev.soldunov.wye.Ui.service
            grep -qxF 'Exec=${wye.package}/bin/wye-ui' $services/dev.soldunov.wye.Ui.service
            unit=${wye.package}/share/systemd/user/wye.service
            grep -qxF 'Type=dbus' $unit
            grep -qxF 'BusName=dev.soldunov.wye' $unit
            grep -qxF 'ExecStart=${wye.package}/bin/wye service' $unit
            grep -qxF 'KillMode=process' $unit
            # No template placeholder left anywhere.
            if grep -rF '@bindir@' ${wye.package}/share; then exit 1; fi
            test -x ${wye.package}/bin/wye
            test -x ${wye.package}/bin/wye-ui
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
          nix-fmt = pkgs.runCommand "nix-fmt-check" { nativeBuildInputs = [ pkgs.nixfmt ]; } ''
            nixfmt --check ${./flake.nix} ${./nix}/*.nix
            touch $out
          '';
        }
      );

      devShells = forAllSystems (
        pkgs:
        let
          inherit ((mkBuild pkgs).wye) qt;
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
              ++ [ qt.qmllint ];
            inherit (qt) buildInputs;
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
            # Debug builds compile the cxx-qt C++ without -O, where
            # _FORTIFY_SOURCE only prints a warning per file.
            hardeningDisable = [ "fortify" ];
            # `target/debug/wye-ui` finds the QML modules and plugins the
            # installed one gets from its wrapper.
            shellHook = qt.exportQmake + "\n" + qt.exportRunEnv;
          };
        }
      );

      formatter = forAllSystems (pkgs: pkgs.nixfmt);

      overlays.default = final: _prev: {
        wye = self.packages.${final.stdenv.hostPlatform.system}.default;
      };
    };
}
