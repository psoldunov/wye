# 02 · Picker

The picker is the floating chooser that appears when a link resolves to the Picker target.
It is the surface users see most often, so it must appear fast, take keyboard focus, and
get out of the way.

The design covers one state: dark theme, two shown browsers (a browser and a browser
profile), icon size Large, names shown, URL hidden, profile badge shown.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../media/kde/screenshots/dark/picker.png">
    <img src="../media/kde/screenshots/light/picker.png" width="905" alt="The Wye picker on KDE Plasma: six browser and profile tiles with hotkeys, and the link it is about to open">
  </picture>
</p>

<p align="center"><em>As implemented on KDE Plasma.</em></p>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../media/kde/screenshots/dark/picker-tile-menu.png">
    <img src="../media/kde/screenshots/light/picker-tile-menu.png" width="450" alt="The right-click menu of a picker tile: Open, Open in Private Window, Open in New Window, Open in Background, Make Primary Browser">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../media/kde/screenshots/dark/picker-more.png">
    <img src="../media/kde/screenshots/light/picker-more.png" width="510" alt="The picker overflow menu with the Open In submenu, Copy Link, Create Rule and Settings">
  </picture>
</p>

<p align="center"><em>As implemented on KDE Plasma.</em></p>

## Layout

| ID | Requirement | Evidence |
|---|---|---|
| PICK-01 | Borderless floating panel with rounded corners, a translucent dark background with blur behind it, and a thin light outline. No title bar. | Specified |
| PICK-02 | The panel appears on top of the app where the link was clicked, near the pointer. | Specified (over the source app), Expected (anchored to pointer) |
| PICK-03 | One horizontal row of tiles, one per shown browser, in shown-browsers order ([SHOWN-03](05-browsers.md#shown-browsers-sheet)). | Specified |
| PICK-04 | Each tile, top to bottom: the hotkey character (small, dimmed, centred), the target icon, the display name. | Specified |
| PICK-05 | Names are a single line, truncated at the end with an ellipsis ("Work Chro…"). | Specified |
| PICK-06 | A profile target uses its browser's icon with a **profile badge**: the profile's circular avatar overlapping the icon's bottom-left corner, about 60% of the icon's size. Controlled by [PKS-04](07-picker-settings.md). | Specified |
| PICK-07 | The selected tile has an accent-coloured rounded-rectangle background behind the icon and name. | Specified |
| PICK-08 | A circular "⋯" overflow button follows the last tile, vertically centred on the icons. It opens a menu: **Open In** › (every known target that is not a tile, grouped like the target menu: browsers, Private Browsing, one section per profile browser, then Other…); *separator*; **Copy Link**; **Create Rule…** (PICK-31); *separator*; **Settings…**. | Specified (button), Proposed (menu) |
| PICK-09 | **Show URL** ([PKS-03](07-picker-settings.md)) adds a line under the tiles: the source app's small icon and name ("from Slack"), then the link with the host in bold (the same size as the rest) and the rest dimmed, truncated in the middle so the host and the end of the path stay visible. The full link is in a tooltip. | Specified (setting), Proposed (layout) |
| PICK-10 | **Show browser names** ([PKS-02](07-picker-settings.md)) hides the name labels when off. | Specified (setting) |
| PICK-11 | **Icon size** ([PKS-01](07-picker-settings.md)) switches between Small, Medium and Large tiles. | Specified (setting) |
| PICK-12 | The panel follows the system colour scheme (light/dark) and accent colour. | Proposed |
| PICK-13 | With more than eight tiles, the row wraps into further rows of up to eight tiles. | Proposed |
| PICK-14 | While a held-modifier action is active ([KEY-13](15-keyboard.md#hotkey-scheme)), a hint line under the tiles names it ("Open in a private window") and tiles that cannot open that way are dimmed. | Proposed |
| PICK-15 | The panel appears at once: no fade or zoom when it opens. | Proposed |

Target metrics:

| | Small | Medium | Large |
|---|---|---|---|
| Icon | 24 px | 32 px | 40 px |
| Tile pitch | 36 px | 48 px | 60 px |
| Name text | caption | caption | caption |
| Profile badge | 14 px | 18 px | 24 px |

The reference specifies only Large (52 px icons, 64 px pitch, body text), which reads too large
on a Linux desktop. Large here is about a quarter smaller; Small and Medium icons are 60% and
80% of Large's. The pitch is one and a half icons, so tiles without names (and the badges
overlapping their icons) keep clear of each other. At every size: hotkey character about 10 px
text; selection highlight corner radius about 12 px; panel padding about 12–16 px; panel corner
radius about 14 px; overflow button about 14 px.

## Interaction

| ID | Requirement | Evidence |
|---|---|---|
| PICK-20 | Clicking a tile opens the link in that target and closes the picker. | Expected |
| PICK-21 | Pressing a tile's hotkey character opens the link in that target. Hotkeys come from the shown browsers sheet ([SHOWN-04](05-browsers.md#shown-browsers-sheet)) or from the hotkey scheme ([KEY-10](15-keyboard.md#hotkey-scheme)). | Specified (hotkeys shown), Expected (behaviour) |
| PICK-22 | Left/Right arrow keys (and Tab/Shift+Tab) move the selection; Enter or Space opens the selected target. Pointer hover also moves the selection. All picker keys are configurable ([KEY-20](15-keyboard.md#picker-keys-sheet)). | Expected (defaults), Proposed (configurable) |
| PICK-23 | Escape, a click outside the panel, or loss of focus closes the picker without opening the link. | Expected |
| PICK-24 | The first tile is selected when the picker opens. | Proposed |
| PICK-25 | The picker opens when the resolved target is the Picker (primary browser, alternative browser, a rule, or a web app mapping), when a link arrives from the browser extension and [ADV-10](10-advanced.md#miscellaneous) forces it, and from **Preview Picker** ([PKS-06](07-picker-settings.md)). | Specified (settings) |
| PICK-26 | If the screen is locked and [PKS-05](07-picker-settings.md) is on, the picker does not open; the link goes to the alternative browser. | Specified (setting) |
| PICK-27 | A new link that arrives while the picker is open replaces the pending link; the panel stays open and the URL line (if shown) updates. | Proposed |
| PICK-28 | Targets that are not tiles stay reachable through the "⋯" menu's **Open In** submenu (PICK-08). | Proposed |
| PICK-29 | The picker gives the chosen target focus when it opens the link ([LAUNCH-03](05-browsers.md#discovery-and-launching)). | Proposed |
| PICK-30 | Right-clicking a tile opens a context menu: **Open**, **Open in Private Window**, **Open in New Window**, **Open in Background** (each only when the target supports it), *separator*, **Make Primary Browser**. | Proposed |
| PICK-31 | **Create Rule…** (from the "⋯" menu or `Ctrl+R`) closes the picker and opens the rule editor with a Domain matcher for the link's host and, when known, the source app filled in. The link itself is not opened. | Proposed |
| PICK-32 | Middle-clicking a tile opens the link in that target in the background. | Proposed |
| PICK-33 | Held modifiers apply the private-window, background and new-window actions to whatever the user chooses ([KEY-13](15-keyboard.md#hotkey-scheme)). | Proposed |

## Linux notes

- **Pointer position and placement.** On X11 the picker can read the pointer position and
  place an override-redirect or dialog window there. On Wayland a regular client can
  neither read the global pointer position nor place its own window. Per desktop:
  - GNOME Shell: draw the picker inside Wye's GNOME Shell extension (St widgets). The
    extension knows the pointer position and can blur the background with
    `Shell.BlurEffect`.
  - KDE Plasma: a layer-shell surface (LayerShellQt) on the overlay layer. Blur through
    KWindowEffects. The pointer position is not available to the client; fall back to the
    centre of the active screen unless a KWin script can supply it.
  - Other compositors with layer-shell support (Sway, Hyprland, river, niri and
    others): gtk4-layer-shell, centred on the focused output.
  - Fallback: a normal undecorated dialog, centred by the compositor.
- **Keyboard focus.** A layer-shell picker needs `keyboard_interactivity` set to
  exclusive or on-demand so hotkeys and arrows work without an extra click.
- **No open animation (PICK-15).** KWin types a layer-shell surface by its scope, and its
  Scale effect zooms and fades every new surface of the normal type, which an unknown scope
  gets. The picker asks for the `utility` scope, which neither Scale nor Fading Popups
  animates. (Not `on-screen-display`: Fading Popups fades such a surface in, and KWin takes
  the keyboard focus back from it at once, which cancels the picker.)
- **Focus of the launched browser.** On Wayland, pass an xdg-activation token from the
  picker's key press or click to the launched browser (`XDG_ACTIVATION_TOKEN`), or the
  compositor may refuse to raise it.
- **No blur available.** Use the desktop's popover background colour at full opacity.
