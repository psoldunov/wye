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
          # The entry the package installs, with its absolute Exec and TryExec.
          installed-desktop-entry =
            pkgs.runCommand "wye-installed-desktop-entry-valid"
              { nativeBuildInputs = [ pkgs.desktop-file-utils ]; }
              ''
                entry=${wye.package}/share/applications/dev.soldunov.wye.desktop
                desktop-file-validate $entry
                grep -qxF 'Exec=${wye.package}/bin/wye open %U' $entry
                grep -qxF 'TryExec=${wye.package}/bin/wye' $entry
                # Exactly one TryExec, in the [Desktop Entry] group.
                awk '/^\[Desktop Entry\]$/ { group = 1; next } /^\[/ { group = 0 } group && /^TryExec=/ { n++ } END { exit n != 1 }' $entry
                # Every icon the package installs.
                for icon in scalable 16x16 24x24 32x32; do test -f ${wye.package}/share/icons/hicolor/$icon/apps/dev.soldunov.wye.svg; done
                test -f ${wye.package}/share/icons/hicolor/symbolic/apps/dev.soldunov.wye-symbolic.svg
                touch $out
              '';
          nix-fmt = pkgs.runCommand "nix-fmt-check" { nativeBuildInputs = [ pkgs.nixfmt ]; } ''
            nixfmt --check ${./flake.nix} ${./nix}/*.nix
            touch $out
          '';
        }
      );

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
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
          ];
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
        };
      });

      formatter = forAllSystems (pkgs: pkgs.nixfmt);

      overlays.default = final: _prev: {
        wye = self.packages.${final.stdenv.hostPlatform.system}.default;
      };
    };
}
