# D-Bus API

The Wye service (`wye service`) owns the session-bus name `dev.soldunov.wye` and exports
one object. Every frontend talks to it only through this API: the UI host `wye-ui`, the
Plasma tray applet, the StatusNotifierItem tray inside the service, the `wye` CLI, the
browser extension's native-messaging host, and later a GNOME Shell extension.

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
| `dev.soldunov.wye.Error.NotImplemented` | **Temporary.** The member exists but is not implemented yet. Removed once every member is. |

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
| `Tray` | `s` | JSON `wye_api::tray::TrayMenu`: the icon (`{"kind": "picker"}` or `{"kind": "theme", "name": …}`), `overlay` (`"warning"` or absent), `visible`, and `items` in order, each with `id`, `kind` (`action`, `header`, `radio`, `separator`, `submenu`), `label`, `icon`, `shortcut`, `enabled`, `checked`, `children`. The Plasma applet, the SNI tray and the `wye-ui` popup render it as-is (01-tray-menu.md). |
| `Status` | `s` | JSON `wye_api::status::Status`: `defaultBrowser` (`isDefault`, `current`, `previous`, `keptCurrent`), `config` (`path`, `writable`, `lossless`, `warnings`, `error`), `capabilities` (`heldKeys`, `pointer`, `sourceAppFallbacks`, `clipboardRead`, `clipboardWatch`, `globalShortcuts`, `lockDetection`: the mechanism in use or `null`; `layerShell`), `locked`, `uiState` (`onboardingDone`, `dismissedCallouts`, `lastPage`, `helpArrowSeen`). |
| `ConfigRevision` | `t` | Bumps on every applied change or reload of the configuration. |
| `HistoryRevision` | `t` | Bumps on every history change. |
| `InventoryRevision` | `t` | Bumps when installed apps or browser profiles change (DISC-02). |

### Methods

| Signature | Description |
|-----------|-------------|
| `OpenLink(s url, a{sv} context) → ()` | Route one link (IN-01, IN-05, IN-07). Context keys below. Returns once the decision is made, not after the picker closes. `InvalidArgs` when the link is rejected (PIPE-02) or a context value is malformed; for a rejected link the service also notifies. `Failed` when the target cannot be started; the service notifies with buttons offering up to three other available targets (LAUNCH-07). A link that needs the picker while the screen is locked is held and returns at once; it opens when the screen unlocks, and a newer held link replaces it (PKS-07). |
| `OpenClipboard(b alternative) → ()` | Route the URL on the clipboard (IN-02 to IN-04). `NotFound` when the clipboard holds no URL. |
| `ClipboardHasUrl() → b` | Whether the clipboard holds a URL; tray hosts call it before opening the menu (TRAY-10). |
| `TestLink(s url, a{sv} context) → s` | How the link would be routed, without opening it (IN-08, DLG-TST). Context also takes `skip-network` (`b`). JSON `wye_api::trace::LinkTrace`: `steps` (`kind`, `text`, `url`), `decision` (`open`, `picker`, `rejected`), `rejected`, `target`, `targetName`, `options`, `finalUrl`, `ruleIndex`. |
| `PreviewPicker() → ()` | Show the picker with a sample link; choosing opens nothing (IN-06, PKS-06). |
| `PickerChose(s request_id, s target, a{sv} options) → ()` | The picker's choice (PIPE-13). Options: `private` `b` (PICK-20), `background` `b` (PICK-21), `new-window` `b` (PICK-32), `activation-token` `s` (PICK-33). `NotFound` for a request that is no longer pending. |
| `PickerCancelled(s request_id) → ()` | The picker closed without a choice (PICK-23). |
| `PickerAction(s request_id, s action) → ()` | `copy-link`, or `create-rule`: opens the rule editor pre-filled; the link is not opened (PICK-31). |
| `GetConfig() → (s config, t revision)` | The whole configuration as JSON, same shape as `config.toml`. |
| `UpdateConfig(s merge_patch, t base_revision) → t` | Apply an RFC 7386 JSON merge patch (arrays replace) and save; returns the new revision (SET-06). `base_revision = 0` skips the check. Errors: `ReadOnly`, `Conflict`, `NotLossless`, `InvalidArgs` (lists every problem). |
| `SetPrimary(s target) → ()` | Set the primary browser (TRAY-11). |
| `GetDefaults(s section) → s` | Default values of one section as JSON, for "Reset to Defaults" (KEY-04): `picker.keys`, `shortcuts`, … |
| `GetTargets() → s` | JSON `wye_api::targets::TargetInventory`: every target with `target`, `kind`, `name`, `shortName` (TRAY-12), `icon`, `badge` (`{"image"}` or `{"initial", "color"}`, DISC-08), `browser`, `capabilities` (`private`, `newWindow`, `background`), `packaging`, `missing` (APP-10). TGT-02 to TGT-07, SHOWN-02, PICK-06. |
| `GetApps(b all) → s` | JSON `wye_api::apps::AppList` for the app chooser (DLG-APP): `apps` (browsers only unless `all`) with `id`, `name`, `genericName`, `keywords`, `icon`, `packaging`, `isBrowser`; and `recentSources`. |
| `GetServices() → s` | JSON `wye_api::services::ServiceList`: the web app catalogue with each service's installed app and current mapping (APP-03, APP-05). |
| `GetExpansionCatalogue() → s` | JSON `wye_api::expansion::ExpansionCatalogue`: `wrappers` and `shortLinks`, each with `enabled` (DLG-EXP). |
| `Rescan() → ()` | Rediscover apps and profiles now (TRAY-15, BRW-06). |
| `MakeDefault() → ()` | Make Wye the default browser (DEF-02). |
| `StopBeingDefault() → ()` | Restore the previous default browser (DEF-05). |
| `KeepCurrentDefault() → ()` | Keep another default browser and stop asking (ONB-10, ONB-11). |
| `ExportRules() → s` | Rules and their scripts as TOML (RUL-02). |
| `ImportRules(s text) → u` | Append rules from that TOML; returns how many (RUL-02). |
| `GetScript(s scope) → s` | A script's source. Scope `global` or `rule:<id>`. |
| `SetScript(s scope, s source) → ()` | Save a script. `ScriptSyntax` when it does not compile (SCR-07). |
| `RunScript(s source, s url, a{sv} context) → s` | Test-run a script (SCR-04). JSON `wye_api::scripts::ScriptRun`: `ok`, `url`, `changed` (`[[start, end]]`), `error`, `line`, `micros`, `logs`. |
| `GetHistory() → s` | JSON `wye_api::history::History`: `enabled` and `entries`, newest first, each with `id`, `time`, `originalUrl`, `finalUrl`, `entry`, `source`, `sourceName`, `target`, `targetName`, `reason`, `cleaned`, `expanded` (DLG-HIS). |
| `ClearHistory() → ()` | Forget every entry (DLG-HIS-01, ADV-09). |
| `DeleteHistoryEntry(t id) → ()` | Forget one entry (DLG-HIS-03). `NotFound` for an unknown ID. |
| `ReopenHistoryEntry(t id, s how) → ()` | Open an entry again: `picker` or `same-target` (DLG-HIS-03, TRAY-15). |
| `GetShortcuts() → s` | JSON `wye_api::shortcuts::Shortcuts`: `mechanism` (`portal`, `x11`, `none`) and `bindings` with `action`, `description`, `trigger`, `command` (KEY-40, KEY-41). |
| `SetShortcut(s action, s binding) → ()` | Bind `toggle-menu`, `clipboard-primary` or `clipboard-alternative`; an empty binding clears it. On the portal this is `BindShortcuts` with the preferred trigger. `Unavailable` without a mechanism. |
| `ConfigureShortcuts() → ()` | Open the mechanism's own dialog (KEY-40 "Change…"). |
| `UpdateUiState(s merge_patch) → ()` | Merge patch of `uiState` in `Status`: dismissed callouts (BLK-09), last page (SET-08), help arrow (RUL-19), onboarding done (ONB-06). |
| `ShowWindow(s window, s argument) → ()` | Open a window in the UI host: `settings` (argument: page), `first-run`, `history`, `test-rules`, `about`, `script-editor` (argument: scope), `rule-editor` (argument: JSON prefill). `InvalidArgs` for an unknown window, `Unavailable` when the UI host cannot be started or does not answer within 10 s. |
| `ToggleMenu() → ()` | Open or close the tray-menu popup (TRAY-08). |
| `RegisterTray(s kind) → ()` | A tray host announces itself (`plasma-applet`, later `gnome-extension`). The service hides its own StatusNotifierItem while the caller's connection lives. |
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

### Signals

| Signature | Description |
|-----------|-------------|
| `MenuRequested()` | The toggle-menu shortcut fired. A tray host that can open its own menu may do so; otherwise the service shows the `wye-ui` popup. |
| `ScriptFileChanged(s scope)` | A script file changed on disk (SCR-08). |

## Interface `dev.soldunov.wye.KWin1` (internal)

Called back by the one-shot KWin script the service loads through `org.kde.kwin.Scripting`
to read the pointer and the active window (PICK-02, source-app step 4). Not part of the
public contract.

| Signature | Description |
|-----------|-------------|
| `Report(s nonce, i x, i y, s output, u pid, s desktop_file, s resource_class) → ()` | One answer to the query started with `nonce`. |

## UI host (internal)

`wye-ui` owns `dev.soldunov.wye.Ui` and exports `/dev/soldunov/wye/Ui`. It is activatable
(`share/dbus-1/services/dev.soldunov.wye.Ui.service`); the service starts it when a link
may end on the picker and it stays resident. A second `wye-ui` forwards its arguments
with `Windows1.ShowWindow` and exits 0. These interfaces are not part of the public
contract.

### Interface `dev.soldunov.wye.PickerHost1`

A GNOME Shell extension may implement it later under its own name; the service then
prefers it.

| Signature | Description |
|-----------|-------------|
| `ShowPicker(s request_id, s request) → ()` | Show the picker. JSON `wye_api::picker::PickerRequest`: `url` (`full`, `host`, `rest`), `source` (`name`, `icon`), `tiles` (`target`, `name`, `icon`, `badge`, `hotkey`, `capabilities`), `overflow` groups (TGT-02), `settings` (`iconSize`, `showNames`, `showUrl`, `showBadge`), `keys` (`actions`, `modifierActions`), `held`, `placement` (`output`, `x`, `y`; centred when absent), `preview`. A new request replaces the one shown, in the same window (PICK-27). The UI answers with `PickerChose`, `PickerCancelled` or `PickerAction`. |
| `ClosePicker(s request_id) → ()` | Close the picker: superseded, or the screen locked. |
| `ShowMenu(s menu) → ()` | Toggle the tray-menu popup with a JSON `TrayMenu` (TRAY-08). |

### Interface `dev.soldunov.wye.Windows1`

| Signature | Description |
|-----------|-------------|
| `ShowWindow(s window, s argument) → ()` | Open or raise a window (same names as `dev.soldunov.wye1.ShowWindow`). |
| `Quit() → ()` | Quit the UI host. |

### Interface `dev.soldunov.wye.SessionHelper1` (reserved)

Reserved for the GNOME Shell extension; not implemented. `QueryPointer() → (i x, i y, s
output)`, `QueryModifiers() → as`, `FocusedApp() → s`, `ReadClipboard() → s`,
`WatchClipboard(b)` and the signal `ClipboardChanged(s)`.
