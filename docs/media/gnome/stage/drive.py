"""Pointer and keyboard input for the GNOME demo stage, through Clutter
virtual input devices the stage extension creates in the Shell.

Run it with `stage.sh run`. Each argument is one step:

  move:X,Y         put the pointer at X,Y (logical pixels)
  glide:X,Y,MS     move the pointer there over MS milliseconds
  to:TEXT[,MS]     glide to the middle of the topmost Shell text that reads
                   TEXT (a menu item, a picker button), or of a widget with
                   that accessible name (an icon button), 300 ms by default
  click, rclick, mclick
  ctrl-click       click with Ctrl held
  key:NAME         press and release a key: a letter, a digit or one of KEYS
                   (or any Clutter key name, such as F10)
  down:NAME, up:NAME
  type:TEXT        type TEXT
  sleep:MS

Only the Shell's own text is found by `to:`; inside a GTK window, use the
coordinates of the window (lib.sh) or keyboard navigation.

The pointer position is kept in $WYE_GNOME_STAGE/pointer between runs, so a
glide starts where the last one ended.
"""

import os
import sys
import time
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from shell import stage  # noqa: E402

STAGE = Path(os.environ.get("WYE_GNOME_STAGE", "/tmp/wye-gnome-stage"))

# Short names for Clutter key names (KEY_<name>).
KEYS = {
    "esc": "Escape",
    "tab": "Tab",
    "enter": "Return",
    "ctrl": "Control_L",
    "shift": "Shift_L",
    "alt": "Alt_L",
    "super": "Super_L",
    "space": "space",
    "up": "Up",
    "down": "Down",
    "left": "Left",
    "right": "Right",
    "backspace": "BackSpace",
    "menu": "Menu",
    "home": "Home",
    "end": "End",
}

BUTTONS = {"click": 1, "mclick": 2, "rclick": 3}
FRAME = 1 / 60

pointer_file = STAGE / "pointer"
try:
    position = [float(v) for v in pointer_file.read_text().split()]
except (OSError, ValueError):
    position = [0.0, 0.0]


def move(x, y):
    position[:] = [x, y]
    stage("move", x, y)


def glide(x, y, ms):
    steps = max(1, round(ms / 1000 / FRAME))
    x0, y0 = position
    start = time.monotonic()
    for step in range(1, steps + 1):
        t = step / steps
        eased = t * t * (3 - 2 * t)
        move(x0 + (x - x0) * eased, y0 + (y - y0) * eased)
        time.sleep(max(0.0, start + step * FRAME - time.monotonic()))


def glide_to_text(argument):
    text, _, ms = argument.rpartition(",")
    if not text or not ms.isdigit():
        text, ms = argument, "300"
    box = stage("textBox", text)
    if not box:
        raise SystemExit(f"drive.py: no text {text!r} shows")
    glide(box["x"] + box["width"] / 2, box["y"] + box["height"] / 2, float(ms))


def button(number):
    stage("button", number, True)
    time.sleep(0.08)
    stage("button", number, False)


def key_name(name):
    if len(name) == 1:
        return name
    return KEYS.get(name.lower(), name)


def key(name, pressed):
    stage("key", key_name(name), pressed)


def type_text(text):
    for char in text:
        stage("char", char, True)
        stage("char", char, False)
        time.sleep(0.03)


for step in sys.argv[1:]:
    verb, _, argument = step.partition(":")
    if verb == "move":
        move(*(float(v) for v in argument.split(",")))
    elif verb == "glide":
        x, y, ms = (float(v) for v in argument.split(","))
        glide(x, y, ms)
    elif verb == "to":
        glide_to_text(argument)
    elif verb in BUTTONS:
        button(BUTTONS[verb])
    elif verb == "ctrl-click":
        key("ctrl", True)
        button(1)
        key("ctrl", False)
    elif verb in ("key", "down", "up"):
        if verb in ("key", "down"):
            key(argument, True)
        if verb == "key":
            time.sleep(0.05)
        if verb in ("key", "up"):
            key(argument, False)
    elif verb == "type":
        type_text(argument)
    elif verb == "sleep":
        time.sleep(float(argument) / 1000)
    else:
        raise SystemExit(f"drive.py: unknown step {step!r}")

pointer_file.write_text(f"{position[0]} {position[1]}\n")
