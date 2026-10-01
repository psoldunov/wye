// Unit tests of the extension's pure modules: `node test-model.mjs`.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {test} from 'node:test';
import * as keys from './keys.mjs';
import * as model from './model.mjs';
import * as session from './session-model.mjs';
import * as tray from './tray-model.mjs';

function fixture(name) {
    try {
        return JSON.parse(readFileSync(new URL(`../../crates/wye-ui/fixtures/${name}`, import.meta.url))).cases;
    } catch (error) {
        throw new Error(`Cannot load Wye UI fixture ${name}`, {cause: error});
    }
}

const ev = (key, mods = [], extra = {}) => ({key, text: key.length === 1 ? key : undefined, mods, ...extra});
const defaults = keys.keymap({});

test('every shipped picker fixture parses', () => {
    for (const {argument} of fixture('picker.json').filter(c => c.action === 'show')) {
        const picker = model.parsePicker(JSON.stringify(argument));
        assert.ok(picker.tiles.length <= (argument.tiles ?? []).length);
        assert.ok(picker.metrics.icon > 0);
    }
});

test('every shipped tray fixture parses', () => {
    for (const {argument} of fixture('tray-menu.json'))
        assert.equal(tray.parseTray(JSON.stringify(argument)).items.length, argument.items.length);
});

test('bad payloads are refused', () => {
    assert.throws(() => model.parsePicker('nope'), /Invalid picker JSON/);
    assert.throws(() => model.parsePicker('{"tiles":[]}'), /no url/);
    assert.throws(() => tray.parseTray('{"items":[{"kind":"action"}]}'), /Invalid tray item/);
    assert.throws(() => tray.parseTray('{"items":[{"kind":"bogus","id":"x"}]}'), /Invalid tray item/);
});

test('tiles without a target are left out, hotkeys get labels (PICK-04)', () => {
    const picker = model.parsePicker(JSON.stringify({
        url: {full: 'https://example.com/'},
        tiles: [{name: 'bad'}, {name: 'Firefox', target: {app: 'firefox.desktop'}, hotkey: 'F'},
            {name: 'Comma', target: {app: 'x.desktop'}, hotkey: ','}],
    }));
    assert.deepEqual(picker.tiles.map(t => [t.name, t.hotkey, t.hotkeyLabel]),
        [['Firefox', 'f', 'F'], ['Comma', 'comma', ',']]);
    assert.equal(picker.size, 'medium');
    assert.deepEqual(picker.metrics, {icon: 32, pitch: 48, badge: 18});
});

test('badges read either shape and refuse bad colours (PICK-06)', () => {
    const picker = model.parsePicker(JSON.stringify({
        url: {full: 'https://a.b/'},
        tiles: [
            {name: 'A', target: {app: 'a'}, badge: {initial: 'Work', color: '#336699'}},
            {name: 'B', target: {app: 'b'}, badge: {image: '/tmp/b.png'}},
            {name: 'C', target: {app: 'c'}, badge: {initial: 'C', color: 'red'}},
            {name: 'D', target: {app: 'd'}, badge: {image: 'relative.png'}},
        ],
    }));
    assert.deepEqual(picker.tiles.map(t => t.badge),
        [{initial: 'W', color: '#336699'}, {image: '/tmp/b.png'}, {initial: 'C', color: null}, null]);
    assert.equal(model.isLight('#336699'), false);
    assert.equal(model.isLight('#f5c211'), true);
});

test('the URL line splits and cuts like wye-core (PICK-09)', () => {
    assert.deepEqual(model.linkParts('https://www.github.com/psoldunov/wye/pull/7?x=1#top'),
        {host: 'github.com', rest: '/psoldunov/wye/pull/7?x=1#top'});
    assert.deepEqual(model.linkParts('https://example.com/'), {host: 'example.com', rest: ''});
    assert.deepEqual(model.linkParts('not a url', 'h', '/r'), {host: 'h', rest: '/r'});
    assert.deepEqual(model.linkParts('https://user:pw@Example.COM:8080/a?b'), {host: 'example.com', rest: '/a?b'});
    assert.deepEqual(model.linkParts('mailto:x@y.z'), {host: 'x@y.z', rest: ''});
    assert.equal(model.middleTruncate('abcdefghij', 5), 'ab…ij');
    assert.equal(model.middleTruncate('abc', 5), 'abc');
    assert.equal(model.middleTruncate('abc', 1), '…');
    assert.equal(model.middleTruncate('abc', 0), '');
    assert.deepEqual(model.truncateLink({host: 'host.com', rest: '/0123456789'}, 12),
        {host: 'host.com', rest: '/0…9'});
    assert.deepEqual(model.truncateLink({host: 'averyveryverylonghost.example', rest: '/x'}, 10),
        {host: 'avery…mple', rest: ''});
    const picker = model.parsePicker(JSON.stringify({
        url: {full: `https://example.com/${'a'.repeat(100)}`},
        source: {name: 'Slack', icon: 'slack'},
    }));
    assert.equal([...picker.url.host + picker.url.rest].length, model.URL_LINE_CHARS);
    assert.equal(picker.url.sourceName, 'Slack');
});

test('tile widths and rows follow the metrics (PICK-05, PICK-13)', () => {
    const large = model.METRICS.large;
    assert.equal(model.tileWidth(10, large, 6, true), 60);
    assert.equal(model.tileWidth(80, large, 6, true), 94);
    assert.equal(model.tileWidth(500, large, 6, true), 120);
    assert.equal(model.tileWidth(500, large, 6, false), 60);
    assert.equal(model.columns(12, 100, 2, 2000), 8);
    assert.equal(model.columns(3, 100, 2, 2000), 3);
    assert.equal(model.columns(12, 100, 2, 305), 3);
    assert.equal(model.columns(12, 100, 2, 10), 1);
    assert.deepEqual(model.rows([1, 2, 3, 4, 5], 2), [[1, 2], [3, 4], [5]]);
});

test('a larger name font widens the cap in KDE\'s proportion (PICK-05)', () => {
    const large = model.METRICS.large;
    // GNOME's caption: 0.9 of an 11 pt font.
    const gnome = 0.9 * 11 * 96 / 72;
    assert.equal(model.tileWidth(500, large, 6, true, gnome), Math.round(120 * gnome / model.KDE_NAME_PX));
    assert.equal(model.tileWidth(130, large, 6, true, gnome), 144);
    // A smaller font never shrinks the cap below KDE's.
    assert.equal(model.tileWidth(500, large, 6, true, 8), 120);
});

test('names are cut with an ellipsis right after the last letter (PICK-05)', () => {
    const graphemes = text => [...new Intl.Segmenter().segment(text)].length;
    const fits = max => text => graphemes(text) <= max;
    assert.equal(model.fitEnd('Firefox', fits(10)), 'Firefox');
    assert.equal(model.fitEnd('Work (Google Chrome)', fits(13)), 'Work (Google…');
    // No space before the ellipsis where the cut falls after one.
    assert.equal(model.fitEnd('Work (Google Chrome)', fits(14)), 'Work (Google…');
    assert.equal(model.fitEnd('Brave Web Browser', fits(11)), 'Brave Web…');
    assert.equal(model.fitEnd('Zen', fits(0)), '…');
    // Graphemes stay whole.
    assert.equal(model.fitEnd('Café 👩‍💻 Work', fits(7)), 'Café 👩‍💻…');
});

test('the clipboard hands on one line of plain text, never secrets (EXT-12)', () => {
    assert.equal(session.offeredText('  https://example.com/a \n', ['text/plain']), 'https://example.com/a');
    assert.equal(session.offeredText('hunter2', ['text/plain', session.PASSWORD_HINT]), '');
    assert.equal(session.offeredText('https://example.com/', ['image/png', 'text/plain']), '');
    assert.equal(session.offeredText('a\nb', ['text/plain']), '');
    assert.equal(session.offeredText(null, []), '');
    assert.equal(session.offeredText(`https://example.com/${'x'.repeat(session.MAX_TEXT)}`, ['text/plain']), '');
});

test('only links leave the Shell, never other text (EXT-12, EXT-13, TRAY-10)', () => {
    const plain = ['text/plain'];
    // A password copied from a manager that sets no hint stays.
    assert.equal(session.offeredText('correct-horse-battery-staple', plain), '');
    assert.equal(session.offeredText('hunter2', plain), '');
    assert.equal(session.offeredText('example.com/page', plain), '');
    assert.equal(session.offeredText('ftp://example.com/', plain), '');
    assert.equal(session.offeredText('https://', plain), '');
    assert.equal(session.offeredText('https://example.com/a b', plain), '');
    // What the service uses: web links and mailto (EXT-13).
    assert.equal(session.offeredText('HTTPS://Example.com', plain), 'HTTPS://Example.com');
    assert.equal(session.offeredText('http://localhost:8080/x?y=1#z', plain), 'http://localhost:8080/x?y=1#z');
    assert.equal(session.offeredText('mailto:ana@example.com?subject=hi', plain), 'mailto:ana@example.com?subject=hi');
    assert.equal(session.offeredText('mailto:@example.com', plain), '');
    assert.equal(session.offeredText('mailto:ana', plain), '');
});

test('the pointer is in logical pixels in both layout modes (PICK-02)', () => {
    const monitor = {x: 2560, y: 0};
    // Logical layout: stage pixels are logical, the UI scale is 1.
    assert.deepEqual(session.logicalPointer({x: 2560 + 700, y: 420}, monitor, 1, 2), {x: 700, y: 420});
    // Physical layout at scale 2: stage pixels are device pixels.
    assert.deepEqual(session.logicalPointer({x: 2560 + 1400, y: 840}, monitor, 2, 2), {x: 700, y: 420});
    // Physical layout, a scale-1 monitor beside a scale-2 one.
    assert.deepEqual(session.logicalPointer({x: 2560 + 700, y: 420}, monitor, 2, 1), {x: 700, y: 420});
});

test('the panel is centred on the pointer inside the work area (PICK-02)', () => {
    const area = {x: 0, y: 32, width: 1280, height: 768};
    assert.deepEqual(model.place({x: 640, y: 400}, {width: 200, height: 100}, area, 12), {x: 540, y: 350});
    assert.deepEqual(model.place({x: 5, y: 5}, {width: 200, height: 100}, area, 12), {x: 12, y: 44});
    assert.deepEqual(model.place({x: 1279, y: 799}, {width: 200, height: 100}, area, 12), {x: 1068, y: 688});
});

test('held modes choose targets as the core does (KEY-13, PICK-32, PICK-33)', () => {
    const firefox = {target: {app: 'firefox.desktop'}, caps: {private: true, newWindow: true}};
    const work = {target: {profile: {app: 'google-chrome.desktop', id: 'P1'}}, caps: {private: false, newWindow: true}};
    assert.deepEqual(model.choose(firefox, null), {target: {app: 'firefox.desktop'}, options: {}});
    assert.deepEqual(model.choose(firefox, 'private'), {target: {private: 'firefox.desktop'}, options: {}});
    assert.deepEqual(model.choose(firefox, 'background'), {target: firefox.target, options: {background: true}});
    assert.deepEqual(model.choose(work, 'new-window'), {target: work.target, options: {'new-window': true}});
    // Unsupported: opens normally (the tile is dimmed, PICK-14).
    assert.deepEqual(model.choose(work, 'private'), {target: work.target, options: {}});
    assert.equal(model.supports('background', {caps: {private: false, newWindow: false}}), true);
    assert.equal(model.desktopId(work.target), 'google-chrome.desktop');
    assert.equal(model.desktopId({private: 'firefox'}), 'firefox.desktop');
    assert.equal(model.desktopId({custom: 'x'}), null);
});

test('the tile menu lists what the target supports (PICK-30)', () => {
    const labels = entry => model.tileMenu(entry).map(e => e.label);
    assert.deepEqual(labels({target: {app: 'f'}, caps: {private: true, newWindow: true}}),
        ['Open', 'Open in Private Window', 'Open in New Window', 'Open in Background', '', 'Make Primary Browser']);
    assert.deepEqual(labels({target: {private: 'f'}, caps: {private: true, newWindow: false}}),
        ['Open', 'Open in Background', '', 'Make Primary Browser']);
});

test('Open In lists headings then targets (PICK-08, PICK-28)', () => {
    const rows = model.openInRows([
        {label: 'Private Browsing', entries: [{name: 'Firefox (Private)', icon: 'firefox'}]},
        {label: '', entries: [{name: 'Other…', icon: null}]},
    ]);
    assert.deepEqual(rows, [
        {kind: 'header', label: 'Private Browsing'},
        {kind: 'item', label: 'Firefox (Private)', icon: 'firefox', group: 0, item: 0},
        {kind: 'item', label: 'Other…', icon: null, group: 1, item: 0},
    ]);
});

test('key names canonicalise like wye-core (KEY-03)', () => {
    assert.equal(keys.canonicalKey('O'), 'o');
    assert.equal(keys.canonicalKey(','), 'comma');
    assert.equal(keys.canonicalKey(' '), 'space');
    assert.equal(keys.canonicalKey('ISO_Left_Tab'), 'Tab');
    assert.equal(keys.canonicalKey('kp_enter'), 'KP_Enter');
    assert.equal(keys.canonicalKey('f5'), 'F5');
    assert.equal(keys.canonicalKey('Shift_L'), null);
    assert.equal(keys.canonicalKey('Я'), 'я');
    assert.equal(keys.displayKey('comma'), ',');
    assert.equal(keys.displayKey('KP_Enter'), 'Enter');
    assert.equal(keys.displayKey('f'), 'F');
    assert.deepEqual(keys.parseBinding('shift+ctrl+O'), {mods: ['Ctrl', 'Shift'], key: 'o'});
    assert.deepEqual(keys.parseBinding('Ctrl++'), {mods: ['Ctrl'], key: 'plus'});
    assert.equal(keys.parseBinding('Hyper+x'), null);
    assert.equal(keys.modifierName('control'), 'Ctrl');
    assert.equal(keys.modifierName('meta'), 'Super');
    assert.equal(keys.modifierName('Fn'), null);
});

test('keys dispatch like the KDE picker (PICK-21, PICK-22, KEY-13)', () => {
    const hotkeys = ['f', 'w', null];
    assert.deepEqual(keys.dispatch(defaults, ev('Return'), hotkeys), {action: 'open', mode: null});
    assert.deepEqual(keys.dispatch(defaults, ev('Tab', ['Shift']), hotkeys), {action: 'previous', mode: null});
    assert.deepEqual(keys.dispatch(defaults, ev('Return', ['Shift']), hotkeys), {action: 'open', mode: 'private'});
    assert.deepEqual(keys.dispatch(defaults, ev('c', ['Ctrl']), hotkeys), {action: 'copy-link', mode: null});
    assert.deepEqual(keys.dispatch(defaults, ev('W', ['Shift']), hotkeys), {index: 1, mode: 'private'});
    assert.deepEqual(keys.dispatch(defaults, ev('f', ['Ctrl']), hotkeys), {index: 0, mode: 'background'});
    assert.equal(keys.dispatch(defaults, ev('f', ['Ctrl', 'Shift']), hotkeys), null);
    assert.equal(keys.dispatch(defaults, ev('x'), hotkeys), null);
    // KEY-11: a Cyrillic layout's а on the Latin f key.
    assert.deepEqual(keys.dispatch(defaults, {key: 'Cyrillic_a', text: 'а', keycode: 41, mods: []}, hotkeys),
        {index: 0, mode: null});
    const custom = keys.keymap({actions: {open: ['o']}, modifierActions: {private: ['Super']}});
    assert.deepEqual(keys.dispatch(custom, ev('o'), []), {action: 'open', mode: null});
    assert.equal(keys.dispatch(custom, ev('Return'), []), null);
    assert.deepEqual(keys.dispatch(custom, ev('Escape'), []), {action: 'cancel', mode: null});
    assert.equal(keys.modeFor(custom, ['Super']), 'private');
    assert.equal(keys.modeFor(defaults, []), null);
});

test('selection wraps and held modifiers track presses (PICK-22, KEY-13)', () => {
    assert.equal(keys.select('next', 2, 3), 0);
    assert.equal(keys.select('previous', 0, 3), 2);
    assert.equal(keys.select('last', 0, 3), 2);
    assert.equal(keys.select('next', 0, 0), 0);
    assert.deepEqual(keys.heldAfter([], 'Shift_L', true), ['Shift']);
    assert.deepEqual(keys.heldAfter(['Shift', 'Ctrl'], 'Shift_R', false), ['Ctrl']);
    assert.deepEqual(keys.heldAfter(['Alt'], 'a', true), ['Alt']);
    assert.equal(keys.chordLabel(['Ctrl'], 'comma'), 'Ctrl+,');
    assert.equal(keys.chordLabel([], 'p'), 'P');
    assert.equal(keys.chordLabel([], 'Shift_L'), null);
});

test('the tray model reads icons, shortcuts and emblems (TRAY-02, TRAY-13, ONB-11)', () => {
    const menu = tray.parseTray(JSON.stringify({
        icon: {kind: 'app'}, overlay: 'warning', visible: true,
        items: [
            {id: 'separator:0', kind: 'separator'},
            {id: 'primary:picker', kind: 'radio', label: 'Picker', shortcut: 'P', checked: true},
            {id: 'separator:1', kind: 'separator'},
            {id: 'separator:2', kind: 'separator'},
            {id: 'more', kind: 'submenu', label: 'More', children: [
                {id: 'quit', kind: 'action', label: 'Quit Wye', shortcut: 'Ctrl+Q'},
                {id: 'off', kind: 'action', label: 'Off', shortcut: '9', enabled: false},
            ]},
            {id: 'separator:3', kind: 'separator'},
        ],
    }));
    assert.equal(menu.icon, tray.APP_ICON);
    assert.equal(menu.warning, true);
    assert.equal(tray.parseTray('{"items":[]}').icon, tray.PICKER_ICON);
    assert.equal(tray.parseTray('{"icon":{"kind":"theme","name":"firefox"},"items":[]}').icon, 'firefox');
    assert.equal(tray.itemForShortcut(menu.items, 'p')?.id, 'primary:picker');
    assert.equal(tray.itemForShortcut(menu.items, 'Ctrl+q')?.id, 'quit');
    assert.equal(tray.itemForShortcut(menu.items, '9'), null);
    assert.equal(tray.itemForShortcut(menu.items, null), null);
    assert.deepEqual(tray.tidySeparators(menu.items).map(i => i.id), ['primary:picker', 'separator:1', 'more']);
    assert.equal(tray.hasIcons(menu.items), false);
    assert.ok(tray.WINDOW_ITEMS.has('settings'));
});

test('Wye windows are recognised by any of their IDs', () => {
    assert.equal(model.isWyeWindow([null, 'dev.soldunov.wye']), true);
    assert.equal(model.isWyeWindow(['dev.soldunov.wye.Gtk.desktop']), true);
    assert.equal(model.isWyeWindow(['dev.soldunov.wyeish', 'firefox']), false);
});
