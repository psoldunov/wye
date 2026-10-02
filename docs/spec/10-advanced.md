# 10 · Advanced page

URL expansion, the global transform script, global keyboard shortcuts, history, the
browser-extension override, and the frontend.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../media/kde/screenshots/dark/settings-advanced.png">
    <img src="../media/kde/screenshots/light/settings-advanced.png" width="600" alt="The Settings window on the Advanced page on KDE Plasma">
  </picture>
</p>

<p align="center"><em>As implemented on KDE Plasma.</em></p>

## Layout

```
URL Expansion
┌──────────────────────────────────────────────────────────────┐
│ Expand redirect and short URLs          [on ] [Configure…]   │
└──────────────────────────────────────────────────────────────┘
URL Transformation
┌──────────────────────────────────────────────────────────────┐
│ Transform all URLs before matching rules [off] [Edit Script…]│
│   Runs after URL expansion and tracking removal.             │
└──────────────────────────────────────────────────────────────┘
Keyboard Shortcuts
┌──────────────────────────────────────────────────────────────┐
│ Toggle menu                                [Record Shortcut] │
│ Open URL from clipboard with primary browser  [Record Sh…]   │
│ Open URL from clipboard with alternative browser [Record…]   │
└──────────────────────────────────────────────────────────────┘
  Picker keys are on the Picker page; the alternative browser
  key is on the Browsers page.
History
┌──────────────────────────────────────────────────────────────┐
│ Store history of the last 100 opened links  [off] [Show…]    │
└──────────────────────────────────────────────────────────────┘
Miscellaneous
┌──────────────────────────────────────────────────────────────┐
│ Force show picker when opening from browser extension  [on ] │
│   When opening, hold Alt to not force show the picker.       │
│ Bypass key                          [Shift][Ctrl][Alt][Super]│
└──────────────────────────────────────────────────────────────┘
Interface
┌──────────────────────────────────────────────────────────────┐
│ Frontend                                     [Automatic ▾]   │
│   Automatic uses GNOME on GNOME and KDE everywhere else.     │
│   Applies to windows opened after the change.                │
└──────────────────────────────────────────────────────────────┘
```

## URL expansion

| ID | Requirement | Value in design | Evidence |
|---|---|---|---|
| ADV-01 | Group **URL Expansion**: switch row **Expand redirect and short URLs** with a **Configure…** button beside the switch. | on | Specified |
| ADV-02 | **Configure…** opens the URL expansion sheet ([DLG-EXP](17-dialogs.md#url-expansion-sheet)): which redirect wrappers and short-link services Wye expands, plus timeouts. | — | Specified (button), Proposed (sheet) |

## URL transformation

| ID | Requirement | Value in design | Evidence |
|---|---|---|---|
| ADV-03 | Group **URL Transformation**: switch row **Transform all URLs before matching rules** with an **Edit Script…** button beside the switch. Subtitle: "Runs after URL expansion and tracking removal." | off | Specified |

## Script editor

| ID | Requirement | Evidence |
|---|---|---|
| ADV-04 | **Edit Script…** here and in the rule editor ([RUL-25](08-rules.md#rule-editor-sheet)) opens the script editor ([16-script-editor.md](16-script-editor.md)). | Specified (button), Proposed (editor) |

## Keyboard shortcuts

| ID | Requirement | Evidence |
|---|---|---|
| ADV-05 | Group **Keyboard Shortcuts**, shortcut recorder row ([BLK-16](03-settings-window.md#shared-building-blocks)) **Toggle menu**: opens or closes the tray menu ([TRAY-08](01-tray-menu.md)). Unset by default. | Specified |
| ADV-06 | Recorder row **Open URL from clipboard with primary browser** ([IN-03](11-url-pipeline.md#entry-points)). Unset by default. | Specified |
| ADV-07 | Recorder row **Open URL from clipboard with alternative browser** ([IN-04](11-url-pipeline.md#entry-points)). Unset by default. | Specified |
| ADV-08 | A dimmed note under the group points to the other key settings, with links that switch pages: "Picker keys are on the Picker page; the alternative browser key is on the Browsers page." The full key inventory is in [15-keyboard.md](15-keyboard.md). | Proposed |

Every recorder can be cleared (a clear button appears once a shortcut is set). How
recorders behave per desktop, and the CLI commands for compositors without a shortcuts
portal, are in [15-keyboard.md](15-keyboard.md#global-shortcuts).

## History

| ID | Requirement | Value in design | Evidence |
|---|---|---|---|
| ADV-09 | Group **History**: switch row **Store history of the last 100 opened links**. Proposed: a **Show…** button beside the switch opens the history window ([DLG-HIS](17-dialogs.md#history-window)); the tray's "More" submenu offers the same. Turning the switch off asks whether to delete the stored history. | off | Specified (switch), Proposed (button, off behaviour) |

## Miscellaneous

| ID | Requirement | Value in design | Evidence |
|---|---|---|---|
| ADV-10 | Group **Miscellaneous**: switch row **Force show picker when opening from browser extension**, subtitle "When opening, hold \<bypass key\> to not force show the picker." With the switch on, links from the browser extension ([IN-05](11-url-pipeline.md#entry-points)) always open the picker, whatever the rules say, unless the bypass key is held. | on | Specified |
| ADV-11 | Modifier chooser row **Bypass key** ([BLK-18](03-settings-window.md#shared-building-blocks)) under ADV-10: the modifier that skips the forced picker. Default **Alt**. | — | Proposed |

## Interface

| ID | Requirement | Value in design | Evidence |
|---|---|---|---|
| ADV-12 | Group **Interface**: popup row **Frontend** ([BLK-04](03-settings-window.md#shared-building-blocks) for plain values) with **Automatic**, **KDE** and **GNOME**, subtitle "Automatic uses GNOME on GNOME and KDE everywhere else. Applies to windows opened after the change." It picks the frontend that shows the picker, the tray-menu popup and the windows (`advanced.frontend`: `auto`, `kde`, `gnome`). **Automatic**: in a GNOME Shell session (`org.gnome.Shell` runs; Budgie and GNOME Flashback, which also name GNOME in `XDG_CURRENT_DESKTOP`, are not one) the Shell extension's picker while the extension runs, else the GTK host's, and the GTK host's windows; elsewhere `wye-ui`. **KDE**: `wye-ui` on every desktop. **GNOME**: on every desktop, the Shell extension's picker while the extension runs, else the GTK host's; the GTK host's windows. It suits window managers, where Automatic means KDE. A frontend that is not installed, cannot be reached or does not serve the call hands it to the other frontend and logs a warning, so a link is never lost (a window host that answers with an error keeps the error). The choice applies to the next picker, tray popup or window; open ones stay where they are. The Nix modules set it with `programs.wye.frontend`. | Automatic | Proposed |
