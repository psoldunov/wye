# Wye browser extension

Sends a link, or the page you are on, to Wye (BEXT-01 to BEXT-06 in
[`docs/spec/18-onboarding.md`](../../docs/spec/18-onboarding.md)). Links from the
extension enter Wye's pipeline as IN-05: the browser is the source app, and
with **Force show picker when opening from browser extension** on (ADV-10) the
picker opens unless the bypass key (ADV-11, default Alt) is held.

- **Open Link with Wye** in the context menu of a link, **Open Page with Wye**
  in the page's context menu and on the toolbar button (BEXT-01).
- **Open Page with Wye** on `Alt+Shift+W`, changeable in the browser's
  extension-shortcuts page (BEXT-02).
- Options page: **Close the tab after sending the page to Wye** (BEXT-03).
- When Wye's helper is missing, the toolbar button opens a popup that says so
  and how to install it (BEXT-06).

Firefox reports the keys held during a click, so the bypass key and the
alternative-browser key work from the context menu and the toolbar button
(BEXT-05). Chromium does not report them; Wye then reads them from the
session where it can (KEY-06).

## How it reaches Wye

The extension calls the native-messaging host `dev.soldunov.wye`, which is the
program `wye-native-host` shipped with Wye. The browser starts it and exchanges
messages with it: a 32-bit length in native byte order followed by JSON.

| Message | Meaning |
|---|---|
| `{"url": "…", "modifiers": ["Shift", …] \| null, "pageOrLink": "page" \| "link"}` | Open the URL with Wye. `modifiers` are the browser's names (`Shift`, `Ctrl`, `Alt`, `Command`, `MacCtrl`); `null` when unknown. |
| `{"ping": true}` | Is the host installed? Opens nothing. |

The host answers `{"ok": true}` or `{"error": "…"}`. It calls `OpenLink` on the
Wye service (`docs/dbus-api.md`) with `entry = "extension"`; the call starts the
service when it is not running.

## Installing the host

Browsers find the host through a manifest file in their own directory. Wye
writes one for every browser it finds:

```sh
wye extension install   # write the manifests
wye extension remove    # delete them
```

`wye-native-host --install` and `--remove` do the same.

| Browser family | Manifest directory |
|---|---|
| Chrome, Chromium, Brave, Vivaldi, Edge, Thorium, Helium | `~/.config/<browser>/NativeMessagingHosts/` |
| Firefox (and forks that use `~/.mozilla`) | `~/.mozilla/native-messaging-hosts/` |
| LibreWolf, Waterfox, Floorp, Zen | `~/.<browser>/native-messaging-hosts/` |

A manifest is written only where the browser's directory exists, so start
each browser once before installing. The manifest names the host by the path
found on `PATH` (for example `/etc/profiles/per-user/<you>/bin/wye-native-host`
on NixOS), which survives upgrades; run `--install` again if Wye moved.

Flatpak and Snap browsers are not supported: their sandbox cannot start a
program outside it.

## Loading the extension

Assemble the unpacked extension for your browser family:

```sh
frontends/extension/build.sh firefox  /tmp/wye-extension-firefox
frontends/extension/build.sh chromium /tmp/wye-extension-chromium
```

- **Firefox:** open `about:debugging#/runtime/this-firefox`, **Load Temporary
  Add-on…**, choose `manifest.json` in the Firefox directory. The add-on ID is
  `wye@soldunov.dev`, which the host manifest allows. A temporary add-on is
  removed when Firefox quits; a permanent install needs a signed build.
- **Chromium family:** open `chrome://extensions`, turn on **Developer mode**,
  **Load unpacked**, choose the Chromium directory. The `key` in
  `manifest.chromium.json` fixes the extension ID to
  `lphepmclmllmbbkjkdhjbdgbjfpmmdnn`, which the host manifest allows.

Change the shortcut on `about:addons` → gear → **Manage Extension Shortcuts**
(Firefox) or `chrome://extensions/shortcuts` (Chromium).

## Files

| File | What |
|---|---|
| `background.js` | Menus, toolbar button, shortcut, native messaging (shared by both families). |
| `popup.html`, `popup.js` | The missing-host popup (BEXT-06). |
| `options.html`, `options.js` | The options page (BEXT-03). |
| `pages.css` | Style of both pages. |
| `manifest.firefox.json` | Firefox: event-page background, gecko ID `wye@soldunov.dev`. |
| `manifest.chromium.json` | Chromium: service-worker background, fixed `key`. |
| `icons/` | Wye's icon, rendered from `data/icons/hicolor/scalable/apps/dev.soldunov.wye.svg`. |
| `build.sh` | Assembles one family's unpacked extension. |
