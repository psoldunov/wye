{
  lib,
  craneLib,
  coreutils,
  kdePackages,
  libglvnd,
  libxkbcommon,
  llvmPackages,
  pkg-config,
  qt6,
  runCommand,
  stdenvNoCC,
  writeShellApplication,
  findutils,
  gtk4,
  libadwaita,
  python3,
  wrapGAppsHook4,
}:
let
  root = ../.;
  kde = kdePackages;

  # Qt 6 and KDE Frameworks for the UI host `wye-ui` (crates/wye-ui). The
  # checks and the dev shell use the same set through `qt` below.
  # GNU ld.bfd cannot link Qt from Cargo, so cxx-qt-build passes
  # `-fuse-ld=lld` when an `lld` command exists and the deprecated gold
  # otherwise. Only the wrapped `ld.lld` goes on PATH (under both names): the
  # wrapper adds the store paths as RUNPATH like the default linker does,
  # and a whole bintools package would also replace `ld` itself.
  lldLinker = runCommand "wye-ld-lld" { } ''
    mkdir -p $out/bin
    ln -s ${llvmPackages.bintools}/bin/ld.lld $out/bin/ld.lld
    ln -s ${llvmPackages.bintools}/bin/ld.lld $out/bin/lld
  '';

  # The GTK host uses system-style PyGObject modules, not pip packages.  Its
  # wrapper combines this interpreter with wrapGAppsHook4's GI typelib paths.
  gtkPython = python3.withPackages (ps: [ ps.pygobject3 ]);

  qt = rec {
    # cxx-qt-build reads Qt's whole layout (headers, libraries,
    # qmltyperegistrar, qmlcachegen) from one `qmake -query`. nixpkgs installs
    # every Qt module in its own store path, which cxx-qt does not support
    # (KDAB/cxx-qt#1153), so the build sees one merged prefix instead.
    env = qt6.env "wye-qt-${kde.qtbase.version}" [
      kde.qtdeclarative
      kde.qtsvg
      kde.qtwayland
    ];

    # QML modules and Qt plugins `wye-ui` loads at run time: Kirigami and
    # kirigami-addons for every window, the desktop style, LayerShellQt for
    # the picker, KWindowSystem (also linked by the C++ shim), syntax
    # highlighting for the script editor and KDeclarative's KeySequenceItem
    # (`org.kde.kquickcontrols`). Their propagated inputs come along.
    runtimeInputs = [
      kde.qtbase
      kde.qtdeclarative
      kde.qtsvg
      kde.qtwayland
      kde.kirigami.unwrapped
      kde.kirigami-addons
      kde.qqc2-desktop-style
      kde.layer-shell-qt
      kde.kwindowsystem
      kde.syntax-highlighting
      kde.kdeclarative
    ];

    # Every derivation that compiles the workspace needs these: cxx-qt-lib
    # and wye-ui build C++ against Qt in their build scripts.
    nativeBuildInputs = [
      pkg-config
      lldLinker
    ];
    buildInputs = [
      env
      # `-lGLX -lOpenGL` from Qt6Gui's link interface.
      libglvnd
      libxkbcommon
    ]
    ++ runtimeInputs;
    # Shell code: point cxx-qt-build at the merged prefix. It runs after the
    # setup hooks, because qtbase's hook exports QMAKE of qtbase alone.
    exportQmake = "export QMAKE=${env}/bin/qmake";

    # Kirigami finds its own QML files (StyleSelector::installRoot) through
    # QML_IMPORT_PATH, QML2_IMPORT_PATH or Qt's built-in import path only, not
    # through nixpkgs' NIXPKGS_QT6_QML_IMPORT_PATH. Without it every
    # Kirigami control is registered with an empty URL ("qmlRegisterType
    # requires absolute URLs.") and its styled variants are never found.
    wrapperArgs = [
      "--prefix"
      "QML_IMPORT_PATH"
      ":"
      "${kde.kirigami.unwrapped}/lib/qt-6/qml"
    ];

    # `wye-qt-env CMD…` runs CMD with the QML import path and plugin path
    # the installed `wye-ui` gets from its wrapper, for shells and checks that
    # run an unwrapped build.
    runEnv = stdenvNoCC.mkDerivation {
      name = "wye-qt-env";
      dontUnpack = true;
      nativeBuildInputs = [ qt6.wrapQtAppsHook ];
      buildInputs = runtimeInputs;
      dontWrapQtApps = true;
      installPhase = ''
        runHook preInstall
        mkdir -p $out/bin
        makeQtWrapper ${coreutils}/bin/env $out/bin/wye-qt-env ${lib.escapeShellArgs wrapperArgs}
        runHook postInstall
      '';
    };

    # `wye-qmllint [QML_FILE…]`, from the repository root after a cargo
    # build of wye-ui: qmllint with every import path of `runEnv` and the
    # module cxx-qt-build generated (`$WYE_QML_MODULES`, by default
    # target/cxxqt/qml_modules). Without arguments it lints every QML file
    # of crates/wye-ui/qml. Any warning fails.
    qmllint = writeShellApplication {
      name = "wye-qmllint";
      runtimeInputs = [
        env
        findutils
      ];
      text = ''
        modules=''${WYE_QML_MODULES:-''${CARGO_TARGET_DIR:-target}/cxxqt/qml_modules}
        if [ ! -f "$modules/dev/soldunov/wye/ui/qmldir" ]; then
          echo "wye-qmllint: no $modules/dev/soldunov/wye/ui/qmldir; build wye-ui first" >&2
          exit 1
        fi
        imports=(-I "$modules")
        IFS=: read -r -a dirs <<< "$(${runEnv}/bin/wye-qt-env printenv NIXPKGS_QT6_QML_IMPORT_PATH)"
        for dir in "''${dirs[@]}"; do
          if [ -d "$dir" ]; then imports+=(-I "$dir"); fi
        done
        if [ "$#" -eq 0 ]; then
          mapfile -t files < <(find crates/wye-ui/qml -name '*.qml' | sort)
          set -- "''${files[@]}"
        fi
        qmllint --max-warnings 0 "''${imports[@]}" "$@"
      '';
    };

    # Shell code: export the variables of `runEnv`.
    exportRunEnv = ''
      for var in QT_PLUGIN_PATH NIXPKGS_QT6_QML_IMPORT_PATH QML_IMPORT_PATH; do
        value=$(${runEnv}/bin/wye-qt-env printenv "$var" || true)
        if [ -n "$value" ]; then export "$var=$value"; fi
      done
    '';
  };

  # Only what the Rust build reads: the workspace manifest and lock file, the
  # crates' Rust sources and manifests, the three data files wye-core embeds
  # with include_str!, the desktop entry the CLI tests install as a
  # fixture, and wye-ui's QML, C++ shim and self-test fixtures.
  # commonCargoSources over the whole repository would also pick up every
  # other *.toml (.ensemblr/settings.toml, deny.toml, …), so editing those
  # would rebuild every check.
  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      (lib.fileset.intersection (craneLib.fileset.commonCargoSources root) ../crates)
      # wye-script embeds its JavaScript prelude and template with include_str!.
      (lib.fileset.fileFilter (file: file.hasExt "js") ../crates)
      ../data/services.toml
      ../data/expansion.toml
      ../data/tracking-parameters.toml
      ../data/applications/dev.soldunov.wye.desktop
      # The D-Bus service-file templates crates/wye/tests/e2e.rs activates.
      ../data/dbus
      # The KWin query script wye-service embeds with include_str!.
      ../data/kwin/wye-query.js
      (lib.fileset.maybeMissing ../crates/wye-ui/qml)
      (lib.fileset.maybeMissing ../crates/wye-ui/cpp)
      (lib.fileset.maybeMissing ../crates/wye-ui/fixtures)
    ];
  };

  commonArgs = {
    inherit src;
    strictDeps = true;
    pname = "wye";
    version = (lib.importTOML ../Cargo.toml).workspace.package.version;
    inherit (qt) nativeBuildInputs buildInputs;
    preBuild = qt.exportQmake;
    # The Qt wrapper goes on wye-ui alone (postFixup below): `wye` launches
    # browsers, which must not inherit Qt's plugin and QML paths.
    dontWrapQtApps = true;
  };

  cargoArtifacts = craneLib.buildDepsOnly commonArgs;

  package = craneLib.buildPackage (
    commonArgs
    // {
      inherit cargoArtifacts;
      cargoExtraArgs = "--locked --package wye --package wye-native-host --package wye-ui";
      nativeBuildInputs = commonArgs.nativeBuildInputs ++ [
        qt6.wrapQtAppsHook
        wrapGAppsHook4
      ];
      buildInputs = commonArgs.buildInputs ++ [
        gtk4
        libadwaita
      ];
      # Each frontend wrapper is applied explicitly in postFixup.
      dontWrapGApps = true;
      # Tests run as their own flake check.
      doCheck = false;
      # nixpkgs' fixup would move lib/systemd/user to share/systemd/user, where
      # NixOS' `systemd.packages` does not look; the unit lives in share and
      # lib/systemd/user links to it (see postInstall).
      dontMoveSystemdUserUnits = true;
      # The source entry runs `wye` from $PATH; the installed one names this
      # package's binary (the main Exec and every desktop action's), so launchers find it even when the profile's bin
      # directory is not on their PATH, and TryExec hides the entry once the
      # binary is gone.
      postInstall = ''
        entry=$out/share/applications/dev.soldunov.wye.desktop
        install -Dm644 ${../data/applications/dev.soldunov.wye.desktop} $entry
        substituteInPlace $entry \
          --replace-fail 'Exec=wye open %U' "Exec=$out/bin/wye open %U" \
          --replace-fail 'Exec=wye settings' "Exec=$out/bin/wye settings" \
          --replace-fail 'Exec=wye clipboard' "Exec=$out/bin/wye clipboard"
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
        install -Dm644 ${../data/icons/hicolor/symbolic/apps/dev.soldunov.wye-picker-symbolic.svg} \
          $out/share/icons/hicolor/symbolic/apps/dev.soldunov.wye-picker-symbolic.svg
        # The GTK settings host runs as a module from the package's own source.
        for source in ${../frontends/gtk/wye_gtk}/*.py; do
          install -Dm644 "$source" $out/lib/wye-gtk/wye_gtk/"''${source##*/}"
        done
        # GNOME Shell discovers extensions here. Installation deliberately does
        # not enable it: each user chooses whether Shell owns the picker/tray.
        for source in \
          ${../frontends/gnome-shell/extension.js} \
          ${../frontends/gnome-shell/picker.js} \
          ${../frontends/gnome-shell/model.mjs} \
          ${../frontends/gnome-shell/metadata.json} \
          ${../frontends/gnome-shell/stylesheet.css}; do
          install -Dm644 "$source" \
            $out/share/gnome-shell/extensions/wye@dev.soldunov/"''${source##*/}"
        done
        # D-Bus activation (DEF-04) and the systemd user units of the service
        # and the UI host, with the absolute path of this package's binaries.
        for template in \
          ${../data/dbus/dev.soldunov.wye.service.in}:share/dbus-1/services/dev.soldunov.wye.service \
          ${../data/dbus/dev.soldunov.wye.Ui.service.in}:share/dbus-1/services/dev.soldunov.wye.Ui.service \
          ${../data/dbus/dev.soldunov.wye.Gtk.service.in}:share/dbus-1/services/dev.soldunov.wye.Gtk.service \
          ${../data/systemd/wye.service.in}:share/systemd/user/wye.service \
          ${../data/systemd/wye-ui.service.in}:share/systemd/user/wye-ui.service \
          ${../data/systemd/wye-gtk.service.in}:share/systemd/user/wye-gtk.service; do
          target=$out/''${template#*:}
          install -Dm644 "''${template%%:*}" "$target"
          substituteInPlace "$target" --replace-fail '@bindir@' "$out/bin"
        done
        # NixOS' `systemd.packages` reads lib/systemd/user, not share/.
        mkdir -p $out/lib/systemd/user
        for unit in wye.service wye-ui.service wye-gtk.service; do
          ln -s ../../../share/systemd/user/$unit $out/lib/systemd/user/$unit
        done
      '';
      postFixup = ''
        wrapQtApp $out/bin/wye-ui ${lib.escapeShellArgs qt.wrapperArgs}
        # `gappsWrapperArgs` supplies GTK, libadwaita and GI typelib lookup;
        # source stays separate so Python can import the un-packaged frontend.
        makeWrapper ${gtkPython}/bin/python $out/bin/wye-gtk \
          --prefix PYTHONPATH : $out/lib/wye-gtk \
          "''${gappsWrapperArgs[@]}" \
          --add-flags '-m wye_gtk'
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
  inherit
    commonArgs
    cargoArtifacts
    package
    qt
    ;
}
