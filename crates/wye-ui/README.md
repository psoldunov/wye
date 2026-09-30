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
| `tray-menu` | `toggle` (argument: `TrayMenu` JSON plus optional `placement`; shows the popup, or closes it when shown) |
| `settings` | `show` (key `settings`, argument a page; key `rule-editor`, argument a JSON prefill; key `test-rules`) |
| `script-editor`, `history`, `about`, `onboarding` | `show` (key: the window name, `first-run` for `onboarding`; argument: its argument) |

The script editor's argument is a scope (`global`, `rule:<id>`) or JSON
`{"scope": …, "ruleName": …}`; a caller that knows the rule's name (the rule editor) passes
it so the title needs no `GetConfig`. Its fixture cases add `"fixture"` with the script's
`source` and the `RunScript` answer (`crates/wye-ui/src/script_editor/opening.rs`).

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

## Settings window

`qml/settings/SettingsWindow.qml` shows one page (`<Name>Page.qml`, a `WyePage`) at a time;
sheets a page opens go in its `overlays` and open lazily from a `Loader`. Everything a page
reads or writes goes through the `SettingsBackend` singleton (`src/bridge/settings.rs`, logic in
`src/settings/`): configuration, status, targets and services as JSON properties; one invokable
per kind of change (`setValue`, `setTarget`, `setServiceTarget`, `setModifiers`, `applyPatch`,
`shownToggle` …) that shows the change at once and saves a merge patch with `UpdateConfig`
(a `Conflict` reloads and applies the patch again). A query invokable (`targetMenu`,
`shownRows`) is called in a binding that reads `SettingsBackend.generation` first, so it runs
again when the data changes. Components with a `path` save themselves; bind their value with
`page.value("general.tray-icon", default)`.

The building blocks are in `qml/components/`, each with its API at the top of its file:
`WyePage`, `WyeGroupCard` (BLK-01), `WyeRow` (BLK-02), `WyeSwitchRow` (BLK-03),
`WyeTargetRow` + `WyeTargetMenu` (BLK-04, TGT), `WyeButtonRow` (BLK-05), `WyeRadioRow`
(BLK-06), `WyeTextRow` (BLK-07), `WyeChoiceRow`, `WyeHelpButton` (BLK-08), `WyeCallout`
(BLK-09), `WyeDisabledRow` (BLK-10), `WyeSheet` (BLK-11), `WyeEmptyState` (BLK-12),
`WyeListToolbar` (BLK-13), `WyeSection` (BLK-14), `WyeChecklist` (BLK-15),
`WyeShortcutRecorder` and `WyeShortcutChips` (BLK-16), `WyeLinkText` (BLK-17),
`WyeModifierChooser` + `WyeModifierRow` (BLK-18), `WyeAppChooser` (DLG-APP),
`WyeGlobalShortcutRow` (ADV-05 to ADV-07), `WyeCopyButton`, `WyeConfirmDialog`.

`fixtures/settings.json` feeds the window with `key: "settings"` and an `argument` that is a
page name or an object `{page, fixture, scheme, sheet}`. `fixture` is what the service would
have returned (`src/settings/fixture.rs`); a later fixture changes only the parts it names.
Every change then stays local. Add a case for each state a page can show.

The Picker, Extras and Advanced pages and their sheets (`PickerKeysSheet`, `ExpansionSheet`) use
these. Their rules are plain Rust in `src/settings/`: `keys` (KEY-21 clashes, the patch that
resolves them), `shortcuts` (global shortcut rows, with the KEY-41 command hints when
`GetShortcuts` fails or reports no mechanism), `expansion` (DLG-EXP rows, domain validation).
The invokables that call the service for them are declared in `bridge/settings.rs` and
implemented in `src/settings/pages.rs`. A fixture may carry `shortcuts` and `expansion` (what
`GetShortcuts` and `GetExpansionCatalogue` return) and a `sheet` name (`shown-browsers`,
`app-chooser`, `picker-keys`, `expansion`, `history-confirm`) that opens on its page.

## Rules, History, About and first run

The **Rules page** (`qml/settings/RulesPage.qml`, sheets in `qml/rules/`) keeps its rules in plain
Rust under `src/rules/` (`list` rows and summaries, `ops` move/toggle/delete/undo as merge patches,
`draft` the editor's draft and its validity, `tester` the rule tester's trace view, `files`
import and export). `RulesBackend` (`bridge/rules.rs`) and `TesterBackend` (`bridge/tester.rs`)
only convert types. `ShowWindow("rule-editor" | "test-rules")` reaches the page through
`RulesBackend.request`, so the picker's "Create Rule…" (PICK-31) and the history's both prefill the
editor. `fixtures/rules.json` and `fixtures/tester.json` feed the `settings` surface.

**History** (`qml/history/`, `src/history/`, `HistoryBackend`), **About** (`qml/about/`,
`src/about/`, `AboutBackend`) and **first run** (`qml/onboarding/`, `src/onboarding/`,
`OnboardingBackend`, shown with `ShowWindow("first-run", …)`) each have their own window and
fixture file. History reloads when `HistoryRevision` moves (a 2 s poll while it is visible). The
first-run pages are created up front in a hidden item and pushed on the page stack as items:
pushing a Component or URL makes Kirigami create them without a parent, which Qt warns about.
The About page is built from `WyeGroupCard`: FormCards placed straight into a `ScrollablePage`
never settled their width under Qt 6.11 (the layout polish loop did not end).
