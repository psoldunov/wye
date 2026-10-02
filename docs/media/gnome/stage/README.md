# The GNOME demo stage

The screenshots in `docs/media/gnome/screenshots/{dark,light}/` come from a stage: a headless
GNOME Shell 50 with its own session, system and accessibility buses, runtime directory and
home directory, running this checkout's Wye as `nix build` makes it. It is the GNOME
counterpart of the KDE stage in `docs/media/kde/stage/` and uses the same made-up data: the
browser profiles Work, Acme Client and Research, the demo rules, a transform script and a
history (`home/`). Every stage process starts from an empty environment (`env -i`), so
nothing on the stage reads your own home directory, settings or session. The one thing it
asks of the session you are working in is a transient `app-gnome-*.scope` from your systemd
user manager, which `stage.sh app` runs a command in (so Wye can tell which app a link came
from); the scope ends with the command. The stage's own extension turns on the Shell's
unsafe mode only when `WYE_GNOME_STAGE_MARK` is set, as `stage.sh` sets it for every stage
process.

## What it needs

- Nix with flakes. Everything GNOME comes from the repository's own nixpkgs (`tools.nix`):
  GNOME Shell and Mutter, the GSettings schemas, the Adwaita icons and fonts, the default
  wallpaper, Mesa, xdg-desktop-portal with its GTK backend, AT-SPI, Python with dbus-python,
  Pillow and PyGObject, ImageMagick and oxipng. GNOME does not need to be installed.
- A GPU render node (`/dev/dri/renderD*`) the flake's Mesa can drive.
- The demo's browsers installed on the host: Firefox, Google Chrome, Zen and Brave. The stage
  copies their desktop entries and hicolor icons, so the picker shows the real icons; a
  browser the host lacks is left out (with a warning). The same goes for the apps the demo's
  links came from, Slack, Telegram and Fractal: Wye names a source app ("from Slack" in the
  history and the rules) only when it is installed. GNOME Terminal, the picker's source app,
  comes from nixpkgs (`tools.nix`).
- `systemd-run --user` (the source-app scope) and `fusermount3` (the document portal).

## Use

```sh
docs/media/gnome/stage/stage.sh up dark     # or light
docs/media/gnome/stage/capture.sh           # every screenshot, into screenshots/<scheme>/
docs/media/gnome/stage/capture.sh picker    # some steps only
docs/media/gnome/stage/stage.sh down
```

The steps are `picker`, `tray`, `settings_pages`, `rule_editor`, `rule_tester`, `windows`
and `sheets`. A stage takes one colour scheme: for the other, run `stage.sh down` and
`stage.sh up` again. The stage lives in `/tmp/wye-gnome-stage` (`WYE_GNOME_STAGE` moves
it); its logs are in `log/` there (`shell.log` holds the Shell, the session bus and every
process it starts). `WYE_SHOTS=/some/dir` captures somewhere else than the tracked
gallery, for trying a change out.

What runs on the stage, and how to swap it:

| Variable | Default | Use |
|----------|---------|-----|
| `WYE_PACKAGE` | `nix build .#default` of the working tree | A Wye package already built, such as `nix build "git+file://$PWD?rev=$(git rev-parse HEAD)#default"` while the working tree does not build. |
| `WYE_GTK_BIN` | the package's `bin/wye-gtk` | Another GTK host, such as `$PWD/target/debug/wye-gtk` from `nix develop -c cargo build -p wye-gtk`. D-Bus starts it as `dev.soldunov.wye.Gtk`. |
| `WYE_SHELL_EXTENSION` | `frontends/gnome-shell` of the working tree | Another copy of the Shell extension (`wye@dev.soldunov`). It is copied on `up`: run `down` and `up` again after changing it. |
| `WYE_GTK_APP_ID` | `dev.soldunov.wye` | The application ID the capture finds the GTK host's windows by (the host's `adw::Application` ID; its D-Bus name is `dev.soldunov.wye.Gtk`). |
| `WYE_GNOME_STAGE_BLOCK` | | More D-Bus names to keep from starting. |
| `WYE_GNOME_STAGE_TZ` | a fixed-offset zone in which the stage starts at about 22:00 | The stage's time zone (POSIX `TZ`). The default keeps the demo history, 21 hours of it, under "Today". |

To restart the GTK host after a rebuild, without restarting the stage, kill it (its PID:
`stage.sh run gdbus call --session -d org.freedesktop.DBus -o /org/freedesktop/DBus -m
org.freedesktop.DBus.GetConnectionUnixProcessID dev.soldunov.wye.Gtk`); the next window
starts it again.

To look at the stage while you work on it:

```sh
S=docs/media/gnome/stage
$S/stage.sh run python3 $S/shot.py screen /tmp/screen.png      # the whole screen
$S/stage.sh run python3 $S/shell.py windows                     # windows, their IDs and frames
$S/stage.sh run python3 $S/shot.py window ID /tmp/window.png    # one window, with its shadow
$S/stage.sh run python3 $S/drive.py glide:640,400,300 click     # pointer and keys (see its docstring)
$S/stage.sh run python3 $S/shell.py eval 'Main.panel.statusArea'  # any Shell JavaScript
$S/stage.sh run env GI_TYPELIB_PATH=/tmp/wye-gnome-stage/tools/atspi/lib/girepository-1.0 \
    python3 $S/ui.py dump                                       # the GTK widgets
$S/stage.sh run wye settings rules                              # the CLI, on the stage
```

## How it works

| File | Does |
|------|------|
| `stage.sh` | Starts `gnome-shell --headless --virtual-monitor 2560x1600` inside `dbus-run-session`, puts the monitor at scale 2 through `org.gnome.Mutter.DisplayConfig` (1280×800 logical), then the real `wye service`. Writes the stage's GSettings (keyfile backend), D-Bus configurations and activation files, installs both Shell extensions, and copies the host's browser entries. `stage.sh app` runs a command in an `app-gnome-<desktop ID>-*.scope`, as the Shell starts apps, so the picker can say which app a link came from. |
| `extension/` | The stage's own Shell extension, `stage@wye.soldunov.dev`. It puts the Shell in unsafe mode, so `org.gnome.Shell.Eval` and `org.gnome.Shell.Screenshot` answer on the stage's bus, and adds `globalThis.wyeStage`: Clutter virtual pointer and keyboard, windows by application ID and title, window captures, and the boxes of Shell actors (what appeared since a mark, a text, a panel button). It also keeps the overview, notifications and the unsafe-mode icon out of the screenshots. Never install it in a real session. |
| `shell.py` | `org.gnome.Shell.Eval` and the extension's helpers, from the command line. |
| `drive.py` | Pointer and keyboard input through the extension's virtual devices; `to:TEXT` glides to a Shell text (a menu item, a picker button), or to a widget with that accessible name when it shows no text (the picker's "⋯", "More targets"). |
| `shot.py` | Screenshots: the screen through `org.gnome.Shell.Screenshot`, a window from its compositor actor (`Meta.WindowActor.get_image`). `ScreenshotWindow` crops to the frame and loses the shadow GTK draws; the actor keeps it, so a window is captured as GTK drew it, with its shadow and rounded corners on a transparent background, as the KDE shots keep KWin's. |
| `ui.py` | The GTK host's widgets through AT-SPI, by accessible name: run a button's action, or the box of a row for the pointer. |
| `capture.sh` | The screenshot steps. Windows are captured whole; the picker and the tray menu are Shell surfaces, cut out of the screen with wallpaper around them (the tray from the panel down, so the indicator shows; its More and Recent Links pages are each cut to the menu as it shows then). PNGs are optimised losslessly with oxipng. |
| `lib.sh` | Helpers `capture.sh` uses. |
| `home/` | The made-up home directory (the KDE stage's, with GNOME Terminal and Fractal for Konsole and NeoChat), copied fresh on every `up`. History times are seconds before the stage starts, in the stage's own time zone. |
| `tools.nix` | Everything the stage takes from nixpkgs. |

The steps find what they click by name, not by position: the picker's and the tray's texts
in the Shell (`drive.py to:`), the GTK host's buttons and rows through AT-SPI (a name may
list alternatives, `"Choose: Shown browsers|Choose…"`: a row's button is named by its
words and its row's title, older hosts by its label alone), and windows by the GTK host's application ID and title (Settings is titled
after its page; the rule editor, the rule tester and the settings sheets are AdwDialogs
inside it, so they are captured with it). The names and titles are variables at the top of
`capture.sh`; change them there when a surface renames something. Window sizes are the KDE
stage's, in logical pixels of the frame.

The stage's environment, in short, and why:

- Its own session bus configuration (`session-bus.conf`): the one dbus ships includes
  `/etc/dbus-1`, which on NixOS brings the host's services, xdg-desktop-portal among them.
- Its own, empty system bus: the Shell needs one, and the panel then shows no host
  network, battery or Bluetooth.
- Its own accessibility bus (a plain `dbus-daemon` with at-spi2-core's configuration):
  the one `org.a11y.Bus` starts uses dbus-broker, which needs systemd to start the registry.
- xdg-desktop-portal with only the GTK backend's Settings portal: GTK 4 reads the colour
  scheme and fonts from it on Wayland, and from nothing else. No other portal is
  configured, so nothing can ask for permission in a session nobody watches.
- The Qt UI host (`dev.soldunov.wye.Ui`) is blocked: on GNOME, Wye falls back to it when
  the GTK host or the Shell extension is missing, and a KDE window has no place in these
  shots. A missing GTK host shows as an error from `wye settings` instead.
- Animations and hot corners are off: menus and windows appear at once, and the virtual
  pointer starts in the corner that would open the overview.

## Known gaps

- Recorded GIFs: the KDE stage has `demo.sh`; this stage takes still screenshots only.
- The browsers come from the host (as on the KDE stage), so the picker's icons and the
  "Open In" list depend on what the host has installed. Starting one from the picker
  would start the host's browser with the stage's home directory.
- `wye open` is run inside an `app-gnome-org.gnome.Terminal-*.scope` of the host's systemd
  user manager; GNOME Terminal itself does not run.
- After About closed, the GTK host was seen to exit and the next window asked for (first
  run) not to open until D-Bus started the host again; `capture.sh` asks for the first-run
  window a second time for that.
- GTK 4's AT-SPI boxes are window coordinates from the frame; a widget inside extra
  margins can be reported a few pixels off (seen with a `Gtk.Box` margin). Buttons are
  pressed through their action instead, so only rows rely on the box.
