import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';
import Clutter from 'gi://Clutter';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import {parseTray} from './model.mjs';
import {Picker} from './picker.js';

const SERVICE = 'dev.soldunov.wye';
const HOST = 'dev.soldunov.wye.Gnome';
const PATH = '/dev/soldunov/wye';
const HOST_PATH = '/dev/soldunov/wye/Gnome';
const IFACE = 'dev.soldunov.wye1';
const XML = `<node><interface name="dev.soldunov.wye.PickerHost1">
  <method name="ShowPicker"><arg type="s" direction="in"/><arg type="s" direction="in"/></method>
  <method name="ClosePicker"><arg type="s" direction="in"/></method>
  <method name="ShowMenu"><arg type="s" direction="in"/></method>
</interface></node>`;

export default class WyeExtension extends Extension {
    enable() {
        this._enabled = true;
        this._registered = false;
        this._serviceOwner = null;
        this._connection = null;
        this._logo = new Gio.FileIcon({file: Gio.File.new_for_path(`${this.path}/wye-logo.svg`)});
        this._pickerIcon = new Gio.FileIcon({file: Gio.File.new_for_path(
            `${this.path}/wye-picker-symbolic.svg`)});
        this._picker = new Picker((method, signature, args) => this._call(method, signature, args), this._logo);
        this._indicator = new PanelMenu.Button(0.0, 'Wye', false);
        this._icon = new St.Icon({gicon: this._logo, icon_size: 20,
            style_class: 'system-status-icon'});
        this._indicator.add_child(this._icon);
        this._appearance = new Gio.Settings({schema_id: 'org.gnome.desktop.interface'});
        this._appearanceId = this._appearance.connect('changed::color-scheme', () => this._updateAppearance());
        this._updateAppearance();
        this._indicator.menu.actor.connect('captured-event', (_actor, event) => {
            if (!this._indicator.menu.isOpen || event.type() !== Clutter.EventType.KEY_PRESS)
                return Clutter.EVENT_PROPAGATE;
            const state = event.get_state();
            const key = Clutter.keyval_name(event.get_key_symbol());
            const chord = `${state & Clutter.ModifierType.CONTROL_MASK ? 'Ctrl+' : ''}${key}`.toLowerCase();
            const find = items => {
                for (const item of items) {
                    if (item.shortcut?.toLowerCase() === chord && item.enabled !== false &&
                        (item.kind === 'action' || item.kind === 'radio'))
                        return item.id;
                    const nested = find(item.children ?? []);
                    if (nested)
                        return nested;
                }
                return null;
            };
            const id = find(this._tray?.items ?? []);
            if (!id)
                return Clutter.EVENT_PROPAGATE;
            this._activate(id);
            return Clutter.EVENT_STOP;
        });
        this._indicator.menu.connect('open-state-changed', (_menu, open) => {
            if (open) {
                this._refreshTray();
                this._call('ClipboardHasUrl', null, []).then(([hasUrl]) => {
                    if (this._indicator?.menu.isOpen && this._clipboardItem)
                        this._clipboardItem.setSensitive(hasUrl);
                }).catch(error => logError(error, 'Wye clipboard state'));
            }
        });
        Main.panel.addToStatusArea(this.uuid, this._indicator);
        this._indicator.hide();
        this._ownerId = Gio.bus_own_name(Gio.BusType.SESSION, HOST,
            Gio.BusNameOwnerFlags.NONE,
            connection => this._acquired(connection),
            () => {},
            () => this._lost());
    }

    _updateAppearance() {
        if (this._appearance.get_string('color-scheme') === 'prefer-light')
            this._indicator.menu.actor.add_style_class_name('wye-tray-light');
        else
            this._indicator.menu.actor.remove_style_class_name('wye-tray-light');
    }

    _acquired(connection) {
        if (!this._enabled)
            return;
        this._connection = connection;
        this._export = Gio.DBusExportedObject.wrapJSObject(XML, {
            ShowPickerAsync: ([id, request], invocation) =>
                this._hostCall(invocation, () => this._picker.show(id, request)),
            ClosePickerAsync: ([id], invocation) =>
                this._hostCall(invocation, () => this._picker.close(id)),
            ShowMenuAsync: ([menu], invocation) =>
                this._hostCall(invocation, () => this._toggleMenu(menu)),
        });
        this._export.export(connection, HOST_PATH);
        this._changedId = connection.signal_subscribe(SERVICE,
            'org.freedesktop.DBus.Properties', 'PropertiesChanged', PATH, IFACE,
            Gio.DBusSignalFlags.NONE, (_bus, _sender, _path, _iface, _signal, params) => {
                const [name, changed] = params.deepUnpack();
                if (name === IFACE && changed.Tray)
                    this._setTray(changed.Tray.deepUnpack());
            });
        this._watchId = Gio.bus_watch_name_on_connection(connection, SERVICE,
            Gio.BusNameWatcherFlags.NONE,
            (_bus, _name, owner) => this._appeared(owner),
            () => this._vanished());
    }

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
                } catch (_error) {
                    denied();
                    return;
                }
                if (!this._enabled || this._connection !== connection ||
                    owner !== invocation.get_sender()) {
                    denied();
                    return;
                }
                try {
                    action();
                    invocation.return_value(null);
                } catch (error) {
                    invocation.return_dbus_error('dev.soldunov.wye.Error.Failed', error.message);
                }
            });
    }

    _lost() {
        if (this._enabled)
            logError(new Error(`${HOST} name lost`), 'Wye GNOME extension');
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
        this._connection = null;
        this._serviceOwner = null;
        this._registered = false;
    }

    async _appeared(owner) {
        this._serviceOwner = owner;
        try {
            await this._call('RegisterTray', '(s)', ['gnome-extension']);
            if (this._enabled && this._serviceOwner === owner) {
                this._registered = true;
                await this._refreshTray();
            }
        } catch (error) {
            logError(error, 'Wye tray registration');
        }
    }

    _vanished() {
        this._serviceOwner = null;
        this._registered = false;
        this._indicator?.hide();
        this._picker?.close();
    }

    _call(method, signature, args) {
        if (!this._connection || !this._enabled)
            return Promise.reject(new Error('Wye session bus unavailable'));
        return new Promise((resolve, reject) => {
            this._connection.call(SERVICE, PATH, IFACE, method,
                signature ? new GLib.Variant(signature, args) : null, null,
                Gio.DBusCallFlags.NONE, 10000, null, (bus, result) => {
                    try {
                        resolve(bus.call_finish(result).deepUnpack());
                    } catch (error) {
                        reject(error);
                    }
                });
        });
    }

    async _refreshTray() {
        if (!this._serviceOwner)
            return;
        try {
            const [variant] = await new Promise((resolve, reject) => {
                this._connection.call(SERVICE, PATH, 'org.freedesktop.DBus.Properties',
                    'Get', new GLib.Variant('(ss)', [IFACE, 'Tray']), null,
                    Gio.DBusCallFlags.NONE, 10000, null, (bus, result) => {
                        try {
                            resolve(bus.call_finish(result).deepUnpack());
                        } catch (error) {
                            reject(error);
                        }
                    });
            });
            if (this._enabled && this._serviceOwner)
                this._setTray(variant.deepUnpack());
        } catch (error) {
            logError(error, 'Wye tray refresh');
        }
    }

    _setTray(json) {
        let tray;
        try {
            tray = parseTray(json);
        } catch (error) {
            logError(error, 'Wye tray payload');
            return;
        }
        this._tray = tray;
        this._indicator.visible = tray.visible !== false;
        this._icon.gicon = tray.icon?.kind === 'theme'
            ? Gio.ThemedIcon.new_from_names([tray.icon.name, 'web-browser'])
            : tray.icon?.kind === 'picker' ? this._pickerIcon : this._logo;
        this._icon.style_class = tray.overlay === 'warning'
            ? 'system-status-icon wye-warning' : 'system-status-icon';
        this._renderMenu(tray.items);
    }

    _renderMenu(items) {
        this._clipboardItem = null;
        this._indicator.menu.removeAll();
        const brand = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false,
            style_class: 'wye-tray-heading'});
        brand.add_child(new St.Icon({gicon: this._logo, icon_size: 34}));
        const label = new St.BoxLayout({vertical: true});
        label.add_child(new St.Label({text: 'Wye', style_class: 'wye-tray-title'}));
        label.add_child(new St.Label({text: 'Open links your way', style_class: 'wye-tray-subtitle'}));
        brand.add_child(label);
        this._indicator.menu.addMenuItem(brand);
        const add = (destination, entries) => {
            for (const entry of entries) {
                if (entry.kind === 'separator') {
                    destination.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
                } else if (entry.kind === 'header') {
                    destination.addMenuItem(new PopupMenu.PopupSeparatorMenuItem(entry.label));
                } else if (entry.kind === 'submenu') {
                    const submenu = new PopupMenu.PopupSubMenuMenuItem(entry.label ?? 'More');
                    destination.addMenuItem(submenu);
                    add(submenu.menu, entry.children ?? []);
                } else {
                    const item = new PopupMenu.PopupMenuItem(entry.label ?? entry.id);
                    item.add_style_class_name('wye-tray-item');
                    const icon = entry.id === 'primary:picker' ? this._logo : entry.icon
                        ? Gio.ThemedIcon.new_from_names([entry.icon, 'web-browser']) : null;
                    if (icon)
                        item.insert_child_at_index(new St.Icon({gicon: icon, icon_size: 20,
                            style_class: 'wye-tray-icon'}), 1);
                    if (entry.kind === 'radio' && entry.checked)
                        item.setOrnament(PopupMenu.Ornament.DOT);
                    if (entry.shortcut)
                        item.add_child(new St.Label({text: entry.shortcut, style_class: 'wye-shortcut',
                            x_expand: true, x_align: Clutter.ActorAlign.END}));
                    item.setSensitive(entry.enabled !== false);
                    item.connect('activate', () => this._activate(entry.id));
                    destination.addMenuItem(item);
                    if (entry.id === 'clipboard' || entry.id === 'open-clipboard')
                        this._clipboardItem = item;
                }
            }
        };
        add(this._indicator.menu, items);
    }

    _activate(id) {
        this._indicator.menu.close();
        this._call('ActivateTrayItem', '(s)', [id])
            .catch(error => logError(error, `Wye menu action ${id}`));
    }

    _toggleMenu(json) {
        if (this._indicator.menu.isOpen) {
            this._indicator.menu.close();
            return;
        }
        this._setTray(json);
        this._indicator.menu.open();
    }

    disable() {
        this._appearance.disconnect(this._appearanceId);
        this._picker?.destroy();
        this._picker = null;
        this._enabled = false;
        if (this._registered && this._connection) {
            this._connection.call(SERVICE, PATH, IFACE, 'UnregisterTray', null, null,
                Gio.DBusCallFlags.NONE, 3000, null, (bus, result) => {
                    try {
                        bus.call_finish(result);
                    } catch (error) {
                        logError(error, 'Wye tray unregister');
                    }
                });
        }
        this._unwatch();
        if (this._ownerId)
            Gio.bus_unown_name(this._ownerId);
        this._ownerId = 0;
        this._indicator?.destroy();
        this._indicator = null;
    }
}
