# Distribution packages

Docker builds of a `.deb` and an `.rpm` from the checkout, and a smoke test that installs
each one in a clean container and launches it.

| Package | Built on | Installs on |
|---------|----------|-------------|
| `dist/wye_<version>_amd64.deb` | `debian:testing` | Debian testing (forky) and sid |
| `dist/wye-<version>-1.fc44.x86_64.rpm` | `fedora:44` | Fedora 44 |

Debian 13 (trixie) cannot build `wye-gtk`: it enables GTK 4.22 (`gnome_50`), libadwaita 1.9
and GtkSourceView 5.18, and trixie has GTK 4.18, libadwaita 1.7 and GtkSourceView 5.16.

## Build

```sh
packaging/deb/build.sh    # dist/wye_<version>_amd64.deb, dist/lintian.txt
packaging/rpm/build.sh    # dist/wye-<version>-1.fc44.{x86_64,src}.rpm (+ debuginfo), dist/rpmlint.txt
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
