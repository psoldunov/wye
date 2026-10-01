"""Pure helpers for Wye's GTK settings and routing controls."""

from __future__ import annotations

from collections.abc import Sequence
from typing import Any
from uuid import uuid4


def nested(data: object, *keys: str, default: Any = None) -> Any:
    value = data
    for key in keys:
        if not isinstance(value, dict):
            return default
        value = value.get(key, default)
    return value


def patch_for(path: tuple[str, ...], value: object) -> dict[str, object]:
    result: dict[str, object] = {path[-1]: value}
    for key in reversed(path[:-1]):
        result = {key: result}
    return result


def target_label(target: object, targets: object) -> str:
    if isinstance(target, dict):
        for item in nested(targets, "targets", default=[]):
            if isinstance(item, dict) and item.get("target") == target:
                return str(item.get("name", "Unknown target"))
        if target.get("picker"):
            return "Picker"
        if target.get("default"):
            return "Default"
    return "Default"


def filter_history(entries: Sequence[object], query: str) -> list[dict[str, object]]:
    needle = query.casefold().strip()
    return [item for item in entries if isinstance(item, dict) and any(needle in str(item.get(field, "")).casefold() for field in ("finalUrl", "originalUrl", "sourceName", "targetName"))]


def rule_patch(existing: dict[str, object], name: str, target: dict[str, object], kind: str, pattern: str, source: str) -> dict[str, object]:
    if not name.strip() or not (pattern.strip() or source.strip()):
        raise ValueError("Give the rule a name and a URL pattern or source app.")
    rule = dict(existing)
    rule.update({"id": str(existing.get("id") or uuid4()), "name": name.strip(), "target": target,
                 "url-matchers": [{"kind": kind, "pattern": pattern.strip()}] if pattern.strip() else [],
                 "source-apps": [source.strip()] if source.strip() else []})
    return rule


def expansion_disabled(disabled: list[str], service_id: str, enabled: bool) -> list[str]:
    values = [item for item in disabled if item.casefold() != service_id.casefold()]
    return values if enabled else [*values, service_id]
