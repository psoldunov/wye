# Front ends that are not Rust: the browser extension (18-onboarding.md,
# BEXT-01).
{
  lib,
  stdenvNoCC,
  jq,
  zip,
  version,
}:
{
  # The unpacked extension of each browser family and one zip of each, as
  # `frontends/extension/build.sh` assembles them. Wye writes the
  # native-messaging manifests itself at run time (the service at every
  # start, or `wye extension install`); nothing here touches a browser's
  # directories.
  extension = stdenvNoCC.mkDerivation {
    pname = "wye-extension";
    inherit version;
    src = ../frontends/extension;
    nativeBuildInputs = [
      jq
      zip
    ];
    dontConfigure = true;
    buildPhase = ''
      runHook preBuild
      for family in firefox chromium; do
        sh ./build.sh "$family" "$family"
        # Stamp the workspace version rather than trust the checked-in copy.
        jq --arg version ${lib.escapeShellArg version} '.version = $version' \
          "$family/manifest.json" > manifest.stamped.json
        mv manifest.stamped.json "$family/manifest.json"
        # Fixed timestamps and order keep the zip reproducible.
        (cd "$family" && find . -type f | LC_ALL=C sort | TZ=UTC zip -X -q -@ "../wye-extension-$family.zip")
      done
      runHook postBuild
    '';
    installPhase = ''
      runHook preInstall
      mkdir -p $out/share/wye/extension
      for family in firefox chromium; do
        cp -r "$family" $out/share/wye/extension/
        cp "wye-extension-$family.zip" $out/share/wye/extension/
      done
      runHook postInstall
    '';
    meta = {
      description = "Wye browser extension: send a link or page to Wye";
      homepage = "https://github.com/psoldunov/wye";
      license = lib.licenses.mit;
      platforms = lib.platforms.linux;
    };
  };
}
