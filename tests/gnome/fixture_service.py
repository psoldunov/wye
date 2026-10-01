#!/usr/bin/python3
"""Small Wye D-Bus stand-in for exercising the real GNOME Shell extension."""
import json
from pathlib import Path

from gi.repository import Gio, GLib

ROOT = Path(__file__).resolve().parents[2]
PICKER = json.dumps(json.loads((ROOT / 'crates/wye-ui/fixtures/picker.json').read_text())['cases'][0]['argument'])
tray = json.loads((ROOT / 'crates/wye-ui/fixtures/tray-menu.json').read_text())['cases'][0]['argument']
tray['overlay'] = None
tray['items'][3:4] = [
    {'id': 'primary:work', 'kind': 'radio', 'label': 'Work Chrome Profile',
     'icon': 'google-chrome', 'shortcut': '2'},
    {'id': 'primary:zen', 'kind': 'radio', 'label': 'Zen Browser',
     'icon': 'zen-browser', 'shortcut': '3'},
    {'id': 'primary:brave', 'kind': 'radio', 'label': 'Brave Web Browser',
     'icon': 'brave-browser', 'shortcut': '4'},
]
TRAY = json.dumps(tray)
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
  <method name="ForwardHost"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/></method>
  <method name="DropName"/>
  <method name="TakeName"/>
  <property name="Tray" type="s" access="read"/>
</interface></node>'''


def main():
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    interface = Gio.DBusNodeInfo.new_for_xml(XML).interfaces[0]
    owner_id = Gio.bus_own_name_on_connection(bus, 'dev.soldunov.wye',
                                                Gio.BusNameOwnerFlags.NONE, None, None)

    def method(_connection, _sender, _path, _iface, name, params, invocation):
        nonlocal owner_id
        print(f'{name}: {params.unpack()}', flush=True)
        if name == 'ForwardHost':
            action, first, second = params.unpack()
            if action not in ('ShowPicker', 'ClosePicker', 'ShowMenu'):
                invocation.return_dbus_error('dev.soldunov.wye.InvalidMethod', action)
                return
            signature = '(ss)' if action == 'ShowPicker' else '(s)'
            args = (first, second) if action == 'ShowPicker' else (first,)

            def finished(connection, result):
                try:
                    connection.call_finish(result)
                    invocation.return_value(None)
                except GLib.Error as error:
                    invocation.return_gerror(error)

            bus.call('dev.soldunov.wye.Gnome', '/dev/soldunov/wye/Gnome',
                     'dev.soldunov.wye.PickerHost1', action, GLib.Variant(signature, args),
                     None, Gio.DBusCallFlags.NONE, 10000, None, finished)
            return
        if name == 'DropName':
            Gio.bus_unown_name(owner_id)
            owner_id = 0
        elif name == 'TakeName':
            owner_id = Gio.bus_own_name_on_connection(bus, 'dev.soldunov.wye',
                                                        Gio.BusNameOwnerFlags.NONE, None, None)
        if name == 'ClipboardHasUrl':
            invocation.return_value(GLib.Variant('(b)', (True,)))
        else:
            invocation.return_value(None)

    def property_get(_connection, _sender, _path, _iface, name):
        if name != 'Tray':
            raise ValueError(f'Unknown property: {name}')
        return GLib.Variant('s', TRAY)

    bus.register_object('/dev/soldunov/wye', interface, method, property_get, None)
    print('fixture ready', flush=True)
    GLib.MainLoop().run()


if __name__ == '__main__':
    main()
