"""The stage's GNOME Shell, through org.gnome.Shell.Eval and the helpers the
stage extension puts in `globalThis.wyeStage` (extension/extension.js).

Run it with `stage.sh run`. Coordinates are logical pixels.

  shell.py eval JS                 print what JS returns, as JSON
  shell.py ready                   exit 0 once Eval answers
  shell.py scale N                 put the monitor at scale N
  shell.py clean                   hide the overview, drop notifications
  shell.py windows                 every window, as JSON lines
  shell.py wait APP [TITLE [S]]    wait up to S seconds (10) for the newest
                                   window whose app id contains APP and whose
                                   title matches the regular expression
                                   TITLE; print its id
  shell.py frame ID                print "x y w h" of a window's frame
  shell.py activate ID             focus and raise a window
  shell.py place ID W H            give its frame this size, centred in the
                                   work area
  shell.py close ID
  shell.py mark                    remember which Shell actors show
  shell.py shown                   print "x y w h" around the actors that show
                                   now and did not at `mark`
  shell.py text TEXT               print "x y w h" of the topmost text TEXT
  shell.py indicator NAME          print "x y w h" of the panel button whose
                                   status-area key contains NAME
  shell.py clipboard TEXT          put TEXT on the clipboard
  shell.py owner NAME              exit 0 if NAME has an owner on the bus
"""

import json
import sys
import time

import dbus

bus = dbus.SessionBus()


def shell():
    return dbus.Interface(bus.get_object("org.gnome.Shell", "/org/gnome/Shell"), "org.gnome.Shell")


def evaluate(code):
    """Run JS in the Shell; return what it returns, decoded from JSON."""
    ok, answer = shell().Eval(code)
    if not ok:
        raise SystemExit(f"shell.py: the Shell refused {code!r}: {answer}")
    return json.loads(answer) if answer else None


def stage(method, *arguments):
    """Call a method of the stage extension's helper."""
    return evaluate(f"wyeStage.{method}({', '.join(json.dumps(a) for a in arguments)})")


def ready():
    try:
        return evaluate("typeof wyeStage") == "object"
    except (dbus.DBusException, SystemExit):
        return False


def set_scale(scale):
    """Put every monitor at this scale, for this session only. The logical
    layout (scale-monitor-framebuffer) keeps the stage at screen / scale."""
    config = dbus.Interface(
        bus.get_object("org.gnome.Mutter.DisplayConfig", "/org/gnome/Mutter/DisplayConfig"),
        "org.gnome.Mutter.DisplayConfig",
    )
    serial, monitors, _logical, _properties = config.GetCurrentState()
    logical = []
    x = 0
    for monitor in monitors:
        connector = monitor[0][0]
        modes = monitor[1]
        mode = next((m for m in modes if m[6].get("is-current")), modes[0])
        logical.append(
            dbus.Struct(
                (
                    dbus.Int32(x),
                    dbus.Int32(0),
                    dbus.Double(scale),
                    dbus.UInt32(0),
                    dbus.Boolean(x == 0),
                    [dbus.Struct((connector, mode[0], dbus.Dictionary({}, signature="sv")))],
                )
            )
        )
        x += int(mode[1] / scale)
    # Method 1: temporary, nothing written to monitors.xml.
    config.ApplyMonitorsConfig(serial, dbus.UInt32(1), logical, dbus.Dictionary({}, signature="sv"))


def wait_window(app, title="", seconds=10.0):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        window = stage("find", app, title)
        if window:
            return window
        time.sleep(0.2)
    raise SystemExit(f"shell.py: no window of {app!r} titled /{title}/ after {seconds} s")


def print_box(found, what):
    if not found:
        raise SystemExit(f"shell.py: {what} does not show")
    print(found["x"], found["y"], found["width"], found["height"])


def main(arguments):
    command, *rest = arguments or ["help"]
    if command == "eval" and len(rest) == 1:
        print(json.dumps(evaluate(rest[0])))
    elif command == "ready" and not rest:
        sys.exit(0 if ready() else 1)
    elif command == "scale" and len(rest) == 1:
        set_scale(float(rest[0]))
    elif command == "clean" and not rest:
        stage("clean")
    elif command == "windows" and not rest:
        for window in stage("windows"):
            print(json.dumps(window))
    elif command == "wait" and 1 <= len(rest) <= 3:
        seconds = float(rest[2]) if len(rest) > 2 else 10.0
        print(wait_window(rest[0], rest[1] if len(rest) > 1 else "", seconds)["id"])
    elif command == "frame" and len(rest) == 1:
        window = next((w for w in stage("windows") if w["id"] == int(rest[0])), None)
        print_box(window and window["frame"], f"window {rest[0]}")
    elif command == "activate" and len(rest) == 1:
        stage("activate", int(rest[0]))
    elif command == "place" and len(rest) == 3:
        stage("place", *(int(value) for value in rest))
    elif command == "close" and len(rest) == 1:
        stage("close", int(rest[0]))
    elif command == "mark" and not rest:
        stage("mark")
    elif command == "shown" and not rest:
        print_box(stage("shownBox"), "nothing new")
    elif command == "text" and len(rest) == 1:
        print_box(stage("textBox", rest[0]), repr(rest[0]))
    elif command == "indicator" and len(rest) == 1:
        print_box(stage("indicatorBox", rest[0]), f"the {rest[0]} indicator")
    elif command == "clipboard" and len(rest) == 1:
        stage("clipboard", rest[0])
    elif command == "owner" and len(rest) == 1:
        sys.exit(0 if bus.name_has_owner(rest[0]) else 1)
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
