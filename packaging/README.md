# Distribution packages

Docker builds of a `.deb`, an `.rpm` and an AppImage from the checkout, and a smoke test
that installs each one in a clean container and launches it. Every
[release](https://github.com/psoldunov/wye/releases) carries all three for x86_64 and
aarch64, built and smoke-tested by GitHub Actions (see [CI and releases](#ci-and-releases)).

| Package | Built on | Installs on |
|---------|----------|-------------|
| `dist/wye_<version>_{amd64,arm64}.deb` | `debian:testing` | Debian testing (forky) and sid |
| `dist/wye-<version>-1.fc44.{x86_64,aarch64}.rpm` | `fedora:44` | Fedora 44 |
| `dist/Wye-<version>-{x86_64,aarch64}.AppImage` | `archlinux:latest` (x86_64), Arch Linux ARM (aarch64) | any x86_64 or aarch64 distribution; tested on Debian 12 and 13, Fedora 43 and 44 |

Each script builds for the architecture of the machine it runs on (`uname -m`, x86_64 or
aarch64): Docker runs the build natively, without emulation, and the test containers run
that architecture too. To build the other architecture, run the script on a machine of
that architecture.

Debian 13 (trixie) cannot build `wye-gtk`: it enables GTK 4.22 (`gnome_50`), libadwaita 1.9
and GtkSourceView 5.18, and trixie has GTK 4.18, libadwaita 1.7 and GtkSourceView 5.16.

## Build

```sh
packaging/deb/build.sh    # dist/wye_<version>_<arch>.deb, dist/lintian.txt
packaging/rpm/build.sh    # dist/wye-<version>-1.fc44.{<arch>,src}.rpm (+ debuginfo), dist/rpmlint.txt
```

The version is the workspace `version` in `Cargo.toml`; nothing else changes per release.
Each script builds its image, copies the source into a container (tracked files plus
untracked files Git does not ignore), builds there and copies the packages out, owned by
you. The worktree's `target/` is not used: the cargo registry and target directory live in
Docker volumes (`wye-deb-cargo`, `wye-deb-target`, `wye-rpm-cargo`, `wye-rpm-target`), so
a rebuild takes 2 to 3 minutes; a cold one about 6 (`.deb`) or 8 (`.rpm`). Cargo fetches
crates during the build, so it needs the network.

Environment: `DOCKER` (the container CLI, default `docker`; podman works), `DOCKER_CONFIG`
(read by the docker CLI, e.g. `DOCKER_CONFIG=/some/dir` with an empty `config.json` when
stale Docker Hub credentials refuse anonymous pulls), `WYE_DEB_IMAGE`, `WYE_RPM_IMAGE`,
`WYE_DEB_TEST_IMAGE`, `WYE_RPM_TEST_IMAGE`.

## Test

```sh
packaging/deb/test.sh     # fresh debian:testing
packaging/rpm/test.sh     # fresh fedora:44
```

Each installs the package with `apt-get` or `dnf`, so its dependencies resolve from the
distribution's archive, adds the test-only tools (Xvfb, desktop-file-utils, dbus,
ImageMagick) and runs [`smoke-test.sh`](smoke-test.sh), which:

- runs `ldd` on the four binaries, `wye --version` and `--help`, and `wye-native-host` on
  closed stdin;
- validates the desktop entry and checks that the D-Bus service files and systemd user
  units name installed binaries, and that no user unit is enabled for every user;
- runs `wye-ui --self-test` and `wye-gtk --self-test`, which load every surface and fail on
  any QML, GTK or GLib warning (a missing QML module shows up here);
- on a private session bus and an Xvfb display, lets the bus activate the installed service,
  runs `wye settings` with the `kde` and then the `gnome` frontend, and checks that
  `wye-ui` and then `wye-gtk` are still up 8 seconds later with no load errors and drew
  something.

Screenshots go to `dist/screenshots/{debian,fedora}-wye-{ui,gtk}-settings.png`, logs to
`dist/smoke-logs/`. The script exits 1 when any check fails.

## What the packages install

[`install.sh`](install.sh) stages the tree for both formats, the same files as the
`postInstall` of `nix/package.nix` (keep the two in step): the four binaries in
`/usr/bin`, the desktop entry, the hicolor icons, the GNOME Shell extension (installed, not
enabled), the D-Bus service files and the systemd user units in `/usr/lib/systemd/user`.

Neither package enables a user unit: launch at login is each user's switch in Settings
(GEN-01), which writes an XDG autostart entry.

`wye-ui` loads its QML modules (Kirigami, Kirigami Addons, the KDE desktop style,
LayerShellQt, KSyntaxHighlighting, KQuickControls) at run time, so no automatic dependency
scan finds them: `deb/debian/control` and `rpm/wye.spec` list them by hand, after
`qt.runtimeInputs` in `nix/package.nix`. The `.rpm` also requires `libGLESv2.so.2`, which
GTK opens at run time; without it GTK falls back to Vulkan and draws nothing on Xvfb.

cxx-qt uses Qt's private headers, so the `.deb` depends on `qt6-base-private-abi` and
`qt6-declarative-private-abi` of the Qt it was built with: rebuild it whenever Debian moves
to a new Qt release.

## AppImage

```sh
packaging/appimage/build.sh   # dist/Wye-<version>-<arch>.AppImage
packaging/appimage/test.sh    # fresh debian:12, debian:13, fedora:43, fedora:44
```

One file that runs on any x86_64 (or, for the aarch64 build, any aarch64) distribution,
built the
[AnyLinux](https://github.com/pkgforge-dev/Anylinux-AppImages) way: `quick-sharun` bundles
every library the four programs load, glibc and its dynamic loader included, plus Mesa (with
llvmpipe, so both windows draw without the host's GL), the QML modules and Qt plugins
`wye-ui` loads, GTK's modules and data (GtkSourceView's language files included) and the
Adwaita icons. Kirigami's Breeze icons are
compiled into `libKF6BreezeIcons`; fonts and CA certificates are the host's. Nothing is
loaded from the host's library directories, so the host's glibc age does not matter.
`sharun` runs each program through the bundled loader, and the
[uruntime](https://github.com/VHSgunzo/uruntime) runtime mounts the DwarFS image with FUSE,
falls back to user namespaces without FUSE, and extracts to `TMPDIR` when neither works.

The build image is `archlinux:latest` ([`appimage/Dockerfile`](appimage/Dockerfile)), the
base the AnyLinux tooling is made and tested on, with GTK 4.22, libadwaita 1.9, Qt 6.11 and
KF6 6.30. The official image is x86_64 only, so on aarch64 `build.sh` starts from
`ghcr.io/pkgforge-dev/archlinux:aarch64` instead: Arch Linux ARM, packaged by the project
behind the AnyLinux tooling, which builds its own aarch64 AppImages on it. Arch Linux ARM
builds Arch's package recipes, so the package list is the same; `WYE_APPIMAGE_BASE`
overrides the base. The tools are pinned per architecture, each with its SHA-256 checked
when the image is built:
`quick-sharun.sh` at a commit of pkgforge-dev/Anylinux-AppImages, `sharun` 3.5.0
(pkgforge-dev/Anylinux-sharun, with the `anylinux.so` helper that keeps the bundle's
environment away from the programs Wye launches) and `appimagetool` 0.5.2
(pkgforge-dev/appimagetool, which embeds uruntime 0.8.1 and `mkdwarfs`, both pinned by
digest in its source). The sharun and appimagetool digests are the ones quick-sharun pins;
bumping quick-sharun means copying its new `SHARUN_SHA`/`APPIMAGETOOL_SHA` into the
Dockerfile.

`build.sh` works like the other two: the source goes into a container as a tarball, cargo's
registry and target directory live on the volumes `wye-appimage-cargo` and
`wye-appimage-target`, and the AppImage comes out owned by you. A cold build takes about 9
minutes once the image exists (cargo 4.5, quick-sharun 3, packing 1); a rebuild about 4.5,
since quick-sharun always redeploys and runs each program under strace to find what it
dlopens. Environment: `DOCKER`, `DOCKER_CONFIG`, `WYE_APPIMAGE_IMAGE`, `WYE_APPIMAGE_BASE`,
and for the test `WYE_APPIMAGE_TEST_IMAGES`.

quick-sharun would also rewrite `/usr/lib` and `/usr/share` inside bundled binaries to
`/tmp/<name>` paths fixed at build time, linked to the mount by a hook. Any local user could
plant those links first (p11-kit would then load their PKCS#11 modules), so the Dockerfile
turns that patching off. Unpatched, p11-kit reads the host's own root-owned paths like any
other program, and GtkSourceView finds its language files and style schemes through
`XDG_DATA_DIRS`, which sharun starts with the AppDir's `share/`. The only patch left is the
bundled dynamic loader's: it no longer reads `/etc` (the host's `ld.so.cache` and
`ld.so.preload`).

`build.sh` leaves out what Wye never uses along with every library only those pulled in:
Qt Multimedia's FFmpeg plugin and glycin's HEIF loader (FFmpeg, x264/x265, AV1, VPX and
about 35 more), Qt's GTK 3 platform theme (quick-sharun keeps it to load the host's GTK 3
into the bundled Qt), and plugins whose libraries the build system lacks. It then fails if
any bundled file needs a library the AppDir lacks, or if a `/tmp` mapping hook appeared.

The x86_64 AppImage is about 160 MB (620 MB unpacked). Mesa's llvmpipe dominates (libLLVM 165 MB
and libgallium 53 MB unpacked), then Qt and the KDE Frameworks, ICU (38 MB) and the Breeze
icons compiled into `libKF6BreezeIcons` (25 MB). gnutls and p11-kit stay: GTK links
`libcups`, which needs gnutls.

### Self-integration

Every start from the AppImage brings these up to date, writing only what changed (into a
temporary file, then `mv`), in about 20 ms:

- `~/.local/share/applications/dev.soldunov.wye.desktop`: `Exec` and `TryExec` name the
  AppImage, quoted, so its path may contain spaces; the Settings and clipboard actions stay.
  `update-desktop-database` runs on that directory when it exists and the entry changed.
- `~/.local/share/dbus-1/services/dev.soldunov.wye{,.Ui,.Gtk}.service`: `Exec` runs the
  AppImage with `service`, `wye-ui` or `wye-gtk`. They have no `SystemdService=`: the
  AppImage installs no systemd user units, and dbus-broker would otherwise ask systemd for
  one that does not exist. The bus starts the service and the windows by activation. When
  one of them changed, the AppImage asks the bus to reload (`ReloadConfig`, through
  `busctl`, `dbus-send` or `gdbus`, whichever the host has) and waits for it: dbus-broker
  picks up a new service file only some time after it changed, so the first
  `AppImage settings` would otherwise fail with "The name is not activatable".
- `~/.local/share/icons/hicolor/*/apps/dev.soldunov.wye*.svg`.
- `~/.local/share/gnome-shell/extensions/wye@dev.soldunov`, installed and not enabled, as
  the packages do.
- `~/.local/bin/wye` and `~/.local/bin/wye-native-host`, symlinks to the AppImage, and
  `~/.local/bin` first on the `PATH` of everything the AppImage starts. The service writes
  the launch-at-login entry with the `wye` it finds on `PATH`, and the browser host manifests
  (at each start, when a browser's directory appears, or on `wye extension install`) name the `wye-native-host` it finds there; without the links both would record the
  AppImage's temporary mount point. Browsers start the native host from its manifest
  without arguments, hence a link with that name.

Each file the AppImage writes carries a marker (an `X-Wye-AppImage=true` key, a comment, a
`.x-wye-appimage` stamp in the extension), and it rewrites a file only when the file is
missing or carries that marker, or for the links, when the link points at an AppImage.
Anything else at those paths stays as it is, with one warning on stderr per start.

The runtime's environment (`$APPDIR/bin` first on `PATH`, other paths into the AppImage,
markers such as `APPDIR` and `URUNTIME`, `XDG_CACHE_HOME` moved to `AppImage-Cache`) is
Wye's own and stays with Wye's programs. The apps the service launches get the session's
instead: every variable loses its `:`-separated entries that point into the AppImage, the
markers are unset, and `HOME` and the XDG base directories come back (LAUNCH-08). `PATH`
keeps the `~/.local/bin` the AppImage puts first. Otherwise `flatpak run` finds the
bundled `bwrap` shim before `/usr/bin/bwrap`, and no Flatpak browser starts.

When another Wye installation is visible to the session, the AppImage integrates nothing
and says so on stderr. That is the case when `dev.soldunov.wye.service` (D-Bus) or
`dev.soldunov.wye.desktop` exists in `$XDG_DATA_HOME` without the marker (home-manager
links the D-Bus files there), or in any directory of `$XDG_DATA_DIRS`, `/usr/local/share`
or `/usr/share` (a `.deb`, an `.rpm`, NixOS, a Nix profile). The AppImage still runs, but
the menu entry, D-Bus activation and `wye` on `PATH` stay that installation's: files under
`~/.local` would shadow it, and break it once the AppImage is deleted. When an earlier
AppImage start already integrated one, the warning says to run `--remove-integration`.

Integration is skipped, with a warning, when `HOME` is unset or belongs to another user
(`sudo -E` keeps the user's `HOME`, and root would leave root-owned directories in it). A
relative `XDG_DATA_HOME` is ignored, as the XDG Base Directory Specification asks. A
read-only `HOME` only produces a warning. Two starts at once (a link click starts `open` and
then, by D-Bus activation, the service) take turns through `flock` on
`$XDG_RUNTIME_DIR/wye-appimage.lock`, and the GNOME Shell extension is copied next to
`extensions/` and renamed into place, so neither ever sees half a copy. The AppImage runtime's
`APPIMAGE` and `OWD` variables are unset before the program starts: every browser and app
Wye launches would inherit them, and an Electron app's updater takes `APPIMAGE` as the file
to replace.

The AppImage is a multicall binary. Through a link, the link's name picks the program
(`wye`, `wye-native-host`, `wye-ui`, `wye-gtk`); otherwise a first argument `wye-ui`,
`wye-gtk` or `wye-native-host` does; everything else goes to `wye`.

To undo the integration, make another browser the default (the desktop's default-browser
setting otherwise names a menu entry that no longer exists), switch off launch at login in
Settings and run `wye extension remove` (the service writes the browser manifests itself, so
they exist unless you opted out), then:

```sh
./Wye-<version>-<arch>.AppImage --remove-integration
```

It deletes exactly the marked files and links and nothing else. Do this before deleting
the AppImage or switching to a `.deb`, an `.rpm` or home-manager.

### Running it on a desktop

```sh
chmod +x Wye-<version>-<arch>.AppImage
./Wye-<version>-<arch>.AppImage settings
```

Keep the file where it is: the integration names its path, and a start from a new path
rewrites everything to that path. The menu entry hides itself (`TryExec`) once the file is
gone. Make Wye the default browser from Settings, as with the packages. The programs run
inside the AppImage's mount (`/tmp/.mount_*`); what Wye launches (browsers, apps) gets the
host's environment back.

### Test

[`appimage/test.sh`](appimage/test.sh) starts a fresh container for each of `debian:12`,
`debian:13`, `fedora:43` and `fedora:44`, installs only the test tools (Xvfb, xauth, dbus,
desktop-file-utils, ImageMagick, a font, `file`, CA certificates), removes the Mesa packages
Xvfb pulls in, checks that no Qt, GTK, KDE or Mesa library is left, puts the AppImage in a
directory whose name has a space, and runs [`smoke-test.sh`](smoke-test.sh) with
`WYE_APPIMAGE` set. Docker has no FUSE, so it exports `APPIMAGE_EXTRACT_AND_RUN=1`. In that
mode the smoke test:

- launches the AppImage once with a home-manager-style symlink and a plain file planted at
  two of its icon paths, and checks that the launch wrote all eleven integration files and
  links, left both planted files alone, and warned about them; a second launch must
  rewrite nothing;
- lists the libraries of the four programs with the bundled loader, without the host's
  `ld.so.cache`, and checks that each one comes from the AppImage;
- runs the CLI checks, the two self-tests and the live session through links named after
  the programs, and checks the integrated desktop entry, D-Bus services and links; no other
  Wye D-Bus service file exists, so the live session activates `wye-ui` and `wye-gtk`
  through the integrated ones;
- makes a fake browser the primary one, opens a link through the service started from the
  AppImage, and checks that the fake browser's environment has no path into the AppImage,
  no runtime marker and the session's `XDG_CACHE_HOME` (LAUNCH-08);
- runs `--remove-integration` with the planted files back in place and checks that it
  removed the eleven files and links it wrote and nothing else;
- launches it once with a package's desktop entry in a directory on `XDG_DATA_DIRS` and
  once with a home-manager-style D-Bus service link in `XDG_DATA_HOME`, and checks that it
  wrote nothing either time and said which installation it found.

The live session uses `dbus-run-session`, which is dbus-daemon, so the reload for
dbus-broker is not exercised here.

Screenshots go to `dist/screenshots/appimage-<distro>-wye-{ui,gtk}-settings.png` (e.g.
`appimage-debian-12-…`), logs to `dist/smoke-logs/appimage-<distro>/`.

### Limitations

- The bundled Mesa drives the GPUs its distribution builds it for (Intel and AMD on x86_64,
  plus the Arm drivers of Arch Linux ARM on aarch64) and the software llvmpipe renderer,
  the only one the smoke tests use. Wayland compositors
  and X servers on the NVIDIA proprietary driver hand GL to NVIDIA's own libraries; sharun
  points GLVND at them when the `nvidia` kernel module is loaded, which is untested here.
  Software rendering always works and is enough for Wye's windows.
- Only X11 (Xvfb) is tested; Wayland, the layer-shell picker and the tray need a real
  session.
- Qt does not load the host's Plasma platform theme (`plasma-integration` is not bundled):
  colours and icons follow `kdeglobals` through Kirigami's desktop style, the font comes
  from fontconfig rather than from Plasma's font settings.
- The AppImage carries no update information (zsync): download the next release's.

## CI and releases

[`.github/workflows/packages.yml`](../.github/workflows/packages.yml) runs each format's
`build.sh` and then its `test.sh` for x86_64 on an `ubuntu-24.04` runner and for aarch64 on
an `ubuntu-24.04-arm` runner, six jobs in all. Each job uploads its package as the artifact
`package-<format>-<arch>`, and the screenshots, smoke-test logs and lint reports as
`smoke-<format>-<arch>`, also when it failed. It runs as part of each release and from the
Actions tab (workflow dispatch); pull requests do not run it, so a change to `packaging/`
or `data/` is checked by dispatching it on the branch (or by running the scripts locally).
Pull requests run only `ci.yml` and `rust-doctor.yml`.

[`.github/workflows/release.yml`](../.github/workflows/release.yml) runs on a pushed tag
`vX.Y.Z`. It calls `packages.yml` and, once all six jobs passed, publishes the GitHub
release with the six packages, the three browser extension zips (from the x86_64 Nix build
of the tag, which is not part of `packages.yml`) and their `SHA256SUMS`; a package that
fails its smoke test is never published. A re-run of a release replaces the packages and
zips of the existing release and leaves its notes alone.
