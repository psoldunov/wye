// The tray menu's pure model (01-tray-menu.md): the service's `Tray`
// property read into icons and menu entries. No GNOME imports: shared by
// the Shell extension and `test-model.mjs`.
import {decode} from './model.mjs';

// TRAY-02, GEN-02: theme names and the extension's bundled stand-ins for a
// system without Wye's icons installed.
export const APP_ICON = 'dev.soldunov.wye-symbolic';
export const PICKER_ICON = 'dev.soldunov.wye-picker-symbolic';
export const BUNDLED = {
    [APP_ICON]: 'wye-symbolic.svg',
    [PICKER_ICON]: 'wye-picker-symbolic.svg',
};

// Tray items that open a Wye window; the extension raises it (TRAY-16).
export const WINDOW_ITEMS = new Set(['settings', 'history', 'test-rules', 'set-up', 'about']);

const KINDS = new Set(['action', 'header', 'radio', 'separator', 'submenu']);

function object(value) {
    return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function item(value) {
    if (!object(value) || typeof value.id !== 'string' || !KINDS.has(value.kind))
        throw new Error('Invalid tray item');
    if (value.children !== undefined && !Array.isArray(value.children))
        throw new Error('Invalid tray children');
    return {
        id: value.id,
        kind: value.kind,
        label: typeof value.label === 'string' ? value.label : '',
        icon: typeof value.icon === 'string' && value.icon !== '' ? value.icon : null,
        shortcut: typeof value.shortcut === 'string' && value.shortcut !== '' ? value.shortcut : null,
        enabled: value.enabled !== false,
        checked: value.checked === true,
        children: (value.children ?? []).map(item),
    };
}

/**
 * Reads the `Tray` property (docs/dbus-api.md).
 *
 * @param {string} json
 */
export function parseTray(json) {
    const value = decode(json, 'tray');
    if (!Array.isArray(value.items))
        throw new Error('Invalid tray items');
    const icon = object(value.icon) ? value.icon : {kind: 'picker'};
    return {
        icon: icon.kind === 'theme' && typeof icon.name === 'string' && icon.name !== ''
            ? icon.name : icon.kind === 'app' ? APP_ICON : PICKER_ICON,
        // TRAY-18, ONB-11: the warning emblem.
        warning: value.overlay === 'warning',
        visible: value.visible !== false,
        items: value.items.map(item),
    };
}

/**
 * The enabled action or radio item whose shortcut is this chord (TRAY-13,
 * KEY-51), searching submenus too.
 *
 * @param {object[]} items
 * @param {string|null} chord as `chordLabel` shows it
 * @returns {object|null}
 */
export function itemForShortcut(items, chord) {
    if (!chord)
        return null;
    for (const entry of items) {
        if (entry.enabled && (entry.kind === 'action' || entry.kind === 'radio') &&
            entry.shortcut?.toLowerCase() === chord.toLowerCase())
            return entry;
        const nested = itemForShortcut(entry.children, chord);
        if (nested)
            return nested;
    }
    return null;
}

/**
 * Whether these held modifiers make a primary-browser row open the browser
 * rather than select it: Ctrl or Shift, not Alt or Super (TRAY-20).
 *
 * @param {string[]} mods as `Keys.sortModifiers` gives them
 */
export function opensWith(mods) {
    return mods.includes('Ctrl') || mods.includes('Shift');
}

/**
 * The mark a radio row shows: none while a click opens rather than selects,
 * so the row keeps the mark's room without the mark (TRAY-21).
 *
 * @param {boolean} checked
 * @param {boolean} opening as `opensWith` tells
 * @returns {'dot'|'no-dot'|'none'}
 */
export function radioMark(checked, opening) {
    if (opening)
        return 'none';
    return checked ? 'dot' : 'no-dot';
}

/**
 * Whether entries at one level show icons, so the others keep the column
 * and every label lines up (TRAY-14).
 *
 * @param {object[]} items
 */
export function hasIcons(items) {
    return items.some(entry => entry.icon !== null);
}

/**
 * Drops separators at either end and doubled ones, which a hidden item can
 * leave behind.
 *
 * @param {object[]} items
 */
export function tidySeparators(items) {
    const out = [];
    for (const entry of items) {
        if (entry.kind === 'separator' && (out.length === 0 || out.at(-1).kind === 'separator'))
            continue;
        out.push(entry);
    }
    while (out.at(-1)?.kind === 'separator')
        out.pop();
    return out;
}
