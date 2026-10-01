# What the stage scripts need beyond the desktop itself: Python with the
# Wayland and D-Bus bindings, the KDE protocol files, the image tools, and the
# Mesa that matches the Qt Wye is built with (the host's drivers may not).
{ pkgs }:
{
  python = pkgs.python3.withPackages (p: [
    p.dbus-python
    p.pillow
    p.pywayland
  ]);
  protocols = pkgs.kdePackages.plasma-wayland-protocols;
  gifski = pkgs.gifski;
  oxipng = pkgs.oxipng;
  mesa = pkgs.mesa;
}
