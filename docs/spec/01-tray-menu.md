# 01 · Tray icon and menu

The tray icon is Wye's only always-visible surface. Its menu gives quick access to the
primary-browser choice, the clipboard action and Settings.

## Tray icon

| ID | Requirement | Evidence |
|---|---|---|
| TRAY-01 | Wye shows an icon in the system tray while it runs. | Specified |
| TRAY-02 | The icon follows the **Tray icon** setting ([GEN-02](04-general.md)). With the value "Primary Browser", the icon is the current primary browser's icon. When the primary browser is the Picker, the icon is the picker glyph: a bulleted-list symbol (three dots, three lines). | Specified |
| TRAY-03 | The icon changes as soon as the primary browser changes. | Expected |
| TRAY-04 | The **Show tray icon** setting ([GEN-03](04-general.md)) hides the icon. Wye keeps running and keeps handling links. | Specified (setting), Expected (behaviour) |
| TRAY-05 | When the icon is hidden, starting Wye again (app launcher or command line) opens the Settings window, so the user can always get back in. | Expected |
| TRAY-06 | The icon shows a pressed/highlighted state while its menu is open. | Specified |
| TRAY-07 | A primary click on the icon opens the menu. | Specified |
| TRAY-08 | The **Toggle menu** global shortcut ([ADV-05](10-advanced.md#keyboard-shortcuts)) opens and closes the menu. | Specified (setting) |
| TRAY-19 | A middle click on the icon opens the Settings window, where the tray host supports it (StatusNotifierItem `SecondaryActivate`). | Proposed |

## Menu layout

Items in exact order. The example has two shown browsers: one browser and one browser
profile.

| # | Item | Kind | Shortcut shown | Behaviour |
|---|---|---|---|---|
| 1 | Open URL from Clipboard | action | none | Opens the URL on the clipboard through the pipeline ([IN-02](11-url-pipeline.md#entry-points)). Disabled when the clipboard holds no URL. |
| 2 | *separator* | | | |
| 3 | Primary Browser | section header | | Non-interactive, dimmed. |
| 4 | Picker (picker glyph) | radio item | `P` | Checked when the primary browser is the Picker. |
| 5… | One item per shown browser, in shown-browsers order, each with its app icon and display name | radio item | `1`, `2`, … | Checked item is the primary browser. |
| | *separator* | | | |
| | Settings… | action | `Ctrl+,` | Opens the Settings window ([SET-04](03-settings-window.md)). |
| | More | submenu | `›` | See TRAY-15. |
| | *separator* | | | |
| | Quit Wye | action | `Ctrl+Q` | Quits Wye. |

| ID | Requirement | Evidence |
|---|---|---|
| TRAY-10 | "Open URL from Clipboard" is enabled only when the clipboard contains a URL. The menu refreshes this state each time it opens. | Specified (disabled state), Expected (rule) |
| TRAY-11 | The "Primary Browser" group is a radio group of the Picker plus every shown browser. Choosing an item sets the primary browser at once and saves it; the Browsers page ([BRW-01](05-browsers.md)) shows the same value. | Specified |
| TRAY-12 | A browser profile item shows only the profile name (for example "Work") with the parent browser's icon, not "Work (Chrome)". | Specified |
| TRAY-13 | Menu shortcuts: `P` selects the Picker; digits `1`–`9` select shown browsers by position. Items beyond the ninth get no digit. | Specified (`P`, `1`, `2`), Expected (limit) |
| TRAY-14 | Menu item icons are full-colour app icons, drawn large (about 26 px) where the tray host allows it. | Specified |
| TRAY-15 | "More" submenu, in order: **History…** (history window, [DLG-HIS](17-dialogs.md#history-window)); **Recent Links** › (only while history is on: the last 10 links, each shown as host and path, middle-truncated; choosing one opens it in the picker); *separator*; **Test Rules…** ([DLG-TST](17-dialogs.md#rule-tester)); **Rescan Browsers**; **Set Up Wye…** ([ONB](18-onboarding.md#first-run)); *separator*; **Help** (opens the documentation website); **About Wye** ([DLG-ABT](17-dialogs.md#about-window)). | Specified (entry), Proposed (contents) |
| TRAY-16 | "Settings…" opens the Settings window or raises it if already open. | Expected |
| TRAY-17 | Quitting Wye while it is the default browser means links stop opening. Proposed: when Wye quits, the next link still starts it (D-Bus activation of the handler, [DEF-04](11-url-pipeline.md#default-browser-registration)), so "Quit" only removes the tray icon until then. | Proposed |
| TRAY-18 | While Wye is not the default browser, the menu starts with **Make Wye Default Browser** and a separator, and the icon carries a warning overlay ([ONB-11](18-onboarding.md#default-browser-status)). | Proposed |

Target metrics: menu width about 250 px; section header in a smaller, dimmed font;
checkmark column at the leading edge; shortcut column right-aligned and dimmed.

## Linux notes

- **KDE Plasma and most other desktops** (XFCE, Cinnamon, MATE, Budgie, LXQt, waybar and
  other wlroots bars): StatusNotifierItem with a com.canonical.dbusmenu menu. DBusMenu
  supports everything above: `type=separator`, `enabled=false` (for the section header
  and the disabled clipboard item), `toggle-type=radio` with `toggle-state`, `icon-name`
  or `icon-data`, `shortcut`, and `children-display=submenu`. Set `ItemIsMenu=true` so a
  primary click opens the menu in Plasma. Use the DBusMenu `AboutToShow` call to refresh
  TRAY-10 before the menu appears.
- **GNOME Shell** has no StatusNotifierItem host by default. Follow Token Station: ship a
  GNOME Shell extension that draws the icon and a native `PopupMenu`, and keep the SNI
  item as a fallback for users of the AppIndicator extension.
- Tray hosts render the `shortcut` property inconsistently and some ignore it. Treat
  TRAY-13 as best effort outside the GNOME extension.
- TRAY-08 (open the menu from a shortcut): StatusNotifierItem has no "open your menu"
  call, so the host cannot be asked to show the menu. Options: pop up the same menu as a
  Wye-owned popup (see picker placement limits in [13](13-linux-platform.md)), or map the
  shortcut to the GNOME extension's own menu. See [14](14-open-questions.md).
- TRAY-10 on Wayland: an unfocused app cannot read the clipboard with the core protocol.
  See clipboard access in [13](13-linux-platform.md).
