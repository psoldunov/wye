# 17 · Secondary dialogs and windows

Surfaces that the settings pages and tray menu open: the app chooser, the URL expansion
sheet, the history window, the rule tester and the About window.

## App chooser

Used by "Other…" in every target menu ([TGT-06](05-browsers.md#target-menu)), by "+" in
the shown browsers sheet ([SHOWN-05](05-browsers.md#shown-browsers-sheet)) and by "+"
under Source Apps ([RUL-16](08-rules.md#rule-editor-sheet)).

```
Choose App
[🔍 Search apps                                   ]
Recent Sources                       (Source Apps only)
  ◉ Slack                                           ☐
  ◉ Thunderbird                                     ☐
Browsers
  ◉ Firefox                                         ☐
All Apps
  ◉ Discord                 Flatpak                 ☐
  …
[Browse…]                                 [Cancel] [Add]
```

| ID | Requirement | Evidence |
|---|---|---|
| DLG-APP-01 | Modal sheet with a search entry at the top (focused on open), filtering by name, desktop ID and keywords. | Proposed |
| DLG-APP-02 | Sections: **Recent Sources** (only when choosing source apps: apps that sent links during this session, most recent first, kept in memory even when history is off), **Browsers**, **All Apps**. Rows: icon, name, and a dimmed packaging badge ("Flatpak", "Snap") where relevant. | Proposed |
| DLG-APP-03 | Single choice for targets (a click chooses and closes); multiple choice with checkboxes for source apps (**Add** confirms). | Proposed |
| DLG-APP-04 | **Browse…** picks an executable or a `.desktop` file for apps without an installed desktop entry. | Proposed |

## URL expansion sheet

Opened by **Configure…** ([ADV-02](10-advanced.md#url-expansion)).

```
URL Expansion
┌ ⊗ Redirect wrappers are unwrapped on your computer. Short links need one ┐
│   request to the short-link service to find where they lead.            │
└──────────────────────────────────────────────────────────────────────────┘
Redirect Wrappers
  Google search results (google.com/url)                          [on ]
  Facebook and Instagram (l.facebook.com, l.instagram.com)        [on ]
  Microsoft Safe Links (*.safelinks.protection.outlook.com)       [on ]
  Slack (slack-redir.net)                                         [on ]
  Steam (steamcommunity.com/linkfilter)                           [on ]
  YouTube (youtube.com/redirect)                                  [on ]
Short Links                                                          [+]
  bit.ly                                                          [on ]
  t.co                                                            [on ]
  tinyurl.com                                                     [on ]
  lnkd.in                                                         [on ]
  …
Behaviour
  Timeout                                             [ 1.5 s ⌃⌄]
  Maximum redirects                                   [   5   ⌃⌄]
  Notify when a link cannot be expanded                           [off]
                                                              [Done]
```

| ID | Requirement | Evidence |
|---|---|---|
| DLG-EXP-01 | Group **Redirect Wrappers**: services whose real target sits in a query parameter. Wye unwraps them locally, without network access. One switch per service. | Proposed |
| DLG-EXP-02 | Group **Short Links**: short-link domains Wye resolves by following HTTP redirects. One switch per domain; "+" adds a custom domain, and custom domains can be removed. | Proposed |
| DLG-EXP-03 | Network resolution sends `HEAD` (falling back to `GET` without reading the body), sends no cookies, follows at most the configured number of redirects, and gives up after the timeout. Only enabled domains are ever contacted. | Proposed |
| DLG-EXP-04 | Group **Behaviour**: **Timeout** (0.5–5 s, default 1.5 s), **Maximum redirects** (1–10, default 5), **Notify when a link cannot be expanded** (default off). | Proposed |
| DLG-EXP-05 | The service list ships as data with Wye (like the web app catalogue) and must be verified entry by entry before release. | Proposed |

## History window

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../media/kde/screenshots/dark/history.png">
    <img src="../media/kde/screenshots/light/history.png" width="450" alt="The History window listing recent links with reason chips such as picker choice, rule, cleaned and expanded">
  </picture>
</p>

<p align="center"><em>As implemented on KDE Plasma.</em></p>

Opened from the tray "More" submenu and from **Show…** on the Advanced page
([ADV-09](10-advanced.md#history)).

```
History                                     [🔍]        [Clear History]
┌──────────────────────────────────────────────────────────────────────┐
│ ◉  github.com/example/repo/pull/42                                   │
│    from Slack · Firefox · 14:32 · cleaned                            │
│ ◉  meet.google.com/abc-defg-hij                                      │
│    from Thunderbird · Work (Chrome) · 14:05 · rule “Meetings”        │
└──────────────────────────────────────────────────────────────────────┘
```

| ID | Requirement | Evidence |
|---|---|---|
| DLG-HIS-01 | A normal window, about 560 × 480 px, resizable. Header: title, search toggle, **Clear History** (asks for confirmation). | Proposed |
| DLG-HIS-02 | Rows, newest first: target icon; the final link (host emphasised, rest dimmed, middle-truncated); a second line with source app, target, time, and why (rule name, web app mapping, fallback, picker choice) plus "cleaned" or "expanded" when the link was changed. The original link is in the row's tooltip. | Proposed |
| DLG-HIS-03 | Double-click or Enter opens the link through the picker. Context menu: **Open in Picker**, **Open in \<target\> Again**, **Copy Link**, **Copy Original Link**, **Create Rule…** (rule editor pre-filled with the host and source app), **Delete Entry**. | Proposed |
| DLG-HIS-04 | Empty states: "No History" / "Links you open appear here." When history is off: "History Is Off" with a **Turn On** button. | Proposed |

## Rule tester

Opened from the Rules page "⋯" menu ([RUL-02](08-rules.md#rules-page)) and from **Test** in
the rule editor.

```
Test Rules
  Link          [https://bit.ly/abc123                        ]
  Source app    ◉ Slack                                        ⌃⌄
  Held keys     Shift  Ctrl  Alt  Super
Steps
  Expanded      https://github.com/example/repo?utm_source=x
  Cleaned       https://github.com/example/repo
  Transformed   unchanged
  Matched       rule “GitHub in Firefox” (before built-in rules)
  Opens in      ◉ Firefox
                                                          [Done]
```

| ID | Requirement | Evidence |
|---|---|---|
| DLG-TST-01 | Inputs: **Link**, optional **Source app**, **Held keys** (modifier chooser). Results update as the inputs change. Nothing opens. | Proposed |
| DLG-TST-02 | **Steps** lists each pipeline stage ([PIPE](11-url-pipeline.md#processing-order)) that changed something or decided the target, in order, ending with the final target and its options (private, profile, background, new window). Network expansion runs for real; a "Skip network" switch avoids it. | Proposed |
| DLG-TST-03 | Clicking the matched rule opens it in the rule editor. | Proposed |

## About window

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../media/kde/screenshots/dark/about.png">
    <img src="../media/kde/screenshots/light/about.png" width="450" alt="The About window of Wye">
  </picture>
</p>

<p align="center"><em>As implemented on KDE Plasma.</em></p>

| ID | Requirement | Evidence |
|---|---|---|
| DLG-ABT-01 | Opened from the tray "More" submenu. GNOME: `AdwAboutDialog`; KDE: `Kirigami.AboutPage` from KAboutData. Name, icon, version, website, issue tracker, licence, credits. | Proposed |
| DLG-ABT-02 | A **Troubleshooting** section lists what Wye detected in this session: desktop and session type; default-browser status; tray mechanism; how the picker is placed; whether held modifiers, source apps, clipboard watching and global shortcuts are available and through which mechanism ([13](13-linux-platform.md#capability-matrix)). A **Copy** button copies it for bug reports. | Proposed |
