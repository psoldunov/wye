{
  lib,
  craneLib,
}:
let
  root = ../.;

  # Cargo sources plus the data files the crates embed with include_str!.
  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions [
      (craneLib.fileset.commonCargoSources root)
      ../data
    ];
  };

  commonArgs = {
    inherit src;
    strictDeps = true;
    pname = "wye";
    version = (craneLib.crateNameFromCargoToml { cargoToml = ../crates/wye/Cargo.toml; }).version;
  };

  cargoArtifacts = craneLib.buildDepsOnly commonArgs;

  package = craneLib.buildPackage (
    commonArgs
    // {
      inherit cargoArtifacts;
      cargoExtraArgs = "--locked --package wye";
      # Tests run as their own flake check.
      doCheck = false;
      postInstall = ''
        install -Dm644 ${../data/applications/dev.soldunov.wye.desktop} \
          $out/share/applications/dev.soldunov.wye.desktop
        install -Dm644 ${../data/icons/hicolor/scalable/apps/dev.soldunov.wye.svg} \
          $out/share/icons/hicolor/scalable/apps/dev.soldunov.wye.svg
      '';
      meta = {
        description = "Native Linux browser picker";
        homepage = "https://github.com/psoldunov/wye";
        license = lib.licenses.mit;
        mainProgram = "wye";
        platforms = lib.platforms.linux;
      };
    }
  );
in
{
  inherit commonArgs cargoArtifacts package;
}
