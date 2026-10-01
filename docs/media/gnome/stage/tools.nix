# What the GNOME stage needs, all from the repository's own nixpkgs: the Shell
# and the session parts it reads (schemas, icons, fonts, the default
# wallpaper), the Mesa they render with (the host's drivers may not load),
# and the tools the stage scripts run.
{ pkgs }:
let
  schemaPackages = [
    pkgs.gnome-shell
    pkgs.mutter
    pkgs.gsettings-desktop-schemas
    pkgs.gtk4
  ];
in
{
  shell = pkgs.gnome-shell;
  dbus = pkgs.dbus;
  # gdbus and gsettings.
  glib = pkgs.glib.bin;
  # glib-compile-schemas, for an extension that ships its own schemas.
  compile = pkgs.glib.dev;
  mesa = pkgs.mesa;
  oxipng = pkgs.oxipng;
  magick = pkgs.imagemagick;

  # drive.py and shot.py talk D-Bus; ui.py finds widgets through AT-SPI.
  python = pkgs.python3.withPackages (p: [
    p.dbus-python
    p.pillow
    p.pygobject3
  ]);
  # The accessibility bus (its configuration and registry daemon) and the
  # typelibs ui.py loads: Atspi and what it depends on.
  atspi = pkgs.symlinkJoin {
    name = "wye-stage-atspi";
    paths = map pkgs.lib.getLib [
      pkgs.at-spi2-core
      pkgs.gobject-introspection
      pkgs.glib
    ];
  };

  # Every schema the Shell, Mutter and GTK read, compiled into one directory:
  # GSETTINGS_SCHEMA_DIR points there.
  schemas = pkgs.runCommand "wye-stage-schemas" { nativeBuildInputs = [ pkgs.glib.dev ]; } ''
    mkdir -p $out
    for package in ${toString schemaPackages}; do
      for file in $package/share/gsettings-schemas/*/glib-2.0/schemas/*.xml \
                  $package/share/gsettings-schemas/*/glib-2.0/schemas/*.override; do
        # The Shell ships Mutter's schema too: the later copy wins.
        [ -e "$file" ] && install -m644 "$file" $out/
      done
    done
    glib-compile-schemas $out
  '';

  # The data directories of the session: icon themes, the a11y bus that GTK
  # registers with, and the desktop entry of the app the picker's link comes
  # from (GNOME Terminal, "Terminal"). Nothing else, so no host service
  # starts on the stage bus.
  share = pkgs.symlinkJoin {
    name = "wye-stage-share";
    # GNOME Terminal first: adwaita-icon-theme's share/icons/hicolor is a
    # symlink to hicolor-icon-theme, and joined before Terminal it left
    # hicolor/scalable/apps without Terminal's icon (the picker's "from
    # Terminal" then showed the generic one).
    paths = [
      pkgs.gnome-terminal
      pkgs.adwaita-icon-theme
      pkgs.hicolor-icon-theme
      pkgs.at-spi2-core
    ];
  };

  # The Settings portal, as GNOME serves it: GTK 4 reads the colour scheme,
  # accent and fonts from it, and from nothing else on Wayland. The GTK
  # backend reads them from GSettings. stage.sh allows no other portal.
  portal = pkgs.symlinkJoin {
    name = "wye-stage-portal";
    paths = [
      pkgs.xdg-desktop-portal
      pkgs.xdg-desktop-portal-gtk
    ];
  };

  # Adwaita Sans and Mono, GNOME's default fonts, and nothing from the host.
  fonts = pkgs.makeFontsConf {
    fontDirectories = [
      pkgs.adwaita-fonts
      pkgs.noto-fonts-color-emoji
    ];
  };

  # GNOME's default wallpaper as PNG: the Shell loads it through gdk-pixbuf,
  # which has no JPEG XL loader here. 8 bits and no larger than the 2560×1600
  # screen needs: the 16-bit original takes the Shell seconds to load.
  wallpaper = pkgs.runCommand "wye-stage-wallpaper" { nativeBuildInputs = [ pkgs.imagemagick ]; } ''
    mkdir -p $out
    for variant in l d; do
      magick ${pkgs.gnome-backgrounds}/share/backgrounds/gnome/adwaita-$variant.jxl \
        -resize '2560x2560>' -depth 8 $out/adwaita-$variant.png
    done
  '';
}
