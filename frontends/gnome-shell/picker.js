import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';
import * as ModalDialog from 'resource:///org/gnome/shell/ui/modalDialog.js';
import {parsePicker, keyAction, choiceOptions} from './model.mjs';

const SIZES = {small: 24, medium: 32, large: 40};
const DEFAULT_KEYS = {actions: {
    open: ['Return', 'KP_Enter', 'space'], cancel: ['Escape'],
    next: ['Right', 'Tab'], previous: ['Left', 'Shift+Tab'],
    first: ['Home'], last: ['End'], 'copy-link': ['Ctrl+c'],
    more: ['Menu'], 'create-rule': ['Ctrl+r'],
}, modifierActions: {private: ['Shift'], background: ['Ctrl'], 'new-window': ['Alt']}};

function chord(event) {
    const key = Clutter.keyval_name(event.get_key_symbol()) === 'ISO_Left_Tab'
        ? 'Tab' : Clutter.keyval_name(event.get_key_symbol());
    const state = event.get_state();
    const prefixes = [];
    if (state & Clutter.ModifierType.CONTROL_MASK)
        prefixes.push('Ctrl');
    if (state & Clutter.ModifierType.MOD1_MASK)
        prefixes.push('Alt');
    if (state & Clutter.ModifierType.SHIFT_MASK)
        prefixes.push('Shift');
    if (state & Clutter.ModifierType.SUPER_MASK)
        prefixes.push('Super');
    return [...prefixes, key].join('+');
}

function modifiers(event, keys, held) {
    const state = event?.get_state() ?? 0;
    const active = new Set(held);
    if (state & Clutter.ModifierType.SHIFT_MASK)
        active.add('Shift');
    if (state & Clutter.ModifierType.CONTROL_MASK)
        active.add('Ctrl');
    if (state & Clutter.ModifierType.MOD1_MASK)
        active.add('Alt');
    if (state & Clutter.ModifierType.SUPER_MASK)
        active.add('Super');
    return Object.fromEntries(Object.entries(keys.modifierActions ?? {}).map(([name, bindings]) =>
        [name, bindings?.some(binding => active.has(binding)) ?? false]));
}

export class Picker {
    constructor(call) {
        this._call = call;
        this._dialog = new ModalDialog.ModalDialog({
            destroyOnClose: false, shouldFadeIn: false, shouldFadeOut: false,
            styleClass: 'wye-picker',
        });
        this._dialog.connect('closed', () => {
            if (this._id) {
                const id = this._id;
                this._id = null;
                this._call('PickerCancelled', '(s)', [id])
                    .catch(error => logError(error, 'Wye picker cancel'));
            }
        });
        this._dialog.connect('captured-event', (_actor, event) => this._onEvent(event));
    }

    show(id, json) {
        if (!id)
            throw new Error('Picker request ID is empty');
        const request = parsePicker(json);
        this._id = id; // A replacement supersedes the old request; never cancel its ID.
        this._request = request;
        this._keys = request.keys ?? DEFAULT_KEYS;
        this._selected = 0;
        this._busy = false;
        this._render();
        this._dialog.setInitialKeyFocus(this._tiles[0] ?? this._dialog.dialogLayout);
        if (this._dialog.state === ModalDialog.State.CLOSED && !this._dialog.open(global.get_current_time())) {
            this._id = null;
            throw new Error('Unable to grab keyboard for Wye picker');
        }
        this._tiles[0]?.grab_key_focus();
    }

    close(id) {
        if (id && id !== this._id)
            return;
        this._id = null;
        if (this._dialog.state !== ModalDialog.State.CLOSED)
            this._dialog.close(global.get_current_time());
    }

    destroy() {
        this._cancel();
        this._dialog.destroy();
    }

    _onEvent(event) {
        if (!this._id)
            return Clutter.EVENT_PROPAGATE;
        if (event.type() === Clutter.EventType.BUTTON_PRESS &&
            !this._dialog.dialogLayout.contains(event.get_source())) {
            this._cancel();
            return Clutter.EVENT_STOP;
        }
        if (event.type() !== Clutter.EventType.KEY_PRESS)
            return Clutter.EVENT_PROPAGATE;
        const key = chord(event);
        const bare = Clutter.keyval_name(event.get_key_symbol()) === 'ISO_Left_Tab'
            ? 'Tab' : Clutter.keyval_name(event.get_key_symbol());
        const prefixes = key.split('+').slice(0, -1);
        const heldBindings = Object.values(this._keys.modifierActions ?? {}).flat();
        const permitted = prefixes.every(prefix => heldBindings.includes(prefix));
        const action = keyAction(this._keys, key) ??
            (permitted ? keyAction(this._keys, bare) : null);
        if (action === 'cancel')
            this._cancel();
        else if (action === 'next' || action === 'previous')
            this._select(this._selected + (action === 'next' ? 1 : -1));
        else if (action === 'first' || action === 'last')
            this._select(action === 'first' ? 0 : this._request.tiles.length - 1);
        else if (action === 'open')
            this._choose(this._request.tiles[this._selected], event);
        else if (action === 'more')
            this._more();
        else if (action === 'copy-link' || action === 'create-rule')
            this._action(action);
        else {
            const index = permitted ? this._request.tiles.findIndex(tile =>
                tile.hotkey?.toLowerCase() === bare.toLowerCase()) : -1;
            if (index < 0)
                return Clutter.EVENT_PROPAGATE;
            this._choose(this._request.tiles[index], event);
        }
        return Clutter.EVENT_STOP;
    }

    _cancel() {
        const id = this._id;
        this.close();
        if (id)
            this._call('PickerCancelled', '(s)', [id])
                .catch(error => logError(error, 'Wye picker cancel'));
    }

    async _answer(method, signature, args) {
        if (this._busy || !this._id)
            return;
        const id = this._id;
        this._busy = true;
        try {
            await this._call(method, signature, [id, ...args]);
            if (this._id === id)
                this.close(id);
        } catch (error) {
            logError(error, `Wye ${method}`);
            if (this._id === id) {
                this._busy = false;
                this._status.text = 'Could not complete action. Try again or press Escape.';
            }
        }
    }

    _choose(tile, event = null, overrides = {}) {
        if (!tile || !this._id)
            return;
        const active = {...modifiers(event, this._keys, this._request.held ?? []), ...overrides};
        for (const [name, enabled] of Object.entries(active)) {
            const capability = name === 'new-window' ? 'newWindow' : name;
            if (enabled && !tile.capabilities?.[capability]) {
                this._status.text = `${tile.name} does not support ${name}`;
                return;
            }
        }
        const options = choiceOptions(active, tile.capabilities);
        const variants = Object.fromEntries(Object.entries(options).map(([key, value]) =>
            [key, new GLib.Variant('b', value)]));
        this._answer('PickerChose', '(ssa{sv})', [JSON.stringify(tile.target), variants]);
    }

    _action(action) {
        this._answer('PickerAction', '(ss)', [action]);
    }

    _select(index) {
        const count = this._tiles.length;
        if (!count)
            return;
        this._selected = (index + count) % count;
        this._tiles.forEach((tile, i) => {
            if (i === this._selected)
                tile.add_style_pseudo_class('selected');
            else
                tile.remove_style_pseudo_class('selected');
        });
        this._tiles[this._selected].grab_key_focus();
    }

    _render() {
        this._dialog.contentLayout.remove_all_children();
        this._tiles = [];
        const request = this._request;
        const settings = request.settings ?? {};
        const iconSize = SIZES[settings.iconSize] ?? SIZES.medium;
        const content = new St.BoxLayout({vertical: true, style_class: 'wye-picker-content'});
        this._dialog.contentLayout.add_child(content);
        const tiles = request.tiles;
        const heldActions = modifiers(null, this._keys, request.held ?? []);
        for (let start = 0; start < tiles.length; start += 8) {
            const row = new St.BoxLayout({style_class: 'wye-picker-row'});
            content.add_child(row);
            for (let i = start; i < Math.min(start + 8, tiles.length); i++) {
                const tile = tiles[i];
                const button = new St.Button({style_class: 'wye-tile', can_focus: true,
                    accessible_name: `${tile.name}${tile.hotkey ? `, ${tile.hotkey}` : ''}`});
                const body = new St.BoxLayout({vertical: true, x_align: Clutter.ActorAlign.CENTER});
                body.add_child(new St.Label({text: tile.hotkey ?? ' ', style_class: 'wye-hotkey'}));
                const icon = new St.Icon({gicon: Gio.ThemedIcon.new_from_names(
                    [tile.icon ?? 'web-browser', 'web-browser']), icon_size: iconSize});
                body.add_child(icon);
                if (settings.showBadge !== false && tile.badge?.initial)
                    body.add_child(new St.Label({text: tile.badge.initial, style_class: 'wye-badge'}));
                if (settings.showNames !== false)
                    body.add_child(new St.Label({text: tile.name, style_class: 'wye-tile-name'}));
                button.set_child(body);
                if (Object.entries(heldActions).some(([name, enabled]) => enabled &&
                    !tile.capabilities?.[name === 'new-window' ? 'newWindow' : name]))
                    button.add_style_pseudo_class('unavailable');
                button.connect('clicked', () => this._choose(tile));
                button.connect('enter-event', () => this._select(i));
                button.connect('button-press-event', (_actor, event) => {
                    if (event.get_button() === 2) {
                        this._choose(tile, event, {background: true});
                        return Clutter.EVENT_STOP;
                    }
                    if (event.get_button() === 3) {
                        this._context(tile);
                        return Clutter.EVENT_STOP;
                    }
                    return Clutter.EVENT_PROPAGATE;
                });
                row.add_child(button);
                this._tiles.push(button);
            }
        }
        const more = new St.Button({label: '⋯', style_class: 'wye-more', can_focus: true,
            accessible_name: 'More opening choices'});
        more.connect('clicked', () => this._more());
        content.add_child(more);
        if (settings.showUrl && request.url) {
            const text = `${request.source?.name ? `from ${request.source.name} · ` : ''}${request.url.host ?? ''}${request.url.rest ?? ''}`;
            content.add_child(new St.Label({text, style_class: 'wye-url',
                accessible_name: request.url.full}));
        }
        const hint = Object.entries(heldActions).filter(([, enabled]) => enabled)
            .map(([name]) => name === 'new-window' ? 'new window' : name).join(', ');
        if (hint)
            content.add_child(new St.Label({text: `Open in ${hint}`, style_class: 'wye-hint'}));
        this._status = new St.Label({text: tiles.length ? '' : 'No browsers available',
            style_class: 'wye-status'});
        content.add_child(this._status);
        this._extra = new St.BoxLayout({vertical: true, style_class: 'wye-extra'});
        this._extraScroll = new St.ScrollView({style_class: 'wye-extra-scroll',
            hscrollbar_policy: St.PolicyType.NEVER, vscrollbar_policy: St.PolicyType.AUTOMATIC});
        this._extraScroll.add_child(this._extra);
        this._extraScroll.hide();
        content.add_child(this._extraScroll);
        if (tiles.length)
            this._select(0);
    }

    _extraButton(label, callback) {
        const button = new St.Button({label, can_focus: true, style_class: 'wye-extra-button'});
        button.connect('clicked', callback);
        this._extra.add_child(button);
    }

    _more() {
        this._extra.remove_all_children();
        this._extraScroll.show();
        for (const group of this._request.overflow) {
            if (group.label)
                this._extra.add_child(new St.Label({text: group.label, style_class: 'wye-group'}));
            for (const tile of group.tiles)
                this._extraButton(tile.name, () => this._choose(tile));
        }
        this._extraButton('Copy Link', () => this._action('copy-link'));
        this._extraButton('Create Rule…', () => this._action('create-rule'));
        this._extraButton('Settings…', () => this._call('ShowWindow', '(ss)', ['settings', ''])
            .then(() => this._cancel()).catch(error => logError(error, 'Wye settings')));
    }

    _context(tile) {
        this._extra.remove_all_children();
        this._extraScroll.show();
        this._extraButton(`Open ${tile.name}`, () => this._choose(tile));
        for (const [name, option] of [['Private Window', 'private'],
            ['New Window', 'newWindow'], ['Background', 'background']]) {
            if (tile.capabilities?.[option])
                this._extraButton(`Open in ${name}`, () => this._choose(tile, null,
                    {[option === 'newWindow' ? 'new-window' : option]: true}));
        }
        this._extraButton('Make Primary Browser', () => this._call('SetPrimary', '(s)',
            [JSON.stringify(tile.target)]).catch(error => logError(error, 'Wye primary browser')));
    }
}
