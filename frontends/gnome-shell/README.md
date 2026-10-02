# Wye for GNOME Shell

A GNOME Shell extension (`wye@dev.soldunov`, GNOME Shell 48 or later; developed and
checked on 50) that draws Wye's picker and its panel menu inside the Shell, the GNOME
counterpart of the KDE picker and tray ([02-picker.md](../../docs/spec/02-picker.md),
[01-tray-menu.md](../../docs/spec/01-tray-menu.md)).

It owns `dev.soldunov.wye.Gnome` and exports `dev.soldunov.wye.PickerHost1` at
`/dev/soldunov/wye/Gnome` (`ShowPicker(ss)`, `ClosePicker(s)`, `ShowMenu(s)`), and
`dev.soldunov.wye.SessionHelper1` beside it ([Session helper](#session-helper); see
[docs/dbus-api.md](../../docs/dbus-api.md)). While the name is owned on a GNOME session,
the service sends the picker and the menu here instead of to `wye-ui`. Only the current
owner of `dev.soldunov.wye` may call the host. The extension registers with the service as
the `gnome-extension` tray host (`RegisterTray`), so the service hides its own
StatusNotifierItem, and it registers again whenever the service restarts.

## What it does

The picker is a floating panel at the pointer, kept inside the monitor's work area
(PICK-02), shown at once with no animation (PICK-15):

- one row of equal tiles (up to eight per row, PICK-13), each with its hotkey above the
  icon, the icon, the profile badge over the icon's lower-left corner with its initial
  centred on the glyph's ink (PICK-06), and the name cut at the end with "…" right after
  the last letter that fits (PICK-05); tiles grow up to twice the pitch as on KDE, scaled
  by how much larger GNOME's caption font is than Plasma's small font; the selection is an accent-tinted fill with an accent
  outline (PICK-07); a "⋯" button ends the row (PICK-08);
- the URL line under the tiles: source app, "from App", host in bold, the rest dimmed and
  cut in the middle (PICK-09); the held-modifier hint (PICK-14); the preview note (PKS-06);
- a frosted panel: what lies under it, blurred (`Shell.BlurEffect` on a clone of the window
  group, cut to the panel's rounded corners by a small shader), under a translucent tint,
  a hairline border and a soft shadow; light or dark with the Shell's style and accent
  colour (PICK-01, PICK-12).

Keys follow the request's keymap exactly as the KDE picker reads it (`keys.mjs` ports
wye-core's keymap): hotkeys, arrows, Tab, Home/End, Enter/Space, Escape, `Ctrl+C`,
`Ctrl+R`, Menu, and held modifiers for a private window, the background or a new window
(PICK-21, PICK-22, KEY-13, PICK-33). Hover selects once the pointer moves (PICK-24),
middle click opens in the background (PICK-32), right click opens the tile menu (PICK-30),
"⋯" opens Open In, Copy Link, Create Rule… and Settings… (PICK-08, PICK-28, PICK-31)
beside the button, so it covers no tile. Menus are the Shell's own popup menus; Open In
opens as a page in place of the menu, with a back row, as GTK's popover menus slide in a
submenu, and scrolls only when the list is taller than the screen. A second request replaces the first in place
(PICK-27); a click outside or Escape cancels (PICK-23).

When the user chooses, the extension asks the Shell for a startup-notification ID for the
browser's desktop entry, tied to the click or key press, and sends it as
`activation-token` with `PickerChose` (PICK-29). Mutter accepts it as an XDG activation
token. No token is sent for a background launch or a target with no desktop entry.

The panel menu mirrors the KDE tray: the icon follows the `Tray` model (primary browser,
picker glyph or Wye's symbolic icon, TRAY-02, GEN-02) with a warning emblem while Wye is
not the default browser (ONB-11, TRAY-18); the menu is built from the model in its order,
with radio items and icons for the primary browser, shortcut labels (TRAY-13), and More
and Recent Links as pages in place of the menu, with a back row (Right opens, Left or
BackSpace goes back), so the menu is one page tall and fits the screen (TRAY-15). The
model is refreshed and the clipboard checked each time the menu opens, apart: a clipboard
that cannot be read only leaves "Open URL from Clipboard" disabled (TRAY-10). `P`, `1`–`9`, `Ctrl+,` and `Ctrl+Q`
choose their item (KEY-51). The middle button opens Settings (TRAY-19). `ShowMenu`
toggles the menu (TRAY-08), at the pointer while the icon is hidden (TRAY-04).

The windows the menu and the picker open (Settings, History, Test Rules, Set Up, About,
the rule editor) are started by the service without an activation token, so GNOME's
focus-stealing prevention would show "Wye is ready" instead. After such a request the
extension activates the Wye window (app ID or WM class `dev.soldunov.wye…`) that appears
or asks for attention within eight seconds (TRAY-16).

## Session helper

Mutter offers no data-control protocol, no layer shell and no way to ask for the pointer
or the focused window, so on GNOME the service asks the Shell, through
`dev.soldunov.wye.SessionHelper1` (`session-helper.js`): `ReadClipboard`,
`WriteClipboard`, `WatchClipboard` with `ClipboardChanged`, `QueryModifiers`,
`QueryPointer` and `FocusedApp`. With it, "Open URL from Clipboard", Copy Link, the
copy-time rewrites on the Extras page (EXT-12 to EXT-15), held keys (KEY-06), the
pointer for other picker hosts (PICK-02) and the source app of a link from a sandboxed
app (source-app step 4) work on GNOME. Privacy:

- only the current owner of `dev.soldunov.wye` may call it, as with `PickerHost1`;
- the clipboard is read when the service asks, and watched only while it asked to, which
  it does only while a copy-time rewrite is switched on;
- each change goes to the service alone (a unicast signal), never as a broadcast, and
  watching stops when the service goes away;
- only links leave the Shell: one token with no whitespace that is an `http` or `https`
  link with a host, or a `mailto:` address; any other text (a password from a manager that
  marks no secret included), content a password manager marked secret, content offered
  with an image and anything longer than 8192 characters read as empty
  (`session-model.mjs`, EXT-12).

## Files

| File | Contents |
|------|----------|
| `extension.js` | The D-Bus host, the service watch, tray registration. |
| `session-helper.js` | `SessionHelper1`: clipboard, held keys, pointer, focused app. |
| `picker.js`, `picker-view.js`, `blur.js` | The picker: behaviour, widgets, frosted backdrop. |
| `tray.js`, `menus.js` | The panel icon and its menu; menu pieces and pages shared with the picker. |
| `focus.js` | Activation tokens and raising Wye's windows. |
| `model.mjs`, `keys.mjs`, `tray-model.mjs`, `session-model.mjs` | Pure logic, no GNOME imports, tested with Node. |
| `icons.js`, `*.svg`, `stylesheet.css`, `metadata.json` | Icons, style, metadata. |

## Install

The Nix package installs the extension to `share/gnome-shell/extensions/wye@dev.soldunov`
without enabling it. By hand:

```sh
dest=~/.local/share/gnome-shell/extensions/wye@dev.soldunov
mkdir -p "$dest"
cp -r frontends/gnome-shell/. "$dest"/
rm -f "$dest"/README.md "$dest"/test-*.mjs
gnome-extensions enable wye@dev.soldunov
```

Log out and back in on Wayland after the first installation. The Wye service runs
separately.

## Checks

```sh
node frontends/gnome-shell/test-model.mjs
for f in frontends/gnome-shell/*.js frontends/gnome-shell/*.mjs; do node --check "$f"; done
```

The Node tests cover the pure modules: request parsing, metrics and tile widths, the URL
line, placement, held-modifier choices, tile and Open In menus, key names and dispatch
(against the KDE picker's behaviour), the tray model and shortcuts. They do not run the
Shell. For that, run the extension in a headless GNOME Shell (`gnome-shell --headless
--virtual-monitor 1280x800 --wayland --no-x11` on a private session bus) with a fake Wye
service; `docs/media/gnome/stage/` builds such a session for the screenshot gallery. Watch
the Shell's log for `JS ERROR` and warnings while you enable, use and disable it.

## Limits

- Activation tokens: the token is minted for the target's desktop entry; when the Shell
  does not know that entry (an unusual install), the browser launches without one and
  GNOME may not raise it.
- Windows opened from the menu are raised by the extension, not by a token: a Wye window
  that appears or asks for attention in the eight seconds after the request is activated.
- The tray menu has no clipboard preview: `ClipboardHasUrl` only says whether there is a
  URL.
- Submenus open as pages in place of their menu, as GTK's popover menus do, not to the
  side as on KDE.
- The picker has no tooltips for cut names or the full link; the full link and name are in
  the accessible names.
- The picker and menu are drawn on the monitor under the pointer; `placement` from the
  service is not used, because the Shell knows the pointer itself.
- `FocusedApp` gives the desktop ID only: no process ID or window class, which the KWin
  helper adds on Plasma.
- Settings reads the clipboard capabilities from `Status` when it opens: an extension
  enabled while Settings is open shows there once Settings is opened again.
- `wye debug probe` runs in its own process, which the helper does not answer, so it
  shows no GNOME helper; the service itself uses it.
- The host and the helper trust whoever owns `dev.soldunov.wye` on the session bus. While
  the service is not running, any unsandboxed process of the same user could take that name
  and then call them: show a picker, read a link from the clipboard, write the clipboard.
  The session bus has no stronger identity to check, and a sandboxed app (Flatpak) cannot
  own the name; an unsandboxed process of the user can already read the user's files and
  run code in their session.
