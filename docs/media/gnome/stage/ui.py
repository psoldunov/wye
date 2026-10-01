"""Widgets inside the stage's GTK windows, through AT-SPI on the stage's own
accessibility bus: what drive.py's pointer cannot find by itself.

Run it with `stage.sh run` (lib.sh's `ui` does, with the typelib path).
APP is a part of the accessible application name ("" for any app but the
Shell); NAME is the accessible name (the label) of a widget, matched exactly
or else as the start of it; ROLE narrows the match ("push button", "label").

  ui.py dump [APP]             every showing widget: role, name, and its box
                               in window coordinates
  ui.py press NAME [ROLE [APP]]
                               run the widget's first action (a button's
                               click), as a pointer click would
  ui.py box NAME [ROLE [APP]]  print "x y w h" of the widget, logical pixels
                               from the top left of the window's frame

Window coordinates start at the frame, the corner Meta's frame rectangle
names (lib.sh adds them up); the client's shadow lies outside.
"""

import sys

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402

SHELL = "gnome-shell"
DEPTH = 40


def applications(app):
    desktop = Atspi.get_desktop(0)
    for index in range(desktop.get_child_count()):
        application = desktop.get_child_at_index(index)
        if application is None:
            continue
        name = application.get_name() or ""
        if name == SHELL and app != SHELL:
            continue
        if app in name:
            yield application


def showing(accessible):
    states = accessible.get_state_set()
    return states.contains(Atspi.StateType.SHOWING) and states.contains(Atspi.StateType.VISIBLE)


def walk(accessible, depth=0, seen=None):
    # Each object once: GTK reaches a view stack's visible page through
    # every page object as well, so a page would show several times.
    seen = set() if seen is None else seen
    if depth > DEPTH:
        return
    for index in range(accessible.get_child_count()):
        child = accessible.get_child_at_index(index)
        if child is None or child.path in seen or not showing(child):
            continue
        seen.add(child.path)
        yield child
        yield from walk(child, depth + 1, seen)


def extents(accessible):
    box = accessible.get_extents(Atspi.CoordType.WINDOW)
    return box.x, box.y, box.width, box.height


def find(name, role="", app=""):
    exact = prefix = None
    for application in applications(app):
        for accessible in walk(application):
            if role and accessible.get_role_name() != role:
                continue
            label = (accessible.get_name() or "").strip()
            if label == name and exact is None:
                exact = accessible
            elif label.startswith(name) and prefix is None:
                prefix = accessible
    found = exact or prefix
    if found is None:
        raise SystemExit(f"ui.py: no widget {name!r}" + (f" ({role})" if role else ""))
    return found


def dump(app=""):
    for application in applications(app):
        print(f"[{application.get_name()}]")
        for accessible in walk(application):
            name = (accessible.get_name() or "").replace("\n", " ")
            print(accessible.get_role_name(), repr(name), *extents(accessible))


def press(accessible):
    if accessible.get_n_actions() < 1:
        raise SystemExit(f"ui.py: {accessible.get_name()!r} has no action; click its box instead")
    accessible.do_action(0)


def main(arguments):
    command, *rest = arguments or ["help"]
    if command == "dump" and len(rest) <= 1:
        dump(*rest)
    elif command == "press" and 1 <= len(rest) <= 3:
        press(find(*rest))
    elif command == "box" and 1 <= len(rest) <= 3:
        print(*extents(find(*rest)))
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
