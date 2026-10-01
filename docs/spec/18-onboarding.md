# 18 · First run, default-browser status and browser extension

## First run

A small window that walks through what Wye needs before it is useful. It opens on first
start, and later from the tray "More" submenu (**Set Up Wye…**).

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../media/kde/screenshots/dark/first-run.png">
    <img src="../media/kde/screenshots/light/first-run.png" width="400" alt="The first-run welcome page of Wye">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../media/kde/screenshots/dark/first-run-browsers.png">
    <img src="../media/kde/screenshots/light/first-run-browsers.png" width="400" alt="The first-run page where the user chooses browsers">
  </picture>
</p>

<p align="center"><em>As implemented on KDE Plasma.</em></p>

```
┌───────────────────────── Welcome to Wye ─────────────────────────┐
│                          [ Wye icon ]                            │
│  Wye opens every link in the browser you want. It becomes your   │
│  default browser and forwards each link to the right place.      │
│                                                   [Get Started]  │
└──────────────────────────────────────────────────────────────────┘
```

| ID | Step | Content | Evidence |
|---|---|---|---|
| ONB-01 | Welcome | Icon, the two-sentence explanation above, **Get Started**. | Proposed |
| ONB-02 | Default browser | "Make Wye your default browser" with the current default named ("Currently: Firefox"). **Make Default** ([DEF-02](11-url-pipeline.md#default-browser-registration)) turns into a checkmark and "Wye is your default browser" on success. **Skip** is allowed; the step explains that Wye then only sees links from the clipboard and the browser extension. | Proposed |
| ONB-03 | Browsers | **Primary browser** target popup, pre-set to the Picker; the previous default browser is listed first in the menu. A checklist of detected browsers and profiles for the picker, with the first six browsers pre-checked (no profiles, no private windows). Hotkeys are left to the shown browsers sheet. | Proposed |
| ONB-04 | Desktop integration | **Launch at login** switch (on). On GNOME, when Wye's Shell extension is not enabled: "Enable GNOME Shell integration" with an **Enable** button, and a line on what it adds (tray icon, picker at the pointer, held keys, clipboard features). On other desktops without a tray host: a note that the tray icon needs one. | Proposed |
| ONB-05 | Browser extension (optional) | Why the extension exists (links clicked inside a browser never leave it) and links to install it for Chromium-based and Firefox-based browsers. **Done** closes the window and opens nothing else. | Proposed |
| ONB-06 | Navigation | Back button on every step after the first; progress dots at the bottom. GNOME: `AdwNavigationView` in an `AdwDialog`; KDE: a Kirigami page stack in a dialog. Closing the window early counts as done, and the General page keeps showing the default-browser state. | Proposed |

## Default-browser status

| ID | Requirement | Evidence |
|---|---|---|
| ONB-10 | General page, group **Default Browser** ([GEN-05](04-general.md)): a row "Wye is your default browser" with a checkmark, or "\<App\> is your default browser" with a warning icon and a **Make Default** button. When Wye is the default, the row's button is **Stop Being Default**, which restores the remembered previous browser ([DEF-05](11-url-pipeline.md#default-browser-registration)). Below it: switch **Also open local HTML files** ([DEF-07](11-url-pipeline.md#default-browser-registration)). | Proposed |
| ONB-11 | When another app takes over (DEF-03): the notification described there, a warning overlay on the tray icon (StatusNotifierItem `OverlayIconName`, or an emblem in the GNOME extension), and a first item in the tray menu, **Make Wye Default Browser**, until fixed or dismissed with "Keep \<App\>". | Proposed |

## Browser extension

A companion extension for Chromium-based and Firefox-based browsers. It exists because
a link clicked inside a browser is opened by that browser and never reaches Wye.

| ID | Requirement | Evidence |
|---|---|---|
| BEXT-01 | Link context menu: **Open Link with Wye**. Page context menu and toolbar button: **Open Page with Wye**. | Specified (extension exists), Proposed (UI) |
| BEXT-02 | A browser-level keyboard shortcut for "Open Page with Wye", changeable in the browser's own extension-shortcuts page. | Proposed |
| BEXT-03 | Options page: **Close the tab after sending the page to Wye** (default off). | Proposed |
| BEXT-04 | The extension talks to Wye through native messaging. Wye installs the native-messaging host manifest for every detected Chromium-based and Firefox-based browser, per browser config directory (Flatpak browsers need manifests inside their sandboxed config and a way to reach Wye; verify). If Wye is not running, the host starts it. | Proposed |
| BEXT-05 | Links from the extension enter the pipeline as [IN-05](11-url-pipeline.md#entry-points); the sending browser is the source app, and held modifiers come from the click event, so they work even where the session cannot report them. | Proposed |
| BEXT-06 | When the native host is missing, the extension's toolbar popup says so and links to setup instructions. | Proposed |
