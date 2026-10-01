# GNOME 48's GTK/libadwaita and Shell runtime under OrbStack Docker.
FROM fedora:42
RUN dnf -y install \
    adwaita-icon-theme dbus-daemon gjs gnome-shell \
    gtk4 libadwaita mesa-dri-drivers python3 python3-gobject \
    xorg-x11-server-Xvfb google-noto-sans-fonts \
    && dnf clean all
WORKDIR /workspace
ENV PYTHONPATH=/workspace/frontends/gtk \
    XDG_CURRENT_DESKTOP=GNOME \
    GDK_BACKEND=x11 \
    GSK_RENDERER=cairo
