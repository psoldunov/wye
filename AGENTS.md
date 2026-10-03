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
| `data/` | Shipped data (`services.toml`, `expansion.toml`, `tracking-parameters.toml`); `applications/` the desktop entry; `icons/` the hicolor icons and their generator; `dbus/` and `systemd/` `@bindir@` templates for the D-Bus service files and the systemd user units (`wye.service`, `wye-ui.service`); `kwin/` the KWin query script. |
| `nix/`, `flake.nix` | `package.nix` (crane package and Qt wiring), `frontends.nix` (browser extension), `hm-module.nix` and `nixos-module.nix` with the shared `channel.nix`, `release.nix` and `release.json` (the latest release), `tests/modules.nix` (module evaluation check). Checks and dev shell. |
| `packaging/` | Docker builds of the `.deb` (`deb/`, Debian testing), the `.rpm` (`rpm/`, Fedora 44) and the AppImage (`appimage/`, Arch base, sharun bundles every library including glibc; its `AppRun` integrates the AppImage into `~/.local` on every start), the shared `install.sh` (mirrors the `postInstall` of `nix/package.nix`) and `smoke-test.sh` (installs each in a clean container and launches both UI hosts). See its README. |
| `.github/workflows/` | `ci.yml` (`nix flake check`, advisories), `rust-doctor.yml`, `release.yml` (tag build, GitHub release, pull request recording it in `nix/release.json`). |
| `docs/spec/` | The specification. |
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
   Run it through the `ci-check` script (or `cachix watch-exec psoldunov -- nix flake check -L --keep-going`) with `CACHIX_AUTH_TOKEN` set before pushing: it fills the public psoldunov Cachix cache, so CI substitutes instead of building.

## Commits

Conventional Commits: `feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`, `perf:`, `ci:`.

## Releases

Push a tag `vX.Y.Z` that equals the workspace `version` in `Cargo.toml`, on master.
`.github/workflows/release.yml` builds it, creates the GitHub release with generated notes,
and opens a pull request that writes the tag's commit and content hash to
`nix/release.json`. Merging that pull request is what makes `programs.wye.channel =
"release"` the default. Never edit `nix/release.json` by hand. The workflow needs the
repository setting "Allow GitHub Actions to create and approve pull requests".

A pull request opened with `GITHUB_TOKEN` does not start other workflows, so `ci.yml` does
not run on it by itself: close and reopen it (or push a commit to it) to run the checks
before merging. Re-running the workflow is safe: the record step replaces its
`release/vX.Y.Z` branch and reuses an open pull request.

The modules of a newer flake may install an older release's package, so the package layout
they rely on is a contract (listed in `nix/channel.nix`): add paths, never move or rename
them.
