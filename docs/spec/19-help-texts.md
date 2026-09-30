# 19 · Help texts

Content for every help button ([BLK-08](03-settings-window.md#shared-building-blocks)).
Popovers stay short: two to four sentences, one example where it helps. Text in angle
brackets is filled in at runtime.

## Browsers page

**Alternative browser** ([BRW-02](05-browsers.md))

> Hold \<alternative-browser key\> while you open a link in another app, and Wye opens it
> in the alternative browser instead of following your rules. Use it as an escape hatch
> when a rule sends a link somewhere you do not want this time. Change the key in the row
> below.

When held keys are not available in the session, add:

> This session does not tell apps which keys are held, so the key only works for links
> from the Wye browser extension.

**Browser profiles** ([BRW-05](05-browsers.md))

> Wye finds the profiles of Chromium-based browsers (Chrome, Chromium, Brave, Vivaldi,
> Edge) and Firefox-based browsers (Firefox, Zen, LibreWolf, Floorp). Profiles appear in
> every browser menu and can be added to the picker. If a profile is missing, click
> Rescan.

## Rule editor

**Open in background** ([RUL-22](08-rules.md#rule-editor-sheet))

> Open the link without switching to the browser, so you can keep working. Some
> desktops always bring the browser to the front when it was not running yet.

**Force new window** ([RUL-23](08-rules.md#rule-editor-sheet))

> Open the link in a new browser window instead of a new tab in the current window.
> Available when "Open in" is a browser that supports it.

**Rules help** (the "?" in the rule editor's button bar, [RUL-19](08-rules.md#rule-editor-sheet))

This one opens a dialog, not a popover, with these sections:

1. **How rules work.** Rules are checked from the top of the list down; the first rule
   that matches decides where the link opens. A rule can check the link, the app it was
   clicked in, and the keys held while clicking.
2. **URL matchers.** Wye removes `https://` and a leading `www.` before matching.

   | Kind | Pattern | Matches | Does not match |
   |---|---|---|---|
   | Domain | `github.com` | `github.com/x`, `gist.github.com/y` | `notgithub.com` |
   | Starts with | `docs.google.com/spreadsheets` | `docs.google.com/spreadsheets/d/1` | `docs.google.com/document/d/1` |
   | Contains | `/pull/` | `github.com/a/b/pull/7` | `github.com/a/b/issues/7` |
   | Wildcard | `*.atlassian.net/browse/*` | `team.atlassian.net/browse/ABC-1` | `atlassian.net/wiki` |
   | Regular expression | `^meet\.google\.com/[a-z]{3}-` | `meet.google.com/abc-defg-hij` | `meet.google.com/landing` |

3. **Source apps.** The rule matches only links opened from one of these apps. Some apps
   (for example sandboxed Flatpak apps) cannot always be identified; rules with source
   apps then do not match.
4. **Held keys.** The rule matches only while exactly these modifier keys are held.
5. **Before or after built-in rules.** Built-in rules are the mappings on the Apps page.
   "Before" lets a rule override them.
6. **Transform URL.** A script that rewrites the link when this rule matches. See the
   script editor's Reference.
7. **Testing.** Use **Test Rules…** in the Rules page menu to see which rule a link hits.

## Extras page

**Remove tracking parameters when opening links** ([EXT-01](09-extras.md))

> Removes parameters that only track where you came from, such as `utm_source`,
> `fbclid` and `gclid`. Parameters a page needs to work are kept.
> Example: `shop.example/item?id=7&utm_source=news` becomes `shop.example/item?id=7`.

## Advanced page

**Bypass key** ([ADV-11](10-advanced.md#miscellaneous))

> Hold this key while opening a link from the browser extension to follow your rules
> instead of showing the picker.

**Global shortcuts without a shortcuts service** ([KEY-41](15-keyboard.md#global-shortcuts))

> Your desktop does not let apps register shortcuts. Bind the command in your
> compositor's configuration instead, for example:
> `bindsym $mod+Shift+o exec wye clipboard`

## Unavailable-feature explanations

Shown as the help text of a disabled control ([KEY-06](15-keyboard.md#controls),
[09 Linux notes](09-extras.md#linux-notes)).

> **Held keys**: This session does not tell apps which keys are held. On GNOME, enable
> Wye's Shell integration. Links from the Wye browser extension still carry held keys.

> **Clipboard features**: This session does not let apps watch the clipboard. On GNOME,
> enable Wye's Shell integration.
