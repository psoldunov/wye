// The picker's pure model (02-picker.md): request parsing, metrics, the URL
// line, held-modifier choices, tile and overflow menus, placement. A port of
// wye-core's picker and wye-ui's picker view so GNOME and KDE agree. No
// GNOME imports: shared by the Shell extension and `test-model.mjs`.
import {canonicalKey, displayKey} from './keys.mjs';

export const TILES_PER_ROW = 8;
// The longest URL line, in characters, before the middle is cut (PICK-09).
export const URL_LINE_CHARS = 64;
const ELLIPSIS = '…';

// PICK-11 target metrics: icon, tile pitch and profile badge, in pixels.
export const METRICS = {
    small: {icon: 24, pitch: 36, badge: 14},
    medium: {icon: 32, pitch: 48, badge: 18},
    large: {icon: 40, pitch: 60, badge: 24},
};

// PICK-14 hint lines.
export const HINTS = {
    private: 'Open in a private window',
    background: 'Open in the background',
    'new-window': 'Open in a new window',
};

function object(value) {
    return value !== null && typeof value === 'object' && !Array.isArray(value);
}

/**
 * Parses a JSON payload that must be an object.
 *
 * @param {string} json
 * @param {string} kind what it is, for the error
 */
export function decode(json, kind) {
    let value;
    try {
        value = JSON.parse(json);
    } catch (error) {
        throw new Error(`Invalid ${kind} JSON`, {cause: error});
    }
    if (!object(value))
        throw new Error(`Invalid ${kind}: expected object`);
    return value;
}

function badge(value) {
    if (!object(value))
        return null;
    if (typeof value.image === 'string' && value.image.startsWith('/'))
        return {image: value.image};
    if (typeof value.initial === 'string' && value.initial !== '') {
        const color = typeof value.color === 'string' && /^#[0-9a-f]{6}$/i.test(value.color)
            ? value.color : null;
        return {initial: [...value.initial][0], color};
    }
    return null;
}

function capabilities(value) {
    const caps = object(value) ? value : {};
    return {private: caps.private === true, newWindow: caps.newWindow === true};
}

// One target: a tile or an Open In entry. A tile without a usable target is
// left out, as the KDE view does.
function entry(value, withHotkey) {
    if (!object(value) || !object(value.target) || typeof value.name !== 'string')
        return null;
    const hotkey = withHotkey && typeof value.hotkey === 'string' ? canonicalKey(value.hotkey) : null;
    return {
        target: value.target,
        name: value.name,
        icon: typeof value.icon === 'string' && value.icon !== '' ? value.icon : null,
        badge: badge(value.badge),
        hotkey,
        hotkeyLabel: hotkey ? displayKey(hotkey) : '',
        caps: capabilities(value.capabilities),
    };
}

/**
 * Splits a link for the URL line (PICK-09), as wye-core's `LinkParts`:
 * host without `www.`, then path, query and fragment.
 *
 * @param {string} full
 * @param {string} host the service's host, used when `full` is not a URL
 * @param {string} rest
 */
export function linkParts(full, host = '', rest = '') {
    // GJS has no URL class; this reads what the URL line needs.
    const match = /^[a-z][a-z0-9+.-]*:(?:\/\/([^/?#]*))?([^?#]*)(\?[^#]*)?(#.*)?$/i.exec(full.trim());
    if (!match)
        return {host, rest};
    const [, authority, path = '', query = '', fragment = ''] = match;
    const name = (authority ?? '').replace(/^.*@/, '').replace(/:\d*$/, '').toLowerCase();
    if (!name)
        return {host: path, rest: ''};
    return {
        host: name.replace(/^www\./, ''),
        rest: (path === '/' ? '' : path) + query + fragment,
    };
}

/**
 * Cuts `text` to `max` characters with an ellipsis in the middle.
 *
 * @param {string} text
 * @param {number} max
 */
export function middleTruncate(text, max) {
    const chars = [...text];
    if (chars.length <= max)
        return text;
    if (max <= 0)
        return '';
    if (max === 1)
        return ELLIPSIS;
    const keep = max - 1;
    const head = Math.ceil(keep / 2);
    const tail = keep - head;
    return chars.slice(0, head).join('') + ELLIPSIS + (tail ? chars.slice(-tail).join('') : '');
}

/**
 * Host and rest cut to `max` characters, the rest first (PICK-09).
 *
 * @param {{host: string, rest: string}} parts
 * @param {number} max
 */
export function truncateLink({host, rest}, max) {
    const hostLength = [...host].length;
    if (hostLength + [...rest].length <= max)
        return {host, rest};
    if (hostLength >= max)
        return {host: middleTruncate(host, max), rest: ''};
    return {host, rest: middleTruncate(rest, max - hostLength)};
}

/**
 * Reads a `PickerRequest` (docs/dbus-api.md `ShowPicker`) into what the
 * picker draws.
 *
 * @param {string} json
 */
export function parsePicker(json) {
    const value = decode(json, 'picker');
    if (!object(value.url) || typeof value.url.full !== 'string')
        throw new Error('Invalid picker request: no url');
    if (value.tiles !== undefined && !Array.isArray(value.tiles))
        throw new Error('Invalid picker tiles');
    if (value.overflow !== undefined && !Array.isArray(value.overflow))
        throw new Error('Invalid picker overflow');
    const settings = object(value.settings) ? value.settings : {};
    const size = METRICS[settings.iconSize] ? settings.iconSize : 'medium';
    const groups = (value.overflow ?? []).filter(object).map(group => ({
        label: typeof group.label === 'string' ? group.label : '',
        entries: (Array.isArray(group.tiles) ? group.tiles : [])
            .map(tile => entry(tile, false)).filter(Boolean),
    })).filter(group => group.entries.length > 0);
    const source = object(value.source) && typeof value.source.name === 'string'
        ? {name: value.source.name, icon: typeof value.source.icon === 'string' ? value.source.icon : ''}
        : {name: '', icon: ''};
    const link = truncateLink(linkParts(value.url.full, value.url.host ?? '', value.url.rest ?? ''),
        URL_LINE_CHARS);
    return {
        tiles: (value.tiles ?? []).map(tile => entry(tile, true)).filter(Boolean),
        overflow: groups,
        size,
        metrics: METRICS[size],
        showNames: settings.showNames !== false,
        showUrl: settings.showUrl === true,
        showBadge: settings.showBadge !== false,
        url: {...link, full: value.url.full, sourceName: source.name, sourceIcon: source.icon},
        keys: object(value.keys) ? value.keys : {},
        held: Array.isArray(value.held) ? value.held.filter(m => typeof m === 'string') : [],
        preview: value.preview === true,
    };
}

/**
 * Whether a target can open the `mode` way; the background is best effort
 * for every target (LAUNCH-04).
 *
 * @param {string} mode
 * @param {{caps: {private: boolean, newWindow: boolean}}} entry
 */
export function supports(mode, entry) {
    switch (mode) {
    case 'private':
        return entry.caps.private;
    case 'new-window':
        return entry.caps.newWindow;
    default:
        return true;
    }
}

/**
 * The target and `PickerChose` options for choosing `entry` the `mode` way
 * (KEY-13, PICK-32, PICK-33): a private window turns `{"app"}` into its
 * private target; an unsupported mode opens normally.
 *
 * @param {object} entry
 * @param {string|null} mode
 * @returns {{target: object, options: {background?: boolean, 'new-window'?: boolean}}}
 */
export function choose(entry, mode) {
    const plain = {target: entry.target, options: {}};
    if (!mode || !supports(mode, entry))
        return plain;
    if (mode === 'private')
        return typeof entry.target.app === 'string' ? {target: {private: entry.target.app}, options: {}} : plain;
    return {target: entry.target, options: {[mode]: true}};
}

/**
 * The desktop ID a target launches, for its activation token (PICK-29).
 *
 * @param {object} target configuration JSON
 * @returns {string|null}
 */
export function desktopId(target) {
    const id = typeof target?.app === 'string' ? target.app
        : typeof target?.private === 'string' ? target.private
            : typeof target?.profile?.app === 'string' ? target.profile.app : null;
    if (!id)
        return null;
    return id.endsWith('.desktop') ? id : `${id}.desktop`;
}

/**
 * The tile's context menu (PICK-30): Open, each open variant the target
 * supports, a separator, Make Primary Browser.
 *
 * @param {object} entry
 * @returns {{action: string, label: string}[]} a separator has an empty action
 */
export function tileMenu(entry) {
    const offerPrivate = entry.caps.private && typeof entry.target.private !== 'string';
    return [
        {action: 'open', label: 'Open'},
        ...offerPrivate ? [{action: 'open-private', label: 'Open in Private Window'}] : [],
        ...entry.caps.newWindow ? [{action: 'open-new-window', label: 'Open in New Window'}] : [],
        {action: 'open-background', label: 'Open in Background'},
        {action: '', label: ''},
        {action: 'make-primary', label: 'Make Primary Browser'},
    ];
}

// The mode each tile-menu action opens with.
export const TILE_ACTION_MODES = {
    open: null,
    'open-private': 'private',
    'open-new-window': 'new-window',
    'open-background': 'background',
};

/**
 * The Open In list (PICK-08, PICK-28): each group's label as a heading
 * (when it has one), then its targets.
 *
 * @param {{label: string, entries: object[]}[]} groups
 */
export function openInRows(groups) {
    return groups.flatMap((group, g) => [
        ...group.label ? [{kind: 'header', label: group.label}] : [],
        ...group.entries.map((e, i) => ({kind: 'item', label: e.name, icon: e.icon, group: g, item: i})),
    ]);
}

// KDE's picker writes the names in Plasma's small font (8 pt, so 10.67 px)
// and caps a tile at twice the pitch (PICK-05). GNOME's caption font is
// larger; the cap keeps KDE's proportion of tile to name font.
export const KDE_NAME_PX = 8 * 96 / 72;

/**
 * Every tile's width (PICK-05): as wide as the longest name needs, from the
 * pitch up to twice the pitch, scaled from KDE's name font to this one.
 *
 * @param {number} widestName the natural width of the longest name, px
 * @param {{pitch: number}} metrics
 * @param {number} padding the tile's inner padding, px
 * @param {boolean} showNames
 * @param {number} [namePx] the names' font size, px
 */
export function tileWidth(widestName, metrics, padding, showNames, namePx = KDE_NAME_PX) {
    if (!showNames)
        return metrics.pitch;
    const cap = metrics.pitch * 2 * Math.max(1, namePx / KDE_NAME_PX);
    const wanted = Math.ceil(widestName) + 2 * padding + 2;
    return Math.round(Math.max(metrics.pitch, Math.min(wanted, cap)));
}

function graphemes(text) {
    if (typeof Intl?.Segmenter === 'function')
        return Array.from(new Intl.Segmenter(undefined, {granularity: 'grapheme'}).segment(text), s => s.segment);
    return Array.from(text);
}

/**
 * The text cut at the end to what `fits` (PICK-05): the longest start that
 * fits with "…" after it, without a space before the "…". The whole text
 * when it fits; "…" alone when nothing else does.
 *
 * @param {string} text
 * @param {(candidate: string) => boolean} fits
 */
export function fitEnd(text, fits) {
    if (fits(text))
        return text;
    const parts = graphemes(text);
    const cut = count => `${parts.slice(0, count).join('').trimEnd()}${ELLIPSIS}`;
    let low = 0;
    let high = parts.length - 1;
    while (low < high) {
        const middle = Math.ceil((low + high) / 2);
        if (fits(cut(middle)))
            low = middle;
        else
            high = middle - 1;
    }
    return cut(low);
}

/**
 * Tiles per row (PICK-13): up to eight, fewer when a row would not fit.
 *
 * @param {number} count tiles
 * @param {number} width the tile width
 * @param {number} spacing between tiles
 * @param {number} room the width a row may take
 */
export function columns(count, width, spacing, room) {
    const fit = Math.floor((room + spacing) / (width + spacing));
    return Math.max(1, Math.min(count, TILES_PER_ROW, fit));
}

/**
 * Splits tiles into rows of `perRow`.
 *
 * @template T
 * @param {T[]} items
 * @param {number} perRow
 * @returns {T[][]}
 */
export function rows(items, perRow) {
    const out = [];
    for (let i = 0; i < items.length; i += perRow)
        out.push(items.slice(i, i + perRow));
    return out;
}

/**
 * Where the panel goes (PICK-02): centred on the pointer, kept inside the
 * work area with a margin.
 *
 * @param {{x: number, y: number}} pointer
 * @param {{width: number, height: number}} size
 * @param {{x: number, y: number, width: number, height: number}} area
 * @param {number} margin
 */
export function place(pointer, size, area, margin) {
    const clamp = (value, low, high) => Math.max(low, Math.min(value, Math.max(low, high)));
    return {
        x: Math.round(clamp(pointer.x - size.width / 2, area.x + margin,
            area.x + area.width - size.width - margin)),
        y: Math.round(clamp(pointer.y - size.height / 2, area.y + margin,
            area.y + area.height - size.height - margin)),
    };
}

/**
 * Whether a badge colour is light, so its initial is drawn dark (PICK-06).
 *
 * @param {string} color `#rrggbb`
 */
export function isLight(color) {
    const [r, g, b] = [1, 3, 5].map(i => parseInt(color.slice(i, i + 2), 16) / 255);
    const linear = c => c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    return 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b) > 0.4;
}

/**
 * Whether a window belongs to Wye: its Wayland app ID, GTK application ID,
 * WM class or the app the Shell matched it to starts with Wye's ID.
 *
 * @param {(string|null|undefined)[]} ids
 */
export function isWyeWindow(ids) {
    return ids.some(id => typeof id === 'string' &&
        /^dev\.soldunov\.wye(\.|$)/i.test(id.replace(/\.desktop$/, '')));
}
