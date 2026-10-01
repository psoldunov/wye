# D-Bus API

The Wye service (`wye service`) owns the session-bus name `dev.soldunov.wye` and exports
one object. Every frontend talks to it only through this API: the UI host `wye-ui`, the
StatusNotifierItem tray inside the service, the `wye` CLI, the browser extension's
native-messaging host, and later a GNOME Shell extension.

- Bus name: `dev.soldunov.wye`
- Object path: `/dev/soldunov/wye`
- Interfaces: `dev.soldunov.wye1` (public), `org.freedesktop.Application` (desktop
  activation), `dev.soldunov.wye.KWin1` (internal)

The name is D-Bus activatable (`share/dbus-1/services/dev.soldunov.wye.service`, with
`SystemdService=wye.service`), so any call starts the service when it is not running.
Only one service runs per session: it requests the name with `DoNotQueue` and exits with
status 75 when the name is taken; `wye.service` lists 75 in `RestartPreventExitStatus=`.
The service also stops when it loses the name or the bus connection; the next call
starts a new one. `wye service --activate` only calls `StartServiceByName` and exits (the
XDG autostart entry uses it, GEN-01).

The names, error names, JSON payload types and zbus proxies are in the `wye-api` crate
(`crates/wye-api`). Code should use those rather than repeating strings.

## Conventions

- Structured data travels as JSON in an `s` value. Keys are camelCase. The schema of
  each payload is the serde type named below (`wye_api::…`). Fields that are absent
  take their defaults, so a client may ignore keys it does not know.
- A **target** in JSON has the configuration's shape: `{"picker": true}`,
  `{"default": true}`, `{"app": "firefox.desktop"}`, `{"private": "firefox.desktop"}`,
  `{"profile": {"app": "google-chrome.desktop", "id": "Profile 1"}}`,
  `{"custom": "…"}` (`wye_api::TargetSpec`). A `target` string argument is that JSON.
- Every property emits `org.freedesktop.DBus.Properties.PropertiesChanged` with its new
  value when it changes.
- An `a{sv}` argument may omit any key.

## Errors

| Name | Meaning |
|------|---------|
| `org.freedesktop.DBus.Error.InvalidArgs` | The caller sent something the service refuses. The message lists every problem. |
| `org.freedesktop.DBus.Error.Failed` | The service could not do it (a failed write, an internal error). |
| `dev.soldunov.wye.Error.ReadOnly` | The configuration file cannot be written, for example a read-only file from home-manager. |
| `dev.soldunov.wye.Error.Conflict` | `base_revision` is stale: the configuration changed since the caller read it. Reload and re-apply. |
| `dev.soldunov.wye.Error.NotLossless` | Saving would drop values the configuration file contains. |
| `dev.soldunov.wye.Error.NotFound` | The thing asked for does not exist: a history entry, a URL on the clipboard, a pending picker request. |
| `dev.soldunov.wye.Error.Unavailable` | This session cannot do it (no portal, no clipboard protocol). |
| `dev.soldunov.wye.Error.ScriptSyntax` | A script does not compile. The message is `line:column: text` (SCR-07). |
| `dev.soldunov.wye.Error.NotImplemented` | Reserved: returned by a service older than the caller for a member it does not have. The current service implements every member. |

## Interface `org.freedesktop.Application`

Launchers call this for `DBusActivatable=true` (DEF-04). `platform_data` may carry
`activation-token` (Wayland) and `desktop-startup-id` (X11); the service hands them to
the app it launches (LAUNCH-03).

| Signature | Description |
|-----------|-------------|
| `Activate(a{sv} platform_data) → ()` | Started without a link: the first-run window when onboarding is not done, else Settings when the tray icon is hidden or no tray exists (TRAY-05), else nothing. Until onboarding state and the tray exist, Settings always opens. A UI host that cannot be reached is logged; the call still succeeds. |
| `Open(as uris, a{sv} platform_data) → ()` | Each URI enters the pipeline as a handler link (IN-01). Source-app detection starts at the caller's PID (`GetConnectionCredentials`), skipping openers such as `kde-open` and `xdg-open` up the parent chain; when the caller is `xdg-desktop-portal`, the focused window is used instead. Every URI is tried; the first error is returned. |
| `ActivateAction(s action_name, av parameter, a{sv} platform_data) → ()` | Desktop actions: `settings`, `clipboard`, `clipboard-alternative`, `menu`, `setup`, `history`, `test-rules`, `about`, `quit` (`wye_api::actions::ApplicationAction`). |

## Interface `dev.soldunov.wye1`

### Properties

All read-only.

| Name | Type | Contents |
|------|------|----------|
| `Version` | `s` | Package version. |
| `Tray` | `s` | JSON `wye_api::tray::TrayMenu`: the icon (`{"kind": "app"}` for `dev.soldunov.wye-symbolic`, `{"kind": "picker"}` for the picker glyph `dev.soldunov.wye-picker-symbolic`, or `{"kind": "theme", "name": …}` for the primary browser's icon; TRAY-02, GEN-02), `overlay` (`"warning"` or absent), `visible`, and `items` in order, each with `id`, `kind` (`action`, `header`, `radio`, `separator`, `submenu`), `label`, `icon`, `shortcut` (as shown: `P`, `1`, `Ctrl+,`), `enabled`, `checked`, `children`. Changes with the configuration, the installed apps, the default-browser registration and the history. The SNI tray, the `wye-ui` popup and any external tray host render it as-is (01-tray-menu.md). |
| `Status` | `s` | JSON `wye_api::status::Status`: `defaultBrowser` (`isDefault`, `current`, `previous`, `keptCurrent`), `config` (`path`, `writable`, `lossless`, `warnings`, `error`), `capabilities` (`heldKeys`, `pointer`, `sourceAppFallbacks`, `clipboardRead`, `clipboardWatch`, `clipboardWrite`, `globalShortcuts`, `lockDetection`: the mechanism in use or `null`; `layerShell`: `true` while the held-key probe runs as a `zwlr_layer_shell_v1` surface, which proves the compositor offers layer shell; `false` otherwise, including when `advanced.held-keys` is `off`; `heldKeys` is `null` while it is `off`), `locked`, `loginManaged` (`true` when `WYE_LOGIN_MANAGED` is `on` or `off`, `1` counting as `on`: login start is managed outside Wye; the autostart entry is never written, and a stale one Wye wrote is removed), `loginManagedOn` (`true` when that outside configuration starts Wye at login), `uiState` (`onboardingDone`, `dismissedCallouts`, `lastPage`, `helpArrowSeen`). Also announced when the session integrations change (the probes detected after start). |
| `ConfigRevision` | `t` | Bumps on every applied change or reload of the configuration. Starts at 1 once the file is read; 0 is never reported (it means "skip the check" in `UpdateConfig`). |
| `HistoryRevision` | `t` | Bumps on every history change. Starts at 1. |
| `InventoryRevision` | `t` | Bumps when installed apps or browser profiles change (DISC-02). Starts at 1 after the first scan. |

### Methods

| Signature | Description |
|-----------|-------------|
| `OpenLink(s url, a{sv} context) → ()` | Route one link (IN-01, IN-05, IN-07). Context keys below. Returns once the link is decided, not after the picker closes. For a Picker decision the request is made pending and `ShowPicker` goes to the UI host in the background; the round trip (`PickerChose`, `PickerCancelled` or `PickerAction`) continues after the call has returned, a launch failure after the choice is reported by notification, and when the UI cannot show the picker the service closes it there and opens the link through the stand-in. `InvalidArgs` when the link is rejected (PIPE-02) or a context value is malformed; for a rejected link the service also notifies. `Failed` when the target cannot be started; the service notifies with buttons offering up to three other available targets (LAUNCH-07). A link that needs the picker while the screen is locked is held and returns at once; it opens when the screen unlocks, and a newer held link replaces it (PKS-07). |
| `OpenClipboard(b alternative) → ()` | Route the URL on the clipboard (IN-02 to IN-04). `NotFound` when the clipboard holds no URL. |
| `ClipboardHasUrl() → b` | Whether the clipboard holds a URL; tray hosts call it before opening the menu (TRAY-10). |
| `TestLink(s url, a{sv} context) → s` | How the link would be routed, without opening it (IN-08, DLG-TST). Context also takes `skip-network` (`b`). JSON `wye_api::trace::LinkTrace`: `steps` (`kind`, `text`, `url`), `decision` (`open`, `picker`, `rejected`), `rejected`, `target`, `targetName`, `options`, `finalUrl`, `ruleIndex`. |
| `PreviewPicker() → ()` | Show the picker with a sample link; choosing opens nothing (IN-06, PKS-06). `Unavailable` when the UI host cannot be reached. |
| `PickerChose(s request_id, s target, a{sv} options) → ()` | The picker's choice (PIPE-13). `target` is the chosen target in configuration JSON (`{"app": …}`, `{"private": …}`, `{"profile": {…}}`, `{"custom": …}`). Options: `private` `b` (turns `{"app"}` into its private target, KEY-13), `background` `b` (PICK-32), `new-window` `b` (PICK-33), `activation-token` `s` (from the picker's own input event, PICK-29; dropped for a background launch). Accepts any target the request showed (tiles and Open In), in any form the picker can produce: plain, or a browser's private target (`{"private": …}`) when the tile supports private windows (KEY-13). Any other target, or an option of the wrong type, is `InvalidArgs`; the request stays pending. `NotFound` when `request_id` is not the pending request (answered, or superseded, PICK-27). A preview request opens nothing. |
| `PickerCancelled(s request_id) → ()` | The picker closed without a choice; the link is dropped (PICK-23). `NotFound` for a request that is not pending. |
| `PickerAction(s request_id, s action) → ()` | Ends the request without opening the link. `copy-link` writes the link to the clipboard (KEY-22); `create-rule` opens the rule editor through `Windows1.ShowWindow("rule-editor", prefill)` with the prefill `{"domain": <host>, "sourceApp": <desktop ID or null>}` (PICK-31). `NotFound` for a request that is not pending. |
| `GetConfig() → (s config, t revision)` | The whole configuration as JSON, same shape as `config.toml`. |
| `UpdateConfig(s merge_patch, t base_revision) → t` | Apply an RFC 7386 JSON merge patch (arrays replace) and save; returns the new revision (SET-06). `base_revision = 0` skips the check. The file is re-read under the write lock; when it changed on disk since the last read, a non-zero `base_revision` gets `Conflict` (reload and retry), while 0 patches the file as it is now. Errors: `ReadOnly`, `Conflict`, `NotLossless`, `InvalidArgs` (lists every problem). |
| `SetPrimary(s target) → ()` | Set the primary browser (TRAY-11). |
| `GetDefaults(s section) → s` | Default values of one section as JSON, for "Reset to Defaults" (KEY-04): `picker.keys`, `shortcuts`, … |
| `GetTargets() → s` | JSON `wye_api::targets::TargetInventory`: every target with `target`, `kind`, `name`, `shortName` (TRAY-12), `icon`, `badge` (`{"image"}` or `{"initial", "color"}`, DISC-08), `browser`, `capabilities` (`private`, `newWindow`, `background`), `packaging`, `missing` (APP-10). TGT-02 to TGT-07, SHOWN-02, PICK-06. |
| `GetApps(b all) → s` | JSON `wye_api::apps::AppList` for the app chooser (DLG-APP): `apps` (browsers only unless `all`) with `id`, `name`, `genericName`, `keywords`, `icon`, `packaging`, `isBrowser`; and `recentSources`. |
| `GetServices() → s` | JSON `wye_api::services::ServiceList`: the web app catalogue with each service's installed app and current mapping (APP-03, APP-05). |
| `GetExpansionCatalogue() → s` | JSON `wye_api::expansion::ExpansionCatalogue`: `wrappers` and `shortLinks`, each with `enabled` (DLG-EXP). |
| `Rescan() → ()` | Rediscover apps and profiles now (TRAY-15, BRW-06). |
| `MakeDefault() → ()` | Make Wye the default browser (DEF-02): `mimeapps.list`, and Plasma's `kdeglobals` on KDE. `ReadOnly` when `mimeapps.list` is managed elsewhere (a symlink or read-only file, for example home-manager's `xdg.mimeApps`); `Failed` when Wye's desktop entry is not installed. |
| `StopBeingDefault() → ()` | Restore the previous default browser (DEF-05), and Plasma's previous value. Nothing changes when Wye is not the default. `NotFound` when no previous browser is remembered or it is no longer installed; `ReadOnly` as for `MakeDefault`. |
| `KeepCurrentDefault() → ()` | Keep another default browser and stop asking (ONB-10, ONB-11). |
| `ExportRules() → s` | Rules and their scripts as TOML (RUL-02). |
| `ImportRules(s text) → u` | Append rules from that TOML; returns how many (RUL-02). |
| `GetScript(s scope) → s` | A script's source. Scope `global` or `rule:<id>` (letters, digits, `-`, `_`). A missing or empty file answers the template (SCR-03). The files are `transform.js` and `rules/<id>.js` next to `config.toml`. |
| `SetScript(s scope, s source) → ()` | Save a script atomically. `ScriptSyntax` when it does not compile (SCR-07); runtime errors do not block saving. |
| `ScriptExists(s scope) → b` | Whether the script's file exists and holds more than whitespace. Settings open the editor when a transform is turned on for a script that does not (SCR-09). |
| `RunScript(s source, s url, a{sv} context) → s` | Test-run a script (SCR-04). JSON `wye_api::scripts::ScriptRun`: `ok`, `url`, `changed` (`[[start, end]]`), `error`, `line`, `micros`, `logs`. A failing script is an answer with `ok: false`; `url` is absent when the script kept the link. Context also takes `rule` (`s`), the name `context.rule` shows. `InvalidArgs` when `url` is not a link. |
| `GetHistory() → s` | JSON `wye_api::history::History`: `enabled` and `entries`, newest first, each with `id`, `time`, `originalUrl`, `finalUrl`, `entry`, `source`, `sourceName`, `target`, `targetName`, `reason`, `cleaned`, `expanded` (DLG-HIS). |
| `ClearHistory() → ()` | Forget every entry (DLG-HIS-01, ADV-09). |
| `DeleteHistoryEntry(t id) → ()` | Forget one entry (DLG-HIS-03). `NotFound` for an unknown ID. |
| `ReopenHistoryEntry(t id, s how) → ()` | Open an entry again: `picker` or `same-target` (DLG-HIS-03, TRAY-15). |
| `GetShortcuts() → s` | JSON `wye_api::shortcuts::Shortcuts`: `mechanism` (`portal`, `x11`, `none`) and `bindings`, one per action (`toggle-menu`, `clipboard-primary`, `clipboard-alternative`) with `action`, `description`, `trigger` (as the mechanism reports it, for example the portal's trigger description; absent when unbound) and `command` (`wye menu`, `wye clipboard`, `wye clipboard --alternative`, to bind by hand, KEY-41) (KEY-40). |
| `SetShortcut(s action, s binding) → ()` | Save `binding` (KEY-03 form, any spelling: `Ctrl+Alt+w`) as `[shortcuts]` `toggle-menu`, `clipboard-primary` or `clipboard-alternative` and bind it; an empty binding clears it (ADV-05 to ADV-07). On the portal this binds every action again on a new session with the saved triggers as `preferred_trigger`; a trigger the user chose in the desktop's settings stays, so read `GetShortcuts` for what is bound. `InvalidArgs` for an unknown action or binding, `Unavailable` without a mechanism (nothing is saved), and `UpdateConfig`'s errors. Editing `[shortcuts]` by hand or through `UpdateConfig` takes effect when the service next starts. |
| `ConfigureShortcuts() → ()` | Open the mechanism's own dialog (KEY-40 "Change…"): the portal's `ConfigureShortcuts` (portal version 2). `Unavailable` without a mechanism or when the portal has none. |
| `UpdateUiState(s merge_patch) → ()` | Merge patch of `uiState` in `Status`: dismissed callouts (BLK-09), last page (SET-08), help arrow (RUL-19), onboarding done (ONB-06). |
| `ShowWindow(s window, s argument) → ()` | Open a window in the UI host: `settings` (argument: page), `first-run`, `history`, `test-rules`, `about`, `script-editor` (argument: scope), `rule-editor` (argument: JSON prefill). `InvalidArgs` for an unknown window, `Unavailable` when the UI host cannot be started or does not answer within 10 s. |
| `ToggleMenu() → ()` | Open or close the tray-menu popup (TRAY-08): emits `MenuRequested`, then calls `PickerHost1.ShowMenu` with the `Tray` model and the pointer. `Unavailable` when the UI host cannot be reached within 10 s. The `toggle-menu` shortcut and `wye menu` call the same. |
| `RegisterTray(s kind) → ()` | Reserved for external tray hosts: Wye's own tray is its StatusNotifierItem on every desktop, KDE Plasma included, and Wye ships no host that calls this. A host announces itself (`plasma-applet`, the applet earlier versions shipped, still accepted; later `gnome-extension`). The service hides its own StatusNotifierItem until the caller has called `UnregisterTray` as often as `RegisterTray`, or its connection closes (instances of one host may share a connection, as applets share plasmashell's). The item shows as soon as the service starts; there is no wait for a host. `InvalidArgs` for another kind. |
| `UnregisterTray() → ()` | Ends one of the caller's registrations (a host calls it when an instance is removed or disabled, since a shared connection outlives it). When none is left and no other host is registered, the StatusNotifierItem comes back at once. Unregistering a caller that never registered changes nothing. |
| `ActivateTrayItem(s id) → ()` | Carry out the `Tray` item `id`, for every tray host: `make-default` (`MakeDefault`), `open-clipboard` (`OpenClipboard(false)`), `primary:picker` / `primary:<n>` (the item's target becomes `browsers.primary`, TRAY-11), `settings`, `history`, `test-rules`, `set-up`, `about` (`ShowWindow`), `recent:<id>` (`ReopenHistoryEntry(id, "picker")`), `rescan`, `help` (opens the project page as a link), `quit`. `InvalidArgs` for a header, separator, submenu or unknown ID; otherwise the error of the call it makes. |
| `GetTroubleshooting() → s` | Plain-text troubleshooting report for the About window (DLG-ABT-02). |
| `Quit() → ()` | Stop the service (TRAY-17). The next link starts it again through D-Bus activation. |

### `OpenLink`, `TestLink` and `RunScript` context

Keys in `wye_api::context`.

| Key | Type | Meaning |
|-----|------|---------|
| `source-desktop-id` | `s` | Desktop ID of the app the link came from. |
| `source-executable` | `s` | The source app's executable, when there is no desktop ID. |
| `source-pid` | `u` | Detect the source app from this process up, with the installed apps (all steps of source-app detection). Used only when `source-desktop-id` is absent; `source-executable`, if given, is the fallback when detection finds nothing. `wye open` sends its parent. |
| `activation-token` | `s` | `XDG_ACTIVATION_TOKEN` for the launched app (LAUNCH-03). |
| `startup-id` | `s` | `DESKTOP_STARTUP_ID` for the launched app (LAUNCH-03). |
| `held` | `as` | Modifiers held when the link was opened: `Shift`, `Ctrl`, `Alt`, `Super`. |
| `held-known` | `b` | Whether `held` is known. When false or absent, the service probes. |
| `entry` | `s` | `handler` (default), `clipboard`, `extension`, `cli`. |
| `force` | `s` | `none` (default), `picker` (PIPE-11), `alternative` (PIPE-06). |
| `skip-network` | `b` | `TestLink` only: do not contact short-link services. |
| `rule` | `s` | `RunScript` only: the rule name the script sees as `context.rule`. |

### Signals

| Signature | Description |
|-----------|-------------|
| `MenuRequested()` | The toggle-menu shortcut fired. A tray host that can open its own menu may do so; otherwise the service shows the `wye-ui` popup. |
| `ScriptFileChanged(s scope)` | A script file changed on disk other than through `SetScript` (SCR-08). Watched once the service starts its tasks or a client called `GetScript`/`SetScript`. |

## Interface `dev.soldunov.wye.KWin1` (internal)

Called back by the one-shot KWin script the service loads through `org.kde.kwin.Scripting`
to read the pointer and the active window (PICK-02, source-app step 4). Not part of the
public contract.

| Signature | Description |
|-----------|-------------|
| `Report(s nonce, i x, i y, s output, i pid, s desktop_file, s resource_class) → ()` | One answer to the query started with `nonce`. `x` and `y` are relative to `output`; `pid` is `i` because `KWin`'s `callDBus` sends every JavaScript number as `int32` (0 when there is no active window). |

## UI host (internal)

`wye-ui` owns `dev.soldunov.wye.Ui` and exports `/dev/soldunov/wye/Ui`. It is activatable
(`share/dbus-1/services/dev.soldunov.wye.Ui.service`, with
`SystemdService=wye-ui.service`, a `Type=dbus` user unit that nothing starts at login);
the service starts it when a link may end on the picker and it stays resident. A second `wye-ui` forwards its arguments
with `Windows1.ShowWindow` and exits 0. These interfaces are not part of the public
contract.

### Interface `dev.soldunov.wye.PickerHost1`

A GNOME Shell extension may implement it later under its own name; the service then
prefers it.

| Signature | Description |
|-----------|-------------|
| `ShowPicker(s request_id, s request) → ()` | Show the picker. JSON `wye_api::picker::PickerRequest`: `url` (`full`, `host`, `rest`), `source` (`name`, `icon`), `tiles` (`target` in configuration JSON, `name`, `icon`, `badge`, `hotkey` as a canonical XKB key name such as `"1"` or `"f"`, `capabilities`), `overflow` groups for Open In (TGT-02, PICK-28), `settings` (`iconSize`, `showNames`, `showUrl`, `showBadge`), `keys` (`actions` under `open`, `cancel`, `next`, `previous`, `first`, `last`, `copy-link`, `more`, `create-rule`; `modifierActions` under `private`, `background`, `new-window`), `held`, `placement` (`output`, `x`, `y` in the output's logical coordinates; centred when absent), `preview`. A new request replaces the one shown, in the same window (PICK-27). The UI answers with `PickerChose`, `PickerCancelled` or `PickerAction`. The service calls it with a 10 s deadline (bus activation included); when the UI host cannot be reached the link opens through the stand-in and a notification says so. |
| `ClosePicker(s request_id) → ()` | Close the picker without an answer: the screen locked (the link then waits for the unlock, PKS-07). A request that is not the one shown is ignored. |
| `ShowMenu(s menu) → ()` | Toggle the tray-menu popup (TRAY-08): show it, or close it when it is shown. JSON `wye_api::tray::TrayMenu` (the `Tray` property) plus `placement` (`output`, `x`, `y` as in `ShowPicker`; centred when absent). The popup's corner is at the pointer; choosing an item calls `ActivateTrayItem` with its ID and closes the popup; `P` and `1`–`9` choose the item with that shortcut (KEY-51). |

### Interface `dev.soldunov.wye.Windows1`

| Signature | Description |
|-----------|-------------|
| `ShowWindow(s window, s argument) → ()` | Open or raise a window (same names as `dev.soldunov.wye1.ShowWindow`). |
| `Quit() → ()` | Quit the UI host. |

### Interface `dev.soldunov.wye.SessionHelper1` (reserved)

Reserved for the GNOME Shell extension; not implemented. `QueryPointer() → (i x, i y, s
output)`, `QueryModifiers() → as`, `FocusedApp() → s`, `ReadClipboard() → s`,
`WatchClipboard(b)` and the signal `ClipboardChanged(s)`.

## Browser extension: native messaging (BEXT-04, BEXT-05)

Not D-Bus, but a client of `OpenLink`. The browser extension (`frontends/extension/`)
talks to the native-messaging host `dev.soldunov.wye`, the program `wye-native-host`,
over stdin and stdout: each message is a 32-bit length in native byte order followed by
UTF-8 JSON. A link is `{"url", "modifiers", "pageOrLink"}` (`modifiers`: the browser's
names of the keys held during the click, or `null` when it does not report them);
`{"ping": true}` only checks that the host is installed. The host answers `{"ok": true}`
or `{"error": "…"}`. For each link it calls `OpenLink` with `entry` = `extension`, `held`
and `held-known` = `true` when the browser reported the keys, and the browser (the
host's parent process) as the source: `source-desktop-id` when its desktop ID is known,
else `source-executable` and `source-pid`. The call starts the service through D-Bus
activation. `wye-native-host --install` writes the host manifest for every detected
browser (`wye_desktop::native_messaging`), `--remove` deletes them; `wye extension
install|remove` run the same code.
