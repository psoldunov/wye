#!/usr/bin/python3
"""Drive a running GNOME Shell 48 and assert its real extension UI before screenshots."""
import json
import sys
import time
from pathlib import Path

import gi

gi.require_version('GdkPixbuf', '2.0')
from gi.repository import GdkPixbuf, Gio, GLib

ROOT = Path(__file__).resolve().parents[2]
OUT = Path(sys.argv[1])
OUT.mkdir(parents=True, exist_ok=True)
bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)


def call(destination, path, interface, method, args=None):
    result = bus.call_sync(destination, path, interface, method, args, None,
                           Gio.DBusCallFlags.NONE, 10000, None)
    return result.unpack()


def shell(expression):
    success, answer = call('org.gnome.Shell', '/org/gnome/Shell',
                           'org.gnome.Shell', 'Eval', GLib.Variant('(s)', (expression,)))
    assert success, f'Shell Eval failed: {answer}'
    value = json.loads(answer) if answer else None
    return json.loads(value) if isinstance(value, str) and value.startswith(('{', '[')) else value


def host(method, *args):
    signature = '(ss)' if method == 'ShowPicker' else '(s)'
    return call('dev.soldunov.wye.Gnome', '/dev/soldunov/wye/Gnome',
                'dev.soldunov.wye.PickerHost1', method, GLib.Variant(signature, args))


def shot(name, picker_dialog=False, tray_menu=False):
    filename = OUT / f'{name}.png'
    for attempt in range(6):
        probe = str(OUT / f'{name}-{attempt}.png')
        success, used = call('org.gnome.Shell.Screenshot', '/org/gnome/Shell/Screenshot',
                             'org.gnome.Shell.Screenshot', 'Screenshot',
                             GLib.Variant('(bbs)', (False, False, probe)))
        assert success and Path(used).stat().st_size > 10000, (success, used)
        image = GdkPixbuf.Pixbuf.new_from_file(used)
        pixel = image.get_pixels()
        # GNOME's software renderer can capture a frame mid-redraw. Verify the
        # dialog/menu's gray background, not merely a successful D-Bus reply.
        x, y = (1350, 200) if tray_menu else (840, 550)
        offset = y * image.get_rowstride() + x * image.get_n_channels()
        ready = (not picker_dialog and not tray_menu) or pixel[offset] > 24
        if ready:
            Path(used).replace(filename)
            print(f'{name}: {filename} ({filename.stat().st_size} bytes)', flush=True)
            return
        Path(used).unlink()
        time.sleep(1)
    raise AssertionError(f'{name}: picker dialog never rendered in Shell screenshot')


picker = json.loads((ROOT / 'crates/wye-ui/fixtures/picker.json').read_text())['cases'][0]['argument']
tray = json.loads((ROOT / 'crates/wye-ui/fixtures/tray-menu.json').read_text())['cases'][0]['argument']
# Browser packages are intentionally absent; use the real theme's generic browser icon.
for tile in picker['tiles']:
    tile['icon'] = 'web-browser'
for group in picker['overflow']:
    for tile in group['tiles']:
        tile['icon'] = 'web-browser'
for entry in tray['items']:
    if entry.get('icon') in ('firefox', 'chromium'):
        entry['icon'] = 'web-browser'

shell('Main.overview.hide();')
time.sleep(8)  # Let Shell's privileged-container warning expire before capture.
state = shell('JSON.stringify({active: Main.extensionManager.lookup("wye@dev.soldunov").state, '
              'indicator: !!Main.panel.statusArea["wye@dev.soldunov"]})')
assert isinstance(state, dict) and state['indicator'], state
host('ShowPicker', 'first', json.dumps(picker))
host('ShowPicker', 'replacement', json.dumps(picker))
state = shell('JSON.stringify({id: Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._id, '
              'count: Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._tiles.length, '
              'selected: Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._tiles[0].has_style_pseudo_class("selected"), '
              'background: Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._tiles[0].get_theme_node().get_background_color().to_string()})')
assert isinstance(state, dict) and state['id'] == 'replacement' and state['count'] == 2 and state['selected'], state
assert state['background'] != '#00000000', f'PICK-07: selected tile invisible: {state}'
time.sleep(2)
shot('shell-picker', picker_dialog=True)
shell('Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._more();')
time.sleep(1)
shot('shell-picker-overflow', picker_dialog=True)
host('ClosePicker', 'replacement')
shell('Main.panel.statusArea["wye@dev.soldunov"].menu.close();')
host('ShowMenu', json.dumps(tray))
assert shell('Main.panel.statusArea["wye@dev.soldunov"].menu.isOpen')
time.sleep(2)
shot('shell-tray', tray_menu=True)
shell('Main.panel.statusArea["wye@dev.soldunov"].menu.close();')
host('ShowPicker', 'choice', json.dumps(picker))
shell('Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._choose('
      'Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._request.tiles[1]);')
time.sleep(1)
assert 'PickerChose:' in Path('/workspace/fixture.log').read_text(), 'PICK-20: choice not delivered'
assert shell('Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._id') is None
Gio.Settings.new('org.gnome.shell').set_strv('enabled-extensions', [])
time.sleep(1)
assert 'UnregisterTray:' in Path('/workspace/fixture.log').read_text(), 'tray not unregistered on disable'
print('PASS: live Shell 48 extension, host D-Bus, replacement, selected tile, overflow, tray, choice, disable', flush=True)
