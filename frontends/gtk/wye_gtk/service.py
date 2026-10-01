"""Client for Wye's published session D-Bus contract; called off the GTK thread."""

from __future__ import annotations

import json
from importlib import import_module
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from collections.abc import Mapping


class ServiceError(RuntimeError):
    """A human-readable D-Bus operation failure."""


def parse_revision(value: object) -> int:
    try:
        return int(str(value))
    except (TypeError, ValueError) as error:
        raise ServiceError("Wye returned an invalid configuration revision") from error


class ServiceClient:
    """Read and update Wye through dev.soldunov.wye1, never local mock state."""

    def __init__(self) -> None:
        gi = import_module("gi")
        gi.require_version("Gio", "2.0")
        gi.require_version("GLib", "2.0")
        self._Gio = import_module("gi.repository.Gio")
        self._GLib = import_module("gi.repository.GLib")
        try:
            self._proxy = self._Gio.DBusProxy.new_for_bus_sync(
                self._Gio.BusType.SESSION,
                self._Gio.DBusProxyFlags.NONE,
                None,
                "dev.soldunov.wye",
                "/dev/soldunov/wye",
                "dev.soldunov.wye1",
                None,
            )
        except self._GLib.Error as error:
            raise ServiceError(f"Cannot reach Wye service: {error.message}") from error
        self.revision = 0

    def _call(self, method: str, signature: str = "()", values: tuple[object, ...] = ()) -> tuple[object, ...]:
        try:
            result = self._proxy.call_sync(
                method,
                self._GLib.Variant(signature, values),
                self._Gio.DBusCallFlags.NONE,
                10_000,
                None,
            )
        except self._GLib.Error as error:
            raise ServiceError(error.message) from error
        return result.unpack()

    def _json(self, method: str, signature: str = "()", values: tuple[object, ...] = ()) -> dict[str, object]:
        (payload,) = self._call(method, signature, values)
        try:
            decoded = json.loads(str(payload))
        except json.JSONDecodeError as error:
            raise ServiceError(f"{method} returned invalid JSON") from error
        if not isinstance(decoded, dict):
            raise ServiceError(f"{method} returned a JSON value instead of an object")
        return decoded

    def state(self) -> dict[str, object]:
        config_text, revision = self._call("GetConfig")
        try:
            config = json.loads(str(config_text))
        except json.JSONDecodeError as error:
            raise ServiceError("GetConfig returned invalid JSON") from error
        if not isinstance(config, dict):
            raise ServiceError("GetConfig returned a JSON value instead of an object")
        self.revision = parse_revision(revision)
        state: dict[str, object] = {"config": config, "revision": self.revision}
        for key, method, signature, values in (
            ("targets", "GetTargets", "()", ()),
            ("services", "GetServices", "()", ()),
            ("apps", "GetApps", "(b)", (True,)),
            ("shortcuts", "GetShortcuts", "()", ()),
            ("expansion", "GetExpansionCatalogue", "()", ()),
            ("history", "GetHistory", "()", ()),
        ):
            state[key] = self._json(method, signature, values)
        state["troubleshooting"] = self._call("GetTroubleshooting")[0]
        status = self._proxy.get_cached_property("Status")
        if status is not None:
            try:
                decoded = json.loads(status.unpack())
                state["status"] = decoded if isinstance(decoded, dict) else {}
            except (TypeError, json.JSONDecodeError) as error:
                raise ServiceError("Status returned invalid JSON") from error
        else:
            raise ServiceError("Wye did not provide its Status property")
        return state

    def update(self, patch: Mapping[str, object]) -> int:
        (revision,) = self._call("UpdateConfig", "(st)", (json.dumps(patch), self.revision))
        self.revision = parse_revision(revision)
        return self.revision

    def action(self, method: str) -> None:
        self._call(method)

    def script(self, scope: str) -> str:
        (source,) = self._call("GetScript", "(s)", (scope,))
        return str(source)

    def set_script(self, scope: str, source: str) -> None:
        self._call("SetScript", "(ss)", (scope, source))

    def run_script(self, source: str, url: str) -> dict[str, object]:
        return self._json("RunScript", "(ssa{sv})", (source, url, {}))

    def test_link(self, url: str) -> dict[str, object]:
        return self._json("TestLink", "(sa{sv})", (url, {}))
