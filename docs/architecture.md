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
| `crates/wye-gtk` | The GTK 4 / libadwaita host `wye-gtk` that owns `dev.soldunov.wye.Gtk` and serves `Windows1` (Settings, the script editor, the onboarding, about and history windows) and `PickerHost1` (the picker and the tray-menu popup, with gtk4-layer-shell on wlroots compositors and KDE, an undecorated window elsewhere). A D-Bus client of the service; holds no routing logic. Shares wye-ui's Qt-free models from source. |
| `frontends/gnome-shell/` | The GNOME Shell extension `wye@dev.soldunov` (Shell 48+): owns `dev.soldunov.wye.Gnome`, serves `PickerHost1`, and draws the picker and the panel tray menu inside the Shell. |
| `frontends/extension/` | The Firefox and Chromium browser extension: one set of files, a manifest per family. |
| `data/` | Shipped data (`services.toml`, `expansion.toml`, `tracking-parameters.toml`), the desktop entry `dev.soldunov.wye.desktop`, the hicolor icon, and templates with `@bindir@` for the D-Bus service files (`data/dbus/`) and the systemd user units (`data/systemd/wye.service.in`, `data/systemd/wye-ui.service.in`, `data/systemd/wye-gtk.service.in`), and the KWin query script `data/kwin/wye-query.js`. |
| `nix/`, `flake.nix` | Package (crane; `frontends.nix` adds the extension zips), the `programs.wye` modules for home-manager and NixOS (`hm-module.nix`, `nixos-module.nix`, shared `channel.nix`), the release record `release.json` and the checks: clippy, tests (including `crates/wye/tests/e2e.rs`), fmt, deny, machete, source and installed desktop entry, installed D-Bus files, qmllint, the UI self-test, the GTK self-test, the extension manifests, module evaluation for both channels and nixfmt. Also the dev shell. The package rewrites the installed desktop entry's `Exec` to its own absolute `bin/wye` and adds `TryExec`; the source entry in `data/` stays generic. |

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
| Service | `wye service` (crate `wye` over the `wye-service` library) | `dev.soldunov.wye` | D-Bus activation (`SystemdService=wye.service`); at login, the unit's `WantedBy=graphical-session.target` when a Nix module installs it (`WYE_LOGIN_MANAGED=1`), else the XDG autostart entry when "Launch at login" is on |
| UI host | `wye-ui` (cxx-qt, Kirigami) | `dev.soldunov.wye.Ui` | D-Bus activation by the service (`SystemdService=wye-ui.service`, never at login); stays resident once started |
| GTK host | `wye-gtk` (GTK 4, libadwaita) | `dev.soldunov.wye.Gtk` | D-Bus activation by the service (`SystemdService=wye-gtk.service`, never at login); stays resident once started |
| GNOME Shell extension | `frontends/gnome-shell/` | `dev.soldunov.wye.Gnome` | GNOME Shell, when the extension is enabled; never bus-activatable |
| Tray | StatusNotifierItem inside the service | ksni's own | the service at start, on every desktop, unless an external tray host called `RegisterTray` |
| Link handler fallback | `wye open %U` | none | launchers that do not honour `DBusActivatable` |

Single instance: the service requests its name with `DoNotQueue` and exits with status 75
when the name is taken; `wye.service` lists 75 in `RestartPreventExitStatus=` and uses
`KillMode=process`, so launched browsers survive a restart (LAUNCH-06). The service serves
its objects before requesting the name, so the very first call of a bus activation is
answered. It claims `dev.soldunov.wye` before asking the session anything slow; lock, clipboard, held-key, pointer and focus detection then run within 3 s
each, the shortcuts portal within 20 s in the background
(`crates/wye-service/src/platform/session.rs`); `Status` is announced again once they are
known. A link that needs held keys and arrives while the probes are still being detected
waits up to 300 ms for them. `advanced.held-keys` is read from the service's cached
configuration.

Structured data travels as JSON in `s` values (camelCase), described by the serde types in
`wye-api`, because QML frontends parse JSON far more easily than nested D-Bus structs.

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
   stands in. A chain that names no app leaves the source unknown.
2. **`wye open` (Exec fallback).** It detects the source from its own parent chain, reads
   `XDG_ACTIVATION_TOKEN`/`DESKTOP_STARTUP_ID`, and calls `OpenLink` for each link, bounded
   by 3 s including bus activation. Without a desktop ID it also sends its parent's PID, so
   the service can match the executable against the installed apps. A call that reached
   no service falls back to routing here: no session bus, `ServiceUnknown`/`NameHasNoOwner`/
   `Unknown*`, `Spawn.*`, any `org.freedesktop.systemd1.*` error, `NotImplemented` from an
   older service, or any other error from the bus daemon except `NoReply`/`TimedOut`
   (`crates/wye/src/bus.rs`). `wye open` then routes the link itself with the service's own
   hooks (short-link expansion, transform scripts; `wye_service::offline::OfflineHooks`) and
   `finish`, with the picker stand-in; held keys and the lock state are unknown there. A call
   that was sent but not answered within 3 s is never repeated here, since the service may
   still open the link; `wye open` exits 1. A link the service refused (exit 2) or failed to
   launch (exit 1) is not retried either: the service has already told the user.

In the service a link goes through `crates/wye-service/src/api/link.rs`: it takes a
`Snapshot` of the cached configuration, the installed apps and the remembered default
browser (`api/config.rs`, `snapshot`), runs the pipeline on a blocking thread
(`resolve_with` and `finish`, with the expansion and transform-script hooks of
`api/link/hooks.rs`), and builds the command line with `wye_desktop::build_command`. The `Launcher` starts it with the activation token
set (or removed for a background launch, LAUNCH-03/04) and, when Wye runs from its AppImage,
the session's environment rather than the AppImage's (LAUNCH-08,
`crates/wye-desktop/src/launch/appimage.rs`); its reaper logs an app that exits with a
failure. The `ScopeManager` moves the
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
the frontend's picker host (see "Frontends"), answered with `PickerChose`,
`PickerCancelled` or `PickerAction`. A newer link replaces the pending one (PICK-27). When
no UI host can be reached, the link opens with the stand-in (see "Picker fallback").

### Frontends (ADV-12)

Two frontends serve the same internal interfaces: KDE's `wye-ui` (`dev.soldunov.wye.Ui`,
`PickerHost1` and `Windows1`), and GNOME's Shell extension (`dev.soldunov.wye.Gnome`,
`PickerHost1`, never bus-activatable) with the GTK host `wye-gtk` (`dev.soldunov.wye.Gtk`,
`Windows1` and `PickerHost1`). `advanced.frontend` orders them;
`crates/wye-service/src/api/picker/frontend.rs` is the pure table and
`crates/wye-service/src/api/picker/host.rs` makes the calls:

| `advanced.frontend` | Session | Picker and tray popup, in order | Windows, in order |
|---|---|---|---|
| `auto` (default) | GNOME Shell | Shell, GTK, Qt | GTK, Qt |
| `auto` | any other | Qt | Qt |
| `kde` | any | Qt, Shell, GTK | Qt, GTK |
| `gnome` | any | Shell, GTK, Qt | GTK, Qt |

A GNOME Shell session is one where `org.gnome.Shell` has an owner on the session bus
(`platform/gnome_shell.rs`, asked at each call): Budgie and GNOME Flashback name GNOME in
`XDG_CURRENT_DESKTOP` but run no Shell, so `auto` keeps Qt there. With the extension off,
the GTK host shows the picker as well as the windows, so one toolkit serves both.

A host that is neither running nor activatable is skipped, and the last one is always
called. A picker host that fails in any way, `UnknownInterface` from a GTK host without
`PickerHost1` included, hands the call to the next one with a warning: a link must reach
someone (PIPE-13). A window host only hands over when it cannot be reached; an error it
answers with is reported. A chain shares one 10 s deadline: each host before the last gets
half of what is left and the last all of it, so a single host (KDE under `auto`) keeps the
whole deadline and a chain of hanging hosts reaches the stand-in no later than one host did.
`ClosePicker` goes to every running host, since a request may have reached any of them
before the setting changed or a host left; a host without `PickerHost1` counts as closed,
and one whose name cannot be checked is skipped. The service records which host shows the
pending request: when a newer request shows on another host, the older one is closed where
it shows (PICK-27); when the Shell leaves the bus while it shows the pending request, the
request is shown on the next host, unless the screen is locked (the lock closes the picker
and holds its link; GNOME switches extensions off while locked). After the unlock, the held
link's picker waits up to 2 s for the Shell's name when the Shell heads the hosts (PKS-07).
The setting is read from the cached configuration at each call, so a change applies to the
next picker, popup or window. The warm-up (PICK-25, `ready.rs`) starts the first host that
would show the next picker, none while the Shell runs it; a running host whose
introspection lacks `PickerHost1`, or a host that does not start, is passed over, so with
`gnome` on a desktop without the Shell both the GTK host (windows) and `wye-ui` (picker)
are kept ready. It runs again when the setting changes and when a host it keeps ready
leaves the bus; another host leaving neither restarts anything nor delays the next restart.

The Nix modules set the key with `programs.wye.frontend` (`nix/frontend.nix`): into the
managed file when home-manager's `settings` makes one, else from an `ExecStartPre` of the
`wye` unit that writes it into the writable file at each start (`auto` writes nothing; a
read-only file is left alone, and the file keeps its mode). Unless `kde` is chosen, the
home-manager module also declares the `wye-gtk` unit and its D-Bus activation file, for a
package that ships the GTK host (`passthru.hasGtk`, `nix/channel.nix`).

### Configuration and state

Configuration is `$XDG_CONFIG_HOME/wye/config.toml`. It is hand-editable and never written
with internal state. Every `wye` invocation reads it afresh; the service keeps a cached
copy and reloads it when the file changes (100 ms debounce,
[12-data-model.md](spec/12-data-model.md#storage)). Without inotify the watched files are
polled every 2 s by canonical path and the modification times of the link and its target,
so a Nix profile swapping a symlink is seen. A broken or unreadable file never stops
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
which never writes through a symlink. Atomic replacement stops torn files, not lost updates,
so every writer holds an exclusive `flock` on `state.toml.lock` around load, change and save
(`wye_desktop::State::update`, `StateLock`); readers take no lock. A writer reports an
unreadable state file instead of replacing it, because it may hold the only record of the
previous default browser.

### Loop guard (DEF-06)

Wye is the default browser, so anything that opens a link "in the default browser" opens it
in Wye again. Beyond Wye's own desktop ID, an entry whose `Exec` runs `wye` or a generic
opener (`xdg-open`, `gio`, `kde-open`, `handlr` and the others in
`wye_desktop::loop_guard::OPENERS`) is never a web handler or an available target, and
`build_command` refuses any command line that runs one, or that resolves to the running
executable. `wye default set` never remembers such an entry as the browser to restore, and
`wye default unset` never restores one. Shell wrappers and D-Bus calls to the `OpenURI`
portal are not recognised.

### Back to the source app, sign-in pages and app hosts (DEF-08, DEF-09, APP-13)

The logic is in `wye-core`: `crates/wye-core/src/sign_in.rs` (`is_sign_in_page`) and
`crates/wye-core/src/pipeline/guard.rs` (`Guard`, `SkipReason`). `Pipeline::decide` builds one
`Guard` per link. It can skip the target of a rule (PIPE-07, PIPE-09) or of a web app mapping
(PIPE-08) when that target is an app and not a browser. Browser targets, browser profiles,
private windows, the Picker, Default, the alternative-browser key, the fallback and the
picker's choice are never skipped. DEF-08 applies only to IN-01: a link an app hands to the
default browser. Other entry points are explicit requests. `SourceApp::is_app` compares the
source with the target by desktop ID, then by executable or window class. The browser test is
`Availability::is_browser`, which `wye_desktop::Inventory` answers from `handles_web`, the
same flag the Browsers list uses.

A page is a sign-in page by its host's first label or by one of its first two path segments,
never by later segments, because later segments hold names users choose (a Figma file named
"Login"). The cost is that a page whose own name sits in the first two segments, such as
`t.me/login`, opens in a browser. Routes the generic words miss are listed per service as
`sign-in-paths` in `data/services.toml` (ClickUp's `/api` OAuth page). `ServiceDefinition::is_sign_in`
in `wye_core::catalogue` tests the generic words or the service's own paths, and
`Guard::skip_mapping` uses it. Rules name no service, so `Guard::skip` uses only the generic
detector.

A service's hosts say which links belong to it; its `app-hosts` say which of them its desktop
app opens (APP-13). The two differ because a browser profile that holds the account should
still get every host of the service (Notion Mail, the MCP sign-in), while the Notion app opens
workspace pages on three hosts only and cannot give the rest back to the browser without a
loop. `ServiceDefinition::app_opens_host` compares hosts exactly (`host::host_is`), so
`notion.so` does not cover `mail.notion.so`, and `Guard::skip_mapping` skips a mapping to an
app with `SkipReason::NotAppHost` after the sign-in test. The service still matches, so a
mapping to a browser target is unaffected.

A skip is a trace step of kind `rule` or `web-app`, so the
D-Bus contract gets no new step kinds and the UIs, the rule tester and `wye test` show it
unchanged.

### Default browser (DEF-02, DEF-05)

The current default is the first *installed* desktop ID listed for
`x-scheme-handler/https` in the `mimeapps.list` lookup order, as the mime-apps specification
says; IDs whose entry is missing are skipped. `wye default unset` changes nothing (and
exits 0, saying so) when Wye is no longer the default, so a browser the user chose since is
never overwritten; the remembered browser is kept. With local HTML files on (DEF-07), only
Wye is added to `[Added Associations]` for the HTML types; giving links back to the previous
browser removes Wye there and adds nothing. On Plasma, `BrowserApplication` in
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
focused window stands in (spec step 4, see "Session probes") in two cases: the portal, and a
`wye open` that runs in Wye's own app unit with a chain that names no app (KIO on Plasma 6
starts it as a unit of its own, so its parent is `systemd --user`). Other chains that name no
app (a timer, `systemd-run`) stay unknown, as does the source where no probe can read the
focus.

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
  about 3 ms. GNOME has no layer shell; the Shell extension answers there (below).
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
- **GNOME** (`platform/gnome_shell.rs`, mechanism `gnome-shell`): on a GNOME Shell
  session (`org.gnome.Shell` has an owner when the service starts), whatever no mechanism
  above provides (on Wayland: the clipboard, held modifiers, the pointer and the focused
  app) is asked of the Shell
  extension's `SessionHelper1` ([dbus-api.md](dbus-api.md)). The helper follows
  `dev.soldunov.wye.Gnome`: it is unavailable while the extension does not run or serve
  the interface, and used again as soon as it does. It asks for clipboard changes only
  while a copy-time rewrite is on (`ClipboardProvider::set_watching`, from each
  configuration load), and the extension sends them to the service alone. Plasma never
  reaches this path, so nothing changes there.

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
X11, else Klipper over D-Bus, else on GNOME the Shell extension, else none. The Wayland and X11 providers run a worker thread
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
property and rendered by the tray item and the `wye-ui` popup. Both send back only the chosen item's ID
(`ActivateTrayItem`); `api/tray/action.rs` is the one place that turns an ID into a service
call. Every topic that changes the menu announces `Tray`, and the service follows its own
`PropertiesChanged` to keep its tray item in step.

- **StatusNotifierItem** (`platform/sni.rs`, ksni): Wye's tray on every desktop, KDE
  Plasma included. `Id` `dev.soldunov.wye`, `Title` and tooltip title "Wye", `Category`
  `ApplicationStatus`, `ItemIsMenu` (a primary click opens the menu, TRAY-07). The icon is
  `Tray.icon` (TRAY-02); `IconThemePath` is the package's own `share/icons` when the binary
  runs from an installed package, so hosts find Wye's icons even off `XDG_DATA_DIRS`. While
  Wye is not the default browser, `Status` is `NeedsAttention`, `OverlayIconName` is
  `emblem-warning` and the tooltip says so (TRAY-18, ONB-11); otherwise the tooltip names
  the primary browser. A middle click (`SecondaryActivate`) opens Settings (TRAY-19);
  scrolling does nothing. The menu is the same model as a `DBusMenu`
  (`platform/sni/menu.rs`): one radio group with `toggle-state`, disabled headers,
  separators, the More submenu, `Control`-style shortcuts, and item icons by theme name
  (an absolute PNG path goes as `icon-data`, since most menu hosts do not load paths).
  `AboutToShow` rebuilds the menu with a fresh clipboard state (TRAY-10). The item runs
  only while the icon is visible (TRAY-04), from the moment the service starts.
- **External tray hosts** (decision 8): `RegisterTray` stays in the API for a host that
  draws the tray itself (the GNOME Shell extension, or the Plasma applet earlier versions
  shipped). While one is registered the item is hidden; a host is forgotten when it
  unregisters or its bus connection closes, and the item comes back at once.

The icons are `dev.soldunov.wye-symbolic` (GEN-02 "Wye") and
`dev.soldunov.wye-picker-symbolic` (the picker glyph), generated by
`data/icons/src/generate.py`. `crates/wye-service/tests/tray_sni.rs` reads the item's
properties and menu on a private bus, the way a tray host does.

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
that makes the ID `jdcifhpoallkdjnbflfienpboodjfjei`, the Chrome Web Store item's; the
host manifest also allows Wye 1.0.0's `lphepmclmllmbbkjkdhjbdgbjfpmmdnn`; service
worker); `build.sh` assembles one family's unpacked extension. `nix/frontends.nix` zips
each family and a third zip, the Chromium build without its `key`, which the Chrome Web
Store requires; releases attach all three. It sends links and pages to the
native-messaging host `wye-native-host` (crate `crates/wye-native-host`), which reads length-prefixed JSON, takes the browser (its parent
process) as the source app and the click's held keys when the browser reports them
(Firefox), and calls `OpenLink` with `entry = "extension"`; the pipeline forces the picker
unless the bypass key is held (ADV-10, ADV-11). `wye_desktop::native_messaging` writes the
host manifest into every detected browser's directory (`NativeMessagingHosts/` for the
Chromium family, `native-messaging-hosts/` for the Firefox family), naming the host by
its `PATH` location so upgrades keep it valid. The service runs it at every start and
whenever a browser's top-level directory appears (`crates/wye-service/src/api/extension.rs`,
called by the file watcher: when it arms for an environment, after the bus name is claimed,
and on `Kind::ExtensionHosts`, debounced 2 s, for `~/.mozilla`, `~/.config/BraveSoftware`
and the like created in the home or configuration directory; on a blocking thread, writing
only the manifests that are missing or differ, leaving a symlink in a manifest's place
alone), so installing the extension is all a user does; `wye extension install|remove` and
`wye-native-host --install|--remove` run it by hand (`crates/wye-native-host/src/install.rs`,
used by both). `remove` sets `extension-host-removed` in `state.toml`, which keeps the
service from writing them again until `install` clears it. Flatpak and Snap browsers are
not supported.

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
absolute paths, and the extension zips
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
`WYE_LOGIN_MANAGED` on it to `on` or `off` (the service also accepts `1` as `on`), so the
service never writes the XDG autostart entry and removes one it wrote itself (a regular
file whose `Exec` is `…/wye service --activate`), and Settings shows "Launch at login" as
managed, with the option's value. Both prepend the Nix profile directories to
a `PATH` that ends in `/usr/local/bin:/usr/bin:/bin`, which bare desktop-entry `Exec`s are
resolved against. Native-messaging manifests are written at run time by the service (at
every start and whenever a browser's directory appears) or by `wye extension install`, not
by Nix; a manifest Nix or home-manager links in place is left alone.

## Not yet implemented

- Focused window and pointer on Sway and Hyprland (compositor IPC); held keys there come
  from layer shell only
- Global shortcuts by X11 key grabs; shortcuts need the `GlobalShortcuts` portal
- The browser extension for Flatpak and Snap browsers: their sandbox cannot start
  `wye-native-host` (profile discovery does cover the Flatpak and Snap builds listed in
  `crates/wye-desktop/src/family.rs`)

## Runtime files

| Path | Contents |
|------|----------|
| `$XDG_CONFIG_HOME/wye/config.toml` | User configuration. Hand-edited. |
| `$XDG_STATE_HOME/wye/state.toml` | Internal state: the browsers to restore (`previous-default-browser`, `previous-kdeglobals-browser`), `kept-default`, onboarding and UI state, the script errors already notified (`script-errors-notified`), and whether the user removed the extension host manifests (`extension-host-removed`). |
| `$XDG_STATE_HOME/wye/state.toml.lock` | Empty. The lock every `state.toml` writer holds; left in place. |
| `$XDG_STATE_HOME/wye/history.json` | Recent links (DLG-HIS), newest first, at most 100 entries (decision 10). |
| `$XDG_CONFIG_HOME/autostart/dev.soldunov.wye.desktop` | The XDG autostart entry (`wye service --activate`) while "Launch at login" is on, unless a Nix module owns login start. |
| `$XDG_CONFIG_HOME/mimeapps.list` | Default browser association, written by `wye default set`. |
| `$XDG_RUNTIME_DIR/wye/kwin/wye-query-<nonce>.js` | One `KWin` query script, only while its query runs. |
| `$XDG_CONFIG_HOME/wye/transform.js` | The global transform script (ADV-03). Hand-editable. |
| `$XDG_CONFIG_HOME/wye/rules/<rule-id>.js` | A rule's transform script (RUL-25). Hand-editable. |
