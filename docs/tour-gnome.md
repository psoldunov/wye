# A tour of Wye on GNOME

<p align="center">
  <img src="../data/icons/hicolor/scalable/apps/dev.soldunov.wye.svg" width="96" alt="Wye">
</p>

Every surface of Wye as it looks on GNOME Shell 50. The picker and the tray menu are drawn
by the Wye [GNOME Shell extension](../frontends/gnome-shell/README.md); Settings and the
other windows are GTK 4 and libadwaita, from the [`wye-gtk`](../crates/wye-gtk/README.md)
host. Switch GitHub to light or dark mode to see the other colour scheme. The browser
profiles, rules and links in the images are demo data. Back to the [README](../README.md);
the KDE Plasma tour is in [tour.md](tour.md).

## The picker

The picker opens at the pointer when no rule decides. Each tile is a browser or a browser
profile, with its hotkey above it and the link's source app and address below.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/picker.png">
    <img src="media/gnome/screenshots/light/picker.png" width="905" alt="The Wye picker on GNOME Shell: browser and profile tiles with hotkeys, and the link it is about to open">
  </picture>
</p>

Right-click a tile for the other ways to open the link. The "…" button opens the overflow
menu with the Open In submenu, Copy Link, Create Rule… and Settings….

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/picker-tile-menu.png">
    <img src="media/gnome/screenshots/light/picker-tile-menu.png" width="450" alt="The right-click menu of a picker tile: Open, Open in Private Window, Open in New Window, Open in Background, Make Primary Browser">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/picker-more.png">
    <img src="media/gnome/screenshots/light/picker-more.png" width="510" alt="The picker overflow menu with the Open In submenu, Copy Link, Create Rule and Settings">
  </picture>
</p>

## The tray

The tray menu in the GNOME Shell panel opens the clipboard link and chooses the primary
browser. Its More submenu holds History, Recent Links, Test Rules…, Rescan Browsers, Set Up
Wye…, Help and About.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/tray-menu.png">
    <img src="media/gnome/screenshots/light/tray-menu.png" width="326" alt="The Wye tray menu in the GNOME Shell panel: open URL from clipboard, the primary browser list, Settings, More and Quit">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/tray-more.png">
    <img src="media/gnome/screenshots/light/tray-more.png" width="463" alt="The tray menu with the More submenu open: History, Recent Links, Test Rules, Rescan Browsers, Set Up Wye, Help and About">
  </picture>
</p>

While history is on, Recent Links lists the last ten links you opened; choosing one opens it
in the picker again.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/tray-recent.png">
    <img src="media/gnome/screenshots/light/tray-recent.png" width="820" alt="The tray menu with More and its Recent Links submenu open: the last links opened, as host and path">
  </picture>
</p>

## Settings

Settings has seven pages: General, Browsers, Apps, Picker, Rules, Extras and Advanced.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/settings-general.png">
    <img src="media/gnome/screenshots/light/settings-general.png" width="455" alt="The Settings window on the General page">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/settings-browsers.png">
    <img src="media/gnome/screenshots/light/settings-browsers.png" width="455" alt="The Settings window on the Browsers page">
  </picture>
</p>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/settings-picker.png">
    <img src="media/gnome/screenshots/light/settings-picker.png" width="455" alt="The Settings window on the Picker page">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/settings-extras.png">
    <img src="media/gnome/screenshots/light/settings-extras.png" width="455" alt="The Settings window on the Extras page">
  </picture>
</p>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/settings-apps.png">
    <img src="media/gnome/screenshots/light/settings-apps.png" width="455" alt="The Settings window on the Apps page: web apps that can open in their desktop app or a chosen browser">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/settings-advanced.png">
    <img src="media/gnome/screenshots/light/settings-advanced.png" width="455" alt="The Settings window on the Advanced page">
  </picture>
</p>

Sheets open over the pages for the longer lists. Shown Browsers chooses which browsers and
profiles the picker offers, Picker Keys sets the hotkeys, and URL Expansion sets the
services and limits for expanding short links.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/shown-browsers.png">
    <img src="media/gnome/screenshots/light/shown-browsers.png" width="300" alt="The Shown Browsers sheet: the browsers and profiles the picker offers">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/picker-keys.png">
    <img src="media/gnome/screenshots/light/picker-keys.png" width="300" alt="The Picker Keys sheet: the hotkeys of the picker">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/url-expansion.png">
    <img src="media/gnome/screenshots/light/url-expansion.png" width="300" alt="The URL Expansion sheet of the Advanced page">
  </picture>
</p>

## Rules

Rules are listed top to bottom and the first match wins. A rule matches on the link, on the
source app, or on both.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/settings-rules.png">
    <img src="media/gnome/screenshots/light/settings-rules.png" width="600" alt="The Settings window on the Rules page">
  </picture>
</p>

The rule editor sets the target and the matchers. Test Rules… traces a link through link
cleaning, the transform script and the rules, and shows where it opens.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/rule-editor.png">
    <img src="media/gnome/screenshots/light/rule-editor.png" width="455" alt="The Edit Rule sheet for a rule, with its target and URL matchers">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/rule-tester.png">
    <img src="media/gnome/screenshots/light/rule-tester.png" width="455" alt="The Test Rules sheet tracing a link through cleaning, transform and rule match">
  </picture>
</p>

## Transform scripts

The global script and the optional script of each rule rewrite a link before it opens.
The editor shows the result for a test link as you type.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/script-editor.png">
    <img src="media/gnome/screenshots/light/script-editor.png" width="600" alt="The global transform script editor with a live test result">
  </picture>
</p>

## History and About

The History window lists recent links with the reason each one went where it did. About
shows the version, the licence and where to report a bug.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/history.png">
    <img src="media/gnome/screenshots/light/history.png" width="450" alt="The History window listing recent links with reason chips such as picker choice, rule, cleaned and expanded">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/about.png">
    <img src="media/gnome/screenshots/light/about.png" width="450" alt="The About window of Wye">
  </picture>
</p>

## First run

On first start Wye walks through making itself the default browser and choosing the
browsers to show.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/first-run.png">
    <img src="media/gnome/screenshots/light/first-run.png" width="400" alt="The first-run welcome page of Wye">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/gnome/screenshots/dark/first-run-browsers.png">
    <img src="media/gnome/screenshots/light/first-run-browsers.png" width="400" alt="The first-run page where the user chooses browsers">
  </picture>
</p>

## Regenerating the screenshots

The images come from a headless GNOME Shell session that runs this checkout's Wye with demo
data. [media/gnome/stage/README.md](media/gnome/stage/README.md) says what it needs and how
to run it.
