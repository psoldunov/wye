# Architecture

## Components

| Path | What it is |
|------|-----------|
| `crates/wye-core` | Pure routing core: config, targets, rules, matchers, URL cleaning, redirect unwrapping, web app catalogue and the pipeline that turns a URL into a resolved target. No IO. |
| `crates/wye-desktop` | Linux integration: desktop entry parsing, browser discovery, browser profiles, Exec expansion and launching, `mimeapps.list` default browser, source-app detection. |
| `crates/wye-api` | The D-Bus contract ([dbus-api.md](dbus-api.md)): bus names, object paths, error names, the serde types of every JSON payload, and zbus proxies. Shared by the service and every client. |
| `crates/wye-service` | The session service behind `wye service` (library): owns `dev.soldunov.wye`, serves `dev.soldunov.wye1`, `org.freedesktop.Application` and `dev.soldunov.wye.KWin1`. `bus/` holds the interface impls, which only delegate to one `api/<topic>.rs` per topic; `platform/` puts every session integration behind a trait with a no-op and a fake. tokio + zbus, no Qt. |
| `crates/wye` | The `wye` binary: `service` (runs `wye-service`), `open` (hands links to the service, or routes them itself when it cannot be reached), `test`, `browsers`, `default`, `config`, and the service clients `clipboard`, `menu`, `settings`, `debug`, `extension`. |
| `crates/wye-native-host` | The browser extension's native-messaging host `wye-native-host` (library and binary); `wye extension install\|remove` uses its `install` module. |
| `crates/wye-script` | The transform-script engine (QuickJS through `rquickjs`) for the global and per-rule scripts, with its limits, the `URL` prelude and the result diff. Used by the service. |
| `crates/wye-ui` | The Qt/Kirigami UI host (cxx-qt) that owns `dev.soldunov.wye.Ui` and shows the picker, the tray-menu popup, Settings, the script editor and the onboarding, about and history windows. A D-Bus client of the service; holds no routing logic. |
| `frontends/plasma/` | The Plasma 6 tray applet `dev.soldunov.wye` (pure QML) and its offscreen tests. |
| `frontends/extension/` | The Firefox and Chromium browser extension: one set of files, a manifest per family. |
| `data/` | Shipped data (`services.toml`, `expansion.toml`, `tracking-parameters.toml`), the desktop entry `dev.soldunov.wye.desktop`, the hicolor icon, and templates with `@bindir@` for the D-Bus service files (`data/dbus/`) and the systemd user units (`data/systemd/wye.service.in`, `data/systemd/wye-ui.service.in`). |
| `nix/`, `flake.nix` | Package (crane; `frontends.nix` adds the applet and the extension zips), the `programs.wye` modules for home-manager and NixOS (`hm-module.nix`, `nixos-module.nix`, shared `channel.nix`), the release record `release.json` and the checks: clippy, tests (including `crates/wye/tests/e2e.rs`), fmt, deny, machete, source and installed desktop entry, installed D-Bus files, qmllint, the UI self-test, the applet lint, load and D-Bus smoke tests, the extension manifests, module evaluation for both channels and nixfmt. Also the dev shell. The package rewrites the installed desktop entry's `Exec` to its own absolute `bin/wye` and adds `TryExec`; the source entry in `data/` stays generic. |

## Design decisions

### Pure core

`wye-core` does no IO. It takes a URL, a source, held keys and the configuration, and
returns a decision. Everything that touches the system (files, processes, the desktop
environment) lives in `wye-desktop` or `wye`. The core is testable without a desktop.

### Process model

One Rust service per session plus desktop-native frontends, all talking over the D-Bus
API in [dbus-api.md](dbus-api.md). KDE Plasma comes first.

| Process | Binary | Bus name | Started by |
|---------|--------|----------|------------|
| Service | `wye service` (crate `wye` over the `wye-service` library) | `dev.soldunov.wye` | D-Bus activation (`SystemdService=wye.service`); at login, the unit's `WantedBy=graphical-session.target` when a Nix module installs it (`WYE_LOGIN_MANAGED=1`), else the XDG autostart entry when "Launch at login" is on; or the tray applet |
| UI host | `wye-ui` (cxx-qt, Kirigami) | `dev.soldunov.wye.Ui` | D-Bus activation by the service (`SystemdService=wye-ui.service`, never at login); stays resident once started |
| Plasma tray | plasmoid `dev.soldunov.wye` (pure QML) | none | plasmashell |
| SNI tray | inside the service | ksni's own | the service, unless a tray host called `RegisterTray` (5 s grace on KDE) |
| Link handler fallback | `wye open %U` | none | launchers that do not honour `DBusActivatable` |

Single instance: the service requests its name with `DoNotQueue` and exits with status 75
when the name is taken; `wye.service` lists 75 in `RestartPreventExitStatus=` and uses
`KillMode=process`, so launched browsers survive a restart (LAUNCH-06). The service serves
its objects before requesting the name, so the very first call of a bus activation is
answered. It claims `dev.soldunov.wye` before asking the session anything slow; lock, clipboard, held-key, pointer and focus detection then run within 3 s
each, the shortcuts portal within 20 s in the background
(`crates/wye-service/src/platform/session.rs`).

Structured data travels as JSON in `s` values (camelCase), described by the serde types in
`wye-api`, because the QML applet parses JSON far more easily than nested D-Bus structs.

Inside the service each interface impl is written once in `crates/wye-service/src/bus/` and
delegates every member to a function in `crates/wye-service/src/api/<topic>.rs`; each topic
also owns a `State` type held by `ServiceContext`. Every session integration (held
modifiers, pointer, focused window, lock state, notifications, launching, systemd scopes,
clipboard, global shortcuts, HTTP) sits behind a trait in
`crates/wye-service/src/platform/mod.rs`, with a no-op implementation and a fake, so the
service is tested on a private `dbus-daemon` without a desktop.

Every member of [dbus-api.md](dbus-api.md) is implemented.
`dev.soldunov.wye.Error.NotImplemented` stays in the contract for a client that talks to an
older service. `wye service` runs the service and stops when it loses its bus name or its
bus connection; `wye service --activate` only asks the bus to start it.

### How a link reaches the service

1. **`DBusActivatable=true` (main path).** KIO and GIO call
   `org.freedesktop.Application.Open` on the service. `platform_data` carries the
   activation token. Source-app detection starts at the caller's PID, read with
   `GetConnectionCredentials`, skipping openers (`kde-open`, `xdg-open`, `gio`, …) up the
   parent chain; a caller that is `xdg-desktop-portal` hides the app, so the focused window
   stands in.
2. **`wye open` (Exec fallback).** It detects the source from its own parent chain, reads
   `XDG_ACTIVATION_TOKEN`/`DESKTOP_STARTUP_ID`, and calls `OpenLink` for each link, bounded
   by 3 s including bus activation. Without a desktop ID it also sends its parent's PID, so
   the service can match the executable against the installed apps. When no service
   exists or can be started (no session bus, `ServiceUnknown`/`NameHasNoOwner`, a failed
   activation, `NotImplemented` from an older service), `wye open` routes the link itself
   with the service's own hooks (short-link expansion, transform scripts;
   `wye_service::offline::OfflineHooks`) and `finish`, with the picker stand-in; held keys
   and the lock state are unknown there. A call that was sent but not answered within 3 s
   is never repeated: the service may still open the link. A link the service refused
   (exit 2) or failed to launch (exit 1) is not retried either: the service has already
   told the user.

In the service a link goes through `crates/wye-service/src/api/link.rs`: it takes a
`Snapshot` of the cached configuration, the installed apps and the remembered default
browser (`api/config.rs`, `snapshot`), runs the pipeline on a blocking thread
(`resolve_with` and `finish`, with the expansion and transform-script hooks of
`api/link/hooks.rs`), and builds the command line with `wye_desktop::build_command`. The `Launcher` starts it with the activation token
set (or removed for a background launch, LAUNCH-03/04), and the `ScopeManager` moves the
child into a transient scope `app-wye-<escaped desktop ID>-<random>.scope` through the
user manager's `StartTransientUnit` (LAUNCH-06; failure is logged, and `KillMode=process`
is the backstop). A launch that fails notifies with buttons for up to three other
available targets (LAUNCH-07); the service remembers the last 8 such notifications for
their buttons and forgets older ones. A rejected link notifies with the reason (PIPE-02).
Notifications use a small `org.freedesktop.Notifications` client with the `desktop-entry`
hint. Lock state combines logind's `LockedHint` for the user's graphical session with
`org.freedesktop.ScreenSaver.ActiveChanged`; a link that needs the picker while the screen
is locked waits for the unlock (PKS-07), a newer one replacing it.

A link that needs the picker goes to one function, `to_picker`, which hands it to the
picker broker (`api/picker.rs`): a `PickerRequest` sent with `PickerHost1.ShowPicker` to
`wye-ui`, answered with `PickerChose`, `PickerCancelled` or `PickerAction`. A newer link
replaces the pending one (PICK-27). When the UI host cannot be reached, the link opens with
the stand-in (see "Picker fallback").

### Configuration and state

Configuration is `$XDG_CONFIG_HOME/wye/config.toml`. It is hand-editable and never written
with internal state. Every `wye` invocation reads it afresh; the service keeps a cached
copy and reloads it when the file changes (100 ms debounce,
[12-data-model.md](spec/12-data-model.md#storage)). A broken or unreadable file never stops
a link, but the two paths recover differently (design risk 16): the service keeps the last
good configuration and reports why the file is not in use, while `wye open` routing a link
itself, and every other CLI command, uses the defaults. On the `wye open` path every diagnostic is written to stderr best effort,
and the picker stand-in is announced only after the launch, because apps often start Wye
with a closed or broken stderr.

Internal state lives in `$XDG_STATE_HOME/wye/state.toml`, because home-manager may make the
config file read-only. It holds the browsers to restore (`previous-default-browser`,
`previous-kdeglobals-browser`), onboarding and UI state, and the default the user chose to
keep (`kept-default`); the CLI and the service share the type (`wye_desktop::State`). State and `mimeapps.list` are replaced atomically through one helper
(`wye_desktop::atomic::write`: a temporary file in the same directory, synced, then renamed),
which never writes through a symlink.

### Loop guard (DEF-06)

Wye is the default browser, so anything that opens a link "in the default browser" opens it
in Wye again. Beyond Wye's own desktop ID, an entry whose `Exec` runs `wye` or a generic
opener (`xdg-open`, `gio`, `kde-open`, `handlr` and the others in
`wye_desktop::loop_guard::OPENERS`) is never a web handler or an available target, and
`build_command` refuses any command line that runs one, or that resolves to the running
executable. `wye default set` never remembers such an entry as the browser to restore, and
`wye default unset` never restores one. Shell wrappers and D-Bus calls to the `OpenURI`
portal are not recognised.

### Default browser (DEF-02, DEF-05)

The current default is the first *installed* desktop ID listed for
`x-scheme-handler/https` in the `mimeapps.list` lookup order, as the mime-apps specification
says; IDs whose entry is missing are skipped. `wye default unset` changes nothing (and
exits 0, saying so) when Wye is no longer the default, so a browser the user chose since is
never overwritten; the remembered browser is kept. On Plasma, `BrowserApplication` in
`kdeglobals` is written through `kwriteconfig6` when it is available, else with an atomic
write.

### Startup notification

The desktop entry keeps `StartupNotify=true`: that is how launchers hand Wye the activation
token (`XDG_ACTIVATION_TOKEN`, `DESKTOP_STARTUP_ID`) it passes on to the browser (LAUNCH-03).
The cost: when Wye starts nothing that consumes the token, as with a background launch
(LAUNCH-04, which removes the variables) or a rejected link, the launcher's startup sequence
is left to time out, which on X11 shows a busy cursor for a few seconds.

### Source-app detection

`wye open` walks its parent chain in `/proc`, skipping launch helpers and shells, and takes
the app's desktop ID from its systemd scope or `GIO_LAUNCHED_DESKTOP_FILE`. When the chain
names nothing, as when `gio open` has already exited and Wye was reparented to
`systemd --user`, the app unit in Wye's own `/proc/self/cgroup` is used, unless it is Wye's
own. The service starts at the D-Bus caller's PID instead (see "How a link reaches the
service") and matches the executable against desktop entries' `Exec` (spec step 3,
`wye_desktop::source_app::detect_in`); so does `wye open` when it routes a link itself. The
portal case falls back to the focused window (spec step 4, see "Session probes"); where no
probe can read it, the source is unknown.

### Session probes

Held modifiers (BRW-03, RUL-27, ADV-11), the pointer (PICK-02) and the focused window's app
(source-app step 4) are what a Wayland client cannot normally read. The service finds its
probes once at start-up with self-checks that take no focus
(`Platform::with_session_probes`), and `Status.capabilities` names the mechanism of each, or
none (KEY-06: modifier choosers are disabled without one).

- **Held modifiers on Wayland** (`platform/modifiers/wayland.rs`, mechanism
  `wayland-layer-shell`): the self-check looks for `zwlr_layer_shell_v1`, `wl_seat`,
  `wl_shm` and `wl_compositor`. A probe opens its own Wayland connection, maps a 1×1 fully
  transparent layer surface on the overlay layer with `KeyboardInteractivity::Exclusive` and
  an empty input region, waits for `wl_keyboard.enter` and the `modifiers` that follow, and
  closes the connection, which destroys the surface and returns focus. The mask is read with
  xkbcommon and the compositor's keymap (`platform/modifiers/xkb.rs`): pressed or latched
  Shift, Control, Mod1/Alt and Mod4/Super count; locks do not. On Plasma 6.7 a probe takes
  about 3 ms. GNOME has no layer shell, so held keys are unavailable there.
- **Held modifiers on X11** (`platform/modifiers/x11.rs`, mechanism `x11`): the mask of
  `QueryPointer`, with Mod1 as Alt and Mod4 as Super.
- **Pointer and focused window on Plasma** (`platform/kwin.rs`, mechanism `kwin-script`):
  `data/kwin/wye-query.js` is rendered with a random nonce and the reply address (the
  connection's unique name and `/dev/soldunov/wye`), written to
  `$XDG_RUNTIME_DIR/wye/kwin/wye-query-<nonce>.js`, loaded with
  `org.kde.kwin.Scripting.loadScript(path, "wye-query-<nonce>")` and started. The script
  reads `workspace.cursorPos`, `workspace.screenAt(…)` and `workspace.activeWindow` and calls
  `dev.soldunov.wye.KWin1.Report` back. Only a report from `org.kde.KWin`'s unique name is
  accepted; one query answers both the source lookup and the picker placement (250 ms);
  the script is unloaded and its file removed after the answer or the timeout. The pointer
  is relative to the output under it. About 7 ms on Plasma 6.7. `KWin`'s `callDBus` sends
  every JavaScript number as `int32`, so the script truncates numbers with `| 0` and
  `Report` takes the PID as `i`.
- **Pointer and focused window on other X11 sessions** (`platform/x11.rs`, mechanism
  `x11`): `QueryPointer` and the RandR monitor under it; `_NET_ACTIVE_WINDOW`, then
  `_NET_WM_PID`, `_KDE_NET_WM_DESKTOP_FILE` or `_GTK_APPLICATION_ID`, and `WM_CLASS`.

Every probe gives up after 150 ms: the modifiers and the source app are then unknown and the
picker is centred. `advanced.held-keys = "off"` in `config.toml` (default `"auto"`) switches
the modifier probe off; it is read on every probe, and while off the capability reads as
none. `bindings_need_modifiers` tells whether any binding (the alternative-browser key, a
rule's held keys, the bypass key on an extension link) makes a probe worth running.

`wye debug probe [--delay N]` runs the same probes once in its own process, serving its own
`KWin1` object, and prints what the service would see.

### Network expansion and the clipboard

Short links are followed over HTTP only for enabled short-link domains (DLG-EXP-03); the
core owns the redirect loop (`ExpansionCatalogue::expand_short_link`) and the service answers
one hop at a time (`wye_service::platform::http::Resolver`): `HEAD`, then a body-less `GET`
when the server refuses `HEAD` (403, 405, 501). The client is ureq 3 with rustls and the
platform verifier: redirects off, no cookie store, a `User-Agent` of `Wye/<version>`. One
deadline (`advanced.expansion.timeout-ms`) covers the whole chain; when it runs out the link
continues as far as it got (PIPE-03), and `notify-on-failure` announces it. `TestLink`
follows short links too unless the tester passes `skip-network`. Songlink (EXT-15) is one
`GET` to the Odesli API with a 5 s timeout; any failure leaves the clipboard unchanged.

The clipboard provider is chosen at start (risk 9): Wayland data control
(`ext-data-control-v1`, which KWin 6 offers, else `zwlr-data-control-v1`), else XFIXES on
X11, else Klipper over D-Bus, else none. The Wayland and X11 providers run a worker thread
that owns the connection, reads each new selection that offers plain text (never one with
`x-kde-passwordManagerHint` or an image), and serves Wye's own writes from a data source it
keeps alive until another client takes the clipboard. Only one line of text is passed on.
The copy-time rewrites (EXT-02, EXT-03, EXT-05) go through core `clipboard::decide`, and
the text Wye wrote is remembered so it is not rewritten again (EXT-12). `OpenClipboard`
sends the link through the full pipeline with entry `clipboard`, no source app and held
keys known to be none (the shortcut's own keys are still down); `alternative` acts as the
alternative-browser key (IN-04). Without a watching provider, `Status` reports
`clipboardWatch: null` and the troubleshooting text gives the explanation from
19-help-texts.md.

### Tray (01-tray-menu.md)

One model, `wye_core::tray::TrayMenu`, built in `crates/wye-service/src/api/tray.rs` from
the configuration, the installed apps, the clipboard (TRAY-10), the default-browser
registration (TRAY-18, ONB-11) and the history (TRAY-15), is published as the `Tray`
property and rendered by every tray host. Hosts send back only the chosen item's ID
(`ActivateTrayItem`); `api/tray/action.rs` is the one place that turns an ID into a service
call. Every topic that changes the menu announces `Tray`, and the service follows its own
`PropertiesChanged` to keep its tray item in step.

- **Plasma applet** (`frontends/plasma/dev.soldunov.wye`, pure QML, Plasma 6.4+, in the
  system tray by default): `DaemonClient.qml` is its only D-Bus code. It calls
  `RegisterTray("plasma-applet")` when it loads and whenever the service reappears (which
  also starts the service through D-Bus activation), draws the icon from `Tray.icon` with
  a warning emblem, and opens a native `PlasmaExtras.Menu` on a primary click, asking
  `ClipboardHasUrl` first. The shortcut column is Qt's tab-separated text, so it shows on
  every style (KEY-51: the accelerators are fixed). While the tray icon is off (TRAY-04)
  or no service runs, the applet's status is Hidden: the tray moves it to its hidden items.
  `MenuRequested` is ignored: the toggle-menu shortcut shows the `wye-ui` popup
  (decision 4).
- **StatusNotifierItem** (`platform/sni.rs`, ksni): `ItemIsMenu`, the same model as a
  `DBusMenu` (one radio group, disabled headers, submenus, `Control`-style shortcuts),
  `OverlayIconName` `emblem-warning` and `NeedsAttention` while Wye is not the default.
  `AboutToShow` rebuilds the menu with a fresh clipboard state. It runs only while the
  icon is visible and no tray host is registered; a host is forgotten when its bus
  connection closes, and the item comes back. On KDE the service waits 5 s after start
  for the applet before showing it.

The icons are `dev.soldunov.wye-symbolic` (GEN-02 "Wye") and
`dev.soldunov.wye-picker-symbolic` (the picker glyph), generated by
`data/icons/src/generate.py`. `nix/frontends.nix` stamps the applet's version and the
default package installs it under `share/plasma/plasmoids`; the flake checks lint it,
load it with KPackage and run its `DaemonClient` against a real `wye service` on a private
bus (`frontends/plasma/tests/`).

### Transform scripts (16-script-editor.md)

`crates/wye-script` runs the global script (PIPE-05, ADV-03) and the rules' scripts
(PIPE-14, RUL-25) on QuickJS through `rquickjs`. Every run gets a fresh runtime with a
16 MB memory limit, a 512 KB interpreter stack and an interrupt handler that stops it
50 ms after the user's code starts (SCR-21); the script is compiled as an ES module named
`transform.js` and its `default` export is called with a `URL` and the context object.
`URL` and `URLSearchParams` are a small JavaScript prelude over native functions that
use the `url` crate's `quirks` module (its WHATWG URL API), so getters and setters behave
as in a browser (SCR-20). The prelude is JavaScript rather than `rquickjs` classes because
the class derive emits `unsafe impl`, which the workspace forbids. `console.log` lines are
captured (100 per run); there is no loader, network, file access or timer. A result that
is not `undefined`, a string or a prelude `URL`, or not an `http`/`https` link, is a
runtime error (SCR-23); an unchanged link handed back is always accepted (local HTML
files, DEF-07). `check` compiles without evaluating (SCR-07).

The service (`crates/wye-service/src/api/scripts.rs`) reads `transform.js` and
`rules/<rule-id>.js` next to `config.toml` for every link, so an edit takes effect on the
next link with no reload. `LinkHooks` (`api/link/hooks.rs`) hands the transformer to
`resolve_with` and `finish` on the link path, after the picker (`api/picker.rs`,
`api/picker_fallback.rs`) and in `TestLink`. A failing script leaves the link unchanged; the
link path notifies once per script text (SCR-22), keyed by an FNV-1a hash of scope and
text in `state.toml` (`script-errors-notified`, last 64); `TestLink` never notifies.
`SetScript` refuses a script that does not compile with `ScriptSyntax` (`line:column:
text`) and writes atomically. The configuration directory is watched recursively; a
script whose text differs from what the service last read or wrote is announced with
`ScriptFileChanged(scope)` after 100 ms of quiet (SCR-08), so the service's own saves are
silent. A rule's script needs the rule's `id`; `ImportRules` assigns `rule-<n>` IDs.

The editor (`crates/wye-ui/qml/script/`, `bridge/script_editor.rs`, logic in
`src/script_editor/`) is a `TextArea` with `org.kde.syntaxhighlighting`'s JavaScript
definition, a line-number gutter, auto-indent and bracket matching. It test-runs the text
300 ms after each change with `RunScript` and marks the changed parts of the result, the
error line and the run time (SCR-04, SCR-05); Save is disabled while the last run reported
a `SyntaxError` (SCR-07), and a `ScriptFileChanged` for its scope shows a Reload banner
(SCR-08).

### Shipped data is embedded

`data/services.toml`, `data/expansion.toml`, `data/tracking-parameters.toml` and
`data/kwin/wye-query.js` are embedded at build time. The binary needs no data directory at
run time.

### Global shortcuts (KEY-40, KEY-41, ADV-05 to ADV-07)

`platform/shortcuts.rs` picks the mechanism at start: the `GlobalShortcuts` portal
(`platform/shortcuts/portal.rs`, ashpd) when the session has one, else none. The portal
client uses a session-bus connection of its own, because the host-app registry
(`org.freedesktop.host.portal.Registry.Register("dev.soldunov.wye")`) must name the app
before that connection makes any other portal call. It opens one long-lived session and
binds `toggle-menu`, `clipboard-primary` and `clipboard-alternative`, each with the
`[shortcuts]` value as `preferred_trigger` (KEY-03 names turned into `CTRL+ALT+w`). On
Plasma they then appear in System Settings → Shortcuts, where the user's choice wins over
the preference. `BindShortcuts` is answered once per session and may wait for a portal
dialog, so it runs in a task, and binding again (`SetShortcut`) closes the session and
opens a new one. `Activated` becomes the action ID on the provider's channel;
`api/shortcuts.rs` runs `ToggleMenu` or `OpenClipboard(alternative)` for it. Without a
mechanism, `GetShortcuts` reports `none` and Settings shows the commands to bind by hand
(`wye menu`, `wye clipboard`, `wye clipboard --alternative`). X11 key grabs are not
implemented.

### Tray-menu popup (TRAY-08, decision #14)

`ToggleMenu` (the shortcut, `wye menu`, the `menu` desktop action) emits `MenuRequested`
and sends `PickerHost1.ShowMenu` the `Tray` model plus the pointer from the
`PointerSource`. `wye-ui` shows the same items in a transparent layer-shell overlay
(`qml/traymenu/`, model in `src/tray_menu/`), its corner at the pointer or centred;
a second toggle, a click outside, Escape or focus loss closes it. A chosen item goes back
through `ActivateTrayItem`, the dispatcher every tray host uses; `P` and `1`–`9` choose
directly (KEY-51).

### Browser extension (BEXT-01 to BEXT-06, IN-05)

`frontends/extension/` is one set of files for Firefox (`manifest.firefox.json`, gecko ID
`wye@soldunov.dev`, event page) and Chromium (`manifest.chromium.json`, a fixed `key`
that makes the ID `lphepmclmllmbbkjkdhjbdgbjfpmmdnn`, service worker); `build.sh`
assembles one family's unpacked extension. It sends links and pages to the
native-messaging host `wye-native-host` (crate `crates/wye-native-host`), which reads length-prefixed JSON, takes the browser (its parent
process) as the source app and the click's held keys when the browser reports them
(Firefox), and calls `OpenLink` with `entry = "extension"`; the pipeline forces the picker
unless the bypass key is held (ADV-10, ADV-11). `wye_desktop::native_messaging` writes the
host manifest into every detected browser's directory (`NativeMessagingHosts/` for the
Chromium family, `native-messaging-hosts/` for the Firefox family), naming the host by
its `PATH` location so upgrades keep it valid; `wye extension install|remove` and
`wye-native-host --install|--remove` run it (`crates/wye-native-host/src/install.rs`,
used by both). Flatpak and Snap browsers are not supported.

### Picker fallback

When the picker cannot be shown (the UI host cannot be reached within 10 s, or `wye open`
routes a link itself without a service), a link that resolves to the Picker opens in the
remembered previous default browser, else the first shown browser, else the first
discovered browser (`wye_desktop::stand_in::choose`). The service notifies
(`crates/wye-service/src/api/picker_fallback.rs`); `wye open` prints a warning
(`crates/wye/src/commands/open.rs`). The chooser both share is
`crates/wye-desktop/src/stand_in.rs`.

### Packaging and channels (decision 11)

The flake builds the package (`packages.<system>.wye-git`, also `default`, `wye`) from its
own source: binaries, data files, the D-Bus service files and systemd user unit with
absolute paths, the Plasma applet under `share/plasma/plasmoids`, and the extension zips
under `share/wye/extension`. `packages.<system>.wye-release` exists once `nix/release.json`
records `{version, rev, narHash}`; it is the `default` package of that tag's own flake,
fetched with the locked reference `github:psoldunov/wye/<rev>?narHash=<hash>` through
`builtins.getFlake`, which pure evaluation accepts because the reference is locked. An old
release therefore never meets newer packaging. The modules, though, always come from the
flake the user locked, so they rely only on the package layout every release keeps; the
contract is listed in `nix/channel.nix`. `.github/workflows/release.yml` writes
the file through a pull request after tagging.

`nix/channel.nix` gives the home-manager and NixOS modules the same `programs.wye.channel`
(`release` | `git`, default `release` when a release is recorded) and
`programs.wye.package` (default per channel, overridable). Asking for `release` before one
exists fails with an assertion. The home-manager module also writes `config.toml` from
`settings` (only when set; the file is then read-only), the systemd user units `wye` and
`wye-ui`, the D-Bus files, `xdg.mimeApps` (with added associations for local HTML files,
DEF-07) and the `kdeglobals` browser; the NixOS module wires the package into
`environment.systemPackages`, `services.dbus.packages` and `systemd.packages`. Both own
login start (GEN-01) through the unit's `WantedBy`, controlled by `launchAtLogin`, and set
`WYE_LOGIN_MANAGED=1` on it, so the service never writes or removes the XDG autostart entry
and Settings shows "Launch at login" as managed. Both prepend the Nix profile directories to
a `PATH` that ends in `/usr/local/bin:/usr/bin:/bin`, which bare desktop-entry `Exec`s are
resolved against. Native-messaging manifests are written
at run time by `wye-native-host --install`, not by Nix.

## Not yet implemented

- The GNOME Shell extension (decision 1): the tray on GNOME needs the AppIndicator
  extension for the StatusNotifierItem, and held keys, the pointer and the focused window
  have no GNOME source. The reserved `PickerHost1`, `SessionHelper1` and `RegisterTray`
  contract is where it will plug in.
- The `SessionHelper1` interface (reserved for that extension; no implementation)
- Focused window and pointer on Sway and Hyprland (compositor IPC); held keys there come
  from layer shell only
- Global shortcuts by X11 key grabs; shortcuts need the `GlobalShortcuts` portal
- An AppImage (decision 5); Nix is the only package, through the flake and the home-manager
  and NixOS modules
- The browser extension for Flatpak and Snap browsers: their sandbox cannot start
  `wye-native-host` (profile discovery does cover the Flatpak and Snap builds listed in
  `crates/wye-desktop/src/family.rs`)

## Runtime files

| Path | Contents |
|------|----------|
| `$XDG_CONFIG_HOME/wye/config.toml` | User configuration. Hand-edited. |
| `$XDG_STATE_HOME/wye/state.toml` | Internal state: the browsers to restore (`previous-default-browser`, `previous-kdeglobals-browser`), `kept-default`, onboarding and UI state, and the script errors already notified (`script-errors-notified`). |
| `$XDG_STATE_HOME/wye/history.json` | Recent links (DLG-HIS), newest first, at most 100 entries (decision 10). |
| `$XDG_CONFIG_HOME/autostart/dev.soldunov.wye.desktop` | The XDG autostart entry (`wye service --activate`) while "Launch at login" is on, unless a Nix module owns login start. |
| `$XDG_CONFIG_HOME/mimeapps.list` | Default browser association, written by `wye default set`. |
| `$XDG_RUNTIME_DIR/wye/kwin/wye-query-<nonce>.js` | One `KWin` query script, only while its query runs. |
| `$XDG_CONFIG_HOME/wye/transform.js` | The global transform script (ADV-03). Hand-editable. |
| `$XDG_CONFIG_HOME/wye/rules/<rule-id>.js` | A rule's transform script (RUL-25). Hand-editable. |
