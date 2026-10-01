# wye-gtk

Wye's GTK 4 / libadwaita host: one resident process per session that owns
`dev.soldunov.wye.Gtk`, serves `dev.soldunov.wye.Windows1` and
`dev.soldunov.wye.PickerHost1` at `/dev/soldunov/wye/Gtk` for the service
(`docs/dbus-api.md`, "UI host"), and shows Settings, the other windows, the picker and the
tray-menu popup with libadwaita. It makes no routing decisions; it asks the service over
D-Bus. With the GNOME frontend (`advanced.frontend`, ADV-12) the service sends `ShowWindow`
here, and the picker and the tray-menu popup too unless the GNOME Shell extension
(`frontends/gnome-shell`) owns `dev.soldunov.wye.Gnome`
(`crates/wye-service/src/api/picker/host.rs`): so in GNOME Shell the Shell draws them, and
on window managers and wlroots compositors (Sway, Hyprland, niri, river, Wayfire) this host
does. The KDE counterpart is `wye-ui` (`crates/wye-ui`), whose architecture this crate
mirrors.

## Layout

| Path | What |
|------|------|
| `src/main.rs` | Start-up: take the bus name (or forward to the running host), then GTK. |
| `src/cli.rs` | `wye-gtk [WINDOW [ARG]]`, `--self-test [SURFACE] [--snapshots DIR] [--scheme light\|dark] [--scale N]`. |
| `src/host.rs` | The D-Bus interfaces. They validate and turn each call into a `Command`. |
| `../wye-ui/src/route.rs` | `UiCommand` to surface, action, key and argument (a `Route`): wye-ui's routing table, compiled here from source (`#[path]` in `src/main.rs`). |
| `src/dispatch.rs` | Hands deliveries from the D-Bus thread to the GTK main thread, keeping early ones. |
| `src/service.rs` | `with_bus` (the runtime handle and the connection, on the main thread while the application runs); `request`, `watch`, `follow`: the one way code talks to the service, each awaited on the GTK main context. |
| `src/app.rs` | `adw::Application` (ID `dev.soldunov.wye`, `NON_UNIQUE`), the `GResource`, the surface registry (`Host`, `Presenter`). |
| `src/surface.rs` | The surfaces and which window names each takes. |
| `src/settings/` | The Settings window: GTK-free model mirrored from `wye-ui` (`snapshot`, `patch`, `save`, `sync`, `menu`, `help`, `fixture`), the `SettingsStore`, the window frame and `pages/` (one module per page; `pages/placeholder.rs` is only the fallback for a page name no module handles). |
| `src/settings/sheets/` | Sheets that are not one page's own form: the app chooser (DLG-APP) and the shown browsers sheet (SHOWN-01 to SHOWN-08), modal `AdwDialog`s over the window. |
| `src/rules/` | The parts of the Rules page: rule list, rule editor, rule tester sheet and trace rows, source-app chooser, rules help, import and export (08-rules.md, 17-dialogs.md). The page is `src/settings/pages/rules.rs`. |
| `src/about/` | About (`AdwAboutDialog`, DLG-ABT). |
| `src/history/` | The History window (DLG-HIS): wye-ui's search, row view, sync and fixture logic shared from source, the day groups, the rows and the window; it follows `HistoryRevision` and `InventoryRevision`. |
| `src/onboarding/` | First run (ONB-01 to ONB-06): an `AdwNavigationView` in an `AdwDialog`, with the browsers, integration and extension steps, the footer and the choices model. |
| `src/script_editor/` | The transform script editor (SCR-01 to SCR-10): wye-ui's Qt-free `document`, `editing`, `opening`, `readiness` and `result` shared from source; the `frame`, the GtkSourceView `code` area and its `indenter`, the `test_group`, the `reference` sidebar, the `unsaved` question, the `controller` and its service `calls`. Fixture keys as wye-ui's: `fixture` (`source`, `testUrl`, `run`, `externalChange`, `apps`) and the window's `edit`, `reference`, `confirmClose`. |
| `src/picker/` | The picker: wye-ui's `view` and `state` shared from source, GDK `keys`, the `panel` and `tile`s, the `menus`, the `answer`s to the service. |
| `src/tray_menu/` | The tray-menu popup: wye-ui's `model` shared from source, the rows as a `GMenu` (`menu`). |
| `src/overlay.rs` | The window the picker and the popup draw in: a layer surface or a plain window. |
| `src/widgets/` | The building blocks BLK-01 to BLK-18; inventory in `src/widgets/mod.rs`. |
| `src/links.rs` | Opening a link through Wye's pipeline (BLK-17). |
| `src/error_text.rs` | The sentence a banner shows for a failed call. |
| `src/selftest/` | `--self-test`: children on a private Xvfb, the GLib log check, snapshots. |
| `data/` | The `GResource`: `style.css` (loaded by `AdwApplication`) and `icons/` (Wye's own symbolic icons; the app icon is added from `data/icons/` at the repository root). |
| `fixtures/<surface>.json` | What `--self-test` feeds each surface. |

## Conventions for surface builders

**Run everything in the dev shell.** It has GTK 4.22, libadwaita 1.9, GtkSourceView 5,
`glib-compile-resources`, `dbus-daemon`, `Xvfb` and GNOME's fonts for the self-test.

```sh
nix develop -c cargo build -p wye-gtk
nix develop -c cargo clippy -p wye-gtk --all-targets --locked -- --deny warnings
nix develop -c cargo test -p wye-gtk --locked
nix develop -c target/debug/wye-gtk --self-test            # every surface, on a private Xvfb
nix develop -c target/debug/wye-gtk --self-test settings   # one surface
nix develop -c target/debug/wye-gtk settings rules         # a real window (needs the session bus)
```

**Build UI in Rust.** Builder APIs and small constructors, no `.ui` or Blueprint files. Use
libadwaita's widgets and style classes (`boxed-list`, `suggested-action`, `dimmed`,
`heading`, `card`) before writing CSS; Wye's own CSS goes in `data/style.css`, uses
libadwaita's colour variables only (`--accent-bg-color`, `--card-bg-color`, …) and
classes prefixed `wye-`. No deprecated API (`AdwPreferencesWindow`, `AdwAboutWindow`,
`GtkShortcutsWindow`). Copy comes from the spec; help popovers from
`docs/spec/19-help-texts.md` through `SettingsStore::help_text`.

**Never set `css_classes` on a builder whose widget has classes of its own.** It replaces
them: a window loses `background`, a popover its background, a button `image-button`. Call
`add_css_class` after `build()`, or put `.css_classes([...])` first in a button's chain.

**Surfaces.** A surface is one window or dialog (`src/surface.rs`), shown through a type
implementing `app::Presenter`:

```rust
impl Presenter for MySurface {
    fn present(&self, key: &str, argument: &str) { /* build once, then show and raise */ }
}
```

`present` must show, raise and focus the instance it already has (SET-04): build the window
on first use, keep it (`hide_on_close(true)`), and call `present()` again. Escape and Ctrl+W
close a window (SET-07); closing never quits the host, `Windows1.Quit` does. To add one: a
variant in `Surface` (with `for_window`), the presenter in its own module, its line in
`Host::create` (`src/app.rs`), and `fixtures/<surface>.json`. A case argument that is a JSON
object may carry `"scheme": "light" | "dark"`; under `--self-test-child` `Host::route` applies
it for every surface (the resident host ignores it).

**The picker and the tray menu** are surfaces like the windows, reached through
`PickerHost1` (`src/host.rs`): `ShowPicker` and `ClosePicker` route to `Surface::Picker`
with the actions `show` and `close`, `ShowMenu` to `Surface::TrayMenu` with `toggle`
(wye-ui's `route.rs`); `Presenter::act` hands a presenter the action. `Windows1.Quit` asks
every presenter first (`Presenter::before_quit`): the picker answers a shown request with
`PickerCancelled`, the script editor asks about unsaved changes (SCR-10). `ShowPicker` reads the
request as the picker will and refuses one it cannot show, so the service opens the link
through its stand-in (PICK-23).

- *Logic.* Selection, hotkeys, held modifiers, the tile menu and Open In are wye-ui's
  `src/picker/{view,state}.rs` and `qml.rs`, compiled here from the same files; only the
  key events differ (`src/picker/keys.rs` reads GDK's, XKB-named, with the hardware key code
  for layout-independent hotkeys, KEY-11). The tray menu's rows and accelerators are
  wye-ui's `src/tray_menu/mod.rs`.
- *Look.* The panel follows the GNOME Shell extension: 14 px padding, 16 px corners, the
  popover colours of the light or dark style slightly translucent, a hairline border and a
  shadow; tiles one width with the hotkey row, the icon with the kit's profile badge, the
  name cut at the end; the selected tile tinted and outlined with the accent; the round
  "⋯"; the URL line "from App **host**/path"; the hint in the accent (PICK-01 to PICK-14).
  The style manager follows the desktop's dark style and accent colour (PICK-12). The tray
  menu is a native `GtkPopoverMenu` with nested submenus, radio items for the primary
  browser and the shortcuts on the right (TRAY-08, TRAY-13, TRAY-15).
- *Behaviour* as on KDE: hotkeys, arrows, Enter, Escape, held modifiers with the hint
  (KEY-13, PICK-33), a right click opens the tile's menu (PICK-30), a middle click opens in
  the background (PICK-32), the "⋯" menu has Open In, Copy Link, Create Rule… (Ctrl+R,
  PICK-31) and Settings… (PICK-08, PICK-28); a new request replaces the one shown
  (PICK-27); Escape, a click outside or losing the focus cancels (PICK-23). Choosing asks
  the compositor for an xdg-activation token first (PICK-29). In the popup, `P` and `1`–`9`
  choose (KEY-51).

**Where they appear** (`src/overlay.rs`, 02-picker.md "Linux notes"):

| Session | Window | Placement |
|---|---|---|
| Wayland with `zwlr_layer_shell_v1` (Sway, Hyprland, niri, river, Wayfire, KDE) | gtk4-layer-shell surface on the overlay layer, covering the output, exclusive keyboard | at the pointer when the service reports it, else centred on the focused output |
| X11 | undecorated window | by the window manager |
| Wayland without layer shell (GNOME with the extension off) | undecorated window | by the compositor |

The layer surfaces' namespaces are `wye-picker` and `wye-menu`, for compositor rules. No
open animation is asked for (PICK-15), but a compositor may animate layers; and the panel
is slightly translucent, so a blur rule shows through (PICK-01). For example:

```text
# Hyprland
layerrule = noanim, wye-picker
layerrule = blur, wye-picker
layerrule = ignorealpha 0.5, wye-picker
layerrule = noanim, wye-menu
# SwayFX
layer_effects "wye-picker" blur enable; shadows enable
```

To see them: `wye open https://example.com` with the Picker as the primary browser and
`advanced.frontend = "gnome"` in `config.toml`, and `wye menu`. On X11 and without layer
shell a click outside a picker that keeps the focus cannot be seen; Escape or a click in
another window cancels.

**Calling the service.** From the main thread:

```rust
service::request(
    |proxy| async move { proxy.get_troubleshooting().await },
    glib::clone!(#[weak] label, move |result: Result<String, Error>| { /* on the main thread */ }),
);
```

The call runs on the D-Bus thread with a `Wye1Proxy`; the result comes back on the GTK
main context. Never block the main thread on D-Bus. Capture widgets weakly; a window closed
meanwhile is simply not updated. Without a connection (the self-test) the result is
`Error::Failed`, so code that has a fixture loaded must not call at all.

**Watching the service.** A window that shows what the service holds does not poll: it keeps
the `Subscription` that `service::watch(&["ConfigRevision", …], |change| …)` returns; the
callback runs on the main context when one of those properties changes, when the service
restarts (`Change::Restarted`: read everything) and once as soon as the subscription stands.
`service::follow` carries any other signal stream the same way. Dropping the subscription
stops it.

**Settings pages.** Everything a page reads or writes goes through the window's
`SettingsStore` (`src/settings/store.rs`), a GObject shared by every page:

- read: `value(path)`, `bool_value`, `string_value`, `with_snapshot(|s| …)`,
  `target_menu`, `target_label`, `help_text`, `callout_dismissed`;
- write (SET-06, instant apply, merge patches with conflict handling): `set_value`,
  `set_target`, `set_service_target`, `set_modifiers`, `apply_patch`, `update_ui_state`,
  `dismiss_callout`; machine actions with `act(Action::MakeDefault)`; links with `open_link`;
- follow: `connect_changed` (the snapshot changed), `connect_messages_changed`, the
  properties `loaded`, `writable`, `offline`, `live`.

How a write travels: the store applies the RFC 7386 merge patch to its snapshot at once (the
control already shows the new value), then sends `UpdateConfig(patch, base_revision)` on the
D-Bus thread (`src/settings/save.rs`). On success the store takes the configuration and
revision the service returns. On `Conflict` (someone else wrote first) `save` re-reads the
configuration: a patch without arrays touches only its own keys and is sent again on the new
revision; a patch that replaces an array (rules, shown browsers, key lists) is sent again only
if that array is unchanged, otherwise the save fails with `Conflict` ("changed elsewhere").
Any failure reloads the truth from the service, so the control snaps back, and the window's
banner shows the sentence from `src/error_text.rs`. A page never handles `Conflict` itself;
it only builds patches (`set_value` builds one from a dotted path, `apply_patch` takes one
whole) and rereads on `connect_changed`. Machine-wide actions (`act`) and UI state
(`update_ui_state`, `dismiss_callout`) go through their own calls, not `UpdateConfig`.

Build a page as `src/settings/pages/general.rs` does: an `adw::PreferencesPage`, groups top
to bottom from the widget kit, each configuration control tied to its key with the kit's
`bind(store, path, default)` (it shows the value, saves changes, skips values the store
pushed back, and follows `writable`), and one `show(store, …)` for what no `bind` covers,
run once and on every `connect_changed`. Implement `pages::Page`; `open_sheet(name)` opens a
sheet a fixture asks for, `request(key, argument)` takes `rule-editor` / `test-rules`
(Rules page). Register the page in `pages::build`. Sheets are `widgets::sheet::Sheet`
(`AdwDialog`) presented over `context.window()`.

**Widget kit.** `src/widgets/mod.rs` lists every block with its spec ID; each module states
its API at the top. A page never styles a widget itself.

**Self-test.** Each surface has cases in `fixtures/<surface>.json`, the format `wye-ui` uses:
`{"cases": [{"action": "show", "key": "<window name>", "argument": <string or JSON>}]}`. A
JSON argument travels as its text, exactly as over D-Bus; a file may name another surface
with `"surface"`. For Settings the argument is a page name or `{page, fixture, scheme,
sheet}`: `fixture` is what the service would have returned (`src/settings/fixture.rs`); a
later fixture changes only the parts it names, and every change stays local. Add a case for
each state a surface can show.

**Shared-code follow-up.** `src/selftest/snapshot.rs` names snapshot files exactly as
crates/wye-ui/src/selftest/snapshot.rs does (`sanitize`, the slug rules), so both galleries
read alike; its case type differs from wye-ui's (`Action` here, a string there), so the file
cannot be compiled from source as `route.rs` is. Move the naming into a Qt-free module both
hosts include once crates/wye-ui may change.

The self-test runs each surface in a child process on a private Xvfb (when `Xvfb` is on
`PATH`) with the cairo renderer, an empty `XDG_CONFIG_HOME` (the user's GTK theme and
settings stay out), no portals, no accessibility bus, in-memory GSettings, GNOME's font and
title buttons. It fails on a crash, a missing pass line, or any GLib message of level warning
or worse: the child's log writer prints each as one JSON line (`src/selftest/log.rs`) with
the code that logged it, and there is no allow-list. The flake check `gtk-selftest` runs the
installed, wrapped binary the same way, with the icon theme the wrapper brings.

**Snapshots.** `--snapshots DIR` saves every visible window after each case as
`DIR/<surface>-<NN>-<slug>.png` (`-w2`… for further windows, `-p1`… for each open popover of
a window, such as a target menu, since a popover is a surface of its own), rendered by the
surface's GSK renderer at `--scale` (or `GDK_SCALE`). A sheet stays open across cases until
something closes it; when a later case should show something else, close it in the
presenter first, as the kit gallery does (`visible_dialog()` then `force_close()`):

```sh
nix develop -c target/debug/wye-gtk --self-test settings --snapshots /tmp/shots --scale 2
nix develop -c target/debug/wye-gtk --self-test about --snapshots /tmp/shots --scheme dark
```

A dev tool: there is no window shadow or rounded frame (no compositor). The gallery is
captured on the GNOME stage (`docs/media/gnome/stage/`).

## Packaging

`nix/package.nix` builds `wye-gtk` with the workspace and wraps only this binary with GTK's
run-time environment (`wrapGApp`). The D-Bus activation file and the systemd user unit are
`data/dbus/dev.soldunov.wye.Gtk.service.in` and `data/systemd/wye-gtk.service.in`.
