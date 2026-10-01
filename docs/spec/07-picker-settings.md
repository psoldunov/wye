# 07 · Picker page

Settings for the picker's appearance, its keys, and when it is skipped.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../media/kde/screenshots/dark/settings-picker.png">
    <img src="../media/kde/screenshots/light/settings-picker.png" width="600" alt="The Settings window on the Picker page on KDE Plasma">
  </picture>
</p>

<p align="center"><em>As implemented on KDE Plasma.</em></p>

## Layout

```
Appearance
┌───────────────────────────────────────────────────────┐
│ Icon size                ○ Small  ○ Medium  ● Large   │
│ Show browser names                              [on ] │
│ Show URL                                        [off] │
│ Show profile badge                              [on ] │
└───────────────────────────────────────────────────────┘
Behaviour
┌───────────────────────────────────────────────────────┐
│ Skip picker when screen is locked               [off] │
│   Links will then open in the alternative browser.    │
└───────────────────────────────────────────────────────┘
Keys
┌───────────────────────────────────────────────────────┐
│ Target hotkeys           Assigned per browser     ⌃⌄  │
│ Picker keys                            [Customize…]   │
└───────────────────────────────────────────────────────┘
┌───────────────────────────────────────────────────────┐
│ Preview                             [Preview Picker]  │
│   See the picker as it looks now. Choosing a target   │
│   in it opens nothing.                                │
└───────────────────────────────────────────────────────┘
```

## Requirements

| ID | Requirement | Value in design | Evidence |
|---|---|---|---|
| PKS-01 | Inline radio row **Icon size**: Small, Medium, Large ([PICK-11](02-picker.md)). | Large | Specified |
| PKS-02 | Switch row **Show browser names** ([PICK-10](02-picker.md)). | on | Specified |
| PKS-03 | Switch row **Show URL** ([PICK-09](02-picker.md)). | off | Specified |
| PKS-04 | Switch row **Show profile badge** ([PICK-06](02-picker.md)). | on | Specified |
| PKS-05 | Switch row **Skip picker when screen is locked**, subtitle "Links will then open in the alternative browser." | off | Specified |
| PKS-06 | **Preview Picker**: a row with a short explanation and the button, in a card of its own below the others. Opens the picker with a sample link; choosing a target in preview opens nothing and just closes the picker. | — | Specified (button), Proposed (behaviour) |
| PKS-07 | If PKS-05 applies but the alternative browser is also the Picker, Wye never drops the link: it holds it and shows the picker right after the screen unlocks. | — | Proposed |
| PKS-08 | Group **Keys**, popup row **Target hotkeys** ([KEY-10](15-keyboard.md#hotkey-scheme)). | Assigned per browser | Proposed |
| PKS-09 | Same group, button row **Picker keys** with **Customize…**, which opens the picker keys sheet ([KEY-20](15-keyboard.md#picker-keys-sheet)). | — | Proposed |

Use the "Value in design" column as the default values.

## Linux notes

- Lock state: the logind session's `LockedHint` property (`org.freedesktop.login1.Session`)
  where the screen locker sets it, else `org.freedesktop.ScreenSaver.GetActive` (Plasma
  and others) or `org.gnome.ScreenSaver.GetActive` (GNOME).
- Links can arrive while the screen is locked from scripts, scheduled jobs, or a phone
  sharing a link through KDE Connect or GSConnect.
