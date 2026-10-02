# 14 · Open questions and scope

## Pending decisions

Each has a proposal in the spec; the proposal stands until someone decides otherwise.

| # | Decision | Proposal | Where |
|---|---|---|---|
| 1 | Toolkit strategy: one toolkit on every desktop (GTK 4 + libadwaita), or one frontend per desktop like Token Station (GNOME Shell extension + libadwaita, Plasma/Kirigami, SNI fallback) | Per-desktop frontends over a Rust service: the GNOME extension is needed anyway for the picker, held keys and clipboard on GNOME. Built: KDE (`wye-ui`) and GNOME (Shell extension plus the GTK host `wye-gtk`); [ADV-12](10-advanced.md#interface) lets the user pick either on any desktop, Automatic choosing by session | [03](03-settings-window.md#native-control-mapping), [13](13-linux-platform.md#architecture-precedent), [ADV-12](10-advanced.md#interface) |
| 2 | Held-modifier detection on KDE Plasma and wlroots Wayland sessions | Prototype the transient layer-shell surface; fall back to disabling modifier features with an explanation | [13](13-linux-platform.md#capability-matrix), [KEY-06](15-keyboard.md#controls) |
| 3 | Picker placement on KDE Plasma and wlroots Wayland sessions | Centre on the active screen; investigate a KWin script for the pointer position | [02](02-picker.md#linux-notes) |
| 4 | Script language | JavaScript via QuickJS (`rquickjs`) | [16](16-script-editor.md#script-api) |
| 5 | Firefox-based classic profiles (`profiles.ini`) | Support them alongside profile groups | [BRW-05](05-browsers.md), [DISC-07](05-browsers.md#discovery-and-launching) |
| 6 | Private-window targets in the shown browsers list | Yes | [SHOWN-02](05-browsers.md#shown-browsers-sheet) |
| 7 | Picker available in every target menu | Yes | [TGT-07](05-browsers.md#target-menu) |
| 8 | Linux web app catalogue, and whether installing a desktop app changes its mapping | Verify the candidate list; opt-in only | [06](06-apps.md#service-catalogue), [APP-09](06-apps.md) |
| 9 | Configuration format and location | TOML under `$XDG_CONFIG_HOME/wye/` | [12](12-data-model.md#storage) |
| 10 | Packaging | Nix flake + home-manager module and AppImage first; Flatpak later and reduced | [13](13-linux-platform.md#packaging) |
| 11 | Tracking-parameter dataset and licence | ClearURLs rules if the licence fits, plus a built-in list | [EXT-11](09-extras.md#behaviour) |
| 12 | Local HTML files | Opt-in switch, default off | [DEF-07](11-url-pipeline.md#default-browser-registration) |
| 13 | Clipboard shortcuts: full pipeline or straight to the browser | Full pipeline; the alternative variant behaves like the held key | [IN-03, IN-04](11-url-pipeline.md#entry-points) |
| 14 | "Toggle menu" shortcut where the tray host cannot be asked to open the menu | Show the same menu as a Wye popup at the picker's position | [TRAY-08](01-tray-menu.md), [01 Linux notes](01-tray-menu.md#linux-notes) |
| 15 | App ID | `dev.soldunov.wye` | [DEF-01](11-url-pipeline.md#default-browser-registration) |
| 16 | Default alternative-browser key | Shift | [BRW-03](05-browsers.md) |

## Items to verify

Facts this spec relies on that must be confirmed against current releases:

- URL schemes and Flatpak IDs in the web app catalogue ([06](06-apps.md#service-catalogue)).
- Redirect-wrapper and short-link list ([DLG-EXP-05](17-dialogs.md#url-expansion-sheet)).
- Firefox profile-group storage format and per-browser profile locations ([DISC-07](05-browsers.md#discovery-and-launching)).
- GlobalShortcuts portal support per desktop version, and data-control support per
  compositor ([13](13-linux-platform.md#capability-matrix)).
- Plasma's own default-browser setting ([DEF-02](11-url-pipeline.md#default-browser-registration)).
- Native-messaging hosts for Flatpak browsers ([BEXT-04](18-onboarding.md#browser-extension)).
- Odesli (song.link) API endpoint and terms ([EXT-15](09-extras.md#behaviour)).

## Out of scope

Features that depend on mechanisms Linux does not have. They are not replicated:

- A rule option that limits a rule to links received over Apple's AirDrop. Links from a
  phone through KDE Connect or GSConnect are handled with Source Apps instead
  ([RUL-26](08-rules.md#rule-editor-sheet)).
- A system share-sheet extension. Linux has no cross-desktop share menu.
- Operating-system privacy prompts for reading browser profile data. Native Linux
  packages need no permission; a Flatpak build is a packaging question.
- Built-in mappings for apps that exist only on macOS (App Store, Apple Music).
- The Fn / Globe key as a modifier. Linux software cannot see it; the
  alternative-browser key is configurable instead ([BRW-03](05-browsers.md)).
- Swipe-to-delete in lists. Rules are deleted with a button, the context menu or the
  Delete key ([RUL-06](08-rules.md#rules-page)).
