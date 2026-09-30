# Front ends that are not Rust: the Plasma system-tray applet
# (01-tray-menu.md) and the browser extension (18-onboarding.md, BEXT-01).
{
  lib,
  stdenvNoCC,
  jq,
  zip,
  kdePackages,
  runCommand,
  version,
  # From nix/package.nix (`qt.plasmaQmlModules`), shared with the dev shell.
  qmlModules,
}:
let
  kde = kdePackages;
  plasmoidId = "dev.soldunov.wye";
  # One store path for the applet and its tests, so the tests' relative
  # import of the applet resolves.
  frontend = ../frontends/plasma;
  source = "${frontend}/${plasmoidId}";

  qmlImportPath = lib.makeSearchPath "lib/qt-6/qml" qmlModules;
in
{
  inherit plasmoidId qmlImportPath;

  plasmoid = stdenvNoCC.mkDerivation {
    pname = "wye-plasmoid";
    inherit version;
    src = source;
    nativeBuildInputs = [ jq ];
    # KPlugin.Version is what Plasma shows in "Add Widgets"; stamp it from the
    # workspace version rather than trusting the checked-in copy.
    buildPhase = ''
      runHook preBuild
      jq --arg version ${lib.escapeShellArg version} \
        '.KPlugin.Version = $version' metadata.json > metadata.stamped.json
      mv metadata.stamped.json metadata.json
      runHook postBuild
    '';
    installPhase = ''
      runHook preInstall
      mkdir -p $out/share/plasma/plasmoids/${plasmoidId}
      cp -r . $out/share/plasma/plasmoids/${plasmoidId}/
      runHook postInstall
    '';
    meta = {
      description = "Wye system tray applet for KDE Plasma 6";
      homepage = "https://github.com/psoldunov/wye";
      license = lib.licenses.mit;
      platforms = lib.platforms.linux;
    };
  };

  # The unpacked extension of each browser family and one zip of each, as
  # `frontends/extension/build.sh` assembles them. Wye writes the
  # native-messaging manifests itself at run time (`wye-native-host
  # --install`); nothing here touches a browser's directories.
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
        # Stamp the workspace version, like the applet's KPlugin.Version.
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

  # The metadata Plasma needs to put the applet in the system tray, and
  # qmllint over every QML file of the applet and its tests.
  lint =
    runCommand "wye-plasmoid-lint"
      {
        nativeBuildInputs = [
          jq
          kde.qtdeclarative
        ];
      }
      ''
        jq -e '
          .KPlugin.Id == "${plasmoidId}"
          and .KPackageStructure == "Plasma/Applet"
          and .["X-Plasma-NotificationArea"] == "true"
          and .KPlugin.EnabledByDefault == true
          and (.["X-Plasma-API-Minimum-Version"] | tonumber) >= 6.4
        ' ${source}/metadata.json > /dev/null
        imports=()
        for dir in ${lib.concatStringsSep " " (map (p: "${p}/lib/qt-6/qml") qmlModules)}; do
          imports+=(-I "$dir")
        done
        qmllint "''${imports[@]}" -I ${source}/contents/ui \
          $(find ${frontend} -name '*.qml' | sort)
        touch $out
      '';
}
