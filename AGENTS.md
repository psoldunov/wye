# Wye: agent and contributor guide

Wye is a native Linux browser picker written in Rust. It registers as the desktop's
default web browser and sends every link to the right browser, browser profile, private
window or desktop app, or asks with a small picker. The specification is in
[docs/spec/](docs/spec/README.md); the design is in [docs/architecture.md](docs/architecture.md).

## Layout

| Path | Contents |
|------|----------|
| `crates/wye-core` | Pure routing core: config, targets, rules, matchers, URL cleaning, redirect unwrapping, web app catalogue, pipeline, and the pure models of the picker, tray, history, hooks and clipboard. No IO. |
| `crates/wye-desktop` | Linux integration: desktop entries, browser discovery and profiles (including Firefox profile groups), Exec/launch, `mimeapps.list` default browser, `kdeglobals`, autostart, source-app detection, native-messaging manifests. |
| `crates/wye-api` | The D-Bus contract: bus/interface/error names, JSON payload types (serde, camelCase), zbus proxies. See [docs/dbus-api.md](docs/dbus-api.md). |
| `crates/wye-script` | The transform-script engine (QuickJS through `rquickjs`): global and per-rule scripts, limits, diffs. |
| `crates/wye-service` | The session service library behind `wye service`: `bus/` interface impls delegate to one `api/<topic>.rs` per topic; `platform/` holds session integrations behind traits, with no-op and fake implementations; `platform/sni.rs` is the tray (a StatusNotifierItem with a DBusMenu) on every desktop. |
| `crates/wye` | The `wye` binary and CLI. Its `tests/e2e.rs` runs the real service on a private bus. |
| `crates/wye-native-host` | The browser extension's native-messaging host `wye-native-host` (library and binary); `wye extension install\|remove` uses its `install` module. |
| `crates/wye-ui` | The Qt/Kirigami UI host `wye-ui` (cxx-qt): picker, tray-menu popup, Settings, script editor, onboarding, about and history windows. UI conventions: [crates/wye-ui/README.md](crates/wye-ui/README.md). |
| `crates/wye-gtk` | The GTK 4 / libadwaita host `wye-gtk` (`dev.soldunov.wye.Gtk`): Settings, About and the other windows, plus the picker and the tray-menu popup (gtk4-layer-shell on wlroots compositors and KDE, an undecorated window elsewhere), the widget kit, `--self-test` on a private Xvfb. Conventions: [crates/wye-gtk/README.md](crates/wye-gtk/README.md). |
| `frontends/gnome-shell/` | The GNOME Shell extension `wye@dev.soldunov` (Shell 48+, tested on 50): draws the picker and the panel tray menu on GNOME. See its README. |
| `frontends/extension/` | The Firefox and Chromium extension (one set of files, one manifest per family). See its README. |
| `data/` | Shipped data (`services.toml`, `expansion.toml`, `tracking-parameters.toml`); `applications/` the desktop entry; `icons/` the hicolor icons and their generator; `dbus/` and `systemd/` `@bindir@` templates for the D-Bus service files and the systemd user units (`wye.service`, `wye-ui.service`, `wye-gtk.service`); `kwin/` the KWin query script. |
| `nix/`, `flake.nix` | `package.nix` (crane package and Qt wiring), `frontends.nix` (browser extension), `hm-module.nix` and `nixos-module.nix` with the shared `channel.nix`, `release.nix` and `release.json` (the latest release), `tests/modules.nix` (module evaluation check). Checks and dev shell. |
| `packaging/` | Docker builds of the `.deb` (`deb/`, Debian testing), the `.rpm` (`rpm/`, Fedora 44) and the AppImage (`appimage/`, Arch base, Arch Linux ARM on aarch64, sharun bundles every library including glibc; its `AppRun` integrates the AppImage into `~/.local` on every start), each for the machine's own architecture (x86_64 or aarch64), the shared `install.sh` (mirrors the `postInstall` of `nix/package.nix`) and `smoke-test.sh` (installs each in a clean container and launches both UI hosts). See its README. |
| `.github/workflows/` | `ci.yml` (advisories, on pull requests and pushes to master), `rust-doctor.yml`, `flake-check.yml` (`nix flake check` on x86_64 and aarch64; each release and workflow dispatch only, not pull requests or pushes to master), `packages.yml` (builds and smoke-tests the `.deb`, `.rpm` and AppImage for x86_64 and aarch64 on native runners; each release and workflow dispatch only, not pull requests), `release.yml` (on a push to master that changes the workspace version: `flake-check.yml` and `packages.yml` side by side, then the GitHub release and its tag with the six packages, the three extension zips and `SHA256SUMS`, then a commit recording it in `nix/release.json`). |
| `docs/spec/` | The specification. |
| `docs/investigations/` | Bug investigation reports: one confirmed root cause per file, with the evidence that pinned it down and the fix it calls for. |
| `docs/media/` | Screenshots and the demo GIF, shown in `docs/tour.md`; `kde/stage/` regenerates the KDE ones in a headless KWin and Plasma session, `gnome/stage/` the GNOME ones in a headless GNOME Shell session (their READMEs). |

Every requirement in the spec has an ID such as `PIPE-06`. Cite the ID in code comments,
tests and commit bodies.

## Commands

Host has no Rust toolchain. Run everything through the dev shell:

```sh
nix develop -c cargo build
nix develop -c cargo test --workspace --locked
nix build -L
```

## Review gates

Run in this order. Every gate must pass before a change is ready.

1. `nix develop -c cargo fmt --all --check`
2. `nix develop -c cargo clippy --workspace --all-targets --locked -- --deny warnings`.
   `clippy::pedantic` is on workspace-wide. Each `#[allow]` needs a comment giving the reason.
3. `nix develop -c cargo test --workspace --locked`
4. `nix develop -c rust-doctor --yes --blocking warning`. A change must not introduce findings.
   Install once with `nix develop -c cargo install --locked rust-doctor`. CI installs the
   pinned version and runs it in `.github/workflows/rust-doctor.yml` (read-only token, no pull
   request comment). `rust-doctor.toml` holds the policy: a rule is switched off there only
   with a comment that says why (today `duplicate_major_versions`, for the syn 2/3 split).
5. `nix develop -c cargo deny check` (advisories need the network).
6. `nix flake check -L --keep-going`. Beyond the Rust checks (`clippy`, `test`, `fmt`, `deny`,
   `machete`) it runs `installed-desktop-entry` and `installed-dbus-files` (the installed
   files name this package's binaries), `qmllint` and `ui-selftest` (wye-ui), `gtk-selftest`
   (wye-gtk),
   `extension-manifests` (the browser extension), `modules-eval` (both Nix modules and both
   channels, evaluated with a fake `release.json`) and `nix-fmt`. `cargo test` includes
   `crates/wye/tests/e2e.rs`: the real `wye service` on a private `dbus-daemon`, activated
   from the shipped service file, with a fake browser and a fake `PickerHost1`.
   CI runs it for releases only (`flake-check.yml`, called by `release.yml`, which publishes
   nothing unless it passes), not on pull requests, so gates 1 to 5 are what a pull request is
   checked against. Run it on a branch from the Actions tab (`gh workflow run flake-check.yml
   --ref <branch>`) or locally through the `ci-check` script (or `cachix watch-exec psoldunov
   -- nix flake check -L --keep-going`) with `CACHIX_AUTH_TOKEN` set: it fills the public
   psoldunov Cachix cache, so the release's run substitutes instead of building.

## Commits

Conventional Commits: `feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`, `perf:`, `ci:`.

## Releases

Merging the version bump is the release. The bump is a pull request that sets the workspace
`version` in `Cargo.toml`, the `wye*` entries of `Cargo.lock` and the `version` of both
extension manifests; gates 1 to 5 and its pull request checks are all it needs. On the push
to master, `.github/workflows/release.yml` sees the version change with no tag `vX.Y.Z` yet
and runs `flake-check.yml` and `packages.yml` side by side. When both pass on both
architectures it creates the GitHub release, and with it the tag `vX.Y.Z` on that commit,
with the six packages, the three browser extension zips (`wye-extension-firefox-X.Y.Z.zip`,
`wye-extension-chromium-X.Y.Z.zip`, `wye-extension-chromium-webstore-X.Y.Z.zip`, from the
Nix build), `SHA256SUMS` and generated notes. Then it commits the release's commit and
content hash to `nix/release.json` on master, which is what makes `programs.wye.channel =
"release"` install it. Nothing is published and no tag is created unless every check and
package passes.

Do not push a tag, and do not dispatch `packages.yml` or `flake-check.yml` on the bump's
branch: the release run does both. After a failure, re-run the failed jobs, or fix master
and run `gh workflow run release.yml`, which releases master's version when its tag does not
exist yet. Never edit `nix/release.json` by hand.

The browser extension's listings on addons.mozilla.org and the Chrome Web Store are
updated by hand after the release, from the release's zips: see "Submitting a version" in
[frontends/extension/store/README.md](frontends/extension/store/README.md).

The modules of a newer flake may install an older release's package, so the package layout
they rely on is a contract (listed in `nix/channel.nix`): add paths, never move or rename
them.
