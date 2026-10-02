// Wye for GNOME Shell: the picker (02-picker.md) and the tray
// (01-tray-menu.md) drawn by the Shell.
//
// The extension owns `dev.soldunov.wye.Gnome` and exports
// `dev.soldunov.wye.PickerHost1` (docs/dbus-api.md); the service prefers it
// over `wye-ui` while it is owned. It also exports
// `dev.soldunov.wye.SessionHelper1` (session-helper.js): the clipboard,
// held keys, pointer and focused app the service cannot get from Mutter. It watches the service, registers as its
// tray host and answers the picker with `PickerChose`, `PickerCancelled`
// or `PickerAction`.
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import {WindowActivator, activationToken} from './focus.js';
import {cancelPendingActions} from './menus.js';
import {Picker} from './picker.js';
import {HELPER_IFACE, HELPER_XML, SessionHelper} from './session-helper.js';
import {Tray} from './tray.js';
import {WINDOW_ITEMS} from './tray-model.mjs';

const SERVICE = 'dev.soldunov.wye';
const HOST = 'dev.soldunov.wye.Gnome';
const PATH = '/dev/soldunov/wye';
const HOST_PATH = '/dev/soldunov/wye/Gnome';
const IFACE = 'dev.soldunov.wye1';
const CALL_TIMEOUT_MS = 10000;
const HOST_XML = `<node><interface name="dev.soldunov.wye.PickerHost1">
  <method name="ShowPicker"><arg type="s" direction="in"/><arg type="s" direction="in"/></method>
  <method name="ClosePicker"><arg type="s" direction="in"/></method>
  <method name="ShowMenu"><arg type="s" direction="in"/></method>
</interface></node>`;

function callAsync(connection, iface, method, parameters, path = PATH) {
    return new Promise((resolve, reject) => {
        connection.call(SERVICE, path, iface, method, parameters, null,
            Gio.DBusCallFlags.NONE, CALL_TIMEOUT_MS, null, (bus, result) => {
                try {
                    resolve(bus.call_finish(result).deepUnpack());
                } catch (error) {
                    reject(error);
                }
            });
    });
}

export default class WyeExtension extends Extension {
    enable() {
        this._enabled = true;
        this._connection = null;
        this._serviceOwner = null;
        this._registered = false;
        this._activator = new WindowActivator();
        this._helper = new SessionHelper((destination, text) => this._clipboardChanged(destination, text));
        this._picker = new Picker({
            call: (method, signature, args) => this._call(method, signature, args),
            activator: this._activator,
            token: activationToken,
        });
        this._tray = new Tray({
            dir: this.path,
            uuid: this.uuid,
            activate: id => this._activateTrayItem(id),
            fetch: () => this._fetchTray(),
            clipboardHasUrl: () => this._call('ClipboardHasUrl', null, []).then(([hasUrl]) => hasUrl),
        });
        this._ownerId = Gio.bus_own_name(Gio.BusType.SESSION, HOST, Gio.BusNameOwnerFlags.NONE,
            connection => this._acquired(connection), null, () => this._lost());
    }

    disable() {
        // A menu item's action waiting to run would find nothing left.
        cancelPendingActions();
        // The picker answers its request (cancelled) while the bus is there.
        this._picker?.destroy();
        this._picker = null;
        this._enabled = false;
        if (this._registered && this._connection) {
            this._connection.call(SERVICE, PATH, IFACE, 'UnregisterTray', null, null,
                Gio.DBusCallFlags.NONE, 3000, null, (bus, result) => {
                    try {
                        bus.call_finish(result);
                    } catch (error) {
                        console.warn(`Wye tray unregister: ${error.message}`);
                    }
                });
        }
        this._unwatch();
        if (this._ownerId)
            Gio.bus_unown_name(this._ownerId);
        this._ownerId = 0;
        this._tray?.destroy();
        this._tray = null;
        this._helper?.destroy();
        this._helper = null;
        this._activator?.destroy();
        this._activator = null;
    }

    _acquired(connection) {
        if (!this._enabled)
            return;
        this._connection = connection;
        this._export = Gio.DBusExportedObject.wrapJSObject(HOST_XML, {
            ShowPickerAsync: ([id, request], invocation) =>
                this._hostCall(invocation, () => this._picker.show(id, request)),
            ClosePickerAsync: ([id], invocation) =>
                this._hostCall(invocation, () => this._picker.close(id)),
            ShowMenuAsync: ([menu], invocation) =>
                this._hostCall(invocation, () => this._tray.toggle(menu)),
        });
        this._export.export(connection, HOST_PATH);
        const helper = this._helper;
        this._helperExport = Gio.DBusExportedObject.wrapJSObject(HELPER_XML, {
            QueryPointerAsync: (_args, invocation) => this._hostCall(invocation, () => helper.queryPointer()),
            QueryModifiersAsync: (_args, invocation) => this._hostCall(invocation, () => helper.queryModifiers()),
            FocusedAppAsync: (_args, invocation) => this._hostCall(invocation, () => helper.focusedApp()),
            ReadClipboardAsync: (_args, invocation) => this._hostCall(invocation, () => helper.readClipboard()),
            WriteClipboardAsync: ([text], invocation) =>
                this._hostCall(invocation, () => helper.writeClipboard(text)),
            WatchClipboardAsync: ([watch], invocation) =>
                this._hostCall(invocation, caller => helper.watchClipboard(watch, caller)),
        });
        this._helperExport.export(connection, HOST_PATH);
        this._changedId = connection.signal_subscribe(SERVICE,
            'org.freedesktop.DBus.Properties', 'PropertiesChanged', PATH, IFACE,
            Gio.DBusSignalFlags.NONE, (_bus, _sender, _path, _iface, _signal, params) => {
                const [name, changed] = params.deepUnpack();
                if (name === IFACE && changed.Tray)
                    this._tray?.update(changed.Tray.deepUnpack());
            });
        this._watchId = Gio.bus_watch_name_on_connection(connection, SERVICE,
            Gio.BusNameWatcherFlags.NONE,
            (_bus, _name, owner) => this._appeared(owner),
            () => this._vanished());
    }

    // Only the service's current owner may drive the host and ask the
    // helper. `action` gets the caller's unique name and may return the
    // reply (a GLib.Variant), or a promise of it.
    _hostCall(invocation, action) {
        const connection = this._connection;
        const denied = () => invocation.return_dbus_error(
            'org.freedesktop.DBus.Error.AccessDenied', 'Wye service owner required');
        if (!this._enabled || !connection) {
            denied();
            return;
        }
        // Ask the bus daemon instead of trusting the name-watch cache: the
        // current owner may call before its watch callback arrives.
        connection.call('org.freedesktop.DBus', '/org/freedesktop/DBus',
            'org.freedesktop.DBus', 'GetNameOwner', new GLib.Variant('(s)', [SERVICE]),
            new GLib.VariantType('(s)'), Gio.DBusCallFlags.NONE, 3000, null, (bus, result) => {
                let owner;
                try {
                    [owner] = bus.call_finish(result).deepUnpack();
                } catch {
                    denied();
                    return;
                }
                if (!this._enabled || this._connection !== connection ||
                    owner !== invocation.get_sender()) {
                    denied();
                    return;
                }
                Promise.resolve()
                    .then(() => action(owner))
                    .then(reply => invocation.return_value(reply ?? null))
                    .catch(error => invocation.return_dbus_error(
                        'org.freedesktop.DBus.Error.Failed', error.message));
            });
    }

    // EXT-12: one clipboard change, to the watching service alone.
    _clipboardChanged(destination, text) {
        try {
            this._connection?.emit_signal(destination, HOST_PATH, HELPER_IFACE, 'ClipboardChanged',
                new GLib.Variant('(s)', [text]));
        } catch (error) {
            console.warn(`Wye clipboard change: ${error.message}`);
        }
    }

    _lost() {
        if (this._enabled)
            console.error(`Wye: lost the bus name ${HOST}`);
        this._unwatch();
    }

    _unwatch() {
        if (this._watchId) {
            Gio.bus_unwatch_name(this._watchId);
            this._watchId = 0;
        }
        if (this._changedId) {
            this._connection.signal_unsubscribe(this._changedId);
            this._changedId = 0;
        }
        this._export?.unexport();
        this._export = null;
        this._helperExport?.unexport();
        this._helperExport = null;
        this._helper?.forget();
        this._connection = null;
        this._serviceOwner = null;
        this._registered = false;
    }

    // The service started or restarted: register as its tray host again.
    async _appeared(owner) {
        this._serviceOwner = owner;
        try {
            await this._call('RegisterTray', '(s)', ['gnome-extension']);
            if (!this._enabled || this._serviceOwner !== owner)
                return;
            this._registered = true;
            const json = await this._fetchTray();
            if (json && this._enabled && this._serviceOwner === owner)
                this._tray.update(json);
        } catch (error) {
            console.error(`Wye tray registration: ${error.message}`);
        }
    }

    _vanished() {
        this._serviceOwner = null;
        this._helper?.forget();
        this._registered = false;
        this._tray?.clear();
        this._picker?.close();
    }

    _call(method, signature, args) {
        if (!this._connection || !this._enabled)
            return Promise.reject(new Error('Wye session bus unavailable'));
        return callAsync(this._connection, IFACE, method,
            signature ? new GLib.Variant(signature, args) : null);
    }

    async _fetchTray() {
        if (!this._connection || !this._serviceOwner)
            return null;
        const [variant] = await callAsync(this._connection, 'org.freedesktop.DBus.Properties', 'Get',
            new GLib.Variant('(ss)', [IFACE, 'Tray']));
        return variant.deepUnpack();
    }

    _activateTrayItem(id) {
        // TRAY-16: the window the item opens is raised when it appears.
        if (WINDOW_ITEMS.has(id))
            this._activator.expect();
        this._call('ActivateTrayItem', '(s)', [id])
            .then(() => this._activator?.settle())
            .catch(error => console.error(`Wye menu action ${id}: ${error.message}`));
    }
}
