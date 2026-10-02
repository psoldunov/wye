// Focus for what Wye opens.
//
// PICK-29: the browser the picker chooses gets an activation token minted
// by the Shell for the user's click or key press, sent with `PickerChose`
// (`activation-token`); the service hands it to the browser.
//
// TRAY-16: the windows the tray and the picker's menus open (Settings,
// History, the rule editor…) are started by the service without a token,
// so GNOME's focus-stealing prevention would show "Wye is ready" instead.
// The user asked for them, so the extension activates the Wye window that
// appears, or asks for attention, shortly after the request.
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {isWyeWindow} from './model.mjs';

// How long after a request a Wye window counts as the one asked for.
const EXPECT_MS = 8000;
// When to look again for a Wye window waiting for attention.
const SETTLE_MS = [0, 300, 1000];

/**
 * The time of an input event, or now: Mutter refuses a token whose time is
 * zero, which is what the Shell reports outside an event.
 *
 * @param {import('gi://Clutter').Event|null} event
 */
export function eventTime(event) {
    return event?.get_time() || global.display.get_current_time_roundtrip();
}

/**
 * An XDG activation token for launching `desktopId`, tied to the input
 * event at `time` (PICK-29). Null when the app is unknown here.
 *
 * @param {string|null} desktopId
 * @param {number} time the event's timestamp
 * @returns {string|null}
 */
export function activationToken(desktopId, time) {
    if (!desktopId)
        return null;
    // Not installed here: no token, the service launches without one.
    const info = Shell.AppSystem.get_default().lookup_app(desktopId)?.get_app_info();
    if (!info)
        return null;
    const context = global.create_app_launch_context(time || eventTime(null), -1);
    return context.get_startup_notify_id(info, []) ?? null;
}

function windowIds(window) {
    const app = Shell.WindowTracker.get_default().get_window_app(window);
    return [window.get_gtk_application_id(), window.get_wm_class(),
        window.get_wm_class_instance(), window.get_sandboxed_app_id(), app?.get_id()];
}

export class WindowActivator {
    constructor() {
        this._deadline = 0;
        this._timeoutId = 0;
        this._pending = new Set();
        this._settleIds = new Set();
    }

    /** A Wye window is about to open: raise it when it appears. */
    expect() {
        this._deadline = GLib.get_monotonic_time() + EXPECT_MS * 1000;
        if (this._timeoutId) {
            GLib.source_remove(this._timeoutId);
        } else {
            global.display.connectObject(
                'window-created', (_display, window) => this._created(window),
                'window-demands-attention', (_display, window) => this._attention(window),
                'window-marked-urgent', (_display, window) => this._attention(window),
                this);
        }
        this._timeoutId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, EXPECT_MS, () => {
            this._timeoutId = 0;
            this._stop();
            return GLib.SOURCE_REMOVE;
        });
    }

    /**
     * The request was carried out: a Wye window that already asked for
     * attention before (and so will not ask again) is the one.
     */
    settle() {
        // The window host may present it a moment after the reply.
        for (const delay of SETTLE_MS) {
            const id = GLib.timeout_add(GLib.PRIORITY_DEFAULT, delay, () => {
                this._settleIds.delete(id);
                this._raiseWaiting();
                return GLib.SOURCE_REMOVE;
            });
            this._settleIds.add(id);
        }
    }

    _raiseWaiting() {
        if (!this._active())
            return;
        const window = global.display.list_all_windows().find(w =>
            (w.demands_attention || w.urgent) && isWyeWindow(windowIds(w)));
        if (window)
            this._attention(window);
    }

    _active() {
        return GLib.get_monotonic_time() < this._deadline;
    }

    _created(window) {
        if (!this._active())
            return;
        // The IDs and the focus decision come once the window is shown.
        this._pending.add(window);
        window.connectObject(
            'shown', () => {
                this._forget(window);
                this._attention(window);
            },
            'unmanaged', () => this._forget(window), this);
    }

    _forget(window) {
        window.disconnectObject(this);
        this._pending.delete(window);
    }

    _attention(window) {
        if (!this._active() || !window || window.has_focus() || !isWyeWindow(windowIds(window)))
            return;
        Main.activateWindow(window);
        this._stop();
    }

    _stop() {
        this._deadline = 0;
        global.display.disconnectObject(this);
        for (const window of this._pending)
            window.disconnectObject(this);
        this._pending.clear();
        if (this._timeoutId) {
            GLib.source_remove(this._timeoutId);
            this._timeoutId = 0;
        }
        for (const id of this._settleIds)
            GLib.source_remove(id);
        this._settleIds.clear();
    }

    destroy() {
        this._stop();
    }
}
