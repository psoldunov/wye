# 08 · Rules page and rule editor

Rules are the user's own routing entries. A rule matches on the link (URL matchers), on
where it was clicked (source apps), or both, and sends it to a target with options.

## Rules page

| ID | Requirement | Evidence |
|---|---|---|
| RUL-01 | With no rules, the page shows an empty state ([BLK-12](03-settings-window.md#shared-building-blocks)) inside the list card: an icon, title "No Rules", text "A rule lets you open a specific app based on the URL and source app", and an **Add Rule…** action. When there are rules, the hint "Click a rule to edit it. Drag to reorder." shows below the list. | Specified (empty state), wording adapted |
| RUL-02 | List toolbar ([BLK-13](03-settings-window.md#shared-building-blocks)) at the bottom of the card: **Add Rule…** opens the rule editor for a new rule; **Test Rules…** opens the rule tester; "⋯" opens a menu: **Test Rules…** ([DLG-TST](17-dialogs.md#rule-tester)), *separator*, **Import Rules…** (adds rules from a Wye rules file at the end of the list), **Export Rules…** (all rules, with their scripts, to one file), *separator*, **Delete All Rules…** (asks for confirmation). | Specified (toolbar), Proposed (menu) |
| RUL-03 | Rules are evaluated top to bottom; the first matching rule wins. | Specified |
| RUL-04 | Rules can be reordered by dragging, with **Move Up** and **Move Down** in the row's context menu, and with Alt+Up and Alt+Down on the focused row (which keeps the focus, so the keys can be pressed again). | Expected (dragging), Proposed (menu, keys) |
| RUL-05 | Clicking a rule opens it in the rule editor. | Specified |
| RUL-06 | A rule can be deleted from its row (delete button or context menu, which also has **Edit…**, **Duplicate**, **Move Up** and **Move Down**) and with the Delete key. Deleting shows an undo toast. | Specified (delete), Proposed (Linux interaction) |
| RUL-07 | Row layout: drag handle; rule name; under it a dimmed one-line summary ("github.com, gitlab.com · from Slack · Shift"); at the right the target's icon and name, a switch that turns the rule off without deleting it, and a delete button. A turned-off rule is dimmed and skipped by the pipeline. | Proposed |

## Rule editor sheet

A modal sheet ([BLK-11](03-settings-window.md#shared-building-blocks)) about as large as the
settings window. The body scrolls; the footer note and the button bar stay pinned.

```
New Rule
┌───────────────────────────────────────────────────────┐
│ ⊗ Rules are matched in order from top to bottom …     │
└───────────────────────────────────────────────────────┘
┌───────────────────────────────────────────────────────┐
│ Name                                          [     ] │
│ Open in                     ☰ Default (Picker)    ⌃⌄  │
└───────────────────────────────────────────────────────┘
URL Matchers [+]
The link must match one of these patterns
┌ No Matchers ──────────────────────────────────────────┐
Source Apps [+]
The link must have been clicked in one of these apps
┌ No Source Apps ───────────────────────────────────────┐
Advanced
┌───────────────────────────────────────────────────────┐
│ Open in background (?)                          [off] │
│ Force new window (?)                            [off] │
│ Only when keys are held      Shift  Ctrl  Alt  Super  │
│ Run                         before built-in rules ⌃⌄  │
│ Transform URL                  [Edit Script…]   [off] │
└───────────────────────────────────────────────────────┘
                                          [Delete Rule]
─────────────────────────────────────────────────────────
If you specify both types, at least one of the URL matchers
AND one of the source apps must match.
─────────────────────────────────────────────────────────
(?) ←                          [Test…] [Cancel] [Save]
```

| ID | Requirement | Evidence |
|---|---|---|
| RUL-10 | Sheet title "New Rule" for a new rule; "Edit Rule" when editing. | Specified (new), Expected (edit) |
| RUL-11 | Dismissible callout: "Rules are matched in order from top to bottom of the list." / "The scheme (`https://`) and `www.` are removed from the URL before matching, so you do not need to include those in the “Match” field." / "Click the (?) button for more info." (three paragraphs). | Specified |
| RUL-12 | Card with a text entry row **Name** (focused when the sheet opens) and a target popup row **Open in** (default "Default (\<primary\>)"). | Specified |
| RUL-13 | Section **URL Matchers** with a "+" button, subtitle "The link must match one of these patterns", and a list card that shows "No Matchers" when empty ([BLK-14](03-settings-window.md#shared-building-blocks)). | Specified |
| RUL-14 | Matcher row: a kind popup, a **Match** entry (placeholder shows an example for the chosen kind, e.g. `github.com`), and a remove button. "+" adds an empty row and focuses its entry. Kinds: **Domain** (the host and its subdomains; default), **Starts with**, **Contains**, **Wildcard** (`*` matches any run of characters), **Regular expression**. Invalid regular expressions are flagged inline with the error. Examples per kind are in [19](19-help-texts.md#rule-editor). | Specified (field name "Match"), Proposed (row) |
| RUL-15 | Before matching, Wye removes the scheme and a leading `www.` from the link. Proposed: host comparison ignores case; path comparison respects case. | Specified (normalisation), Proposed (case) |
| RUL-16 | Section **Source Apps** with a "+" button, subtitle "The link must have been clicked in one of these apps", and a list card that shows "No Source Apps" when empty. "+" opens the app chooser; each row shows the app's icon and name and a remove button. | Specified (section), Expected (chooser, rows) |
| RUL-17 | Match logic: within each list, any one entry is enough; if both lists have entries, one URL matcher **and** one source app must match; an empty list does not constrain. Stated in a pinned footer note: "If you specify both types, at least one of the URL matchers **AND** one of the source apps must match." | Specified |
| RUL-18 | **Save** stays disabled until the rule is valid. Proposed validity: a non-empty name, at least one condition (URL matcher, source app or held keys), and every matcher valid. A message above the fields says why Save is disabled; for a new rule it appears after the first edit. | Specified (disabled), Proposed (conditions) |
| RUL-19 | Button bar: help "?" at the left (opens the rules help dialog, [19](19-help-texts.md#rule-editor)), an orange arrow next to it pointing at the help button, **Cancel** and **Save** at the right. Proposed: the arrow is a first-use hint and disappears once help has been opened; a **Test…** button before Cancel opens the rule tester pre-filled with this rule's first matcher ([DLG-TST](17-dialogs.md#rule-tester)). | Specified (bar, arrow), Proposed (arrow behaviour, Test…) |
| RUL-20 | **Cancel** discards changes. **Save** stores the rule and closes the sheet; a new rule is added at the bottom of the list. | Expected (save/cancel), Proposed (position) |
| RUL-21 | Section **Advanced**, one card with the rows RUL-22, RUL-23, RUL-27, RUL-24 and RUL-25 in that order. | Specified (RUL-22 to RUL-25), Proposed (RUL-27 position) |
| RUL-22 | Switch row **Open in background** with help button: the target opens without taking focus ([LAUNCH-04](05-browsers.md#discovery-and-launching)). | Specified |
| RUL-23 | Switch row **Force new window** with help button: open in a new browser window instead of a tab ([LAUNCH-05](05-browsers.md#discovery-and-launching)). Disabled when "Open in" is not a browser that supports it (disabled in the design while "Open in" is Default). | Specified |
| RUL-24 | Popup row **Run**: "before built-in rules" (default) or "after built-in rules", relative to the Apps page mappings ([PIPE-07 to PIPE-09](11-url-pipeline.md#processing-order)). | Specified (first value), Expected (second value) |
| RUL-25 | Row **Transform URL** with an **Edit Script…** button and a switch. When on and the rule matches, the rule's script rewrites the link before it opens. The script editor is the same as for the global transform ([ADV-04](10-advanced.md#script-editor)). | Specified (row), Expected (behaviour) |
| RUL-26 | Links shared from a phone through KDE Connect or GSConnect are matched like any other source app (add the KDE Connect or GSConnect app under Source Apps). | Proposed |
| RUL-27 | Modifier chooser row ([BLK-18](03-settings-window.md#shared-building-blocks)) **Only when keys are held**: when set, the rule matches only while exactly these modifiers are held ([KEY-05](15-keyboard.md#controls)). This is a third condition, combined with AND like the other two. If the chosen set equals the alternative-browser key, the row shows a warning that the alternative browser takes precedence ([PIPE-06](11-url-pipeline.md#processing-order)). | Proposed |
| RUL-28 | When editing an existing rule, a destructive **Delete Rule** button sits at the end of the body. It closes the sheet and removes the rule, with the same undo toast as RUL-06. | Proposed |

Source-app detection on Linux is covered in [13](13-linux-platform.md).
