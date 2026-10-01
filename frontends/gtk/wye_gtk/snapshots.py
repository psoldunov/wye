"""Capture fixture-backed GTK windows, one real rendered window per screenshot."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .fixtures import FixtureError


def snapshot_all(host: Any, directory: Path, pages: tuple[tuple[str, str, str], ...]) -> None:
    host.app.hold()  # Keep the application alive between closing each captured window.
    try:
        directory.mkdir(parents=True, exist_ok=True)
    except OSError as error:
        host._snapshot_error = f"Cannot create screenshot directory: {error}"
        host.app.quit()
        return
    work: list[tuple[str, str, dict[str, object]]] = [(f"settings-{name}", "settings", {"page": name}) for name, _, _ in pages]
    # These views use the settings fixture loaded above, just as their buttons do.
    work += [("shown-browsers", "shown", {}), ("picker-keys", "picker-keys", {}), ("url-expansion", "expansion", {})]
    for step in range(5):
        work.append((f"onboarding-{step}", "first-run", {"step": step}))
    work += [("history", "history", {}), ("about", "about", {}), ("script-editor", "script-editor", {}), ("rule-editor", "rule-editor", {"preview": True}), ("rule-tester", "test-rules", {})]
    index = 0

    def save_current(name: str, window_name: str) -> bool:
        nonlocal index
        try:
            window = host.windows.get(window_name)
            if window is None:
                raise RuntimeError(f"Could not create {window_name}")
            paintable = host.Gtk.WidgetPaintable.new(window)
            snapshot = host.Gtk.Snapshot.new()
            snapshot.scale(2.0, 2.0)
            paintable.snapshot(snapshot, float(window.get_width()), float(window.get_height()))
            node = snapshot.to_node()
            if node is None:
                raise RuntimeError(f"Could not render {name}")
            if not window.get_renderer().render_texture(node, None).save_to_png(str(directory / f"{name}.png")):
                raise RuntimeError(f"Could not save {name}")
            window.close()
            index += 1
            host.GLib.timeout_add(100, capture_next)
        except (OSError, RuntimeError, FixtureError, KeyError) as error:
            host._snapshot_error = str(error)
            host.app.quit()
        return False

    def capture_next() -> bool:
        if index >= len(work):
            host.app.quit()
            return False
        name, window_name, argument = work[index]
        try:
            auxiliary = {
                "shown": host._shown_browsers,
                "picker-keys": host._picker_keys,
                "expansion": host._expansion,
            }
            if window_name in auxiliary:
                auxiliary[window_name]()
                snapshot_sizes = {"shown": (480, 820), "picker-keys": (500, 700), "expansion": (560, 760)}
                host.windows[window_name].set_default_size(*snapshot_sizes[window_name])
            else:
                host.show(window_name, argument)
            host.GLib.timeout_add(250, save_current, name, window_name)
        except (OSError, RuntimeError, FixtureError, KeyError) as error:
            host._snapshot_error = str(error)
            host.app.quit()
        return False

    host.GLib.timeout_add(100, capture_next)
