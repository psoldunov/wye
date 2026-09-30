# 10 · Advanced page

URL expansion, the global transform script, global keyboard shortcuts, history, and the
browser-extension override.

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
