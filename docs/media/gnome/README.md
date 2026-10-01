# Wye on GNOME — screenshot gallery

The GTK 4/libadwaita windows and GNOME Shell extension run **alongside**, not instead of, the KDE frontend. These images show the actual rendered frontends on Fedora 42 in OrbStack. GTK windows were captured in a virtual X display with fixture data; Shell screenshots came from a nested GNOME Shell 48.8 session with a fake Wye D-Bus service. Browser/profile names and icons in the Shell capture are illustrative fixtures. Neither set proves a full Rust-service integration.

| GNOME Shell | Dark | Light |
|---|---|---|
| Picker | [View](screenshots/shell/dark/picker.png) | [View](screenshots/shell/light/picker.png) |
| More choices | [View](screenshots/shell/dark/picker-more.png) | [View](screenshots/shell/light/picker-more.png) |
| Tile menu | [View](screenshots/shell/dark/picker-tile-menu.png) | [View](screenshots/shell/light/picker-tile-menu.png) |
| Tray | [View](screenshots/shell/dark/tray-menu.png) | [View](screenshots/shell/light/tray-menu.png) |
| Tray More | [View](screenshots/shell/dark/tray-more.png) | [View](screenshots/shell/light/tray-more.png) |

| GTK window | Dark | Light |
|---|---|---|
| About | [View](screenshots/gtk/dark/about.png) | [View](screenshots/gtk/light/about.png) |
| Shown Browsers | [View](screenshots/gtk/dark/shown-browsers.png) | [View](screenshots/gtk/light/shown-browsers.png) |
| Picker Keys | [View](screenshots/gtk/dark/picker-keys.png) | [View](screenshots/gtk/light/picker-keys.png) |
| URL Expansion | [View](screenshots/gtk/dark/url-expansion.png) | [View](screenshots/gtk/light/url-expansion.png) |
| History | [View](screenshots/gtk/dark/history.png) | [View](screenshots/gtk/light/history.png) |
| Onboarding 1 — Welcome | [View](screenshots/gtk/dark/onboarding-0.png) | [View](screenshots/gtk/light/onboarding-0.png) |
| Onboarding 2 | [View](screenshots/gtk/dark/onboarding-1.png) | [View](screenshots/gtk/light/onboarding-1.png) |
| Onboarding 3 | [View](screenshots/gtk/dark/onboarding-2.png) | [View](screenshots/gtk/light/onboarding-2.png) |
| Onboarding 4 | [View](screenshots/gtk/dark/onboarding-3.png) | [View](screenshots/gtk/light/onboarding-3.png) |
| Onboarding 5 | [View](screenshots/gtk/dark/onboarding-4.png) | [View](screenshots/gtk/light/onboarding-4.png) |
| Rule editor | [View](screenshots/gtk/dark/rule-editor.png) | [View](screenshots/gtk/light/rule-editor.png) |
| Rule tester | [View](screenshots/gtk/dark/rule-tester.png) | [View](screenshots/gtk/light/rule-tester.png) |
| Script editor | [View](screenshots/gtk/dark/script-editor.png) | [View](screenshots/gtk/light/script-editor.png) |
| Settings — General | [View](screenshots/gtk/dark/settings-general.png) | [View](screenshots/gtk/light/settings-general.png) |
| Settings — Browsers | [View](screenshots/gtk/dark/settings-browsers.png) | [View](screenshots/gtk/light/settings-browsers.png) |
| Settings — Apps | [View](screenshots/gtk/dark/settings-apps.png) | [View](screenshots/gtk/light/settings-apps.png) |
| Settings — Picker | [View](screenshots/gtk/dark/settings-picker.png) | [View](screenshots/gtk/light/settings-picker.png) |
| Settings — Rules | [View](screenshots/gtk/dark/settings-rules.png) | [View](screenshots/gtk/light/settings-rules.png) |
| Settings — Extras | [View](screenshots/gtk/dark/settings-extras.png) | [View](screenshots/gtk/light/settings-extras.png) |
| Settings — Advanced | [View](screenshots/gtk/dark/settings-advanced.png) | [View](screenshots/gtk/light/settings-advanced.png) |

The GTK gallery has 40 images: 20 real GTK windows per theme, including all three settings auxiliary windows. Confirmation and error dialogs are not listed: the fixture workflow cannot reliably trigger them without changing application behavior.

Regenerate with `bash frontends/gtk/capture.sh` and `bash tests/gnome/capture.sh docs/media/gnome/screenshots/shell` after building the image described in `tests/gnome/Dockerfile`. GTK snapshots render at 2×. The KDE reference gallery remains at `docs/tour.md`.
