// The session helper's pure rules (`SessionHelper1`, docs/dbus-api.md):
// what of the clipboard may leave the Shell. No GNOME imports: shared by
// the Shell extension and `test-model.mjs`.

// Password managers mark secrets with this format (EXT-12).
export const PASSWORD_HINT = 'x-kde-passwordManagerHint';

// The longest clipboard text worth handing on: a link, not a document.
export const MAX_TEXT = 8192;

// What Wye does with the clipboard needs links only: `http` and `https`
// for "Open URL from Clipboard", the tracking cleanup and Songlink
// (wye-core `clipboard_link`), `mailto:` for EXT-13. Anything else stays in
// the Shell, so a password copied from a manager that sets no hint does too.
const WEB_LINK = /^https?:\/\/[^/?#@\s]+/i;
const MAILTO = /^mailto:[^@?\s][^?\s]*@/i;

/**
 * Whether one trimmed line is a link Wye can use: a single token, `http` or
 * `https` with a host, or `mailto:` with an address.
 *
 * @param {string} line
 */
export function isLink(line) {
    return !/\s/.test(line) && (WEB_LINK.test(line) || MAILTO.test(line));
}

/**
 * Whether content offered in these formats must stay in the Shell: a
 * password manager's secret, or rich content with an image (EXT-12).
 *
 * @param {string[]} mimetypes
 */
export function isBlocked(mimetypes) {
    return mimetypes.some(type => type === PASSWORD_HINT || type.startsWith('image/'));
}

/**
 * The clipboard text the service may see: one trimmed line of plain text
 * that is a link (`isLink`) and no marked secret; otherwise empty (the only
 * clipboard content Wye uses, IN-02, TRAY-10, EXT-12, EXT-13).
 *
 * @param {string|null} text
 * @param {string[]} mimetypes
 */
export function offeredText(text, mimetypes) {
    if (!text || isBlocked(mimetypes))
        return '';
    const line = text.trim();
    if (line.length > MAX_TEXT || !isLink(line))
        return '';
    return line;
}

/**
 * The pointer in its monitor's logical pixels (PICK-02). Mutter lays
 * monitors out in logical pixels (stage pixels are logical) or in physical
 * ones (stage pixels are the monitor's device pixels); the Shell's UI scale
 * is above 1 only in the second case.
 *
 * @param {{x: number, y: number}} pointer stage pixels
 * @param {{x: number, y: number}} monitor its origin, stage pixels
 * @param {number} uiScale `St.ThemeContext.scale_factor`
 * @param {number} monitorScale the monitor's scale
 */
export function logicalPointer(pointer, monitor, uiScale, monitorScale) {
    const divisor = uiScale > 1 ? monitorScale : 1;
    return {
        x: Math.round((pointer.x - monitor.x) / divisor),
        y: Math.round((pointer.y - monitor.y) / divisor),
    };
}
