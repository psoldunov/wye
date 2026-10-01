# Wye GTK companion

Native GTK 4/libadwaita frontend for GNOME and other GTK desktops. It owns only Settings, onboarding, history, About, the transform-script editor, and rule editor/tester. The GNOME Shell extension owns picker and tray integration.

## Dependencies

Fedora 42 packages: `python3`, `python3-gobject`, `gtk4`, and `libadwaita`. It uses only Python's standard library plus system PyGObject; no pip packages are required.

## Run

```sh
PYTHONPATH=frontends/gtk python3 -m wye_gtk
```

The process owns `dev.soldunov.wye.Gtk` at `/dev/soldunov/wye/Gtk` and implements `dev.soldunov.wye.Windows1.ShowWindow(s,s)` and `Quit()`. Production reads and writes `dev.soldunov.wye1` on the Wye session bus. Service calls run on one worker thread so D-Bus timeouts do not freeze the GUI; failures are shown in an error window.

## Fixture screenshots

The self-test reads the existing `crates/wye-ui/fixtures/*.json` files without modifying them, opens real GTK windows, and writes one screenshot per top-level window plus every Settings page:

```sh
PYTHONPATH=frontends/gtk python3 -m wye_gtk --self-test --snapshots /tmp/wye-gtk-snapshots
```

Run this inside a graphical GNOME/GTK session (or a virtual display). Screenshots render real windows and header bars; fixture mode does not write to the service. A missing fixture or failed image write exits nonzero.

## Scope and limits

Settings, history search/delete/clear (with clear confirmation), URL expansion service toggles, script editing/testing, and one-condition routing-rule editing use the public service API. The rule editor supports one domain or prefix matcher and one source desktop entry; rules with multiple or other matcher kinds display but cannot be saved here, to prevent losing conditions. Picker keys are view-only. This companion does not implement the GNOME Shell picker or tray. Fixture mode only previews controls, without persisting changes. The test-rules page currently tests a URL without source-app or held-key inputs; history does not yet support reopen/copy actions. Browser-extension help is text only, not an install flow.

## Checks

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=frontends/gtk python3 -m unittest discover -s frontends/gtk/tests -v
PYTHONPYCACHEPREFIX=/tmp/wye-gtk-pycache python3 -m compileall -q frontends/gtk
# In the prebuilt Fedora 42 container, with Xvfb and dbus-run-session:
# python3 -m wye_gtk --self-test --snapshots /tmp/shots
```
