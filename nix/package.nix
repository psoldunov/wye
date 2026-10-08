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
  glib,
  gtk4,
  gtk4-layer-shell,
  gtksourceview5,
  libadwaita,
  wrapGAppsHook4,
  adwaita-icon-theme,
  hicolor-icon-theme,
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

  # GTK 4, libadwaita and GtkSourceView 5 for the GNOME window host `wye-gtk`
  # (crates/wye-gtk): its -sys crates find them with pkg-config, and its
  # build script compiles the GResource with `glib-compile-resources` (glib's
  # dev output). gtk4-layer-shell puts the picker and the tray-menu popup on
  # the overlay layer of wlroots compositors (02-picker.md, "Linux notes").
  # The checks and the dev shell use the same set.
  gtk = {
    nativeBuildInputs = [ glib ];
    buildInputs = [
      gtk4
      gtk4-layer-shell
      libadwaita
      gtksourceview5
    ];
  };

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
      # wye-gtk's GResource bundles the app icon for its About dialog.
      ../data/icons/hicolor/scalable/apps/dev.soldunov.wye.svg
      # The D-Bus service-file templates crates/wye/tests/e2e.rs activates.
      ../data/dbus
      # The KWin query script wye-service embeds with include_str!.
      ../data/kwin/wye-query.js
      (lib.fileset.maybeMissing ../crates/wye-ui/qml)
      (lib.fileset.maybeMissing ../crates/wye-ui/cpp)
      (lib.fileset.maybeMissing ../crates/wye-ui/fixtures)
      # wye-gtk's GResource sources (CSS, icons) and self-test fixtures.
      (lib.fileset.maybeMissing ../crates/wye-gtk/data)
      (lib.fileset.maybeMissing ../crates/wye-gtk/fixtures)
    ];
  };

  commonArgs = {
    inherit src;
    strictDeps = true;
    pname = "wye";
    version = (lib.importTOML ../Cargo.toml).workspace.package.version;
    nativeBuildInputs = qt.nativeBuildInputs ++ gtk.nativeBuildInputs;
    buildInputs = qt.buildInputs ++ gtk.buildInputs;
    preBuild = qt.exportQmake;
    # The Qt wrapper goes on wye-ui alone and the GTK one on wye-gtk alone
    # (postFixup below): `wye` launches browsers, which must not inherit
    # either toolkit's plugin, QML or GI paths.
    dontWrapQtApps = true;
  };

  cargoArtifacts = craneLib.buildDepsOnly commonArgs;

  # The flake checks that compile the workspace (clippy, test, qmllint) use
  # the dev profile, as the review gates in AGENTS.md do. With the release
  # profile (thin LTO, one codegen unit) the test build alone took 48 minutes
  # on a CI runner.
  devArgs = commonArgs // {
    CARGO_PROFILE = "dev";
  };
  devCargoArtifacts = craneLib.buildDepsOnly devArgs;

  package = craneLib.buildPackage (
    commonArgs
    // {
      inherit cargoArtifacts;
      cargoExtraArgs = "--locked --package wye --package wye-native-host --package wye-ui --package wye-gtk";
      nativeBuildInputs = commonArgs.nativeBuildInputs ++ [
        qt6.wrapQtAppsHook
        wrapGAppsHook4
      ];
      # Each frontend wrapper is applied explicitly in postFixup.
      dontWrapGApps = true;
      # Tests run as their own flake check.
      doCheck = false;
      # ADV-12: this package ships the GTK host (bin/wye-gtk and its D-Bus
      # and systemd files). The home-manager module declares the host's
      # unit only for a package that says so, so an older release package
      # without it gets nothing that names a missing binary.
      passthru.hasGtk = true;
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
        # GNOME Shell discovers extensions here. Installation deliberately does
        # not enable it: each user chooses whether Shell owns the picker/tray.
        # The whole directory ships except its README and its tests
        # (test-*.mjs), so the extension can add files without an edit here.
        extension=$out/share/gnome-shell/extensions/wye@dev.soldunov
        mkdir -p $extension
        cp -r ${../frontends/gnome-shell}/. $extension/
        chmod -R u+w $extension
        rm -f $extension/README.md $extension/test-*.mjs
        find $extension -type d -exec chmod 755 {} +
        find $extension -type f -exec chmod 644 {} +
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
        # GTK's run-time environment (GSettings schemas, gdk-pixbuf loaders,
        # XDG_DATA_DIRS) from wrapGAppsHook4, on the GTK host alone. The
        # Adwaita icons its windows name come along, after the session's own
        # data directories: a window manager without them shows no
        # missing-image icons.
        wrapGApp $out/bin/wye-gtk \
          --suffix XDG_DATA_DIRS : ${adwaita-icon-theme}/share:${hicolor-icon-theme}/share
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
    devArgs
    devCargoArtifacts
    package
    qt
    gtk
    ;
}
