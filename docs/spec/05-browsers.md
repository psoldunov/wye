# 05 · Browsers page, target menu and shown browsers

The Browsers page sets the two fallback targets (primary and alternative), chooses which
targets appear in the picker and tray menu, and shows browser-profile status. This file
also defines two shared surfaces (the target menu and the shown browsers sheet) and how
Wye discovers and launches targets.

## Browsers page

| ID | Requirement | Evidence |
|---|---|---|
| BRW-01 | Target popup row **Browser**: the primary browser. Value in the design: Picker. The same value as the tray menu's radio group ([TRAY-11](01-tray-menu.md)). | Specified |
| BRW-02 | Target popup row **Alternative browser**, with a help button and subtitle: "Hold \<alternative-browser key\> while opening a link to open it in the alternative browser." The subtitle names the key currently set in BRW-03. Value in the design: Picker. | Specified |
| BRW-03 | Modifier chooser row ([BLK-18](03-settings-window.md#shared-building-blocks)) **Alternative browser key**, directly under BRW-02. Default **Shift**. Ctrl is a poor default because many terminals need Ctrl+click to open a link; Alt+click and Super+click are often taken by the window manager. Matching is exact ([KEY-05](15-keyboard.md#controls)). | Proposed |
| BRW-04 | Button row **Shown browsers**, subtitle "Browsers shown in the picker and the tray menu.", button **Choose…** opens the shown browsers sheet. | Specified |
| BRW-05 | Row **Browser profiles** with a help button and a subtitle that states which profile kinds are supported, with accent-coloured links to documentation. The design's copy: "For Firefox and Zen, only new-style Profile Groups (Firefox 138+) are supported (not ones created with `about:config`). How to migrate." Proposed on Linux: support both kinds ([DISC-07](#discovery-and-launching)) and drop the restriction from the copy. | Specified (row), Proposed (Linux scope) |
| BRW-06 | The profiles row's trailing control shows detection status. Proposed: "N profiles found" plus a **Rescan** button. In a Flatpak build that lacks read access to browser config directories, show what is missing and how to grant it. | Proposed |

## Target menu

The popup that every target popup row opens (Browsers, Apps, rule editor).

| ID | Requirement | Evidence |
|---|---|---|
| TGT-01 | Closed state: the desktop's own combo box showing the current target's icon and name, then its chevron ([BLK-04](03-settings-window.md#shared-building-blocks)). | Specified |
| TGT-02 | Open state: sections separated by separators, in this order: **(a)** "Default (\<primary\>)", only on the Apps page and in the rule editor; **(b)** "Picker" with the picker glyph; **(c)** the service's own desktop app, only on the Apps page and only when installed; **(d)** every installed browser; **(e)** header "Private Browsing" then "\<Browser\> (Private)" for each browser that supports it; **(f)** for each browser with profiles, header "Profiles: \<Browser\>" then one item per profile name; **(g)** "Other…". | Specified |
| TGT-03 | The current value has a checkmark. Every item shows an icon: app icon for apps and browsers, the browser's icon for private and profile items, the glyph for Picker. | Specified |
| TGT-04 | The menu's height is capped; a longer menu scrolls (no scroll arrows). | Specified |
| TGT-05 | Section (d) lists every app registered for `http`/`https`, including non-browsers such as terminal emulators that register as URL handlers. Wye excludes itself. Proposed order: alphabetical by display name. | Specified (non-browsers listed), Proposed (order) |
| TGT-06 | "Other…" opens the app chooser ([DLG-APP](17-dialogs.md#app-chooser)). The chosen app becomes a target and is offered in every other target menu from then on. | Specified (item), Proposed (chooser) |
| TGT-07 | The design omits "Picker" from the Apps page menu. Proposed: offer Picker in every target menu, so a rule or mapping can always ask even when the primary browser is a real browser. | Proposed |

## Shown browsers sheet

| ID | Requirement | Evidence |
|---|---|---|
| SHOWN-01 | **Choose…** opens a modal sheet ([BLK-11](03-settings-window.md#shared-building-blocks)) over the settings window. | Specified |
| SHOWN-02 | One row per candidate target: every installed browser, every profile (named "\<Profile\> (\<Browser\>)", e.g. "Work (Chrome)"), and every app added with "+". Whether private-window targets are listed is not specified; proposed: list them after profiles. | Specified, Proposed (private rows) |
| SHOWN-03 | Row layout ([BLK-15](03-settings-window.md#shared-building-blocks)): checkbox, icon, name, hotkey popup, drag handle. Checked rows sit at the top in the user's order and show a drag handle; unchecked rows follow without one. Dragging reorders checked rows. That order is the picker and tray-menu order. | Specified |
| SHOWN-04 | The hotkey popup assigns one picker hotkey per row (examples in the design: "a", "c", unset). Every row has the popup, checked or not. Proposed: the popup lists "None", a–z and 0–9, and **Other Key…**, which records any single key. A hotkey is unique, so choosing one that is in use clears it on the other row; keys used by picker actions are not offered ([KEY-12](15-keyboard.md#hotkey-scheme)). Unset by default. When the hotkey scheme is not "Assigned per browser" ([KEY-10](15-keyboard.md#hotkey-scheme)), the popups are disabled and show the key the scheme gives. | Specified (popup), Proposed (rules) |
| SHOWN-05 | "+" at the bottom left adds any app through the app chooser ([DLG-APP](17-dialogs.md#app-chooser)). | Specified (button), Proposed (chooser) |
| SHOWN-06 | **Done** (primary button, bottom right) closes the sheet. Checkbox, order and hotkey changes apply at once. | Specified |
| SHOWN-07 | The list scrolls; the footer stays pinned. | Specified |
| SHOWN-08 | An app added with "+" can be removed again (context menu or delete button on its row). | Proposed |

Target metrics: sheet width about 380 px, rows about 32 px tall, icons about 16 px.

## Discovery and launching

| ID | Requirement | Evidence |
|---|---|---|
| DISC-01 | Find browsers: desktop entries whose `MimeType` includes `x-scheme-handler/http` or `x-scheme-handler/https`, across `$XDG_DATA_HOME/applications` and every `$XDG_DATA_DIRS/*/applications` (this includes Flatpak and Snap export directories when the session sets them up). Respect `Hidden`, `NoDisplay`, `OnlyShowIn`/`NotShowIn` and `TryExec`. Exclude Wye. | Proposed |
| DISC-02 | Watch those directories and refresh every menu, the sheet and the picker when apps are installed or removed. | Proposed |
| DISC-03 | Names and icons come from the desktop entry, with icons resolved through the current icon theme. | Proposed |
| DISC-04 | Detect the browser family (Chromium-based, Firefox-based, other) from the desktop ID and `Exec`. The family decides private-window and profile support. | Proposed |
| DISC-05 | Private windows: offer a private target only when Wye knows how to open one: a private-window desktop action in the entry, or the family flag (`--incognito` for Chromium-based, `--private-window` for Firefox-based). | Proposed |
| DISC-06 | Chromium-based profiles: read `Local State` (JSON) in the browser's config directory; `profile.info_cache` maps each profile directory to its display name and avatar. The signed-in avatar image lives in the profile directory. Launch with `--profile-directory=<dir>`. Config directories differ per browser and per packaging (Flatpak apps keep theirs under `~/.var/app/<app-id>/config/`). | Proposed |
| DISC-07 | Firefox-based profiles (Firefox, Zen, LibreWolf, Floorp and others): read both the classic `profiles.ini` and the newer profile-group store (Firefox 138+). Exact locations and formats per browser must be verified. | Proposed |
| DISC-08 | Profile avatars feed the picker's profile badge ([PICK-06](02-picker.md)). Where a profile has no picture, draw a badge from its avatar colour and initial. | Proposed |
| LAUNCH-01 | Launch targets through their desktop entry's `Exec` (field codes expanded), so Flatpak and Snap wrappers keep working. Insert profile, private and new-window arguments before the URL; append the URL when `Exec` has no URL field code. | Proposed |
| LAUNCH-02 | Desktop-app targets that need a translated URL (custom scheme) get the translated URL ([APP-07](06-apps.md)). | Proposed |
| LAUNCH-03 | Give the launched target focus: pass an xdg-activation token (`XDG_ACTIVATION_TOKEN`) on Wayland and a startup-notification ID (`DESKTOP_STARTUP_ID`) on X11. | Proposed |
| LAUNCH-04 | "Open in background" ([RUL-22](08-rules.md#rule-editor-sheet)) launches without an activation token so the compositor does not raise the target. Best effort: the result depends on the compositor and on whether the browser was already running. | Proposed |
| LAUNCH-05 | "Force new window" ([RUL-23](08-rules.md#rule-editor-sheet)) adds `--new-window` for Chromium- and Firefox-based browsers. | Proposed |
| LAUNCH-06 | Launched apps run in their own transient systemd scope (`app-<desktop-id>-<random>.scope`), as desktop launchers do, so they do not belong to Wye's process tree. | Proposed |
| LAUNCH-07 | When a launch fails, show a desktop notification that names the target and offers to open the link somewhere else. | Proposed |
