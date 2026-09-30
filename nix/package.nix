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
    version = (lib.importTOML ../Cargo.toml).workspace.package.version;
  };

  cargoArtifacts = craneLib.buildDepsOnly commonArgs;

  package = craneLib.buildPackage (
    commonArgs
    // {
      inherit cargoArtifacts;
      cargoExtraArgs = "--locked --package wye";
      # Tests run as their own flake check.
      doCheck = false;
      # The source entry runs `wye` from $PATH; the installed one names this
      # package's binary, so launchers find it even when the profile's bin
      # directory is not on their PATH, and TryExec hides the entry once the
      # binary is gone.
      postInstall = ''
        entry=$out/share/applications/dev.soldunov.wye.desktop
        install -Dm644 ${../data/applications/dev.soldunov.wye.desktop} $entry
        substituteInPlace $entry \
          --replace-fail 'Exec=wye open %U' "Exec=$out/bin/wye open %U"
        # Only the main group takes TryExec, not the Exec of any [Desktop Action].
        sed -i "/^\[Desktop Entry\]\$/a TryExec=$out/bin/wye" $entry
        # One line per icon, so data/icons/src (the generators) is never installed.
        install -Dm644 ${../data/icons/hicolor/scalable/apps/dev.soldunov.wye.svg} \
          $out/share/icons/hicolor/scalable/apps/dev.soldunov.wye.svg
        install -Dm644 ${../data/icons/hicolor/16x16/apps/dev.soldunov.wye.svg} \
          $out/share/icons/hicolor/16x16/apps/dev.soldunov.wye.svg
        install -Dm644 ${../data/icons/hicolor/24x24/apps/dev.soldunov.wye.svg} \
          $out/share/icons/hicolor/24x24/apps/dev.soldunov.wye.svg
        install -Dm644 ${../data/icons/hicolor/32x32/apps/dev.soldunov.wye.svg} \
          $out/share/icons/hicolor/32x32/apps/dev.soldunov.wye.svg
        install -Dm644 ${../data/icons/hicolor/symbolic/apps/dev.soldunov.wye-symbolic.svg} \
          $out/share/icons/hicolor/symbolic/apps/dev.soldunov.wye-symbolic.svg
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
