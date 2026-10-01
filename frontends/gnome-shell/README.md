# Wye for GNOME Shell 48+

Native Shell panel indicator and modal picker. This extension does not load GTK or replace the KDE UI. It owns `dev.soldunov.wye.Gnome` at `/dev/soldunov/wye/Gnome` and exports `dev.soldunov.wye.PickerHost1` (`ShowPicker(ss)`, `ClosePicker(s)`, `ShowMenu(s)`). It watches the service `dev.soldunov.wye`, registers `gnome-extension` with `RegisterTray`, reads the `Tray` property and sends `ActivateTrayItem`, `PickerChose`, `PickerCancelled`, or `PickerAction` in response to user actions. When the service restarts it registers again; disabling unexports the host and unregisters the tray.

## Install

```sh
mkdir -p ~/.local/share/gnome-shell/extensions/wye@dev.soldunov
cp frontends/gnome-shell/{extension.js,picker.js,model.mjs,metadata.json,stylesheet.css,wye-logo.svg,wye-picker-symbolic.svg} ~/.local/share/gnome-shell/extensions/wye@dev.soldunov/
gnome-extensions enable wye@dev.soldunov
```

Reload the Shell session (log out and back in on Wayland) after first installation. Install and run the Wye service separately. This extension has no D-Bus activation file: GNOME Shell loads it when enabled. A service build must route picker/menu calls to the GNOME name for the Shell UI to be used; older service builds still send them to `wye-ui`.

## Checks

```sh
node frontends/gnome-shell/test-model.mjs
node --check frontends/gnome-shell/extension.js
node --check frontends/gnome-shell/picker.js
```

For a real GNOME Shell 48 runtime check and screenshots on OrbStack, first build `wye-gnome:42` from `tests/gnome/Dockerfile`, then run `bash tests/gnome/capture.sh`. This starts systemd-logind and a nested Wayland Shell inside Xvfb in a disposable privileged container, installs the extension, starts a fake Wye D-Bus service using the shipped picker/tray fixtures, checks replacement/selection/overflow/menu/choice/disable, and captures the compositor through `org.gnome.Shell.Screenshot`. Results are **actual Shell** images in `.context/gnome-shots/{dark,light}/{picker,picker-more,picker-tile-menu,tray-menu,tray-more}.png` (not committed). To regenerate the tracked set use `bash tests/gnome/capture.sh docs/media/gnome/screenshots/shell`. The test installs small illustrative SVG fixture icons from `tests/gnome/icons/` because browsers are absent in the image. The fixture adds demo browser rows and turns on the URL footer; it does not exercise the real Rust service or GTK settings. Production uses icon theme names and honors the service's `showUrl` setting.

Exercise on a normal GNOME 48+ desktop too: enable/disable while service is running, restart service, open panel menu, choose a primary target, run `wye menu`, invoke `wye open` on a picker-routed link, test hotkeys/arrows/Escape, overflow, secondary target choices, and a second link replacing the first. Watch `journalctl /usr/bin/gnome-shell -f` for Shell errors. Node checks alone do not run the Shell UI.

## Limits

`SessionHelper1` is not exported: clipboard read/watch, source-app focus and held-key query require a separate audited Shell implementation. The picker sends modifier options but does not mint an xdg-activation token. `ShowMenu` opens the native panel menu at the indicator rather than at the supplied pointer placement; the modal picker is centred by Shell rather than pointer-anchored. The modal has no custom blur or profile avatar image rendering. These limitations do not change the D-Bus contract.
