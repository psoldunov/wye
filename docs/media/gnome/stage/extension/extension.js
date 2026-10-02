// The demo stage's own extension (docs/media/gnome/stage/README.md). It puts
// the Shell in unsafe mode, so org.gnome.Shell.Eval and the Screenshot
// interface answer on the stage's bus, keeps the overview and notifications
// out of the screenshots, and gives the stage scripts `globalThis.wyeStage`:
// virtual pointer and keyboard, windows by app id and title, window captures
// with the client's own shadow, and the boxes of Shell actors. Only the
// stage loads it; it must never reach a real session.
import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import St from 'gi://St';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

// A Shell actor this much of the screen is a backdrop (a modal dialog's
// lightbox): its children are what shows.
const BACKDROP = 0.9;

const now = () => GLib.get_monotonic_time();

function box(actor) {
    const rect = actor.get_transformed_extents();
    return {
        x: Math.round(rect.origin.x),
        y: Math.round(rect.origin.y),
        width: Math.round(rect.size.width),
        height: Math.round(rect.size.height),
    };
}

function contains(outer, inner) {
    return inner.x >= outer.x && inner.y >= outer.y &&
        inner.x + inner.width <= outer.x + outer.width &&
        inner.y + inner.height <= outer.y + outer.height;
}

function union(boxes) {
    if (boxes.length === 0)
        return null;
    const left = Math.min(...boxes.map(b => b.x));
    const top = Math.min(...boxes.map(b => b.y));
    const right = Math.max(...boxes.map(b => b.x + b.width));
    const bottom = Math.max(...boxes.map(b => b.y + b.height));
    return {x: left, y: top, width: right - left, height: bottom - top};
}

function* actors(root) {
    yield root;
    for (const child of root.get_children())
        yield* actors(child);
}

function shows(actor) {
    return actor.is_mapped() && actor.opacity > 0 && actor.width > 0 && actor.height > 0;
}

// Whether the actor draws something to read: text or an icon. Layout boxes
// the Shell maps with nothing in them do not count as a popup.
function hasContent(actor) {
    for (const descendant of actors(actor)) {
        if ((descendant instanceof Clutter.Text || descendant instanceof St.Icon) && shows(descendant))
            return true;
    }
    return false;
}

function rect({x, y, width, height}) {
    return {x, y, width, height};
}

function describe(window) {
    return {
        id: window.get_id(),
        title: window.get_title() ?? '',
        app: window.get_gtk_application_id() ?? window.get_wm_class() ?? '',
        focus: window.has_focus(),
        sequence: window.get_stable_sequence(),
        frame: rect(window.get_frame_rect()),
        buffer: rect(window.get_buffer_rect()),
    };
}

class Stage {
    constructor() {
        this._pointer = null;
        this._keyboard = null;
        this._shown = new Set();
    }

    destroy() {
        this._pointer?.run_dispose();
        this._keyboard?.run_dispose();
        this._shown.clear();
    }

    // The overview, notifications and the unsafe-mode notice and panel icon
    // gone: none of them is part of what the screenshots show.
    clean() {
        Main.overview.hide();
        for (const source of Main.messageTray.getSources())
            source.destroy();
        const unsafe = Main.panel.statusArea.quickSettings?._unsafeMode;
        for (const icon of unsafe?.get_children() ?? [])
            icon.hide();
        return true;
    }

    screen() {
        return {width: global.stage.width, height: global.stage.height};
    }

    workArea() {
        const area = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex);
        return {x: area.x, y: area.y, width: area.width, height: area.height};
    }

    // Input. Coordinates are logical stage pixels.
    _device(type) {
        const seat = Clutter.get_default_backend().get_default_seat();
        return seat.create_virtual_device(type);
    }

    move(x, y) {
        this._pointer ??= this._device(Clutter.InputDeviceType.POINTER_DEVICE);
        this._pointer.notify_absolute_motion(now(), x, y);
        return true;
    }

    // button: 1 primary, 2 middle, 3 secondary.
    button(button, pressed) {
        this._pointer ??= this._device(Clutter.InputDeviceType.POINTER_DEVICE);
        this._pointer.notify_button(now(), button,
            pressed ? Clutter.ButtonState.PRESSED : Clutter.ButtonState.RELEASED);
        return true;
    }

    // name: a Clutter key name without KEY_ (Escape, Return, Control_L, a).
    key(name, pressed) {
        const keyval = Clutter[`KEY_${name}`];
        if (keyval === undefined)
            throw new Error(`no key ${name}`);
        return this.keyval(keyval, pressed);
    }

    keyval(keyval, pressed) {
        this._keyboard ??= this._device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
        this._keyboard.notify_keyval(now(), keyval,
            pressed ? Clutter.KeyState.PRESSED : Clutter.KeyState.RELEASED);
        return true;
    }

    char(character, pressed) {
        return this.keyval(Clutter.unicode_to_keysym(character.codePointAt(0)), pressed);
    }

    clipboard(text) {
        St.Clipboard.get_default().set_text(St.ClipboardType.CLIPBOARD, text);
        return true;
    }

    // Windows.
    windows() {
        return global.display.list_all_windows()
            .filter(window => window.get_window_type() !== Meta.WindowType.DESKTOP)
            .map(describe);
    }

    // The newest window whose app id contains `app` and whose title matches
    // the regular expression `title`.
    _find(app, title) {
        const pattern = new RegExp(title ?? '');
        const found = global.display.list_all_windows()
            .filter(window => (window.get_gtk_application_id() ?? window.get_wm_class() ?? '').includes(app ?? ''))
            .filter(window => pattern.test(window.get_title() ?? ''))
            .sort((a, b) => b.get_stable_sequence() - a.get_stable_sequence());
        return found[0] ?? null;
    }

    find(app, title) {
        const window = this._find(app, title);
        return window ? describe(window) : null;
    }

    _window(id) {
        const window = global.display.list_all_windows().find(candidate => candidate.get_id() === id);
        if (!window)
            throw new Error(`no window ${id}`);
        return window;
    }

    activate(id) {
        const window = this._window(id);
        Main.activateWindow(window);
        return describe(window);
    }

    // Give the window's visible frame this size, centred in the work area.
    place(id, width, height) {
        const window = this._window(id);
        const area = this.workArea();
        try {
            if (window.is_maximized?.() || window.get_maximized?.())
                window.unmaximize(Meta.MaximizeFlags?.BOTH);
        } catch (error) {
            logError(error, 'wye stage: unmaximize');
        }
        window.move_resize_frame(true,
            Math.round(area.x + (area.width - width) / 2),
            Math.round(area.y + (area.height - height) / 2),
            width, height);
        return describe(window);
    }

    close(id) {
        this._window(id).delete(global.get_current_time());
        return true;
    }

    // The window as its client drew it, shadow and rounded corners included,
    // in buffer pixels, written to a PNG. Returns its size.
    capture(id, path) {
        const actor = this._window(id).get_compositor_private();
        const surface = actor.get_image(null);
        if (!surface)
            throw new Error(`window ${id} gave no image`);
        surface.writeToPNG(path);
        return {width: surface.getWidth(), height: surface.getHeight()};
    }

    // Shell actors. mark() remembers what shows; shownBox() is the box
    // around what shows now and did not then (a picker, a menu).
    mark() {
        this._shown = new Set([...actors(global.stage)].filter(shows));
        return this._shown.size;
    }

    shownBox() {
        const screen = global.stage.width * global.stage.height;
        const boxes = [];
        const visit = actor => {
            if (!shows(actor))
                return;
            const fresh = !this._shown.has(actor);
            if (fresh) {
                if (!hasContent(actor))
                    return;
                // A group the Shell maps for its children (modalDialogGroup)
                // may be smaller than they are: then its children count.
                const extents = box(actor);
                const holds = actor.get_children().filter(shows)
                    .every(child => contains(extents, box(child)));
                if (holds && extents.width * extents.height < BACKDROP * screen) {
                    boxes.push(extents);
                    return;
                }
            }
            for (const child of actor.get_children())
                visit(child);
        };
        visit(global.stage);
        return union(boxes);
    }

    // The box of the topmost visible text that reads `text` (exactly, or
    // else as the start of it): a menu item, a button. A widget that shows
    // no text (an icon button) is found by its accessible name instead.
    textBox(text) {
        let exact = null;
        let prefix = null;
        let named = null;
        for (const actor of actors(global.stage)) {
            if (!shows(actor))
                continue;
            if (actor instanceof St.Widget && actor.accessible_name?.trim() === text)
                named = actor;
            if (!(actor instanceof Clutter.Text))
                continue;
            const value = actor.text?.trim() ?? '';
            if (value === text)
                exact = actor;
            else if (value.startsWith(text))
                prefix = actor;
        }
        const found = exact ?? named ?? prefix;
        return found ? box(found) : null;
    }

    // The panel button of an extension whose status-area key contains `name`.
    indicatorBox(name) {
        const key = Object.keys(Main.panel.statusArea).find(candidate => candidate.includes(name));
        const indicator = key ? Main.panel.statusArea[key] : null;
        return indicator && shows(indicator) ? box(indicator) : null;
    }
}

export default class WyeStageExtension extends Extension {
    enable() {
        // Unsafe mode lets any client of the bus run code in the Shell: only
        // ever on the stage, which marks every process it starts.
        if (!GLib.getenv('WYE_GNOME_STAGE_MARK')) {
            console.warn('Wye stage: WYE_GNOME_STAGE_MARK is not set, so this is not the stage ' +
                'Shell (docs/media/gnome/stage/stage.sh); not enabling unsafe mode');
            return;
        }
        this._unsafe = true;
        global.context.unsafe_mode = true;
        globalThis.wyeStage = new Stage();
        // The Shell opens in the overview and tells about unsafe mode.
        if (Main.layoutManager._startingUp) {
            this._startup = Main.layoutManager.connect('startup-complete',
                () => GLib.timeout_add(GLib.PRIORITY_DEFAULT, 500, () => {
                    globalThis.wyeStage?.clean();
                    return GLib.SOURCE_REMOVE;
                }));
        }
    }

    disable() {
        if (this._startup)
            Main.layoutManager.disconnect(this._startup);
        this._startup = null;
        globalThis.wyeStage?.destroy();
        delete globalThis.wyeStage;
        if (this._unsafe)
            global.context.unsafe_mode = false;
        this._unsafe = false;
    }
}
