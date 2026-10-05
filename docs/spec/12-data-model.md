# 12 · Data model

Entities and settings the surfaces imply. Field names are descriptive, not a final
schema.

## Entities

**Target**, one of:

| Variant | Fields | Display name |
|---|---|---|
| `Picker` | — | "Picker" |
| `Default` | — | "Default (\<primary\>)"; only valid in web app mappings and rules |
| `App` | desktop ID | entry's `Name` |
| `Private` | browser desktop ID | "\<Browser\> (Private)" |
| `Profile` | browser desktop ID, profile ID (directory or profile-group ID) | profile name; "\<Profile\> (\<Browser\>)" where context is needed |
| `Custom` | desktop ID or executable path, added through "Other…" / "+" | entry's `Name` or file name |

Targets reference desktop IDs, never paths into `/nix/store`, `/var/lib/flatpak` or
similar, so they survive updates. A target whose app disappears is kept and shown as
missing ([APP-10](06-apps.md)); at runtime it falls back to the Picker.

**ShownEntry**: target, hotkey (optional key). The shown list is ordered.

**WebAppMapping**: service ID → target. Only non-Default mappings are stored.

**ServiceDefinition** (shipped catalogue, [APP-07](06-apps.md)): ID, name, URL patterns,
desktop-app identifiers, hand-over (pass-through or scheme translation).

**Rule**: ID, name, enabled, target, URL matchers, source apps, held keys
([RUL-27](08-rules.md#rule-editor-sheet)), open in background, force new window, run
position (`before` / `after` built-in rules), transform (enabled, script).

**UrlMatcher**: kind (`domain`, `prefix`, `contains`, `wildcard`, `regex`), pattern.

**SourceApp**: desktop ID (preferred) or executable name.

**KeyBindings** ([15](15-keyboard.md)): global shortcuts, alternative-browser key,
bypass key, picker action keys, picker held-modifier actions, hotkey scheme.

**HistoryEntry**: time, original URL, final URL, entry point, source app, target, matched
rule or mapping.

## Settings by page

| Page | Settings |
|---|---|
| General | launch at login; tray icon style (`primary-browser`, `wye`); show tray icon; open local HTML files |
| Browsers | primary target; alternative target; alternative-browser key; shown entries |
| Apps | web app mappings |
| Picker | icon size (`small`, `medium`, `large`); show names; show URL; show profile badge; skip when locked; hotkey scheme; picker keys |
| Rules | ordered rules |
| Extras | strip tracking on open; strip tracking on copy; strip `mailto:` on copy; force HTTPS; Songlink on copy |
| Advanced | expand URLs; expansion services and limits; global transform (enabled, script); global shortcuts; history enabled; force picker from extension; bypass key; frontend (`advanced.frontend`: `auto`, `kde`, `gnome`; default `auto`, an unknown value reads as `auto` with a warning; [ADV-12](10-advanced.md#interface)) |
| Internal | dismissed callouts; last settings page; onboarding done; previous default browser; extension host manifests removed by the user ([BEXT-04](18-onboarding.md)); browsers already seen by discovery ([SHOWN-09](05-browsers.md#shown-browsers-sheet)) |

## Storage

- One configuration file under `$XDG_CONFIG_HOME/wye/`, read by the Wye service and
  edited by every frontend, as in Token Station. Wye reloads it when it changes on disk,
  so hand edits and dotfile managers (home-manager, chezmoi) work. Proposed format:
  TOML.
- Scripts live next to it as separate files (`transform.js`, `rules/<rule-id>.js`) so
  they can be edited in any editor and kept in version control.
- History lives under `$XDG_STATE_HOME/wye/`, capped at 100 entries, never synced.
- Discovered browsers and profiles are cached under `$XDG_CACHE_HOME/wye/` and rebuilt
  when missing.

Illustrative configuration (not final):

```toml
[browsers]
primary = { picker = true }
alternative = { app = "firefox.desktop" }
alternative-key = ["Shift"]

[[browsers.shown]]
target = { app = "app.zen_browser.zen.desktop" }
hotkey = "a"

[[browsers.shown]]
target = { profile = { app = "google-chrome.desktop", id = "Profile 1" } }
hotkey = "c"

[apps]
google-meet = { profile = { app = "google-chrome.desktop", id = "Profile 1" } }

[[rules]]
name = "GitHub in Firefox"
target = { app = "firefox.desktop" }
url-matchers = [{ kind = "domain", pattern = "github.com" }]
source-apps = ["com.slack.Slack.desktop"]
run = "before"

[picker]
icon-size = "large"
hotkeys = "per-target"

[picker.keys]
open = ["Return", "KP_Enter", "space"]
cancel = ["Escape"]
private-modifier = ["Shift"]

[advanced]
frontend = "auto"
```
