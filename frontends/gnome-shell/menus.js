// Menu pieces shared by the picker's menus and the tray menu, built on the
// Shell's own PopupMenu so they look like every other GNOME menu.
import Atk from 'gi://Atk';
import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import St from 'gi://St';
import {ensureActorVisibleInScrollView} from 'resource:///org/gnome/shell/misc/animationUtils.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import {eventTime} from './focus.js';

/** Whether the Shell draws its light style (PICK-12). */
export function isLightStyle() {
    return Main.getStyleVariant?.() === 'light';
}

/**
 * Marks an actor for the light or dark Wye colours.
 *
 * @param {St.Widget} actor
 */
export function syncStyle(actor) {
    if (isLightStyle())
        actor.add_style_class_name('wye-light');
    else
        actor.remove_style_class_name('wye-light');
}

// Item actions waiting for their menu to close (onActivate).
const pendingActions = new Set();

/**
 * Drops the item actions still waiting: the extension is being disabled,
 * and what they would use is gone.
 */
export function cancelPendingActions() {
    for (const id of pendingActions)
        GLib.source_remove(id);
    pendingActions.clear();
}

/**
 * Runs `action` once the menu has finished closing, with the activating
 * event's time: an action may close the menu's owner, and the menu with it.
 *
 * @param {PopupMenu.PopupBaseMenuItem} item
 * @param {(time: number) => void} action
 */
export function onActivate(item, action) {
    item.connect('activate', (_item, event) => {
        const time = eventTime(event);
        const id = GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            pendingActions.delete(id);
            action(time);
            return GLib.SOURCE_REMOVE;
        });
        pendingActions.add(id);
    });
    return item;
}

/**
 * The Shell's UI scale: CSS pixels times this are stage pixels. It is 1
 * when Mutter lays monitors out in logical pixels, and the monitors' scale
 * when it lays them out in physical pixels.
 */
export function themeScale() {
    return St.ThemeContext.get_for_stage(global.stage).scale_factor;
}

/**
 * An item with an optional icon.
 *
 * @param {string} text
 * @param {import('gi://Gio').Icon|null} icon
 * @param {(time: number) => void} action
 */
export function actionItem(text, icon, action) {
    const item = icon
        ? new PopupMenu.PopupImageMenuItem(text, icon)
        : new PopupMenu.PopupMenuItem(text);
    return onActivate(item, action);
}

/**
 * A section heading: dimmed and not choosable (TGT-02, TRAY-11).
 *
 * @param {string} text
 * @param {boolean} indent line it up with the labels of items with icons
 */
export function headingItem(text, indent = false) {
    const item = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false,
        style_class: 'wye-menu-heading'});
    if (indent)
        item.add_child(new St.Icon({style_class: 'popup-menu-icon'}));
    const label = new St.Label({text, style_class: 'wye-menu-heading-label',
        y_align: Clutter.ActorAlign.CENTER});
    item.add_child(label);
    item.label_actor = label;
    return item;
}

/**
 * The shortcut column: right-aligned and dimmed (TRAY-13).
 *
 * @param {string} text
 */
export function shortcutLabel(text) {
    return new St.Label({text, style_class: 'wye-menu-shortcut', x_expand: true,
        x_align: Clutter.ActorAlign.END, y_align: Clutter.ActorAlign.CENTER});
}

/**
 * An item that opens a page of the menu in place of the current one, as
 * GTK's popover menus slide in a submenu (TRAY-15, PICK-08): the menu keeps
 * the size of one page, so it fits the screen without inner scrolling.
 * Choosing it, or Right, opens the page; the menu stays open.
 */
export const PageItem = GObject.registerClass(
class WyePageItem extends PopupMenu.PopupBaseMenuItem {
    /**
     * @param {string} text
     * @param {(keyboard: boolean) => void} open
     */
    _init(text, open) {
        super._init({style_class: 'wye-page-item'});
        this._open = open;
        this.label = new St.Label({text, y_expand: true, y_align: Clutter.ActorAlign.CENTER});
        this.add_child(this.label);
        this.label_actor = this.label;
        this.add_child(new St.Icon({icon_name: 'go-next-symbolic', style_class: 'popup-menu-arrow',
            x_expand: true, x_align: Clutter.ActorAlign.END, y_align: Clutter.ActorAlign.CENTER}));
        this.accessible_role = Atk.Role.MENU;
    }

    // No `activate` signal: that would close the menu.
    activate(event) {
        const type = event?.type();
        this._open(type === Clutter.EventType.KEY_PRESS || type === Clutter.EventType.KEY_RELEASE);
    }

    vfunc_key_press_event(event) {
        const forward = this.get_text_direction() === Clutter.TextDirection.RTL
            ? Clutter.KEY_Left : Clutter.KEY_Right;
        if (event.get_key_symbol() === forward) {
            this._open(true);
            return Clutter.EVENT_STOP;
        }
        return super.vfunc_key_press_event(event);
    }
});

// The first item of an inner page: its title, and the way back.
const BackItem = GObject.registerClass(
class WyeBackItem extends PopupMenu.PopupBaseMenuItem {
    _init(text, back) {
        super._init({style_class: 'wye-back-item'});
        this._back = back;
        this.add_child(new St.Icon({icon_name: 'go-previous-symbolic', style_class: 'popup-menu-arrow',
            y_align: Clutter.ActorAlign.CENTER}));
        const label = new St.Label({text, style_class: 'wye-back-label', x_expand: true,
            x_align: Clutter.ActorAlign.CENTER, y_align: Clutter.ActorAlign.CENTER});
        this.add_child(label);
        this.label_actor = label;
        // Keeps the title centred over the icon's width on the other side.
        this.add_child(new St.Widget({style_class: 'wye-back-balance'}));
    }

    activate(event) {
        const type = event?.type();
        this._back(type === Clutter.EventType.KEY_PRESS || type === Clutter.EventType.KEY_RELEASE);
    }
});

/**
 * The pages of one menu: the root, and a stack of pages opened from
 * [`PageItem`]s. Each page is drawn in a scroll view that scrolls only
 * when the page is taller than `maxHeight()` allows (a long Open In list on
 * a short screen).
 */
export class MenuPages {
    /**
     * @param {PopupMenu.PopupMenu} menu
     * @param {object} params
     * @param {() => number} params.maxHeight the most a page's items may
     *   take, logical px
     * @param {(section: PopupMenu.PopupMenuSection, pages: MenuPages) => void} params.root
     *   fills the root page
     */
    constructor(menu, {maxHeight, root}) {
        this._menu = menu;
        this._maxHeight = maxHeight;
        this._stack = [{title: null, key: null, fill: root}];
        this._scroll = null;
        this._section = null;
        menu.connect('open-state-changed', (_menu, open) => {
            // Each opening starts at the root; the chosen item may still
            // be handling its event.
            if (!open && this.inner)
                this._later(() => this.reset());
        });
        menu.actor.connect('key-press-event', (_actor, event) => this._key(event));
        this._pending = 0;
        this._destroyed = false;
        menu.connect('destroy', () => {
            this._destroyed = true;
            if (this._pending)
                GLib.source_remove(this._pending);
            this._pending = 0;
            // The menu took the page down with it.
            this._section = null;
            this._scroll = null;
        });
    }

    /** Whether an inner page shows. */
    get inner() {
        return this._stack.length > 1;
    }

    /**
     * Opens a page from the item with this `key` on the current page.
     *
     * @param {{title: string, key: string, fill: (section: PopupMenu.PopupMenuSection, pages: MenuPages) => void}} page
     * @param {boolean} keyboard focus its first item
     */
    open(page, keyboard) {
        this._later(() => {
            this._stack = [...this._stack, page];
            this.render(keyboard ? 'first' : null);
        });
    }

    /** Back to the root page, drawn again. */
    reset() {
        this._stack = this._stack.slice(0, 1);
        this.render();
    }

    /**
     * Back to the page before, focusing the item that opened this one.
     *
     * @param {boolean} keyboard
     */
    back(keyboard) {
        this._later(() => {
            if (!this.inner)
                return;
            const {key} = this._stack.at(-1);
            this._stack = this._stack.slice(0, -1);
            this.render(keyboard ? key : null);
        });
    }

    // The item that asked is still handling its event: replace the page
    // once that is over, never under it.
    _later(change) {
        if (this._pending)
            GLib.source_remove(this._pending);
        this._pending = GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            this._pending = 0;
            if (!this._destroyed)
                change();
            return GLib.SOURCE_REMOVE;
        });
    }

    /**
     * Draws the current page again, from its `fill`.
     *
     * @param {string|null} [focus] `'first'`, or the key of the item to focus
     */
    render(focus = null) {
        this._clear();
        this._menu.removeAll();
        const page = this._stack.at(-1);
        if (this.inner) {
            this._menu.addMenuItem(new BackItem(page.title, keyboard => this.back(keyboard)));
            this._menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        }
        const section = new PopupMenu.PopupMenuSection();
        page.fill(section, this);
        this._menu.addMenuItem(section);
        // The section, moved into a scroll view as tall as the page allows.
        const scroll = new St.ScrollView({
            style_class: 'wye-menu-page',
            style: `max-height: ${Math.max(120, Math.floor(this._maxHeight()))}px;`,
            hscrollbar_policy: St.PolicyType.NEVER,
            vscrollbar_policy: St.PolicyType.AUTOMATIC,
            overlay_scrollbars: true,
        });
        this._menu.box.remove_child(section.actor);
        scroll.set_child(section.actor);
        this._menu.box.add_child(scroll);
        this._scroll = scroll;
        this._section = section;
        for (const item of section._getMenuItems()) {
            item.connect('key-focus-in', () => ensureActorVisibleInScrollView(scroll, item));
        }
        if (focus)
            this._focus(focus, section);
    }

    _focus(focus, section) {
        const items = section._getMenuItems().filter(item => item.can_focus && item.reactive);
        const target = items.find(item => item._wyeKey === focus) ?? items[0];
        if (target)
            target.grab_key_focus();
        else
            this._menu.actor.navigate_focus(null, St.DirectionType.TAB_FORWARD, false);
    }

    _clear() {
        // The menu remembers its active item through the page's section
        // and would deactivate it, destroyed, when another one lights up.
        if (this._section && this._section._getMenuItems().includes(this._menu._activeMenuItem))
            this._menu._activeMenuItem = null;
        this._section?.destroy();
        this._section = null;
        this._scroll?.destroy();
        this._scroll = null;
    }

    // Left (or BackSpace) goes back a page.
    _key(event) {
        if (!this.inner)
            return Clutter.EVENT_PROPAGATE;
        const back = this._menu.actor.get_text_direction() === Clutter.TextDirection.RTL
            ? Clutter.KEY_Right : Clutter.KEY_Left;
        const symbol = event.get_key_symbol();
        if (symbol !== back && symbol !== Clutter.KEY_BackSpace)
            return Clutter.EVENT_PROPAGATE;
        this.back(true);
        return Clutter.EVENT_STOP;
    }
}

/**
 * A page item that [`MenuPages`] can focus again on the way back.
 *
 * @param {string} key
 * @param {string} text
 * @param {(keyboard: boolean) => void} open
 */
export function pageItem(key, text, open) {
    const item = new PageItem(text, open);
    item._wyeKey = key;
    return item;
}
