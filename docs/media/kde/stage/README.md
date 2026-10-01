# The KDE demo stage

The screenshots in `docs/media/kde/screenshots/` and the GIF `docs/media/kde/demo.gif` come
from a stage: a headless KWin and Plasma 6 session with its own D-Bus bus, runtime
directory and home directory, running this checkout's Wye as `nix build` makes it. The
home directory is the template in `home/`: made-up browser profiles (Work, Acme Client,
Research), a configuration with demo rules, a transform script, a history and a Konsole
reading list. Nothing on the stage reads your own home directory, profiles or history, and
the stage never touches the session you are working in.

## What it needs

- KDE Plasma 6 installed on the host: `kwin_wayland`, `plasmashell`, `kded6`,
  `kscreen-doctor`, `plasma-apply-lookandfeel`, `kwriteconfig6`, `qdbus`, Konsole, and the
  Breeze theme and cursors. The stage runs them, it does not ship them.
- ImageMagick (`magick`), `wl-copy`, `systemd-run --user`, and Firefox for the GIF.
- Nix with flakes. `stage.sh up` builds the checkout (`nix build .#default`) and the tools
  in `tools.nix` (Python with pywayland and dbus-python, the KDE protocol files, gifski,
  oxipng, and the Mesa that matches the Qt Wye is built with) from the repository's own
  nixpkgs.
- Network access for the GIF: Firefox loads a kde.org page.

## Use

```sh
docs/media/kde/stage/stage.sh up dark     # or light
docs/media/kde/stage/capture.sh           # every screenshot, into screenshots/<scheme>/
docs/media/kde/stage/demo.sh              # the GIF (record it on the dark stage)
docs/media/kde/stage/stage.sh down
```

`capture.sh STEP...` takes some of the screenshots only (`picker`, `tray`,
`settings_pages`, `rule_editor`, `rule_tester`, `windows`). A stage takes one colour
scheme: for the other, run `stage.sh down` and `stage.sh up` again. The stage lives in
`/tmp/wye-stage` (`WYE_STAGE` moves it); its logs are in `log/` there.

To look at the stage while you work on it, take a screenshot with
`stage.sh run "$WYE_STAGE/tools/python/bin/python3" shot.py screen out.png`, and move the
pointer or type with `drive.py` (see its docstring) the same way.

## How it works

| File | Does |
|------|------|
| `stage.sh` | Starts `kwin_wayland --virtual` at 1280×800, scale 2, inside `dbus-run-session`, then Plasma and Wye. The made-up home, a private `XDG_RUNTIME_DIR`, no portal backends (so no permission dialogs) and a block list of D-Bus services that would reach outside the stage (KDE Connect, OBEX). `stage.sh app` starts an app in its own systemd scope, as Plasma does, so the picker can say which app a link came from. |
| `drive.py` | Pointer and keyboard input through KWin's fake-input protocol, which the stage allows without a prompt. |
| `shot.py` | Screenshots through KWin's ScreenShot2 D-Bus interface: the screen, the active window with its frame and shadow on a transparent background, and frame recording. |
| `capture.sh` | The screenshot steps. Windows are captured whole; the picker and the tray menu blur what is behind them, so they are cut out of the screen with wallpaper around them. PNGs are optimised losslessly with oxipng. |
| `demo.sh`, `demo.py` | The GIF: a link clicked in Konsole, the picker, the page opening in a Firefox profile. The stage's screenshots have no pointer, so `demo.py` draws the Breeze cursor from `drive.py`'s log, zooms out to the whole screen when the browser opens, and cuts the frames where nothing happens before gifski encodes it. |
| `lib.sh` | Helpers both scripts share: running commands and KWin scripts in the stage. |
| `home/` | The made-up home directory, copied fresh on every `up`. History times are seconds before the stage starts. |

Coordinates in the scripts are logical pixels on the 1280×800 screen; captures are in
device pixels, twice as large. A change to the layout of a surface may need the
positions in `capture.sh` or `demo.sh` adjusted: the comments next to each say what they
point at.

Plasma fills a tray submenu (More, Recent Links) only when Qt first tries to show it, and Qt
shows no empty menu, so the submenu opens on the next pointer motion over its row. A hand on
a mouse never notices; the stage's pointer stops dead, so `capture.sh` glides over each such
row twice. A screenshot taken between a pointer move and a click loses that click in Wye's
tray-menu popup (seen there; other surfaces untested): move the pointer again after
`shot.py` before clicking.

The stage's look-and-feel defaults (`.config/kdedefaults` in its home directory, written by
`plasma-apply-lookandfeel`) come first in `XDG_CONFIG_DIRS`, and the host session's are
left out, as `startplasma` does: the Plasma style and the icon theme follow the stage's
scheme, not your own.

## Known gaps

- The Wye UI host runs with the Mesa from the repository's nixpkgs (`tools.nix`): with the
  host's drivers, its Qt finds no EGL and aborts.
