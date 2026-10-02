// The picker's widgets (02-picker.md, "Layout"): the panel, the tiles, the
// "⋯" button and the URL line. Drawing only; picker.js decides what they
// do. Metrics follow the spec: panel padding 14 px, corner radius 16 px,
// tile padding 6 px, selection radius 12 px, hotkey text 10 px.
import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import Pango from 'gi://Pango';
import St from 'gi://St';
import {Backdrop} from './blur.js';
import {gicon} from './icons.js';
import {KDE_NAME_PX, columns, fitEnd, isLight, rows, tileWidth} from './model.mjs';

export const PANEL_RADIUS = 16;
const TILE_PADDING = 6;
// The gap between tiles, as `.wye-tile-row` spaces them.
const TILE_SPACING = 2;
const MORE_ICON = 16;
// PICK-09: the URL line may be as wide as the tiles or this, whichever is
// larger; the host gives way last, keeping at least this much.
const URL_MIN_WIDTH = 400;
const HOST_MIN_WIDTH = 216;

const VERTICAL = Clutter.Orientation.VERTICAL;

function label(text, styleClass, params = {}) {
    return new St.Label({text, style_class: styleClass, ...params});
}

function ellipsize(widget, mode) {
    widget.clutter_text.ellipsize = mode;
    widget.clutter_text.line_wrap = false;
    return widget;
}

/**
 * Moves a label so its glyphs' ink, not its line box, sits in the middle of
 * its parent: a capital's line box has more room below the baseline than
 * above the cap height, which would put an initial off centre.
 *
 * @param {St.Label} widget
 */
function centreInk(widget) {
    widget.connect('notify::allocation', () => {
        const parent = widget.get_parent();
        if (!parent)
            return;
        const [ink] = widget.clutter_text.get_layout().get_extents();
        // The layout is in device pixels.
        const unit = Pango.SCALE * widget.clutter_text.get_resource_scale();
        const box = widget.get_allocation_box();
        const [width, height] = parent.get_allocation_box().get_size();
        const inkX = box.x1 + (ink.x + ink.width / 2) / unit;
        const inkY = box.y1 + (ink.y + ink.height / 2) / unit;
        widget.translation_x = width / 2 - inkX;
        widget.translation_y = height / 2 - inkY;
    });
    return widget;
}

/**
 * The font size of a styled label, px, from its font's name (such as
 * "Adwaita Sans 9.9" in points or "Cantarell 13px"). Read from the string:
 * the Shell crashes when the collector frees a `PangoFontDescription`
 * taken from a theme node or a `ClutterText`.
 *
 * @param {St.Label} widget
 * @param {number} fallback
 */
function fontPixels(widget, fallback) {
    const match = /([\d.]+)(px)?$/.exec(widget.clutter_text.font_name ?? '');
    if (!match)
        return fallback;
    const size = Number.parseFloat(match[1]);
    return match[2] ? size : size * 96 / 72;
}

/**
 * A profile badge (PICK-06): the profile's picture, or its initial on its
 * colour, in a circle whose ring in the panel's colour cuts it out of the
 * icon under it.
 *
 * @param {{image?: string, initial?: string, color?: string|null}} badge
 * @param {number} size
 */
function badgeActor(badge, size) {
    const ring = Math.max(1, Math.round(size / 16));
    const common = `width: ${size}px; height: ${size}px; border-radius: ${size}px; border-width: ${ring}px;`;
    if (badge.image) {
        const path = badge.image.replaceAll('"', '\\"');
        return new St.Bin({
            style_class: 'wye-badge wye-badge-image',
            style: `${common} background-image: url("${path}"); background-size: cover;`,
        });
    }
    const fill = badge.color ? `background-color: ${badge.color};` : '';
    const initialClass = badge.color && isLight(badge.color) ? 'wye-badge-dark' : 'wye-badge-light';
    const bin = new St.Bin({
        style_class: `wye-badge ${initialClass}`,
        style: `${common} ${fill}`,
    });
    // PICK-06: the initial exactly in the middle of the circle.
    const initial = new St.Widget({layout_manager: new Clutter.BinLayout(), x_expand: true, y_expand: true});
    initial.add_child(centreInk(label(badge.initial, 'wye-badge-initial', {
        style: `font-size: ${Math.round(size * 0.5)}px;`,
        x_align: Clutter.ActorAlign.START,
        y_align: Clutter.ActorAlign.START,
    })));
    bin.set_child(initial);
    return bin;
}

const Tile = GObject.registerClass(
class WyeTile extends St.Button {
    constructor(entry, picker, hasHotkeys, badgeRoom) {
        super({
            style_class: 'wye-tile',
            can_focus: false,
            track_hover: true,
            button_mask: St.ButtonMask.ONE | St.ButtonMask.TWO | St.ButtonMask.THREE,
            accessible_name: entry.name,
        });
        const {metrics, showNames, showBadge} = picker;
        const box = new St.BoxLayout({orientation: VERTICAL, style_class: 'wye-tile-box',
            x_expand: true});
        this.set_child(box);
        this._hotkeyLabel = label(entry.hotkeyLabel || ' ', 'wye-hotkey', {
            x_align: Clutter.ActorAlign.CENTER, visible: hasHotkeys,
        });
        box.add_child(this._hotkeyLabel);
        // PICK-06: the badge over the icon's bottom-left corner.
        const frame = new St.Widget({
            layout_manager: new Clutter.FixedLayout(),
            width: metrics.icon, height: metrics.icon,
            x_align: Clutter.ActorAlign.CENTER,
            // Room for the part of a badge below the icon, in every tile so
            // the names stay in one line.
            style: `margin-bottom: ${badgeRoom}px;`,
        });
        frame.add_child(new St.Icon({gicon: gicon(entry.icon), icon_size: metrics.icon}));
        if (showBadge && entry.badge) {
            const badge = badgeActor(entry.badge, metrics.badge);
            badge.set_position(-Math.round(metrics.badge / 4),
                metrics.icon - Math.round(metrics.badge * 3 / 4));
            frame.add_child(badge);
        }
        box.add_child(frame);
        // PICK-05, PICK-10: one line, cut at the end.
        this._name = entry.name;
        this._nameLabel = ellipsize(label(entry.name, 'wye-tile-name', {
            x_align: Clutter.ActorAlign.CENTER, visible: showNames,
        }), Pango.EllipsizeMode.END);
        box.add_child(this._nameLabel);
    }

    /** The name's natural width, for the common tile width (PICK-05). */
    naturalNameWidth() {
        return this._nameLabel.get_preferred_width(-1)[1];
    }

    /** The name's font size, px, while the tile is in the stage. */
    namePixels() {
        return fontPixels(this._nameLabel, KDE_NAME_PX);
    }

    // PICK-05: the name cut at the end to the tile, "…" right after the
    // last letter that fits (Pango would keep a space before it).
    setWidth(width) {
        this.width = width;
        const room = width - 2 * TILE_PADDING;
        // At its natural width, so the tile centres it; never wider than
        // the tile, so Pango still cuts should the font change under it.
        this._nameLabel.style = `max-width: ${room}px;`;
        // Measured in the label itself, with its own font.
        const text = this._nameLabel.clutter_text;
        text.ellipsize = Pango.EllipsizeMode.NONE;
        // The layout is in device pixels.
        const scale = text.get_resource_scale();
        const measure = candidate => {
            text.text = candidate;
            return text.get_layout().get_pixel_size()[0] / scale <= room;
        };
        this._nameLabel.text = fitEnd(this._name, measure);
        text.ellipsize = Pango.EllipsizeMode.END;
    }

    setSelected(selected) {
        if (selected)
            this.add_style_pseudo_class('selected');
        else
            this.remove_style_pseudo_class('selected');
    }

    setDimmed(dimmed) {
        this.opacity = dimmed ? 89 : 255;
    }
});

/**
 * The panel and everything in it, for one request at a time.
 */
export class PickerView {
    /**
     * @param {object} handlers `hover(index, event)`, `click(index, button)`,
     *   `more()`
     */
    constructor(handlers) {
        this._handlers = handlers;
        // The frame: shadow, then the blurred backdrop, then the tinted
        // panel, all the panel's size.
        this.actor = new St.Widget({
            layout_manager: new Clutter.BinLayout(),
            reactive: true,
            style_class: 'wye-picker',
            accessible_role: 'dialog',
            accessible_name: 'Choose a browser',
        });
        this._shadow = new St.Widget({style_class: 'wye-picker-shadow',
            x_expand: true, y_expand: true});
        this.actor.add_child(this._shadow);
        this._backdrop = null;
        this.panel = new St.BoxLayout({orientation: VERTICAL, style_class: 'wye-picker-panel',
            x_expand: true, y_expand: true});
        this.actor.add_child(this.panel);
        this.tiles = [];
    }

    /** PICK-01: blur behind the panel when the Shell can; else opaque. */
    setBlur(enabled) {
        if (enabled && !this._backdrop) {
            try {
                this._backdrop = new Backdrop(PANEL_RADIUS);
                this.actor.insert_child_above(this._backdrop, this._shadow);
            } catch (error) {
                console.warn(`Wye picker: no blur (${error.message})`);
                this._backdrop = null;
            }
        } else if (!enabled && this._backdrop) {
            this._backdrop.destroy();
            this._backdrop = null;
        }
        if (this._backdrop)
            this.actor.add_style_class_name('wye-blurred');
        else
            this.actor.remove_style_class_name('wye-blurred');
    }

    /** PICK-12: the light or dark panel. */
    setLight(light) {
        if (light)
            this.actor.add_style_class_name('wye-light');
        else
            this.actor.remove_style_class_name('wye-light');
    }

    setStagePosition(x, y) {
        this.actor.set_position(x, y);
        this._backdrop?.setStagePosition(x, y);
    }

    /**
     * Builds the panel for `picker` (model.parsePicker).
     *
     * @param {object} picker
     * @param {number} room the widest a row of tiles may be
     */
    build(picker, room) {
        this.panel.destroy_all_children();
        this.tiles = [];
        const hasHotkeys = picker.tiles.some(entry => entry.hotkey);
        const top = new St.BoxLayout({style_class: 'wye-picker-top', x_align: Clutter.ActorAlign.CENTER});
        this.panel.add_child(top);
        if (picker.tiles.length > 0)
            top.add_child(this._grid(picker, hasHotkeys, room));
        else
            top.add_child(this._empty());
        top.add_child(this._moreColumn(picker, hasHotkeys));
        this.hint = label('', 'wye-hint', {x_align: Clutter.ActorAlign.CENTER, visible: false});
        this.panel.add_child(this.hint);
        if (picker.showUrl)
            this._urlLine(picker.url, top);
        if (picker.preview) {
            this.panel.add_child(label('Preview: choosing a browser opens nothing', 'wye-preview',
                {x_align: Clutter.ActorAlign.CENTER}));
        }
    }

    _grid(picker, hasHotkeys, room) {
        const badgeRoom = picker.showBadge && picker.tiles.some(entry => entry.badge)
            ? Math.ceil(picker.metrics.badge / 4) : 0;
        const tiles = picker.tiles.map((entry, index) => {
            const tile = new Tile(entry, picker, hasHotkeys, badgeRoom);
            tile.connect('motion-event', (_actor, event) => {
                this._handlers.hover(index, event);
                return Clutter.EVENT_PROPAGATE;
            });
            tile.connect('clicked', (_actor, button) => this._handlers.click(index, button));
            return tile;
        });
        this.tiles = tiles;
        // Measure in the stage so the names have their style.
        const grid = new St.BoxLayout({orientation: VERTICAL, style_class: 'wye-tile-grid'});
        const measure = new St.BoxLayout();
        this.panel.add_child(measure);
        tiles.forEach(tile => measure.add_child(tile));
        const widest = Math.max(0, ...tiles.map(tile => tile.naturalNameWidth()));
        const namePx = tiles.length > 0 ? tiles[0].namePixels() : KDE_NAME_PX;
        const width = tileWidth(widest, picker.metrics, TILE_PADDING, picker.showNames, namePx);
        // Cut while styled, in the stage.
        tiles.forEach(tile => tile.setWidth(width));
        tiles.forEach(tile => measure.remove_child(tile));
        measure.destroy();
        const perRow = columns(tiles.length, width, TILE_SPACING, room);
        for (const row of rows(tiles, perRow)) {
            const line = new St.BoxLayout({style_class: 'wye-tile-row'});
            row.forEach(tile => line.add_child(tile));
            grid.add_child(line);
        }
        return grid;
    }

    _empty() {
        const box = new St.BoxLayout({orientation: VERTICAL, style_class: 'wye-empty',
            y_align: Clutter.ActorAlign.CENTER});
        box.add_child(label('No browsers to show', 'wye-empty-title'));
        box.add_child(label('Use ⋯ to open the link another way', 'wye-empty-body'));
        return box;
    }

    // PICK-08: the "⋯" button, centred on the icons: the column repeats a
    // tile's hotkey row and icon box.
    _moreColumn(picker, hasHotkeys) {
        const column = new St.BoxLayout({orientation: VERTICAL, style_class: 'wye-more-column',
            y_align: picker.tiles.length > 0 ? Clutter.ActorAlign.START : Clutter.ActorAlign.CENTER});
        if (picker.tiles.length > 0) {
            column.add_child(label(' ', 'wye-hotkey', {opacity: 0, visible: hasHotkeys}));
        }
        const box = new St.Widget({
            layout_manager: new Clutter.BinLayout(),
            height: picker.tiles.length > 0 ? picker.metrics.icon : -1,
        });
        this.moreButton = new St.Button({
            style_class: 'wye-more-button',
            can_focus: false,
            accessible_name: 'More targets',
            child: new St.Icon({icon_name: 'view-more-horizontal-symbolic', icon_size: MORE_ICON}),
            x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.CENTER,
        });
        this.moreButton.connect('clicked', () => this._handlers.more());
        box.add_child(this.moreButton);
        column.add_child(box);
        return column;
    }

    // PICK-09: "from Slack", the host in bold, the rest dimmed and cut in
    // the middle; centred, capped at the tiles' width or URL_MIN_WIDTH.
    _urlLine(url, top) {
        const line = new St.BoxLayout({style_class: 'wye-url-line', x_align: Clutter.ActorAlign.CENTER,
            accessible_name: url.sourceName ? `From ${url.sourceName}: ${url.full}` : url.full});
        if (url.sourceName && url.sourceIcon) {
            line.add_child(new St.Icon({gicon: gicon(url.sourceIcon, ['application-x-executable']),
                icon_size: 16, style_class: 'wye-url-icon', y_align: Clutter.ActorAlign.CENTER}));
        }
        if (url.sourceName)
            line.add_child(label(`from ${url.sourceName}`, 'wye-url-source', {y_align: Clutter.ActorAlign.CENTER}));
        const host = ellipsize(label(url.host, 'wye-url-host', {y_align: Clutter.ActorAlign.CENTER}),
            Pango.EllipsizeMode.END);
        line.add_child(host);
        const rest = ellipsize(label(url.rest, 'wye-url-rest', {y_align: Clutter.ActorAlign.CENTER,
            x_expand: true, visible: url.rest !== ''}), Pango.EllipsizeMode.MIDDLE);
        line.add_child(rest);
        // Measured in the stage, where the labels have their style.
        this.panel.add_child(line);
        const [, natural] = line.get_preferred_width(-1);
        const cap = Math.max(top.get_preferred_width(-1)[1], URL_MIN_WIDTH);
        if (natural > cap) {
            line.width = cap;
            const hostWidth = host.get_preferred_width(-1)[1];
            host.style = `min-width: ${Math.min(hostWidth, HOST_MIN_WIDTH)}px;`;
        }
    }

    setSelected(index) {
        this.tiles.forEach((tile, i) => {
            tile.setSelected(i === index);
        });
    }

    setDimmed(dimmed) {
        this.tiles.forEach((tile, i) => {
            tile.setDimmed(dimmed[i]);
        });
    }

    /** PICK-14: what the held modifiers do. */
    setHint(text) {
        this.hint.text = text;
        this.hint.visible = text !== '';
    }

    destroy() {
        this.actor.destroy();
    }
}
