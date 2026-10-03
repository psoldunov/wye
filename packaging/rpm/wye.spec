# RPM spec for Wye, one binary package with all four binaries: wye (CLI and
# session service), wye-native-host, wye-ui (Qt/Kirigami) and wye-gtk
# (GTK 4/libadwaita). packaging/rpm/build.sh builds it in the Fedora image of
# packaging/rpm/Dockerfile; by hand, from a tarball of the source tree named
# wye-VERSION.tar.gz with a top directory wye-VERSION/:
#
#   rpmbuild -ba --define "wye_version 1.0.0" packaging/rpm/wye.spec
#
# The version is the workspace `version` of Cargo.toml, passed in as
# wye_version, so nothing here changes per release. build.sh writes it as a
# `%%global wye_version` line at the top of the copy it builds, so the src.rpm
# rebuilds (and rpmlint parses it) without the define. Cargo fetches the crates
# (--locked), so the build needs the network: this is not a mock/Koji spec.
# CARGO_TARGET_DIR, when set, is used as is (build.sh keeps it on a volume so
# rebuilds are incremental).

%{!?wye_version: %{error:define wye_version, the workspace version in Cargo.toml}}

Name:           wye
Version:        %{wye_version}
Release:        1%{?dist}
Summary:        Browser picker that sends every link to the right browser
License:        MIT
URL:            https://github.com/psoldunov/wye
# A tag's GitHub archive has this name and top directory; build.sh makes the
# same tarball from the working tree.
Source0:        %{url}/archive/v%{version}/%{name}-%{version}.tar.gz
# The architectures the release workflow builds and smoke-tests.
ExclusiveArch:  x86_64 aarch64

BuildRequires:  cargo >= 1.88
BuildRequires:  rust >= 1.88
BuildRequires:  gcc
BuildRequires:  gcc-c++
BuildRequires:  lld
BuildRequires:  pkgconfig
BuildRequires:  desktop-file-utils
BuildRequires:  systemd-rpm-macros
BuildRequires:  qt6-qtbase-devel
BuildRequires:  qt6-qtdeclarative-devel
BuildRequires:  pkgconfig(KF6WindowSystem)
BuildRequires:  libglvnd-devel
BuildRequires:  libxkbcommon-devel
BuildRequires:  glib2-devel
BuildRequires:  pkgconfig(gtk4) >= 4.22
BuildRequires:  pkgconfig(libadwaita-1) >= 1.9
BuildRequires:  pkgconfig(gtksourceview-5) >= 5.18
BuildRequires:  pkgconfig(gtk4-layer-shell-0)

# wye-ui loads these QML modules and Qt plugins at run time; nothing links
# them, so the automatic ELF requires do not see them (qt.runtimeInputs in
# nix/package.nix): org.kde.kirigami, org.kde.kirigamiaddons.*, the
# org.kde.desktop style, org.kde.layershell, org.kde.syntaxhighlighting,
# org.kde.kquickcontrols (KeySequenceItem), QtQuick.* and QtQml, the SVG image
# format and the Wayland platform plugin.
Requires:       kf6-kirigami%{?_isa}
Requires:       kf6-kirigami-addons%{?_isa}
Requires:       kf6-qqc2-desktop-style%{?_isa}
Requires:       kf6-kdeclarative%{?_isa}
Requires:       kf6-syntax-highlighting%{?_isa}
Requires:       layer-shell-qt%{?_isa}
Requires:       qt6-qtdeclarative%{?_isa}
Requires:       qt6-qtsvg%{?_isa}
Requires:       qt6-qtwayland%{?_isa}
# wye-gtk names Adwaita icons; the app icons go in hicolor.
Requires:       adwaita-icon-theme
Requires:       hicolor-icon-theme
# GTK's GL renderer dlopens libGLESv2.so.2 (libepoxy), so no ELF require
# names it. Without it GTK falls back to Vulkan, which draws nothing on a
# software X server such as Xvfb.
Requires:       libGLESv2.so.2()(64bit)

%description
Wye registers as the desktop's default web browser and sends every link to
the right browser, browser profile, private window or desktop app, or asks
with a small picker. It ships the session service and command-line tool,
the native-messaging host of the browser extension, a Qt and Kirigami
window host for KDE Plasma, a GTK 4 and Adwaita window host for GNOME and
other desktops, and the GNOME Shell extension (installed, not enabled).

%prep
%autosetup -n %{name}-%{version}

%build
# The release profile strips the binaries (strip = true); keep the symbols so
# find-debuginfo can split them into the debuginfo package. RUSTFLAGS
# (%%build_rustflags: -Cdebuginfo=2, relro/now) and CFLAGS/CXXFLAGS for the
# cxx-qt C++ come from %%set_build_flags.
export CARGO_PROFILE_RELEASE_STRIP=none
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}"
export QMAKE="${QMAKE:-%{_bindir}/qmake6}"
cargo build --release --locked -j %{_smp_build_ncpus} \
  --package wye --package wye-native-host --package wye-ui --package wye-gtk

%install
packaging/install.sh --destdir %{buildroot} --prefix %{_prefix} \
  --target-dir "${CARGO_TARGET_DIR:-$PWD/target}/release"

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/dev.soldunov.wye.desktop
%{buildroot}%{_bindir}/wye --version

%post
%systemd_user_post wye.service wye-ui.service wye-gtk.service

%preun
%systemd_user_preun wye.service wye-ui.service wye-gtk.service

%postun
%systemd_user_postun_with_restart wye.service wye-ui.service wye-gtk.service

%files
%license LICENSE
%doc README.md
%{_bindir}/wye
%{_bindir}/wye-native-host
%{_bindir}/wye-ui
%{_bindir}/wye-gtk
%{_datadir}/applications/dev.soldunov.wye.desktop
%{_datadir}/icons/hicolor/scalable/apps/dev.soldunov.wye.svg
%{_datadir}/icons/hicolor/16x16/apps/dev.soldunov.wye.svg
%{_datadir}/icons/hicolor/24x24/apps/dev.soldunov.wye.svg
%{_datadir}/icons/hicolor/32x32/apps/dev.soldunov.wye.svg
%{_datadir}/icons/hicolor/symbolic/apps/dev.soldunov.wye-symbolic.svg
%{_datadir}/icons/hicolor/symbolic/apps/dev.soldunov.wye-picker-symbolic.svg
# gnome-shell is not required (the extension is optional), so the package
# owns the extensions directories too.
%dir %{_datadir}/gnome-shell
%dir %{_datadir}/gnome-shell/extensions
%{_datadir}/gnome-shell/extensions/wye@dev.soldunov/
%{_datadir}/dbus-1/services/dev.soldunov.wye.service
%{_datadir}/dbus-1/services/dev.soldunov.wye.Ui.service
%{_datadir}/dbus-1/services/dev.soldunov.wye.Gtk.service
%{_userunitdir}/wye.service
%{_userunitdir}/wye-ui.service
%{_userunitdir}/wye-gtk.service

# One entry naming the version being built, so a release needs no edit here;
# the project's history is the GitHub release notes.
%changelog
* Fri Oct 02 2026 Philipp Soldunov <69530789+psoldunov@users.noreply.github.com> - %{version}-%{release}
- Package Wye for Fedora.
