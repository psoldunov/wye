"""Screenshots from the GNOME demo stage.

Run it with `stage.sh run`:

  shot.py screen OUT.png        the whole screen, device pixels, through
                                org.gnome.Shell.Screenshot (no pointer)
  shot.py window ID OUT.png     a window as its client drew it: GTK's own
                                shadow and rounded corners on a transparent
                                background, in buffer (device) pixels
  shot.py changed BEFORE AFTER [TOP [BOTTOM]]
                                print "x y width height", in device pixels,
                                of where two screenshots differ (between
                                rows TOP and BOTTOM)

The stage extension puts the Shell in unsafe mode, so the Screenshot
interface answers any caller on the stage's bus.

A window is captured from its compositor actor, not with the Screenshot
interface's ScreenshotWindow: that one crops to the frame and drops the
shadow GTK draws around a client-side decorated window.
"""

import os
import sys

import dbus
from PIL import Image, ImageChops

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from shell import bus, stage  # noqa: E402


def screen(out):
    screenshot = dbus.Interface(
        bus.get_object("org.gnome.Shell.Screenshot", "/org/gnome/Shell/Screenshot"),
        "org.gnome.Shell.Screenshot",
    )
    ok, _used = screenshot.Screenshot(False, False, os.path.abspath(out))
    if not ok:
        raise SystemExit("shot.py: the Shell took no screenshot")


def window(window_id, out):
    stage("capture", window_id, os.path.abspath(out))
    # Cairo writes premultiplied ARGB as straight RGBA PNG already; nothing to
    # convert. Trim nothing either: the shadow is part of the picture.


def changed_box(before, after, top=0, bottom=None):
    """The box, in device pixels, where two screenshots differ between rows
    TOP and BOTTOM: where a popup opened. Printed as "x y width height"."""
    with Image.open(before) as old, Image.open(after) as new:
        bottom = new.height if bottom is None else bottom
        area = (0, top, new.width, bottom)
        difference = ImageChops.difference(old.convert("RGB").crop(area), new.convert("RGB").crop(area))
        # Ignore faint changes (a blinking cursor, dithering).
        box = difference.convert("L").point(lambda value: 255 if value > 24 else 0).getbbox()
    if box is None:
        raise SystemExit("shot.py: the screenshots are the same")
    left, upper, right, lower = box
    print(left, upper + top, right - left, lower - upper)


def main(arguments):
    command, *rest = arguments or ["help"]
    if command == "screen" and len(rest) == 1:
        screen(rest[0])
    elif command == "window" and len(rest) == 2:
        window(int(rest[0]), rest[1])
    elif command == "changed" and len(rest) in (2, 3, 4):
        bounds = [int(value) for value in rest[2:]]
        changed_box(rest[0], rest[1], *bounds)
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
