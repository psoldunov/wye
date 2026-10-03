# Build image for the Wye .rpm: Fedora 44 with the Rust toolchain, Qt 6 and
# KDE Frameworks (wye-ui), GTK 4 and libadwaita (wye-gtk), rpm-build and
# rpmlint. packaging/rpm/build.sh builds it and runs rpmbuild in it; the
# source goes in with `docker cp`, never as a bind mount.
#
#   docker build -t wye-rpm-build:fc44 packaging/rpm
#
# The build inputs follow nix/package.nix (qt.runtimeInputs, gtk.buildInputs).
FROM fedora:44

# - rust, cargo: Fedora 44 ships 1.98; the workspace needs 1.88.
# - lld: cxx-qt-build links Qt with -fuse-ld=lld when an `lld` command
#   exists, else gold; GNU ld.bfd cannot link Qt from Cargo.
# - qt6-qtbase-devel, qt6-qtdeclarative-devel: qmake6 (cxx-qt-build reads
#   every Qt path from `qmake -query`), moc, rcc, qmltyperegistrar,
#   qmlcachegen and the Gui/Quick/QuickControls2/Widgets modules.
# - kf6-kwindowsystem-devel: the C++ shim of wye-ui links KF6WindowSystem
#   through pkg-config.
# - glib2-devel: glib-compile-resources for the GResource of wye-gtk.
# - glibc-langpack-en: rpmlint switches to en_US.UTF-8 for its spelling check.
# - The kf6-*/layer-shell-qt/qt6-qtsvg/qt6-qtwayland run-time packages are
#   here too so the image can run `wye-ui --self-test` on a fresh build.
RUN dnf install -y --setopt=install_weak_deps=False \
      cargo \
      rust \
      gcc \
      gcc-c++ \
      lld \
      pkgconf-pkg-config \
      git-core \
      rpm-build \
      rpmlint \
      glibc-langpack-en \
      redhat-rpm-config \
      systemd-rpm-macros \
      desktop-file-utils \
      qt6-qtbase-devel \
      qt6-qtdeclarative-devel \
      qt6-qtsvg \
      qt6-qtwayland \
      kf6-kwindowsystem-devel \
      kf6-kirigami \
      kf6-kirigami-addons \
      kf6-qqc2-desktop-style \
      kf6-kdeclarative \
      kf6-syntax-highlighting \
      layer-shell-qt \
      libglvnd-devel \
      libxkbcommon-devel \
      glib2-devel \
      gtk4-devel \
      gtk4-layer-shell-devel \
      gtksourceview5-devel \
      libadwaita-devel \
    && dnf clean all

# Fedora installs the Qt 6 qmake as qmake6; cxx-qt-build reads QMAKE first.
ENV QMAKE=/usr/bin/qmake6 \
    CARGO_HOME=/build/cargo-home \
    CARGO_TARGET_DIR=/build/target
WORKDIR /build
