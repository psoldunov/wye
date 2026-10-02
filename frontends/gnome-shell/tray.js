// The tray (01-tray-menu.md): GNOME has no StatusNotifierItem host, so the
// extension draws Wye's panel icon and its menu with the Shell's own
// PopupMenu, from the service's `Tray` property.
import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import Shell from 'gi://Shell';
import St from 'gi://St';
import * as BoxPointer from 'resource:///org/gnome/shell/ui/boxpointer.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import {bundled, gicon} from './icons.js';
import {chordLabel} from './keys.mjs';
import {MenuPages, headingItem, onActivate, pageItem, shortcutLabel, syncStyle, themeScale} from './menus.js';
import {modifiersOf} from './picker.js';
import * as TrayModel from './tray-model.mjs';

// TRAY-14: full-colour app icons, as large as a GNOME menu row allows.
const ITEM_ICON_SIZE = 20;
// Space kept between the pointer menu and the screen edge (TRAY-08).
const POINTER_MENU_MARGIN = 12;
// What a page of the menu shares the screen with: the panel, the menu's
// padding, the back row and the screen margins (logical px).
const PAGE_CHROME = 140;

const Indicator = GObject.registerClass(
class WyeIndicator extends PanelMenu.Button {
    _init(onMiddleClick) {
        super._init(0.5, 'Wye', false);
        // TRAY-07: the primary (and secondary) button opens the menu;
        // TRAY-19: the middle one opens Settings. Shells that click through
        // a gesture get the split; older ones keep their own click handling.
        if (this._clickGesture)
            this._addGestures(onMiddleClick);
        const stack = new St.Widget({layout_manager: new Clutter.BinLayout(),
            y_align: Clutter.ActorAlign.CENTER});
        this.icon = new St.Icon({style_class: 'system-status-icon'});
        stack.add_child(this.icon);
        // ONB-11: an emblem while Wye is not the default browser.
        this.emblem = new St.Icon({icon_name: 'dialog-warning-symbolic', style_class: 'wye-tray-emblem',
            x_align: Clutter.ActorAlign.END, y_align: Clutter.ActorAlign.END, visible: false,
            // Over the icon's corner, without widening the button.
            translation_x: 6, translation_y: 4});
        stack.add_child(this.emblem);
        this.add_child(stack);
        this.menu.actor.add_style_class_name('wye-tray-menu');
    }

    _addGestures(onMiddleClick) {
        this._clickGesture.set_required_button(Clutter.BUTTON_PRIMARY);
        const secondary = new Clutter.ClickGesture();
        secondary.set_recognize_on_press(true);
        secondary.set_required_button(Clutter.BUTTON_SECONDARY);
        secondary.connect('recognize', () => this.menu.toggle());
        this.add_action(secondary);
        const middle = new Clutter.ClickGesture();
        middle.set_required_button(Clutter.BUTTON_MIDDLE);
        middle.connect('recognize', () => onMiddleClick());
        this.add_action(middle);
    }
});

function itemIcon(name, dir) {
    const icon = new St.Icon({gicon: gicon(name), icon_size: ITEM_ICON_SIZE, style_class: 'wye-menu-icon',
        y_align: Clutter.ActorAlign.CENTER});
    if (TrayModel.BUNDLED[name])
        icon.fallback_gicon = bundled(dir, TrayModel.BUNDLED[name]);
    return icon;
}

// Whether the keyboard is in `menu`.
function hasKeyFocus(menu) {
    const focus = global.stage.key_focus;
    return focus !== null && (focus === menu.actor || menu.actor.contains(focus));
}

// Keeps a level's labels in one column when some of its items have icons.
function iconSlot() {
    return new St.Widget({style_class: 'wye-menu-icon', width: ITEM_ICON_SIZE});
}

export class Tray {
    /**
     * @param {object} params
     * @param {string} params.dir the extension's directory
     * @param {string} params.uuid
     * @param {(id: string) => void} params.activate runs a tray item
     * @param {() => Promise<string|null>} params.fetch the current `Tray` JSON
     * @param {() => Promise<boolean>} params.clipboardHasUrl TRAY-10
     */
    constructor({dir, uuid, activate, fetch, clipboardHasUrl}) {
        this._dir = dir;
        this._activate = activate;
        this._fetch = fetch;
        this._clipboardHasUrl = clipboardHasUrl;
        this._json = null;
        this._model = null;
        // TRAY-10: whether the clipboard held a link when last asked, and
        // each menu's "Open URL from Clipboard" rows.
        this._hasUrl = false;
        // Set by destroy(): a refresh that resumes after it does nothing.
        this._destroyed = false;
        this._clipboardItems = new Map();
        this._indicator = new Indicator(() => this._run('settings'));
        this._indicator.hide();
        this._indicator.menu.connect('open-state-changed', (menu, open) => {
            if (open)
                this._opened(menu);
        });
        this._indicator.menu.actor.connect('key-press-event', (_actor, event) => this._shortcut(event));
        this._pages = new Map();
        this._addPages(this._indicator.menu);
        Main.panel.addToStatusArea(uuid, this._indicator);
        // TRAY-08 without a visible icon: the same menu at the pointer.
        this._anchor = new St.Widget({width: 1, height: 1, opacity: 0});
        Main.uiGroup.add_child(this._anchor);
        this._pointerMenu = new PopupMenu.PopupMenu(this._anchor, 0, St.Side.TOP);
        this._pointerMenu.actor.add_style_class_name('wye-tray-menu');
        Main.uiGroup.add_child(this._pointerMenu.actor);
        this._pointerMenu.actor.hide();
        this._pointerMenu.connect('open-state-changed', (menu, open) => {
            if (open)
                this._opened(menu);
        });
        this._pointerMenu.actor.connect('key-press-event', (_actor, event) => this._shortcut(event));
        this._addPages(this._pointerMenu);
        this._menuManager = new PopupMenu.PopupMenuManager(this._anchor,
            {actionMode: Shell.ActionMode.POPUP});
        this._menuManager.addMenu(this._pointerMenu);
        St.Settings.get().connectObject('notify::color-scheme', () => this._syncStyle(), this);
        this._syncStyle();
    }

    // TRAY-15: More and Recent Links open as pages in place of the menu.
    _addPages(menu) {
        this._pages.set(menu, new MenuPages(menu, {
            maxHeight: () => this._pageRoom(menu),
            root: section => this._fillRoot(menu, section),
        }));
    }

    // The tallest a page may be on the menu's monitor.
    _pageRoom(menu) {
        const monitor = Main.layoutManager.findMonitorForActor(menu.sourceActor) ??
            Main.layoutManager.primaryMonitor;
        const area = Main.layoutManager.getWorkAreaForMonitor(monitor.index);
        return area.height / themeScale() - PAGE_CHROME;
    }

    _syncStyle() {
        syncStyle(this._indicator);
        syncStyle(this._indicator.menu.actor);
        syncStyle(this._pointerMenu.actor);
    }

    /**
     * The service's `Tray` property changed.
     *
     * @param {string} json
     */
    update(json) {
        if (json === this._json)
            return;
        let model;
        try {
            model = TrayModel.parseTray(json);
        } catch (error) {
            console.error(`Wye tray: ${error.message}`);
            return;
        }
        this._json = json;
        this._model = model;
        this._syncIcon();
        // An open menu keeps its rows until it closes, unless it is rebuilt
        // on purpose (`_opened`).
        for (const menu of [this._indicator.menu, this._pointerMenu]) {
            if (!menu.isOpen)
                this._fill(menu);
        }
    }

    /** The service went away: no icon until it is back. */
    clear() {
        this._json = null;
        this._model = null;
        this._indicator.menu.close();
        this._pointerMenu.close();
        this._indicator.hide();
    }

    // TRAY-02, TRAY-04, ONB-11.
    _syncIcon() {
        const {icon, warning, visible} = this._model;
        this._indicator.icon.gicon = gicon(icon, []);
        this._indicator.icon.fallback_gicon = bundled(this._dir,
            TrayModel.BUNDLED[icon] ?? TrayModel.BUNDLED[TrayModel.APP_ICON]);
        this._indicator.emblem.visible = warning;
        this._indicator.visible = visible;
        this._indicator.accessible_name = warning ? 'Wye: not the default browser' : 'Wye';
    }

    /**
     * `ShowMenu` (TRAY-08): open or close the menu, from the panel icon or,
     * with the icon hidden, at the pointer.
     *
     * @param {string} json the `Tray` model the service sent
     */
    toggle(json) {
        const open = [this._indicator.menu, this._pointerMenu].find(menu => menu.isOpen);
        if (open) {
            open.close();
            return;
        }
        this.update(json);
        let menu = this._indicator.menu;
        if (!this._indicator.visible || !this._indicator.mapped) {
            menu = this._pointerMenu;
            this._placeAnchor(menu);
        }
        menu.open(BoxPointer.PopupAnimation.FULL);
        menu.actor.navigate_focus(null, St.DirectionType.TAB_FORWARD, false);
    }

    // The pointer menu's corner at the pointer, moved up as far as the
    // menu needs to stay on the screen.
    _placeAnchor(menu) {
        const [x, y] = global.get_pointer();
        const monitor = Main.layoutManager.monitors.find(m =>
            x >= m.x && x < m.x + m.width && y >= m.y && y < m.y + m.height) ??
            Main.layoutManager.primaryMonitor;
        const area = Main.layoutManager.getWorkAreaForMonitor(monitor.index);
        const [, height] = menu.actor.get_preferred_height(-1);
        const bottom = area.y + area.height - height - POINTER_MENU_MARGIN * themeScale();
        this._anchor.set_position(x, Math.max(area.y, Math.min(y, bottom)));
    }

    // TRAY-10: the menu is current each time it opens. The model and the
    // clipboard are asked apart: a session that cannot read the clipboard
    // only leaves its item disabled.
    _opened(menu) {
        this._refreshModel(menu);
        this._refreshClipboard(menu);
    }

    async _refreshModel(menu) {
        const before = this._json;
        try {
            const json = await this._fetch();
            if (this._destroyed)
                return;
            if (menu.isOpen && json && json !== before) {
                // TRAY-08: the rows the keyboard was on are rebuilt; the
                // keyboard stays in the menu. The model `ShowMenu` sends is
                // not the `Tray` property's text, so this runs right after
                // an opening from the shortcut or `wye menu`.
                const keyboard = hasKeyFocus(menu);
                this.update(json);
                this._fill(menu, keyboard ? 'first' : null);
            }
        } catch (error) {
            if (!this._destroyed)
                console.error(`Wye tray refresh: ${error.message}`);
        }
    }

    async _refreshClipboard(menu) {
        let hasUrl = false;
        try {
            hasUrl = await this._clipboardHasUrl();
        } catch (error) {
            // `Unavailable` where the session cannot read the clipboard.
            console.debug(`Wye tray clipboard: ${error.message}`);
        }
        if (this._destroyed)
            return;
        this._hasUrl = hasUrl;
        if (!menu.isOpen)
            return;
        for (const item of this._clipboardItems.get(menu) ?? [])
            item.setSensitive(item._wyeEnabled && hasUrl);
    }

    _run(id) {
        this._indicator.menu.close();
        this._pointerMenu.close();
        this._activate(id);
    }

    // TRAY-13, KEY-51: P, 1–9 and the shown shortcuts choose their item.
    _shortcut(event) {
        if (!this._model)
            return Clutter.EVENT_PROPAGATE;
        const name = Clutter.keyval_name(event.get_key_symbol()) ?? '';
        const item = TrayModel.itemForShortcut(this._model.items,
            chordLabel(modifiersOf(event.get_state()), name));
        if (!item)
            return Clutter.EVENT_PROPAGATE;
        this._run(item.id);
        return Clutter.EVENT_STOP;
    }

    // The menu from the model, back at its root page; `focus` as for
    // `MenuPages.render`.
    _fill(menu, focus = null) {
        this._pages.get(menu).reset(focus);
    }

    _fillRoot(menu, section) {
        this._clipboardItems.set(menu, []);
        if (this._model)
            this._addItems(menu, section, this._model.items);
    }

    // One page of the menu, in the model's order (01-tray-menu.md).
    _addItems(menu, section, items) {
        const visible = TrayModel.tidySeparators(items);
        const icons = TrayModel.hasIcons(visible);
        const radios = visible.some(entry => entry.kind === 'radio');
        for (const entry of visible)
            section.addMenuItem(this._item(entry, icons, radios, menu));
    }

    _item(entry, icons, radios, menu) {
        switch (entry.kind) {
        case 'separator':
            return new PopupMenu.PopupSeparatorMenuItem();
        case 'header': {
            const item = headingItem(entry.label);
            this._decorate(item, entry, icons, radios);
            return item;
        }
        case 'submenu':
            return this._submenu(entry, icons, radios, menu);
        default:
            return this._action(entry, icons, radios, menu);
        }
    }

    _decorate(item, entry, icons, radios) {
        if (entry.kind === 'radio')
            item.setOrnament(entry.checked ? PopupMenu.Ornament.DOT : PopupMenu.Ornament.NO_DOT);
        else
            item.setOrnament(radios ? PopupMenu.Ornament.NONE : PopupMenu.Ornament.HIDDEN);
        if (entry.icon)
            item.insert_child_above(itemIcon(entry.icon, this._dir), item._ornamentIcon);
        else if (icons)
            item.insert_child_above(iconSlot(), item._ornamentIcon);
    }

    _action(entry, icons, radios, menu) {
        const item = new PopupMenu.PopupMenuItem(entry.label);
        item.add_style_class_name('wye-menu-item');
        this._decorate(item, entry, icons, radios);
        if (entry.shortcut)
            item.add_child(shortcutLabel(entry.shortcut));
        item.setSensitive(entry.enabled);
        if (entry.id === 'open-clipboard') {
            // TRAY-10: enabled while the clipboard holds a link.
            item._wyeEnabled = entry.enabled;
            item.setSensitive(entry.enabled && this._hasUrl);
            this._clipboardItems.get(menu)?.push(item);
        }
        return onActivate(item, () => this._activate(entry.id));
    }

    // TRAY-15: "More", and "Recent Links" inside it, open as a page in
    // place of the menu, as GTK's popover menus do, so the menu never
    // outgrows the screen.
    _submenu(entry, icons, radios, menu) {
        const pages = this._pages.get(menu);
        const item = pageItem(entry.id, entry.label, keyboard => pages.open({
            title: entry.label,
            key: entry.id,
            fill: section => this._addItems(menu, section, entry.children),
        }, keyboard));
        item.add_style_class_name('wye-menu-item');
        this._decorate(item, entry, icons, radios);
        item.setSensitive(entry.enabled && entry.children.length > 0);
        return item;
    }

    destroy() {
        this._destroyed = true;
        St.Settings.get().disconnectObject(this);
        this._menuManager.removeMenu(this._pointerMenu);
        this._pointerMenu.destroy();
        this._anchor.destroy();
        this._indicator.destroy();
        this._indicator = null;
    }
}

