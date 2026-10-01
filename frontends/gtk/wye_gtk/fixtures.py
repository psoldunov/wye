"""Read the existing Qt fixture corpus without copying or changing it."""

from __future__ import annotations

import json
from pathlib import Path


class FixtureError(RuntimeError):
    pass


ROOT = Path(__file__).resolve().parents[3]
FIXTURES = ROOT / "crates" / "wye-ui" / "fixtures"


def load(window: str) -> dict[str, object]:
    names = {"settings": "settings", "first-run": "onboarding", "history": "history", "about": "about", "script-editor": "script-editor", "rule-editor": "rules", "test-rules": "tester"}
    path = FIXTURES / f"{names[window]}.json"
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise FixtureError(f"Cannot read fixture {path}: {error}") from error
    cases = document.get("cases")
    if not isinstance(cases, list) or not cases:
        raise FixtureError(f"Fixture {path} has no cases")
    first = cases[0]
    if not isinstance(first, dict):
        raise FixtureError(f"Fixture {path} has an invalid first case")
    argument = first.get("argument", {})
    return argument if isinstance(argument, dict) else {"value": argument}
