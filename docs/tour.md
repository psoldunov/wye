# A tour of Wye

<p align="center">
  <img src="../data/icons/hicolor/scalable/apps/dev.soldunov.wye.svg" width="96" alt="Wye">
</p>

Every surface of Wye, as it looks on KDE Plasma 6 with Breeze. Switch GitHub to light or
dark mode to see the other colour scheme. The browser profiles, rules and links in the
images are demo data. Back to the [README](../README.md). On GNOME, see the [GNOME tour](tour-gnome.md).

## In action

A link clicked in Konsole opens the picker at the pointer. Picking the Research profile of
Firefox opens the page there.

<p align="center">
  <img src="media/kde/demo.gif" width="780" alt="A link clicked in Konsole opens the Wye picker at the pointer; the user picks the Research profile of Firefox and the page opens there">
</p>

## The picker

The picker opens at the pointer when no rule decides. Each tile is a browser or a browser
profile, with its hotkey above it and the link's source app and address below.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/picker.png">
    <img src="media/kde/screenshots/light/picker.png" width="905" alt="The Wye picker on KDE Plasma: six browser and profile tiles with hotkeys, and the link it is about to open">
  </picture>
</p>

Right-click a tile for the other ways to open the link. The "…" button opens the overflow
menu with the Open In submenu, Copy Link, Create Rule… and Settings….

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/picker-tile-menu.png">
    <img src="media/kde/screenshots/light/picker-tile-menu.png" width="450" alt="The right-click menu of a picker tile: Open, Open in Private Window, Open in New Window, Open in Background, Make Primary Browser">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/picker-more.png">
    <img src="media/kde/screenshots/light/picker-more.png" width="510" alt="The picker overflow menu with the Open In submenu, Copy Link, Create Rule and Settings">
  </picture>
</p>

## The tray

The tray menu opens the clipboard link and chooses the primary browser. Its More submenu
holds History, Recent Links, Test Rules…, Rescan Browsers, Set Up Wye…, Help and About.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/tray-menu.png">
    <img src="media/kde/screenshots/light/tray-menu.png" width="326" alt="The Wye tray menu on KDE Plasma: open URL from clipboard, the primary browser list, Settings, More and Quit">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/tray-more.png">
    <img src="media/kde/screenshots/light/tray-more.png" width="463" alt="The tray menu with the More submenu open: History, Recent Links, Test Rules, Rescan Browsers, Set Up Wye, Help and About">
  </picture>
</p>

While history is on, Recent Links lists the last ten links you opened; choosing one opens it
in the picker again.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/tray-recent.png">
    <img src="media/kde/screenshots/light/tray-recent.png" width="820" alt="The tray menu with More and its Recent Links submenu open: the last links opened, as host and path">
  </picture>
</p>

## Settings

Settings has seven pages: General, Browsers, Apps, Picker, Rules, Extras and Advanced.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/settings-general.png">
    <img src="media/kde/screenshots/light/settings-general.png" width="455" alt="The Settings window on the General page">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/settings-browsers.png">
    <img src="media/kde/screenshots/light/settings-browsers.png" width="455" alt="The Settings window on the Browsers page">
  </picture>
</p>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/settings-picker.png">
    <img src="media/kde/screenshots/light/settings-picker.png" width="455" alt="The Settings window on the Picker page">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/settings-extras.png">
    <img src="media/kde/screenshots/light/settings-extras.png" width="455" alt="The Settings window on the Extras page">
  </picture>
</p>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/settings-apps.png">
    <img src="media/kde/screenshots/light/settings-apps.png" width="455" alt="The Settings window on the Apps page: web apps that can open in their desktop app or a chosen browser">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/settings-advanced.png">
    <img src="media/kde/screenshots/light/settings-advanced.png" width="455" alt="The Settings window on the Advanced page">
  </picture>
</p>

## Rules

Rules are listed top to bottom and the first match wins. A rule matches on the link, on the
source app, or on both.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/settings-rules.png">
    <img src="media/kde/screenshots/light/settings-rules.png" width="600" alt="The Settings window on the Rules page">
  </picture>
</p>

The rule editor sets the target and the matchers. Test Rules… traces a link through link
cleaning, the transform script and the rules, and shows where it opens.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/rule-editor.png">
    <img src="media/kde/screenshots/light/rule-editor.png" width="455" alt="The Edit Rule sheet for the rule “Work links from Slack”, with its target and URL matchers">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/rule-tester.png">
    <img src="media/kde/screenshots/light/rule-tester.png" width="455" alt="The Test Rules sheet tracing a YouTube link through cleaning, transform and rule match">
  </picture>
</p>

## Transform scripts

The global script and the optional script of each rule rewrite a link before it opens.
The editor shows the result for a test link as you type.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/script-editor.png">
    <img src="media/kde/screenshots/light/script-editor.png" width="600" alt="The global transform script editor with a live test result rewriting www.reddit.com to old.reddit.com">
  </picture>
</p>

## History and About

The History window lists recent links with the reason each one went where it did. About
shows the version, the licence and where to report a bug.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/history.png">
    <img src="media/kde/screenshots/light/history.png" width="450" alt="The History window listing recent links with reason chips such as picker choice, rule, cleaned and expanded">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/about.png">
    <img src="media/kde/screenshots/light/about.png" width="450" alt="The About window of Wye">
  </picture>
</p>

## First run

On first start Wye walks through making itself the default browser and choosing the
browsers to show.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/first-run.png">
    <img src="media/kde/screenshots/light/first-run.png" width="400" alt="The first-run welcome page of Wye">
  </picture>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="media/kde/screenshots/dark/first-run-browsers.png">
    <img src="media/kde/screenshots/light/first-run-browsers.png" width="400" alt="The first-run page where the user chooses browsers">
  </picture>
</p>
