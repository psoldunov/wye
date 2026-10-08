# 13 · Linux platform mechanisms

What each feature needs from the desktop, per session type, and where the risk is.
Claims marked "verify" must be checked against current desktop releases before building
on them.

## Capability matrix

| Capability | Used by | X11 | GNOME (Wayland) | KDE Plasma (Wayland) | wlroots and others (Wayland) | Risk |
|---|---|---|---|---|---|---|
| Receive links as default browser | [DEF](11-url-pipeline.md#default-browser-registration) | desktop entry + `mimeapps.list` | same | same | same | low |
| Tray icon and menu | [TRAY](01-tray-menu.md) | SNI + DBusMenu (needs a host) | Wye Shell extension; SNI only with the AppIndicator extension | SNI + DBusMenu | SNI through the bar (waybar and others) | low |
| Picker at the pointer | [PICK-02](02-picker.md) | pointer query + positioned window | Shell extension | layer-shell, no pointer position (centre of active screen) | layer-shell, centred | high |
| Picker blur | [PICK-01](02-picker.md) | compositor-dependent | `Shell.BlurEffect` in the extension | KWindowEffects blur | compositor-specific | low |
| Held modifiers at link time | [BRW-03](05-browsers.md), [RUL-27](08-rules.md), [ADV-11](10-advanced.md) | query pointer/keyboard state | Shell extension (`global.get_pointer()` includes modifier state) | no client API; candidate: a transient layer-shell surface with exclusive keyboard focus reads the modifier state on focus (verify); or a KWin script | same layer-shell candidate (verify) | high |
| Source app | [RUL-16](08-rules.md) | parent-process chain; active window's PID | parent-process chain, systemd scope of the caller (`app-*-<id>-*.scope`), `GIO_LAUNCHED_DESKTOP_FILE` in the caller's environment; extension: focused app (`Shell.WindowTracker`) | parent-process chain and systemd scope; active window through a KWin script | parent-process chain and scope; compositor IPC (Sway, Hyprland) for the focused window | medium |
| Global shortcuts | [ADV-05–07](10-advanced.md#keyboard-shortcuts) | key grabs | GlobalShortcuts portal (GNOME 48+, verify) or an extension keybinding | GlobalShortcuts portal / KGlobalAccel | compositor config calling the `wye` CLI; portal where available | medium |
| Clipboard read on demand | [TRAY-10](01-tray-menu.md), [IN-02](11-url-pipeline.md#entry-points) | selections | extension (`St.Clipboard`) | Klipper D-Bus or data-control protocol | data-control protocol | medium |
| Clipboard watch and rewrite | [EXT-02/03/05](09-extras.md) | XFixes selection events | extension only (no data-control protocol in GNOME's compositor; verify) | data-control protocol | data-control protocol | high without extension |
| Screen-lock state | [PKS-05](07-picker-settings.md) | logind `LockedHint`, `org.freedesktop.ScreenSaver` | `org.gnome.ScreenSaver`, logind | `org.freedesktop.ScreenSaver`, logind | logind when the locker sets the hint | low |
| Launch at login | [GEN-01](04-general.md) | XDG autostart or systemd user unit | same | same | depends on how the session starts autostart entries and user units | low |
| Give focus to the launched app | [LAUNCH-03](05-browsers.md#discovery-and-launching) | `DESKTOP_STARTUP_ID` | xdg-activation token | xdg-activation token | xdg-activation where supported | low |
| Colour scheme and accent | [PICK-12](02-picker.md) | Settings portal `org.freedesktop.appearance` (`color-scheme`, `accent-color`) | same | same | same, when a portal backend runs | low |
| Notifications | [DEF-03](11-url-pipeline.md#default-browser-registration), [LAUNCH-07](05-browsers.md#discovery-and-launching) | `org.freedesktop.Notifications` | same | same | notification daemon (mako, dunst, …) | low |

## Source-app detection details

1. The handler process (`xdg-open` → Wye) walks its parent chain until it leaves
   `xdg-open`, shells and `gio`.
2. It reads that process's systemd cgroup. Desktop launchers put apps in scopes named
   after their desktop ID (`app-<launcher>-<desktop-id>-<random>.scope`), which gives a
   reliable ID even for Electron apps.
3. Fallbacks: `GIO_LAUNCHED_DESKTOP_FILE` in the process environment, then matching the
   executable against desktop entries' `Exec`.
4. The focused window's app stands in for the source in exactly two cases, where the
   desktop allows it (GNOME extension, KWin script, compositor IPC); otherwise the
   source is unknown.
   - Sandboxed apps (Flatpak) open links through the OpenURI portal, so the parent is
     `xdg-desktop-portal`, not the app.
   - The handler (`wye open`) runs in Wye's own app unit
     (`app-dev.soldunov.wye@<uuid>.service` or `app-<launcher>-dev.soldunov.wye-<n>.scope`)
     and its parent chain names no app. A launcher such as KIO on Plasma 6 started it as
     a unit of its own, so its parent is `systemd --user`.

   Other chains that name no app stay unknown: a systemd timer, `systemd-run`, a caller
   that has already exited. A desktop ID or executable given with the link wins over
   focus. The focus is a heuristic: a link from a surface that never takes focus (a
   notification popup, KRunner) is credited to whichever app has focus.

Process names are matched as the kernel records them (`comm`, cut to 15 bytes) and in
their NixOS-wrapped form `.NAME-wrapped`, which `makeWrapper` gives the real program.
So `.xdg-desktop-po` is the portal and `.kde-open-wrapp` is `kde-open`.

Rules with source apps simply do not match when the source is unknown.

## Architecture precedent

Token Station splits into a Rust daemon exposed over D-Bus plus one native frontend per
desktop (a GNOME Shell extension with libadwaita preferences, a Plasma applet in QML, and
a StatusNotifierItem tray for other desktops). Wye uses the StatusNotifierItem on Plasma
as well: an applet added nothing the item's `DBusMenu` cannot show, and its configuration
dialog had nothing to configure. Wye has the same needs (tray presence, a
floating surface, native settings) and additional ones that only the GNOME Shell
extension can meet on GNOME (picker at the pointer, modifier state, clipboard watching).
Wye took the same split: a Rust service plus one frontend per desktop
([14](14-open-questions.md), decision 1).

## Packaging

A Flatpak sandbox gets in the way of almost everything Wye does: reading other apps'
desktop entries and profile folders, launching host apps, inspecting `/proc` and cgroups
for source apps, and installing native-messaging manifests into browsers. So Wye ships
native packages first, as Token Station does: the Nix flake with home-manager and NixOS
modules, and on each release a `.deb`, an `.rpm` and an AppImage for x86_64 and aarch64
([packaging/README.md](../../packaging/README.md)). Flatpak would be a later, reduced build.
