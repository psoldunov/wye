// The picker (02-picker.md) drawn by the Shell: a floating panel at the
// pointer that holds the keyboard until the user chooses or cancels. The
// rules live in model.mjs and keys.mjs, the widgets in picker-view.js;
// this file carries out what the user does and answers the service.
import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import St from 'gi://St';
import * as BoxPointer from 'resource:///org/gnome/shell/ui/boxpointer.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import {eventTime} from './focus.js';
import {gicon} from './icons.js';
import {MenuPages, actionItem, headingItem, isLightStyle, pageItem, syncStyle, themeScale} from './menus.js';
import * as Keys from './keys.mjs';
import * as Model from './model.mjs';
import {PickerView} from './picker-view.js';

// Margin from the work area's edges (PICK-02), CSS px.
const SCREEN_MARGIN = 12;
// PICK-13: what a row of tiles shares the panel's width with: its padding
// and border, and the "⋯" column, CSS px.
const PANEL_CHROME = 2 * 14 + 2 * 1 + 40;
// PICK-24: the pointer must move this far before hover selects.
const HOVER_SLOP = 3;
// What an Open In page shares the screen with: the menu's padding, the
// back row and its separator (logical px).
const PAGE_CHROME = 72;

const MASKS = [
    [Clutter.ModifierType.CONTROL_MASK, 'Ctrl'],
    [Clutter.ModifierType.MOD1_MASK, 'Alt'],
    [Clutter.ModifierType.SHIFT_MASK, 'Shift'],
    [Clutter.ModifierType.SUPER_MASK, 'Super'],
    [Clutter.ModifierType.MOD4_MASK, 'Super'],
];

/**
 * The modifiers in a Clutter state mask, left and right alike (KEY-01).
 *
 * @param {number} state
 */
export function modifiersOf(state) {
    return Keys.sortModifiers(MASKS.filter(([mask]) => state & mask).map(([, name]) => name));
}

function keyEvent(event, mods) {
    const name = Clutter.keyval_name(event.get_key_symbol()) ?? '';
    const unicode = event.get_key_unicode();
    const text = unicode && !/\p{Cc}/u.test(unicode) ? unicode : undefined;
    return {
        key: name === 'ISO_Left_Tab' ? 'Tab' : name,
        text,
        keycode: event.get_key_code(),
        mods,
    };
}

export class Picker {
    /**
     * @param {object} params
     * @param {(method: string, signature: string|null, args: any[]) => Promise} params.call
     *   a method of the service
     * @param {import('./focus.js').WindowActivator} params.activator
     * @param {(desktopId: string|null, time: number) => string|null} params.token
     */
    constructor({call, activator, token}) {
        this._call = call;
        this._activator = activator;
        this._token = token;
        this._id = null;
        this._grab = null;
        this._menus = [];
        // Idle sources to remove when the picker goes.
        this._idles = new Set();
        this._root = new St.Widget({
            reactive: true,
            visible: false,
            layout_manager: new Clutter.FixedLayout(),
            name: 'wyePicker',
        });
        this._root.add_constraint(new Clutter.BindConstraint({
            source: global.stage, coordinate: Clutter.BindCoordinate.ALL,
        }));
        Main.layoutManager.modalDialogGroup.add_child(this._root);
        this._view = new PickerView({
            hover: (index, event) => this._hover(index, event),
            click: (index, button) => this._click(index, button),
            more: () => this._openMore(false),
        });
        this._root.add_child(this._view.actor);
        // The tile menu opens at the pointer (PICK-30).
        this._menuAnchor = new St.Widget({width: 1, height: 1, opacity: 0});
        this._root.add_child(this._menuAnchor);
        this._root.connect('button-press-event', (_actor, event) => this._pressed(event));
        this._root.connect('key-press-event', (_actor, event) => this._key(event, true));
        this._root.connect('key-release-event', (_actor, event) => this._key(event, false));
        this._menuManager = new PopupMenu.PopupMenuManager(this._root,
            {actionMode: Shell.ActionMode.SYSTEM_MODAL});
        St.Settings.get().connectObject('notify::color-scheme', () => this._syncStyle(), this);
        this._syncStyle();
    }

    get isOpen() {
        return this._id !== null;
    }

    _syncStyle() {
        // PICK-12: the Shell's own light or dark style.
        this._view.setLight(isLightStyle());
    }

    /**
     * Shows a request, or replaces the one shown in place (PICK-27).
     *
     * @param {string} id
     * @param {string} json `PickerRequest`
     */
    show(id, json) {
        if (!id)
            throw new Error('Picker request ID is empty');
        const picker = Model.parsePicker(json);
        const replacing = this.isOpen;
        this._closeMenus();
        this._picker = picker;
        this._keymap = Keys.keymap(picker.keys);
        this._held = Keys.sortModifiers(picker.held.map(Keys.modifierName).filter(Boolean));
        this._selected = 0;
        this._hoverOrigin = null;
        this._pointerMoved = false;
        if (!replacing) {
            const [x, y] = global.get_pointer();
            this._pointer = {x, y};
        }
        const monitor = this._monitor();
        const area = Main.layoutManager.getWorkAreaForMonitor(monitor.index);
        // The work area is in stage pixels; the constants in CSS pixels.
        const scale = themeScale();
        this._root.visible = true;
        this._view.setBlur(true);
        this._view.build(picker, area.width - (2 * SCREEN_MARGIN + PANEL_CHROME) * scale);
        this._refresh();
        this._place(area);
        if (!replacing && !this._grabKeyboard()) {
            this._root.visible = false;
            // No clone of the windows stays alive behind a hidden picker.
            this._view.setBlur(false);
            throw new Error('Unable to grab the keyboard for the Wye picker');
        }
        this._id = id;
        this._root.grab_key_focus();
    }

    // PICK-02: the monitor under the pointer.
    _monitor() {
        const {x, y} = this._pointer;
        return Main.layoutManager.monitors.find(m =>
            x >= m.x && x < m.x + m.width && y >= m.y && y < m.y + m.height) ??
            Main.layoutManager.primaryMonitor;
    }

    // PICK-02: centred on the pointer, inside the work area.
    _place(area) {
        const [, width] = this._view.actor.get_preferred_width(-1);
        const [, height] = this._view.actor.get_preferred_height(width);
        const {x, y} = Model.place(this._pointer, {width, height}, area, SCREEN_MARGIN * themeScale());
        this._view.setStagePosition(x, y);
    }

    _grabKeyboard() {
        this._grab = Main.pushModal(this._root, {actionMode: Shell.ActionMode.SYSTEM_MODAL});
        if (!this._grab)
            return false;
        // As a modal dialog does: open menus and notifications step aside.
        Main.layoutManager.emit('system-modal-opened');
        return true;
    }

    /**
     * Closes without answering: the service asked (`ClosePicker`) or went
     * away. Another request's ID is ignored.
     *
     * @param {string} [id]
     */
    close(id) {
        if (id !== undefined && id !== this._id)
            return;
        this._hide();
    }

    _hide() {
        this._id = null;
        this._closeMenus();
        if (this._grab) {
            Main.popModal(this._grab);
            this._grab = null;
        }
        this._root.visible = false;
        // Nothing of the request stays drawn, and no clone keeps painting.
        this._view.setBlur(false);
    }

    // The request ends here; `answer` reports it to the service.
    _finish(answer) {
        const id = this._id;
        if (!id)
            return;
        this._hide();
        answer(id).catch(error => console.error(`Wye picker: ${error.message}`));
    }

    _cancel() {
        this._finish(id => this._call('PickerCancelled', '(s)', [id]));
    }

    _action(action) {
        // PICK-31: the rule editor opens; raise it.
        if (action === 'create-rule')
            this._activator.expect();
        this._finish(id => this._call('PickerAction', '(ss)', [id, action])
            .then(() => action === 'create-rule' && this._activator.settle()));
    }

    _settings() {
        this._cancel();
        this._activator.expect();
        this._call('ShowWindow', '(ss)', ['settings', ''])
            .then(() => this._activator.settle())
            .catch(error => console.error(`Wye settings: ${error.message}`));
    }

    /**
     * Opens the link in `entry` the `mode` way (PICK-20, PICK-21, PICK-29).
     *
     * @param {object} entry
     * @param {string|null} mode
     * @param {number} time the input event's timestamp
     */
    _choose(entry, mode, time) {
        if (!entry || !this.isOpen)
            return;
        const {target, options} = Model.choose(entry, mode);
        const variants = Object.fromEntries(Object.entries(options)
            .map(([key, value]) => [key, new GLib.Variant('b', value)]));
        // PICK-29: no token for a background launch; the service drops it.
        if (!options.background) {
            let token = null;
            try {
                token = this._token(Model.desktopId(target), time);
            } catch (error) {
                console.warn(`Wye picker: no activation token (${error.message})`);
            }
            if (token)
                variants['activation-token'] = new GLib.Variant('s', token);
        }
        this._finish(id => this._call('PickerChose', '(ssa{sv})', [id, JSON.stringify(target), variants]));
    }

    _mode() {
        return Keys.modeFor(this._keymap, this._held);
    }

    _refresh() {
        const mode = this._mode();
        this._view.setSelected(this._selected);
        this._view.setDimmed(this._picker.tiles.map(entry => mode !== null && !Model.supports(mode, entry)));
        this._view.setHint(mode ? Model.HINTS[mode] : '');
    }

    _select(index) {
        this._selected = index;
        this._refresh();
    }

    // PICK-22, PICK-24: hover selects once the pointer has really moved.
    _hover(index, event) {
        if (!this.isOpen)
            return;
        const [x, y] = event.get_coords();
        if (!this._pointerMoved) {
            if (!this._hoverOrigin) {
                this._hoverOrigin = {x, y};
                return;
            }
            if (Math.abs(x - this._hoverOrigin.x) < HOVER_SLOP &&
                Math.abs(y - this._hoverOrigin.y) < HOVER_SLOP)
                return;
            this._pointerMoved = true;
        }
        if (index !== this._selected)
            this._select(index);
    }

    // PICK-20, PICK-30, PICK-32, PICK-33.
    _click(index, button) {
        const entry = this._picker?.tiles[index];
        const event = Clutter.get_current_event();
        const time = eventTime(event);
        if (!entry)
            return;
        if (button === Clutter.BUTTON_SECONDARY) {
            this._select(index);
            this._openTileMenu(index, event);
            return;
        }
        if (button === Clutter.BUTTON_MIDDLE) {
            this._choose(entry, 'background', time);
            return;
        }
        const held = event ? modifiersOf(event.get_state()) : this._held;
        this._choose(entry, Keys.modeFor(this._keymap, held), time);
    }

    // PICK-23: a click outside the panel cancels.
    _pressed(event) {
        if (!this.isOpen)
            return Clutter.EVENT_PROPAGATE;
        const source = global.stage.get_event_actor(event);
        if (!source || source === this._root || !this._view.actor.contains(source)) {
            this._cancel();
            return Clutter.EVENT_STOP;
        }
        return Clutter.EVENT_PROPAGATE;
    }

    // PICK-21, PICK-22, KEY-13, KEY-22.
    _key(event, pressed) {
        if (!this.isOpen)
            return Clutter.EVENT_PROPAGATE;
        const name = Clutter.keyval_name(event.get_key_symbol()) ?? '';
        // Every key event says what is held now (KEY-13).
        this._held = Keys.heldAfter(modifiersOf(event.get_state()), name, pressed);
        if (!pressed || Keys.modifierOfKey(name) !== undefined) {
            this._refresh();
            return Clutter.EVENT_STOP;
        }
        const outcome = Keys.dispatch(this._keymap, keyEvent(event, this._held),
            this._picker.tiles.map(entry => entry.hotkey));
        const time = event.get_time();
        if (!outcome)
            return Clutter.EVENT_PROPAGATE;
        if (outcome.index !== undefined) {
            this._choose(this._picker.tiles[outcome.index], outcome.mode, time);
            return Clutter.EVENT_STOP;
        }
        switch (outcome.action) {
        case 'open':
            this._choose(this._picker.tiles[this._selected], outcome.mode ?? this._mode(), time);
            break;
        case 'cancel':
            this._cancel();
            break;
        case 'copy-link':
        case 'create-rule':
            this._action(outcome.action);
            break;
        case 'more':
            this._openMore(true);
            break;
        default:
            this._select(Keys.select(outcome.action, this._selected, this._picker.tiles.length));
        }
        return Clutter.EVENT_STOP;
    }

    _newMenu(source, alignment, side = St.Side.TOP) {
        const menu = new PopupMenu.PopupMenu(source, alignment, side);
        menu.actor.add_style_class_name('wye-picker-menu');
        syncStyle(menu.actor);
        Main.uiGroup.add_child(menu.actor);
        menu.actor.hide();
        this._menuManager.addMenu(menu);
        menu.connect('open-state-changed', (_menu, open) => {
            if (open)
                return;
            // Back to the picker's keys once the menu lets go.
            const idle = GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
                this._idles.delete(idle);
                if (this._menus.includes(menu)) {
                    this._menus = this._menus.filter(m => m !== menu);
                    menu.destroy();
                }
                if (this.isOpen)
                    this._root.grab_key_focus();
                return GLib.SOURCE_REMOVE;
            });
            this._idles.add(idle);
        });
        this._menus.push(menu);
        return menu;
    }

    _closeMenus() {
        for (const menu of this._menus) {
            menu.close();
            menu.destroy();
        }
        this._menus = [];
    }

    _openMenu(menu, keyboard) {
        menu.open(BoxPointer.PopupAnimation.FULL);
        if (keyboard)
            menu.actor.navigate_focus(null, St.DirectionType.TAB_FORWARD, false);
    }

    // The tallest an Open In page may be: the work area's height, as the
    // menu beside the "⋯" button may move up or down to fit (PICK-08).
    _pageRoom() {
        const area = Main.layoutManager.getWorkAreaForMonitor(this._monitor().index);
        return area.height / themeScale() - 2 * SCREEN_MARGIN - PAGE_CHROME;
    }

    // PICK-30: the tile's menu, at the pointer.
    _openTileMenu(index, event) {
        this._closeMenus();
        const entry = this._picker.tiles[index];
        const [x, y] = event ? event.get_coords() : global.get_pointer();
        this._menuAnchor.set_position(x, y);
        const menu = this._newMenu(this._menuAnchor, 0);
        const icons = {
            'open': gicon(entry.icon),
            'open-private': gicon('view-conceal-symbolic', []),
            'open-new-window': gicon('window-new-symbolic', []),
            'open-background': gicon('view-dual-symbolic', ['window-new-symbolic']),
            'make-primary': gicon('starred-symbolic', []),
        };
        for (const {action, label} of Model.tileMenu(entry)) {
            if (!action) {
                menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
                continue;
            }
            menu.addMenuItem(actionItem(label, icons[action], time => {
                if (action === 'make-primary') {
                    // The picker stays open (PICK-30).
                    this._call('SetPrimary', '(s)', [JSON.stringify(entry.target)])
                        .catch(error => console.error(`Wye primary browser: ${error.message}`));
                    return;
                }
                this._choose(entry, Model.TILE_ACTION_MODES[action], time);
            }));
        }
        this._openMenu(menu, false);
    }

    // PICK-08, PICK-28, PICK-31, KEY-22: the "⋯" menu.
    _openMore(keyboard) {
        if (!this.isOpen)
            return;
        this._closeMenus();
        // Beside the button, so it covers no tile and has the screen's
        // height; on the other side when there is no room.
        const menu = this._newMenu(this._view.moreButton, 0.5, St.Side.LEFT);
        // The button stays pressed while its menu is open.
        menu.connect('open-state-changed', (_menu, open) => this._view.moreButton.set_checked(open));
        const rows = Model.openInRows(this._picker.overflow);
        const pages = new MenuPages(menu, {
            maxHeight: () => this._pageRoom(),
            root: (section, self) => {
                // PICK-28: Open In opens as a page in place of the menu.
                const openIn = pageItem('open-in', 'Open In', fromKeys => self.open({
                    title: 'Open In',
                    key: 'open-in',
                    fill: page => this._fillOpenIn(page, rows),
                }, fromKeys));
                openIn.insert_child_at_index(new St.Icon({gicon: gicon('document-open-symbolic', []),
                    style_class: 'popup-menu-icon'}), 1);
                openIn.setSensitive(rows.length > 0);
                section.addMenuItem(openIn);
                section.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
                section.addMenuItem(actionItem('Copy Link', gicon('edit-copy-symbolic', []), () => this._action('copy-link')));
                section.addMenuItem(actionItem('Create Rule…', gicon('list-add-symbolic', []), () => this._action('create-rule')));
                section.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
                section.addMenuItem(actionItem('Settings…', gicon('emblem-system-symbolic', []), () => this._settings()));
            },
        });
        pages.render();
        this._openMenu(menu, keyboard);
    }

    // PICK-28: every group's heading, then its targets.
    _fillOpenIn(section, rows) {
        for (const row of rows) {
            if (row.kind === 'header') {
                section.addMenuItem(headingItem(row.label, true));
                continue;
            }
            const entry = this._picker.overflow[row.group].entries[row.item];
            section.addMenuItem(actionItem(row.label, gicon(row.icon), time =>
                this._choose(entry, this._mode(), time)));
        }
    }

    destroy() {
        if (this.isOpen)
            this._cancel();
        this._closeMenus();
        for (const idle of this._idles)
            GLib.source_remove(idle);
        this._idles.clear();
        St.Settings.get().disconnectObject(this);
        this._root.destroy();
        this._root = null;
    }
}
