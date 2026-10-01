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
    first, second = (args[0], args[1] if len(args) > 1 else '')
    return call('dev.soldunov.wye', '/dev/soldunov/wye', 'dev.soldunov.wye1',
                'ForwardHost', GLib.Variant('(sss)', (method, first, second)))


def direct_host(method, *args):
    signature = '(ss)' if method == 'ShowPicker' else '(s)'
    return call('dev.soldunov.wye.Gnome', '/dev/soldunov/wye/Gnome',
                'dev.soldunov.wye.PickerHost1', method, GLib.Variant(signature, args))


def fixture(method):
    return call('dev.soldunov.wye', '/dev/soldunov/wye', 'dev.soldunov.wye1', method)


def denied(method, *args):
    try:
        direct_host(method, *args)
    except GLib.Error as error:
        assert 'AccessDenied' in str(error), f'{method}: unexpected error: {error}'
    else:
        raise AssertionError(f'{method}: unauthorized caller accepted')


def shot(name, picker_dialog=False, tray_menu=False):
    filename = OUT / f'{name}.png'
    filename.parent.mkdir(parents=True, exist_ok=True)
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


cases = json.loads((ROOT / 'crates/wye-ui/fixtures/picker.json').read_text())['cases']
picker = cases[1]['argument']
picker['settings']['showUrl'] = True
picker['keys'] = cases[0]['argument']['keys']
for tile, key in zip(picker['tiles'], 'bfwpz', strict=True):
    tile['hotkey'] = key
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

shell('Main.overview.hide();')
time.sleep(8)  # Let Shell's privileged-container warning expire before capture.
state = shell('JSON.stringify({active: Main.extensionManager.lookup("wye@dev.soldunov").state, '
              'indicator: !!Main.panel.statusArea["wye@dev.soldunov"]})')
assert isinstance(state, dict) and state['indicator'], state
denied('ShowPicker', 'spoof', json.dumps(picker))
denied('ClosePicker', 'replacement')
denied('ShowMenu', json.dumps(tray))
assert shell('Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._id') is None
host('ShowPicker', 'first', json.dumps(picker))
host('ShowPicker', 'replacement', json.dumps(picker))
state = shell('JSON.stringify({id: Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._id, '
              'count: Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._tiles.length, '
              'selected: Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._tiles[0].has_style_pseudo_class("selected"), '
              'background: Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._tiles[0].get_theme_node().get_background_color().to_string()})')
assert isinstance(state, dict) and state['id'] == 'replacement' and state['count'] == 5 and state['selected'], state
assert state['background'] != '#00000000', f'PICK-07: selected tile invisible: {state}'
labels = shell('JSON.stringify(Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._tiles'
               '.slice(0, 3).map(tile => { const label = tile.get_child().get_children()[2];'
               'return {name: label.text, lines: label.clutter_text.get_layout().get_line_count(),'
               'ellipsized: label.clutter_text.get_layout().is_ellipsized()}; }))')
assert labels[0] == {'name': 'Brave Web Browser', 'lines': 2, 'ellipsized': False}, labels
assert labels[2] == {'name': 'Work Chrome Profile', 'lines': 3, 'ellipsized': False}, labels
visual = shell('JSON.stringify({brand: !!Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._brand, '
               'url: !!Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._url})')
assert isinstance(visual, dict) and visual['brand'] and visual['url'], visual
for scheme in ('dark', 'light'):
    Gio.Settings.new('org.gnome.desktop.interface').set_string('color-scheme',
        'prefer-dark' if scheme == 'dark' else 'prefer-light')
    time.sleep(2)
    if scheme == 'light':
        host('ClosePicker', 'replacement')
        host('ShowPicker', 'replacement', json.dumps(picker))
    shot(f'{scheme}/picker', picker_dialog=True)
    shell('Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._more();')
    assert shell('global.stage.get_key_focus().accessible_name') == 'Brave'
    time.sleep(1)
    shot(f'{scheme}/picker-more', picker_dialog=True)
    shell('Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._context('
          'Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._request.tiles[0]);')
    assert shell('global.stage.get_key_focus().accessible_name') == 'Open Brave Web Browser'
    time.sleep(1)
    shot(f'{scheme}/picker-tile-menu', picker_dialog=True)
    host('ClosePicker', 'replacement')
    shell('Main.panel.statusArea["wye@dev.soldunov"].menu.close();')
    host('ShowMenu', json.dumps(tray))
    assert shell('Main.panel.statusArea["wye@dev.soldunov"].menu.isOpen')
    time.sleep(2)
    shot(f'{scheme}/tray-menu', tray_menu=True)
    shell('Main.panel.statusArea["wye@dev.soldunov"].menu._getMenuItems().find('
          'item => item.label?.text === "More").menu.open();')
    time.sleep(1)
    shot(f'{scheme}/tray-more', tray_menu=True)
    shell('Main.panel.statusArea["wye@dev.soldunov"].menu.close();')
    if scheme == 'dark':
        host('ShowPicker', 'replacement', json.dumps(picker))
host('ShowPicker', 'choice', json.dumps(picker))
shell('Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._choose('
      'Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._request.tiles[1]);')
time.sleep(1)
assert 'PickerChose:' in Path('/workspace/fixture.log').read_text(), 'PICK-20: choice not delivered'
assert shell('Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._id') is None
host('ShowPicker', 'new-window', json.dumps(picker))
shell('Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._context('
      'Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._request.tiles[0]);')
shell('Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._extra.get_children()[2].emit("clicked", 1);')
time.sleep(1)
assert "PickerChose: ('new-window'," in Path('/workspace/fixture.log').read_text() and (
    "'new-window': True}" in Path('/workspace/fixture.log').read_text()), 'PICK-20: new-window option missing'
[service_sender] = call('org.freedesktop.DBus', '/org/freedesktop/DBus',
                        'org.freedesktop.DBus', 'GetNameOwner',
                        GLib.Variant('(s)', ('dev.soldunov.wye',)))
fixture('DropName')
for _ in range(30):
    if shell('Main.extensionManager.lookup("wye@dev.soldunov").stateObj._serviceOwner') is None:
        break
    time.sleep(0.1)
denied('ShowPicker', 'absent', json.dumps(picker))
call(service_sender, '/dev/soldunov/wye', 'dev.soldunov.wye1', 'TakeName')
for _ in range(30):
    if shell('Main.extensionManager.lookup("wye@dev.soldunov").stateObj._registered'):
        break
    time.sleep(0.1)
assert shell('Main.extensionManager.lookup("wye@dev.soldunov").stateObj._registered'), 'service restart failed'
host('ShowPicker', 'after-restart', json.dumps(picker))
assert shell('Main.extensionManager.lookup("wye@dev.soldunov").stateObj._picker._id') == 'after-restart'
host('ClosePicker', 'after-restart')
Gio.Settings.new('org.gnome.shell').set_strv('enabled-extensions', [])
time.sleep(1)
assert 'UnregisterTray:' in Path('/workspace/fixture.log').read_text(), 'tray not unregistered on disable'
print('PASS: live Shell 48 extension, host D-Bus, replacement, selected tile, overflow, tray, choice, disable', flush=True)
