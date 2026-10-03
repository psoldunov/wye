# Build image for the Wye AppImage (packaging/appimage/build.sh builds and
# uses it): Arch Linux with the Rust toolchain, Qt 6 and KDE Frameworks
# (wye-ui), GTK 4 and libadwaita (wye-gtk), Mesa, and the pinned AnyLinux
# AppImage tooling (quick-sharun, sharun, appimagetool with uruntime).
#
#   docker build -t wye-appimage-build packaging/appimage
#   docker build -t wye-appimage-build \
#     --build-arg BASE=ghcr.io/pkgforge-dev/archlinux:aarch64 packaging/appimage
#
# Arch, because it is the base the AnyLinux tooling documents and tests, and
# it has every toolkit Wye needs (GTK 4.22, libadwaita 1.9, Qt 6.11, KF6 6.30).
# Everything the AppImage bundles, glibc and its dynamic loader included,
# comes from these packages, so the AppImage runs on distributions far older
# than this image. The source goes in with `docker cp` at build time, and the
# cargo registry and target directory live in Docker volumes (see build.sh).
#
# The image is built for the machine's own architecture, x86_64 or aarch64.
# The official archlinux image is x86_64 only; on aarch64 build.sh passes
# BASE, the Arch Linux ARM image of pkgforge-dev/docker-archlinux (the
# project behind the AnyLinux tooling, which builds its aarch64 AppImages on
# it). Arch Linux ARM builds Arch's own package recipes, so the package names
# below are the same.
ARG BASE=archlinux:latest
FROM ${BASE}

# - rust, lld: cxx-qt-build links Qt with -fuse-ld=lld when an `lld` command
#   exists; GNU ld.bfd cannot link Qt from Cargo.
# - qt6-*, kirigami, kirigami-addons, qqc2-desktop-style, kdeclarative,
#   layer-shell-qt, syntax-highlighting, kwindowsystem, breeze-icons: what
#   wye-ui builds against and loads at run time (qt.runtimeInputs in
#   nix/package.nix); quick-sharun copies their QML modules and plugins.
#   hunspell is the library of Sonnet's spell-checking back end (the script
#   editor's text area); the dictionaries are the host's.
# - gtk4, libadwaita, gtksourceview5, gtk4-layer-shell, adwaita-icon-theme,
#   librsvg: wye-gtk and the SVG loader for its icons.
# - mesa: OpenGL drivers, llvmpipe included, so both hosts draw without the
#   host's GL.
# - patchelf, strace, xorg-server-xvfb, xorg-xauth, dbus, ttf-dejavu:
#   quick-sharun's dependencies; it runs each binary under strace on Xvfb for
#   a few seconds to find the libraries it dlopens.
RUN pacman -Syu --noconfirm --needed \
      base-devel \
      git \
      rust \
      lld \
      qt6-base \
      qt6-declarative \
      qt6-svg \
      qt6-wayland \
      kirigami \
      kirigami-addons \
      qqc2-desktop-style \
      kdeclarative \
      layer-shell-qt \
      syntax-highlighting \
      kwindowsystem \
      breeze-icons \
      hunspell \
      gtk4 \
      libadwaita \
      gtksourceview5 \
      gtk4-layer-shell \
      adwaita-icon-theme \
      hicolor-icon-theme \
      librsvg \
      mesa \
      libglvnd \
      patchelf \
      strace \
      wget \
      xorg-server-xvfb \
      xorg-xauth \
      dbus \
      ttf-dejavu \
      desktop-file-utils \
    && pacman -Scc --noconfirm

# The AppImage tooling, each pinned to a release (or, for quick-sharun, a
# commit of pkgforge-dev/Anylinux-AppImages) and checked against its SHA-256.
# The sharun and appimagetool digests are the ones that quick-sharun commit
# pins itself (SHARUN_SHA, APPIMAGETOOL_SHA in quick-sharun.sh), so
# quick-sharun accepts these copies without downloading them again.
# appimagetool-full embeds the uruntime AppImage runtime it writes into the
# AppImage (uruntime v0.8.1, appimage-dwarfs-lite, pinned with its digest in
# appimagetool's src/pinned.rs) and mkdwarfs, so making the AppImage needs no
# network either. sharun and appimagetool are per architecture, each with the
# digest quick-sharun pins for it.
ARG QUICK_SHARUN_COMMIT=ff9260f61b9953bd07dc76d6d8b204ed816e8731
ARG QUICK_SHARUN_SHA256=8026711b271c0d67d37075cd7e8c50dbd9fa635c012f9edb92a2c0d41573664b
ARG SHARUN_VERSION=3.5.0
ARG SHARUN_SHA256_X86_64=a84935e91826cc38834f35eee099269ab643f97a47d3aaf5c171d87fda3d3c1f
ARG SHARUN_SHA256_AARCH64=71af83a5980aeb568e10479e9fe1e0f3ddcaea0be3c406015c53e83124389215
ARG APPIMAGETOOL_VERSION=0.5.2
ARG APPIMAGETOOL_SHA256_X86_64=49999e2ba854fd826aa00cb872a6b7294137d32346aec6e21be63cae49bdc854
ARG APPIMAGETOOL_SHA256_AARCH64=f7fc30fa0aaed7e4d1652117d38b6945f2b2f02cb28e4aacf15b8300be11f0c4
RUN set -eu \
 && arch=$(uname -m) \
 && case $arch in \
      x86_64) sharun_sha256=$SHARUN_SHA256_X86_64 tool_sha256=$APPIMAGETOOL_SHA256_X86_64 ;; \
      aarch64) sharun_sha256=$SHARUN_SHA256_AARCH64 tool_sha256=$APPIMAGETOOL_SHA256_AARCH64 ;; \
      *) echo "no AppImage tooling pinned for $arch" >&2; exit 1 ;; \
    esac \
 && mkdir -p /opt/appimage-tools \
 && cd /opt/appimage-tools \
 && curl -fsSLo quick-sharun \
      "https://raw.githubusercontent.com/pkgforge-dev/Anylinux-AppImages/$QUICK_SHARUN_COMMIT/useful-tools/quick-sharun.sh" \
 && curl -fsSLo "sharun+helper-libs-$arch.tar" \
      "https://github.com/pkgforge-dev/Anylinux-sharun/releases/download/$SHARUN_VERSION/sharun+helper-libs-$arch.tar" \
 && curl -fsSLo appimagetool \
      "https://github.com/pkgforge-dev/appimagetool/releases/download/$APPIMAGETOOL_VERSION/appimagetool-full-$arch-linux" \
 && printf '%s  %s\n' \
      "$QUICK_SHARUN_SHA256" quick-sharun \
      "$sharun_sha256" "sharun+helper-libs-$arch.tar" \
      "$tool_sha256" appimagetool \
    | sha256sum -c - \
 && chmod 755 quick-sharun appimagetool

# quick-sharun rewrites /usr/bin, /usr/lib and /usr/share inside bundled
# binaries and libraries (p11-kit, GtkSourceView, Rust's debug-info path in
# Wye's own) to /tmp/<name>, with the name fixed when the AppImage is built,
# and a hook links those names to the mount at run time. Any local user can
# plant such a link first, and p11-kit would then load their PKCS#11
# modules. It has no switch for that, so its three patch functions are made
# to return at once; the grep fails the build when a newer quick-sharun no
# longer has them. Unpatched, p11-kit reads the host's own root-owned
# paths, and GtkSourceView finds its files through XDG_DATA_DIRS, which
# sharun starts with the AppDir's share/.
RUN sed -i -E \
      '/^_patch_away_usr_(bin|lib|share)_dir\(\) \{$/a\	return 1 # wye: no /tmp path mapping' \
      /opt/appimage-tools/quick-sharun \
 && test "$(grep -c 'return 1 # wye: no /tmp path mapping' /opt/appimage-tools/quick-sharun)" = 3

# cxx-qt-build finds Qt through qmake; cargo's home and target directory are
# the volumes build.sh mounts. APPIMAGETOOL points quick-sharun at the copy
# above; build.sh copies the sharun tarball to quick-sharun's TMPDIR.
ENV QMAKE=/usr/bin/qmake6 \
    CARGO_HOME=/build/cargo-home \
    CARGO_TARGET_DIR=/build/target \
    APPIMAGETOOL=/opt/appimage-tools/appimagetool \
    PATH=/opt/appimage-tools:$PATH
WORKDIR /build
