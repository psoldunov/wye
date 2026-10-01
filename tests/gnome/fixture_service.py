#!/usr/bin/python3
"""Small Wye D-Bus stand-in for exercising the real GNOME Shell extension."""
import json
from pathlib import Path

from gi.repository import Gio, GLib

ROOT = Path(__file__).resolve().parents[2]
PICKER = json.dumps(json.loads((ROOT / 'crates/wye-ui/fixtures/picker.json').read_text())['cases'][0]['argument'])
TRAY = json.dumps(json.loads((ROOT / 'crates/wye-ui/fixtures/tray-menu.json').read_text())['cases'][0]['argument'])
XML = '''<node><interface name="dev.soldunov.wye1">
  <method name="RegisterTray"><arg type="s" direction="in"/></method>
  <method name="UnregisterTray"/>
  <method name="ClipboardHasUrl"><arg type="b" direction="out"/></method>
  <method name="ActivateTrayItem"><arg type="s" direction="in"/></method>
  <method name="PickerChose"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="a{sv}" direction="in"/></method>
  <method name="PickerCancelled"><arg type="s" direction="in"/></method>
  <method name="PickerAction"><arg type="s" direction="in"/><arg type="s" direction="in"/></method>
  <method name="ShowWindow"><arg type="s" direction="in"/><arg type="s" direction="in"/></method>
  <method name="SetPrimary"><arg type="s" direction="in"/></method>
  <property name="Tray" type="s" access="read"/>
</interface></node>'''


def main():
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    interface = Gio.DBusNodeInfo.new_for_xml(XML).interfaces[0]

    def method(_connection, _sender, _path, _iface, name, params, invocation):
        print(f'{name}: {params.unpack()}', flush=True)
        if name == 'ClipboardHasUrl':
            invocation.return_value(GLib.Variant('(b)', (True,)))
        else:
            invocation.return_value(None)

    def property_get(_connection, _sender, _path, _iface, name):
        if name != 'Tray':
            raise ValueError(f'Unknown property: {name}')
        return GLib.Variant('s', TRAY)

    bus.register_object('/dev/soldunov/wye', interface, method, property_get, None)
    Gio.bus_own_name_on_connection(bus, 'dev.soldunov.wye', Gio.BusNameOwnerFlags.NONE, None, None)
    print('fixture ready', flush=True)
    GLib.MainLoop().run()


if __name__ == '__main__':
    main()
