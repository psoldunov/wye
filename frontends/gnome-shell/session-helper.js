// The session helper (`dev.soldunov.wye.SessionHelper1`, docs/dbus-api.md):
// what the service cannot ask Mutter for, answered by the Shell. Mutter has
// no data-control protocol, no layer shell and no pointer or focus query,
// so on GNOME this is how Wye reads and writes the clipboard (IN-02 to
// IN-04, TRAY-10, EXT-12 to EXT-15), knows the held modifiers (BRW-03,
// RUL-27, KEY-06), the pointer (PICK-02) and the focused app (source-app
// step 4).
//
// Privacy: extension.js lets only the owner of `dev.soldunov.wye` call
// this. The clipboard is read when the service asks, and watched only
// while it asked to (a copy-time rewrite is on); each change goes to that
// caller alone, never as a broadcast. Secrets and images never leave the
// Shell (session-model.mjs).
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {modifiersOf} from './picker.js';
import {logicalPointer, offeredText} from './session-model.mjs';

export const HELPER_IFACE = 'dev.soldunov.wye.SessionHelper1';
export const HELPER_XML = `<node><interface name="${HELPER_IFACE}">
  <method name="QueryPointer">
    <arg name="x" type="i" direction="out"/><arg name="y" type="i" direction="out"/>
    <arg name="output" type="s" direction="out"/>
  </method>
  <method name="QueryModifiers"><arg name="modifiers" type="as" direction="out"/></method>
  <method name="FocusedApp"><arg name="desktop_id" type="s" direction="out"/></method>
  <method name="ReadClipboard"><arg name="text" type="s" direction="out"/></method>
  <method name="WriteClipboard"><arg name="text" type="s" direction="in"/></method>
  <method name="WatchClipboard"><arg name="watch" type="b" direction="in"/></method>
  <signal name="ClipboardChanged"><arg name="text" type="s"/></signal>
</interface></node>`;

const CLIPBOARD = St.ClipboardType.CLIPBOARD;

// The connector name of the monitor with this index, or '' when the Shell
// does not say.
function connectorOf(index) {
    try {
        const manager = global.backend.get_monitor_manager();
        const logical = manager.get_logical_monitors().find(m => m.get_number() === index);
        return logical?.get_monitors()[0]?.get_connector() ?? '';
    } catch (error) {
        console.debug(`Wye helper: no connector for monitor ${index}: ${error.message}`);
        return '';
    }
}

// The clipboard's text and formats.
function readClipboard() {
    const clipboard = St.Clipboard.get_default();
    const mimetypes = clipboard.get_mimetypes(CLIPBOARD) ?? [];
    return new Promise(resolve => {
        clipboard.get_text(CLIPBOARD, (_clipboard, text) => resolve(offeredText(text, mimetypes)));
    });
}

export class SessionHelper {
    /**
     * @param {(destination: string, text: string) => void} emit sends
     *   `ClipboardChanged` to one bus name
     */
    constructor(emit) {
        this._emit = emit;
        this._watcher = null;
        this._selectionId = 0;
        // Each change's read; only the newest may be sent.
        this._serial = 0;
    }

    /** PICK-02: the pointer, in its monitor's logical coordinates. */
    queryPointer() {
        const [x, y] = global.get_pointer();
        const monitor = Main.layoutManager.monitors.find(m =>
            x >= m.x && x < m.x + m.width && y >= m.y && y < m.y + m.height);
        const output = monitor ? connectorOf(monitor.index) : '';
        if (!output)
            throw new Error('The Shell does not name the monitor under the pointer');
        const uiScale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        const logical = logicalPointer({x, y}, monitor, uiScale,
            global.display.get_monitor_scale(monitor.index));
        return new GLib.Variant('(iis)', [logical.x, logical.y, output]);
    }

    /** KEY-06: the modifiers held now, as Wye names them. */
    queryModifiers() {
        const [, , state] = global.get_pointer();
        return new GLib.Variant('(as)', [modifiersOf(state)]);
    }

    /** Source-app step 4: the focused window's app; '' when unknown. */
    focusedApp() {
        const window = global.display.focus_window;
        const app = window ? Shell.WindowTracker.get_default().get_window_app(window) : null;
        // A window-backed app has no desktop entry, only a made-up ID.
        const id = app && !app.is_window_backed() ? app.get_id() ?? '' : '';
        return new GLib.Variant('(s)', [id]);
    }

    /** IN-02, TRAY-10: one line of plain text, or ''. */
    async readClipboard() {
        return new GLib.Variant('(s)', [await readClipboard()]);
    }

    /** EXT-12, PICK-28: the service's rewrite or copied link. */
    writeClipboard(text) {
        St.Clipboard.get_default().set_text(CLIPBOARD, text);
    }

    /**
     * EXT-12: send each copied line to `caller` while `watch` holds.
     *
     * @param {boolean} watch
     * @param {string} caller the service's unique name
     */
    watchClipboard(watch, caller) {
        this._watcher = watch ? caller : null;
        const selection = global.display.get_selection();
        if (watch && !this._selectionId) {
            this._selectionId = selection.connect('owner-changed', (_selection, type) => {
                if (type === Meta.SelectionType.SELECTION_CLIPBOARD)
                    this._changed();
            });
        } else if (!watch && this._selectionId) {
            selection.disconnect(this._selectionId);
            this._selectionId = 0;
        }
    }

    async _changed() {
        const watcher = this._watcher;
        if (!watcher)
            return;
        const serial = ++this._serial;
        const text = await readClipboard();
        // A later copy's read supersedes this one, whichever ends first.
        if (serial !== this._serial)
            return;
        // Still the same watcher, and something worth sending.
        if (text && this._watcher === watcher)
            this._emit(watcher, text);
    }

    /** The service went away: stop watching for it. */
    forget() {
        this.watchClipboard(false, null);
        // Reads under way are dropped.
        this._serial++;
    }

    destroy() {
        this.forget();
    }
}
