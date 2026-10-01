"""Read the existing Qt fixture corpus without copying or changing it."""

from __future__ import annotations

import json
from pathlib import Path


class FixtureError(RuntimeError):
    pass


ROOT = Path(__file__).resolve().parents[3]
FIXTURES = ROOT / "crates" / "wye-ui" / "fixtures"


def load_trace() -> dict[str, object]:
    """Use a routed result from the Qt test corpus in the GTK fixture preview."""
    path = FIXTURES / "tester.json"
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise FixtureError(f"Cannot read fixture {path}: {error}") from error
    cases = document.get("cases", [])
    if isinstance(cases, list):
        for case in cases:
            argument = case.get("argument", {}) if isinstance(case, dict) else {}
            trace = argument.get("trace", {}) if isinstance(argument, dict) else {}
            if isinstance(trace, dict) and trace.get("targetName") == "Work (Google Chrome)":
                return trace
    raise FixtureError(f"No routed test result in {path}")


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
    if window == "history" and isinstance(argument, dict):
        # DLG-HIS-02: local preview variety; production reads only service history.
        extra = [
            (9, "https://docs.gnome.org/gtk4/stable/gtk4-getting-started.html", "Mail", "Firefox", "primary browser"),
            (8, "https://developer.mozilla.org/en-US/docs/Web/HTTP/Overview", "Matrix", "Work (Chrome)", "rule ‘Documentation’"),
            (7, "https://en.wikipedia.org/wiki/Linux_distribution", "", "Firefox", "picker choice"),
            (6, "https://example.net/news/release-notes?version=2", "Mail", "Firefox", "web app mapping"),
        ]
        demo = [
            {"id": number, "time": 1789050720 + (number - 5) * 600,
             "finalUrl": url, "originalUrl": url, "sourceName": source,
             "targetName": target, "target": {"profile": {"app": "google-chrome.desktop", "id": "Profile 1"}} if target == "Work (Chrome)" else {"app": "firefox.desktop"},
             "reason": reason, "cleaned": False, "expanded": False}
            for number, url, source, target, reason in extra
        ]
        history = argument.get("fixture", {}).get("history", {})
        return {**argument, "fixture": {**argument["fixture"], "history": {**history, "entries": [*demo, *history["entries"]]}}}
    return argument if isinstance(argument, dict) else {"value": argument}
