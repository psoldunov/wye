"""Pointer and keyboard input for the demo stage, through KWin's fake input.

Run it with `stage.sh run`. Each argument is one step:

  move:X,Y         put the pointer at X,Y (logical pixels)
  glide:X,Y,MS     move the pointer there over MS milliseconds
  click, rclick, mclick
  ctrl-click       click with Ctrl held
  key:NAME         press and release a key: a letter, a digit or one of KEYS
  down:NAME, up:NAME
  type:TEXT        type TEXT (printable ASCII)
  sleep:MS

The pointer position is kept in $WYE_STAGE/pointer between runs, so a glide
starts where the last one ended. With WYE_DRIVE_LOG set, every pointer
position is appended to that file as "time x y", and every click as
"time x y click", for the recording's cursor.
"""

import os
import sys
import time
from pathlib import Path

STAGE = Path(os.environ.get("WYE_STAGE", "/tmp/wye-stage"))
sys.path.insert(0, str(STAGE / "proto"))

from fake_input import OrgKdeKwinFakeInput
from pywayland.client import Display

# Linux input event codes (linux/input-event-codes.h).
KEYS = {
    "esc": 1,
    "tab": 15,
    "enter": 28,
    "ctrl": 29,
    "shift": 42,
    "alt": 56,
    "space": 57,
    "up": 103,
    "left": 105,
    "right": 106,
    "down": 108,
    "backspace": 14,
    "menu": 127,
}
for offset, row in ((2, "1234567890"), (16, "qwertyuiop"), (30, "asdfghjkl"), (44, "zxcvbnm")):
    for index, letter in enumerate(row):
        KEYS[letter] = offset + index

BTN_LEFT, BTN_RIGHT, BTN_MIDDLE = 0x110, 0x111, 0x112
PRESSED, RELEASED = 1, 0
FRAME = 1 / 60

display = Display()
display.connect()
found = {}


def on_global(registry, name, interface, version):
    if interface == "org_kde_kwin_fake_input":
        found["input"] = registry.bind(name, OrgKdeKwinFakeInput, min(version, 6))


registry = display.get_registry()
registry.dispatcher["global"] = on_global
display.roundtrip()
if "input" not in found:
    raise SystemExit("drive.py: KWin offers no fake input; is this the stage?")
fake = found["input"]
fake.authenticate("Wye demo stage", "Records Wye's documentation")

pointer_file = STAGE / "pointer"
log = os.environ.get("WYE_DRIVE_LOG")
try:
    position = [float(v) for v in pointer_file.read_text().split()]
except (OSError, ValueError):
    position = [0.0, 0.0]


def sync():
    display.flush()
    display.roundtrip()


def move(x, y):
    position[:] = [x, y]
    fake.pointer_motion_absolute(x, y)
    sync()
    if log:
        with open(log, "a", encoding="utf-8") as file:
            file.write(f"{time.time():.4f} {x:.1f} {y:.1f}\n")


def glide(x, y, ms):
    steps = max(1, round(ms / 1000 / FRAME))
    x0, y0 = position
    start = time.monotonic()
    for step in range(1, steps + 1):
        t = step / steps
        eased = t * t * (3 - 2 * t)
        move(x0 + (x - x0) * eased, y0 + (y - y0) * eased)
        time.sleep(max(0.0, start + step * FRAME - time.monotonic()))


def button(code):
    if log:
        with open(log, "a", encoding="utf-8") as file:
            file.write(f"{time.time():.4f} {position[0]:.1f} {position[1]:.1f} click\n")
    fake.button(code, PRESSED)
    sync()
    time.sleep(0.08)
    fake.button(code, RELEASED)
    sync()


def key(code, state):
    fake.keyboard_key(code, state)
    sync()


def type_text(text):
    for char in text:
        # X keysyms equal the code point for printable ASCII.
        fake.keyboard_keysym(ord(char), PRESSED)
        fake.keyboard_keysym(ord(char), RELEASED)
        sync()
        time.sleep(0.03)


for step in sys.argv[1:]:
    verb, _, argument = step.partition(":")
    if verb == "move":
        move(*(float(v) for v in argument.split(",")))
    elif verb == "glide":
        x, y, ms = (float(v) for v in argument.split(","))
        glide(x, y, ms)
    elif verb == "click":
        button(BTN_LEFT)
    elif verb == "rclick":
        button(BTN_RIGHT)
    elif verb == "mclick":
        button(BTN_MIDDLE)
    elif verb == "ctrl-click":
        key(KEYS["ctrl"], PRESSED)
        button(BTN_LEFT)
        key(KEYS["ctrl"], RELEASED)
    elif verb in ("key", "down", "up"):
        code = KEYS[argument.lower()]
        if verb in ("key", "down"):
            key(code, PRESSED)
        if verb == "key":
            time.sleep(0.05)
        if verb in ("key", "up"):
            key(code, RELEASED)
    elif verb == "type":
        type_text(argument)
    elif verb == "sleep":
        time.sleep(float(argument) / 1000)
    else:
        raise SystemExit(f"drive.py: unknown step {step!r}")

pointer_file.write_text(f"{position[0]} {position[1]}\n")
display.disconnect()
