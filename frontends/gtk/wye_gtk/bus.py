"""Internal Gtk host D-Bus endpoint: dev.soldunov.wye.Gtk / Windows1."""

from __future__ import annotations

from collections.abc import Callable
from importlib import import_module

from .dispatch import dispatch_window

XML = """<node>
  <interface name='dev.soldunov.wye.Windows1'>
    <method name='ShowWindow'><arg name='window' type='s' direction='in'/><arg name='argument' type='s' direction='in'/></method>
    <method name='Quit'/>
  </interface>
</node>"""


class GtkHost:
    def __init__(self, show: Callable[[str, dict[str, object]], None], quit_app: Callable[[], None]) -> None:
        gi = import_module("gi")
        gi.require_version("Gio", "2.0")
        gi.require_version("GLib", "2.0")
        self._Gio = import_module("gi.repository.Gio")
        self._GLib = import_module("gi.repository.GLib")
        self._show = show
        self._quit_app = quit_app
        self._node = self._Gio.DBusNodeInfo.new_for_xml(XML)
        self._owner = self._Gio.bus_own_name(
            self._Gio.BusType.SESSION,
            "dev.soldunov.wye.Gtk",
            self._Gio.BusNameOwnerFlags.NONE,
            self._bus_acquired,
            None,
            None,
        )

    def _bus_acquired(self, connection: object, _: str) -> None:
        connection.register_object(  # type: ignore[attr-defined]
            "/dev/soldunov/wye/Gtk", self._node.interfaces[0], self._method_call, None, None
        )

    def _method_call(
        self, _: object, __: str, ___: str, ____: str, method: str, parameters: object, invocation: object
    ) -> None:
        try:
            if method == "ShowWindow":
                window, argument = parameters.unpack()  # type: ignore[attr-defined]
                dispatch_window(window, argument, self._show)
            elif method == "Quit":
                self._quit_app()
            else:
                raise ValueError(f"Unknown method: {method}")
        except ValueError as error:
            invocation.return_dbus_error("org.freedesktop.DBus.Error.InvalidArgs", str(error))  # type: ignore[attr-defined]
            return
        invocation.return_value(self._GLib.Variant("()", ()))  # type: ignore[attr-defined]
