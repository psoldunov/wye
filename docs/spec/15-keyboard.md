# 15 · Keyboard shortcuts, hotkeys and modifier keys

Every key Wye reacts to is configurable, from Settings and from the configuration file.
The only fixed keys are the desktop-standard ones (see [KEY-50](#fixed-keys)).

## Inventory

| Binding | Kind | Default | Set on | Config key |
|---|---|---|---|---|
| Toggle menu | global shortcut | unset | Advanced ([ADV-05](10-advanced.md#keyboard-shortcuts)) | `shortcuts.toggle-menu` |
| Open URL from clipboard with primary browser | global shortcut | unset | Advanced ([ADV-06](10-advanced.md#keyboard-shortcuts)) | `shortcuts.clipboard-primary` |
| Open URL from clipboard with alternative browser | global shortcut | unset | Advanced ([ADV-07](10-advanced.md#keyboard-shortcuts)) | `shortcuts.clipboard-alternative` |
| Alternative-browser key | held modifiers | Shift | Browsers ([BRW-03](05-browsers.md)) | `browsers.alternative-key` |
| Bypass key (skip forced picker) | held modifiers | Alt | Advanced ([ADV-11](10-advanced.md#miscellaneous)) | `advanced.bypass-key` |
| Rule held keys | held modifiers, per rule | none | Rule editor ([RUL-27](08-rules.md#rule-editor-sheet)) | `rules[].held-keys` |
| Target hotkeys | single key per shown target | unassigned | Shown browsers sheet ([SHOWN-04](05-browsers.md#shown-browsers-sheet)) | `browsers.shown[].hotkey` |
| Hotkey scheme | choice | Assigned per browser | Picker ([KEY-10](#hotkey-scheme)) | `picker.hotkeys` |
| Picker actions | keys, several per action | see [KEY-22](#picker-keys-sheet) | Picker keys sheet | `picker.keys.*` |
| Picker held-modifier actions | held modifiers | Shift / Ctrl / Alt | Picker keys sheet | `picker.keys.*-modifier` |

## Controls

| ID | Requirement | Evidence |
|---|---|---|
| KEY-01 | **Modifier chooser** ([BLK-18](03-settings-window.md#shared-building-blocks)): four linked toggle buttons, **Shift**, **Ctrl**, **Alt**, **Super**. The binding is the set of pressed buttons; none pressed means off. Left and right modifiers count the same. A chooser is used instead of a recorder because modifier-only combinations cannot be recorded reliably. | Proposed |
| KEY-02 | **Key recorder** ([BLK-16](03-settings-window.md#shared-building-blocks)): click, the button reads "Press keys…", the next key combination is stored. Escape cancels recording, Backspace clears the binding. Picker actions accept several bindings: each shows as a chip with a remove "×", followed by a "+" chip that records another. | Specified (recorder), Proposed (multiple bindings) |
| KEY-03 | Bindings are shown in desktop style ("Ctrl+Shift+O") and stored in the same form with XKB key names (`"Ctrl+Shift+o"`, `"Return"`, `"KP_Enter"`, `"space"`). | Proposed |
| KEY-04 | Every group of bindings has **Reset to Defaults**. | Proposed |
| KEY-05 | Held-modifier matching is exact: a binding of Shift fires only when Shift and no other modifier is held. So a rule on Ctrl+Shift never collides with an alternative-browser key of Shift. | Proposed |
| KEY-06 | Where the session cannot report held modifiers ([13](13-linux-platform.md#capability-matrix)), every modifier chooser is disabled with the subtitle "Not available in this session" and a help button that explains why. | Proposed |

## Hotkey scheme

| ID | Requirement | Evidence |
|---|---|---|
| KEY-10 | Picker page, group **Keys**, popup row **Target hotkeys** ([PKS-08](07-picker-settings.md)): **Assigned per browser** (default; the keys set in the shown browsers sheet), **Numbers 1–9** (by position), **Letters from names** (each target gets the first free letter of its name, automatically), **Off**. The picker shows whichever scheme is active above each tile. | Proposed |
| KEY-11 | Target hotkeys are single keys without modifiers, matched case-insensitively. Proposed for non-Latin layouts: a hotkey matches either the character the active layout produces or the Latin character on the same physical key. | Proposed |
| KEY-12 | A target hotkey cannot be a key used by a picker action; the sheet refuses it and names the action ([KEY-21](#picker-keys-sheet)). | Proposed |
| KEY-13 | Held modifiers change how the chosen target opens, whether the user clicks, presses Enter or presses a hotkey: **private window** (default Shift), **in background** (default Ctrl), **new window** (default Alt). While a modifier is held, the picker shows a hint line ("Open in a private window") and dims targets that cannot open that way. | Proposed |

## Picker keys sheet

Opened from the Picker page, group **Keys**, row **Picker keys** → **Customize…**.

```
Picker Keys
Actions
  Open selected target      [Return] [KP_Enter] [Space] [+]
  Cancel                    [Escape] [+]
  Select next               [Right] [Tab] [+]
  Select previous           [Left] [Shift+Tab] [+]
  Select first              [Home] [+]
  Select last               [End] [+]
  Copy link and close       [Ctrl+C] [+]
  Show more targets         [Menu] [+]
  Create rule from link…    [Ctrl+R] [+]
Hold while choosing
  Open in private window    [Shift] Ctrl  Alt  Super
  Open in background         Shift [Ctrl] Alt  Super
  Open in new window         Shift  Ctrl [Alt] Super
[Reset to Defaults]                              [Done]
```

| ID | Requirement | Evidence |
|---|---|---|
| KEY-20 | The sheet lists every picker action with its bindings (KEY-02) and the three held-modifier actions with modifier choosers (KEY-01). **Done** closes it; changes apply at once. | Proposed |
| KEY-21 | Conflicts: a key belongs to one action and cannot also be a target hotkey; the three held-modifier actions need different modifier sets. On conflict, the recorder shows "Already used by \<action or target\>" with a **Replace** button. | Proposed |
| KEY-22 | Defaults are the values in the drawing above. "Show more targets" opens the "⋯" menu ([PICK-08](02-picker.md)); "Create rule from link…" opens the rule editor pre-filled from the pending link ([PICK-31](02-picker.md#interaction)). | Proposed |

## Global shortcuts

| ID | Requirement | Evidence |
|---|---|---|
| KEY-40 | Global shortcut recorders register with the desktop: X11 uses a key grab; GNOME uses the GlobalShortcuts portal where available, else a keybinding registered by Wye's Shell extension; KDE Plasma uses the GlobalShortcuts portal or KGlobalAccel, so the shortcuts also appear, and can be changed, in System Settings → Shortcuts. When a portal owns the binding, Wye shows the binding the portal reports and **Change…** opens the portal's own dialog. | Proposed |
| KEY-41 | Where no mechanism exists (many wlroots compositors), each recorder row shows the CLI command to bind in the compositor's configuration, with a **Copy** button: `wye menu`, `wye clipboard`, `wye clipboard --alternative`. Example for Sway: `bindsym $mod+Shift+o exec wye clipboard`. | Proposed |

## Fixed keys

| ID | Requirement | Evidence |
|---|---|---|
| KEY-50 | Settings window keys follow desktop conventions and are not configurable: `Ctrl+,` (open), `Ctrl+W` and `Escape` (close), `Ctrl+Q` (quit Wye). | Specified (`Ctrl+,`, `Ctrl+Q`), Expected (rest) |
| KEY-51 | Tray menu accelerators (`P`, `1`–`9`) stay fixed, because tray hosts decide whether and how menu accelerators work. | Proposed |
