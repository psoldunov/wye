// Picker keys (KEY-03, KEY-05, KEY-11, KEY-13, KEY-22), a port of
// wye-core's keybinding and picker keymap so the Shell picker answers keys
// exactly as the KDE picker does. Pure: no GNOME imports, tested with Node.

export const MODIFIERS = ['Ctrl', 'Alt', 'Shift', 'Super'];

// KEY-22 defaults; a request that omits an action keeps these.
export const DEFAULT_ACTIONS = {
    open: ['Return', 'KP_Enter', 'space'],
    cancel: ['Escape'],
    next: ['Right', 'Tab'],
    previous: ['Left', 'Shift+Tab'],
    first: ['Home'],
    last: ['End'],
    'copy-link': ['Ctrl+c'],
    more: ['Menu'],
    'create-rule': ['Ctrl+r'],
};
export const DEFAULT_MODIFIER_ACTIONS = {
    private: ['Shift'],
    background: ['Ctrl'],
    'new-window': ['Alt'],
};
// Order of the actions when two share a binding, as in the core.
const ACTION_ORDER = ['open', 'cancel', 'next', 'previous', 'first', 'last',
    'copy-link', 'more', 'create-rule'];
// KEY-05: private wins over background over new window.
export const MODES = ['private', 'background', 'new-window'];

const NAMED = new Map([
    ['Return', 'Return'], ['KP_Enter', 'Enter'], ['space', 'Space'], ['Escape', 'Esc'],
    ['Tab', 'Tab'], ['BackSpace', 'Backspace'], ['Delete', 'Del'], ['Insert', 'Ins'],
    ['Home', 'Home'], ['End', 'End'], ['Page_Up', 'PgUp'], ['Page_Down', 'PgDown'],
    ['Left', 'Left'], ['Right', 'Right'], ['Up', 'Up'], ['Down', 'Down'], ['Menu', 'Menu'],
    ['Print', 'Print'], ['Pause', 'Pause'], ['KP_Add', '+ (Num)'],
    ['KP_Subtract', '- (Num)'], ['KP_Multiply', '* (Num)'], ['KP_Divide', '/ (Num)'],
    ['KP_Decimal', '. (Num)'],
]);
const PUNCTUATION = new Map([
    ['comma', ','], ['period', '.'], ['slash', '/'], ['backslash', '\\'],
    ['semicolon', ';'], ['apostrophe', '\''], ['minus', '-'], ['equal', '='],
    ['plus', '+'], ['bracketleft', '['], ['bracketright', ']'], ['grave', '`'],
]);
const ALIASES = new Map([
    ['enter', 'Return'], ['esc', 'Escape'], ['backtab', 'Tab'], ['iso_left_tab', 'Tab'],
    ['del', 'Delete'], ['ins', 'Insert'], ['pgup', 'Page_Up'], ['prior', 'Page_Up'],
    ['pgdn', 'Page_Down'], ['next', 'Page_Down'], ['pgdown', 'Page_Down'],
    ['backspace', 'BackSpace'], ['spacebar', 'space'],
]);
const MODIFIER_KEYS = new Map([
    ['shift_l', 'Shift'], ['shift_r', 'Shift'], ['control_l', 'Ctrl'], ['control_r', 'Ctrl'],
    ['alt_l', 'Alt'], ['alt_r', 'Alt'], ['meta_l', 'Alt'], ['meta_r', 'Alt'],
    ['super_l', 'Super'], ['super_r', 'Super'], ['hyper_l', 'Super'], ['hyper_r', 'Super'],
    ['iso_level3_shift', null], ['caps_lock', null], ['num_lock', null],
]);
const MODIFIER_ALIASES = new Map([
    ['ctrl', 'Ctrl'], ['control', 'Ctrl'], ['alt', 'Alt'], ['shift', 'Shift'],
    ['super', 'Super'], ['meta', 'Super'], ['logo', 'Super'],
]);
// US-QWERTY characters by evdev code, for non-Latin layouts (KEY-11).
const US_QWERTY = new Map([
    [2, '1'], [3, '2'], [4, '3'], [5, '4'], [6, '5'], [7, '6'], [8, '7'], [9, '8'],
    [10, '9'], [11, '0'], [12, '-'], [13, '='], [16, 'q'], [17, 'w'], [18, 'e'],
    [19, 'r'], [20, 't'], [21, 'y'], [22, 'u'], [23, 'i'], [24, 'o'], [25, 'p'],
    [26, '['], [27, ']'], [30, 'a'], [31, 's'], [32, 'd'], [33, 'f'], [34, 'g'],
    [35, 'h'], [36, 'j'], [37, 'k'], [38, 'l'], [39, ';'], [40, '\''], [41, '`'],
    [43, '\\'], [44, 'z'], [45, 'x'], [46, 'c'], [47, 'v'], [48, 'b'], [49, 'n'],
    [50, 'm'], [51, ','], [52, '.'], [53, '/'], [71, '7'], [72, '8'], [73, '9'],
    [75, '4'], [76, '5'], [77, '6'], [79, '1'], [80, '2'], [81, '3'], [82, '0'],
]);
const EVDEV_OFFSET = 8;

function singleChar(c) {
    if (c === ' ')
        return 'space';
    for (const [name, p] of PUNCTUATION) {
        if (p === c)
            return name;
    }
    return c.toLowerCase();
}

/**
 * The canonical XKB spelling of a key (KEY-03), or null for a modifier, an
 * empty name or text that is not a key.
 *
 * @param {string} name a key name or character
 * @returns {string|null}
 */
export function canonicalKey(name) {
    if (name === ' ')
        return 'space';
    const trimmed = (name ?? '').trim();
    const chars = [...trimmed];
    if (chars.length === 0)
        return null;
    if (chars.length === 1)
        return singleChar(chars[0]);
    const lower = trimmed.toLowerCase();
    if (MODIFIER_KEYS.has(lower) || MODIFIER_ALIASES.has(lower))
        return null;
    for (const canonical of NAMED.keys()) {
        if (canonical.toLowerCase() === lower)
            return canonical;
    }
    if (PUNCTUATION.has(lower))
        return lower;
    if (ALIASES.has(lower))
        return ALIASES.get(lower);
    const fn = /^f(\d+)$/.exec(lower);
    if (fn && Number(fn[1]) >= 1 && Number(fn[1]) <= 35)
        return `F${Number(fn[1])}`;
    const keypad = /^kp_(\d)$/.exec(lower);
    if (keypad)
        return `KP_${keypad[1]}`;
    return /^[A-Za-z0-9_]+$/.test(trimmed) ? trimmed : null;
}

/**
 * How a canonical key is shown: `F`, `Enter`, `,` (KEY-03, PICK-04).
 *
 * @param {string} canonical
 * @returns {string}
 */
export function displayKey(canonical) {
    if (NAMED.has(canonical))
        return NAMED.get(canonical);
    if (PUNCTUATION.has(canonical))
        return PUNCTUATION.get(canonical);
    if ([...canonical].length === 1)
        return canonical.toUpperCase();
    return canonical.replace('KP_', '').replaceAll('_', ' ');
}

/**
 * The modifier a modifier key changes, `null` for lock keys, or
 * `undefined` when the key is not a modifier at all.
 *
 * @param {string} keyName an XKB key name such as `Shift_L`
 * @returns {string|null|undefined}
 */
export function modifierOfKey(keyName) {
    const lower = (keyName ?? '').toLowerCase();
    return MODIFIER_KEYS.has(lower) ? MODIFIER_KEYS.get(lower) : undefined;
}

/**
 * A modifier's name as Wye writes it (`Shift`, `Ctrl`, `Alt`, `Super`), from
 * any spelling the service accepts; null for anything else.
 *
 * @param {string} text
 */
export function modifierName(text) {
    return MODIFIER_ALIASES.get(String(text).trim().toLowerCase()) ?? null;
}

/** @param {Iterable<string>} mods */
export function sortModifiers(mods) {
    const set = new Set(mods);
    return MODIFIERS.filter(m => set.has(m));
}

function sameMods(a, b) {
    return a.length === b.length && a.every((m, i) => m === b[i]);
}

/**
 * Parses a stored binding such as `Ctrl+Shift+o` (KEY-03).
 *
 * @param {string} text
 * @returns {{mods: string[], key: string}|null} null when it cannot be read
 */
export function parseBinding(text) {
    const trimmed = String(text ?? '').trim();
    let head, key;
    if (trimmed.endsWith('++')) {
        [head, key] = [trimmed.slice(0, -2), 'plus'];
    } else if (trimmed === '+') {
        [head, key] = ['', 'plus'];
    } else {
        const at = trimmed.lastIndexOf('+');
        [head, key] = at < 0 ? ['', trimmed] : [trimmed.slice(0, at), trimmed.slice(at + 1)];
    }
    const mods = [];
    for (const part of head.split('+').map(p => p.trim()).filter(Boolean)) {
        const mod = MODIFIER_ALIASES.get(part.toLowerCase());
        if (!mod)
            return null;
        mods.push(mod);
    }
    const canonical = canonicalKey(key);
    return canonical ? {mods: sortModifiers(mods), key: canonical} : null;
}

/**
 * Whether a key event presses `canonical`: its key name, its text, or the
 * Latin key at the same position (KEY-11).
 *
 * @param {{key?: string, text?: string, keycode?: number}} event
 * @param {string} canonical
 */
export function isKey(event, canonical) {
    const same = candidate => candidate !== undefined && candidate !== null &&
        canonicalKey(candidate) === canonical;
    const latin = US_QWERTY.get((event.keycode ?? 0) - EVDEV_OFFSET);
    return same(event.key) || same(event.text) || same(latin);
}

/**
 * The picker's keymap from the request's `keys` (KEY-20). Unreadable
 * bindings are dropped; missing actions keep their defaults.
 *
 * @param {object} keys `PickerRequest.keys`
 */
export function keymap(keys) {
    const actions = {...DEFAULT_ACTIONS, ...keys?.actions ?? {}};
    const modifierActions = {...DEFAULT_MODIFIER_ACTIONS, ...keys?.modifierActions ?? {}};
    const bindings = ACTION_ORDER.map(action => [action,
        (Array.isArray(actions[action]) ? actions[action] : [])
            .map(parseBinding).filter(Boolean)]);
    const held = MODES.map(mode => [mode, sortModifiers(
        (Array.isArray(modifierActions[mode]) ? modifierActions[mode] : [])
            .map(m => MODIFIER_ALIASES.get(String(m).toLowerCase())).filter(Boolean))]);
    return {bindings, held};
}

/**
 * The way of opening exactly these held modifiers select (KEY-05, KEY-13),
 * or null.
 *
 * @param {ReturnType<typeof keymap>} map
 * @param {string[]} mods sorted modifiers
 */
export function modeFor(map, mods) {
    const sorted = sortModifiers(mods);
    if (sorted.length === 0)
        return null;
    return map.held.find(([, set]) => set.length > 0 && sameMods(set, sorted))?.[0] ?? null;
}

function actionFor(map, event, mods) {
    for (const [action, list] of map.bindings) {
        if (list.some(binding => sameMods(binding.mods, mods) && isKey(event, binding.key)))
            return action;
    }
    return null;
}

/**
 * What a key press does (PICK-21, PICK-22, KEY-13), as the core decides it:
 * an exact binding wins; otherwise, with a held-modifier action down, the
 * press is tried without the modifiers; then tile hotkeys.
 *
 * @param {ReturnType<typeof keymap>} map
 * @param {{key?: string, text?: string, keycode?: number, mods: string[]}} event
 * @param {(string|null)[]} hotkeys canonical hotkey per tile
 * @returns {{action?: string, index?: number, mode: string|null}|null}
 */
export function dispatch(map, event, hotkeys) {
    const mods = sortModifiers(event.mods ?? []);
    const exact = actionFor(map, event, mods);
    if (exact)
        return {action: exact, mode: null};
    const mode = modeFor(map, mods);
    if (mods.length > 0 && !mode)
        return null;
    const bare = actionFor(map, event, []);
    if (bare)
        return {action: bare, mode};
    const index = hotkeys.findIndex(hotkey => hotkey && isKey(event, hotkey));
    return index >= 0 ? {index, mode} : null;
}

/**
 * Moves the selection (PICK-22): next and previous wrap, first and last jump.
 *
 * @param {string} action
 * @param {number} selected
 * @param {number} count
 */
export function select(action, selected, count) {
    if (count === 0)
        return 0;
    switch (action) {
    case 'next':
        return (selected + 1) % count;
    case 'previous':
        return (selected + count - 1) % count;
    case 'first':
        return 0;
    case 'last':
        return count - 1;
    default:
        return Math.min(selected, count - 1);
    }
}

/**
 * The modifiers held after a key event: the event's state, which does not
 * yet count the modifier key being pressed or released (KEY-13).
 *
 * @param {string[]} stateMods modifiers in the event's state
 * @param {string} keyName the XKB name of the key
 * @param {boolean} pressed press or release
 */
export function heldAfter(stateMods, keyName, pressed) {
    const set = new Set(stateMods);
    const mod = modifierOfKey(keyName);
    if (mod)
        pressed ? set.add(mod) : set.delete(mod);
    return sortModifiers(set);
}

/**
 * The chord as a menu shows a shortcut (`Ctrl+,`, `P`), for TRAY-13 and
 * KEY-51.
 *
 * @param {string[]} mods
 * @param {string} keyName
 * @returns {string|null}
 */
export function chordLabel(mods, keyName) {
    const canonical = canonicalKey(keyName === 'ISO_Left_Tab' ? 'Tab' : keyName);
    if (!canonical)
        return null;
    return [...sortModifiers(mods), displayKey(canonical)].join('+');
}
