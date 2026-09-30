# wye-ui

Wye's Qt/Kirigami UI host (design B): one resident process per session that owns
`dev.soldunov.wye.Ui`, serves `dev.soldunov.wye.PickerHost1` and `dev.soldunov.wye.Windows1`
for the service (`docs/dbus-api.md`, "UI host"), and shows every window in QML. It makes
no routing decisions; it asks the service over D-Bus.

## Layout

| Path | What |
|------|------|
| `src/main.rs` | Start-up: single instance, then the Qt event loop. |
| `src/host.rs` | The D-Bus interfaces. They validate and turn each call into a `UiCommand`. |
| `src/route.rs` | `UiCommand` to surface, action, key and argument (the table QML sees). |
| `src/dispatch.rs` | Hands routes from the D-Bus thread to the Qt thread, keeping early ones. |
| `src/service.rs` | The tokio runtime and `service::request`, the one way a backend calls the service. |
| `src/surface.rs` | The surfaces and their QML root files. |
| `src/bridge/` | One `#[cxx_qt::bridge]` per surface or sheet, plus `app.rs`, `window_effects.rs` and the C++ `shim.rs`. |
| `cpp/wye_shim.{h,cpp}` | The only C++: `QApplication`, blur behind a window, xdg-activation tokens (KF6 WindowSystem). |
| `qml/Main.qml` | The engine's root: creates each surface on first use and calls its `handle()`. |
| `qml/<surface>/` | One directory per surface; `qml/components/` for shared pieces. |
| `fixtures/<surface>.json` | What `--self-test` feeds each surface. |

## Conventions for UI units

**Run everything in the dev shell.** It has Qt, KDE Frameworks, `QMAKE` pointing at the
merged Qt prefix cxx-qt needs, the wrapped `ld.lld`, `dbus-daemon`, `qmllint` and
`wye-qmllint`, and exports the QML import and plugin paths so `target/debug/wye-ui` runs.

```sh
nix develop -c cargo build -p wye-ui
nix develop -c cargo clippy -p wye-ui --all-targets --locked -- --deny warnings
nix develop -c cargo test -p wye-ui --locked
nix develop -c target/debug/wye-ui --self-test            # every surface, offscreen
nix develop -c target/debug/wye-ui --self-test picker     # one surface
nix develop -c wye-qmllint                                # after a build; any warning fails
nix develop -c target/debug/wye-ui settings rules         # a real window (needs a session bus)
```

**QML files.** `build.rs` adds every `qml/**/*.qml` and `*.js` to the module
`dev.soldunov.wye.ui`; nothing to register. Each file becomes a type named after its file
stem, so file names must be unique across all directories. Import the module with
`import dev.soldunov.wye.ui` to use the Rust types and the other files. Put files in your
surface's directory; shared ones go in `qml/components/`.

**Surface contract.** A surface root file (listed in `src/surface.rs`) is a window with a
function `handle(action, key, argument)`, called on the Qt thread:

| Surface | Calls |
|---------|-------|
| `picker` | `show` (key: request id, argument: `PickerRequest` JSON; replace what is shown, PICK-27), `close` (key: request id) |
| `tray-menu` | `toggle` (argument: `TrayMenu` JSON) |
| `settings` | `show` (key `settings`, argument a page; key `rule-editor`, argument a JSON prefill; key `test-rules`) |
| `script-editor`, `history`, `about`, `onboarding` | `show` (key: the window name, argument: its argument) |

`Main.qml` keeps one instance of each surface (SET-04): `show` must show, raise and
activate the window it already has.

**Bridges.** A backend is a `#[cxx_qt::bridge]` in `src/bridge/<name>.rs` with a
`#[qml_element]` QObject. `build.rs` compiles every file there except `mod.rs`; a new file
also needs its `pub mod` line in `src/bridge/mod.rs` with the same `#[allow(unsafe_code,
clippy::unnecessary_box_returns, reason = …)]` as the others. The crate denies unsafe code
everywhere else. Keep logic that needs no Qt (models, merge patches, validation) in plain
modules next to it and test it there; a bridge file only converts types and calls it.

**Calling the service.** Implement `cxx_qt::Threading` for the QObject and use
`crate::service::request(self.qt_thread(), |proxy| async move { … }, |object, result| …)`:
the call runs on the D-Bus thread with a `Wye1Proxy`, and the result comes back on the Qt
thread. `bridge/about.rs` is the example. Never block the Qt thread on D-Bus.

**C++ shim.** `WindowEffects` (QML) wraps `cpp/wye_shim.cpp`: `blurBehind(window, on)`
returns false when the compositor has no blur (draw the background opaque then);
`requestActivationToken(window, appId)` returns false off Wayland, otherwise the signal
`activationTokenReady(token)` follows (empty when refused).

**Self-test.** Each surface has `fixtures/<surface>.json`:
`{"cases": [{"action": "show", "key": "…", "argument": <string or JSON>}]}`. A JSON
argument travels as its text, exactly as over D-Bus. The self-test runs each surface in a
child with `QT_QPA_PLATFORM=offscreen` and `QT_FORCE_STDERR_LOGGING=1`, and fails on a
non-zero exit, a missing pass line, or any Qt warning or error outside the allow-list in
`src/selftest/log.rs` (each entry states why it is harmless). Add cases for every state
your surface can show; do not widen the allow-list for your own warnings. Unit tests in
`src/selftest/fixtures.rs` check that picker and tray fixtures decode as the `wye-api`
types.

The flake checks `qmllint` and `ui-selftest` run the same two gates in the Nix sandbox;
`ui-selftest` runs the installed, wrapped `wye-ui`.
