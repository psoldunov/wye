"""Pure D-Bus window argument parsing; kept GI-free for fast tests."""

from __future__ import annotations

import json
from collections.abc import Callable
from typing import Final

WINDOWS: Final = frozenset(
    {"settings", "first-run", "history", "about", "script-editor", "rule-editor", "test-rules"}
)


def dispatch_window(window: str, argument: str, show: Callable[[str, dict[str, object]], None]) -> None:
    """Validate a Windows1 request and pass its decoded argument to the UI."""
    if window not in WINDOWS:
        raise ValueError(f"Unknown Wye window: {window}")
    if argument.startswith("{"):
        try:
            decoded = json.loads(argument)
        except json.JSONDecodeError as error:
            raise ValueError("Window argument is not valid JSON") from error
        if not isinstance(decoded, dict):
            raise ValueError("Window argument must be an object or page name")
        show(window, decoded)
    else:
        show(window, {"value": argument})
