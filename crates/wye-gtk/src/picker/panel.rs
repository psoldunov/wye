//! The picker's panel for one request (02-picker.md, "Layout"): the tile
//! grid, the "⋯" button, the hint, the URL line and the preview note.
//! Drawing only; `super` decides what input does. Metrics follow the spec
//! and the GNOME Shell extension (frontends/gnome-shell/picker-view.js):
//! panel padding 14 px, corner radius 16 px, tile padding 6 px, selection
//! radius 12 px, hotkey text 10 px.

use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::{gdk, glib};
use wye_core::link_text::middle_truncate;

use super::model::{self, TileJson};
use super::state::PickerState;
use super::tile::{self, Layout, Tile};
use super::view::UrlLine;
use crate::widgets::icon;

/// The gap between tiles in a row and between rows.
const COLUMN_SPACING: i32 = 2;
const ROW_SPACING: i32 = 4;
/// PICK-09: the tile row sets the panel's width; the URL line is cut to it,
/// but one or two tiles still leave the link this much room.
const URL_MIN_WIDTH: i32 = 280;
/// The opacity of the link after its host (PICK-09, KDE's 0.6).
const URL_REST_ALPHA: &str = "60%";
/// The "⋯" glyph's size (PICK-08).
const MORE_ICON: i32 = 16;
/// How long the pointer's crossing events take to arrive once a menu's
/// popup is gone, before [`hush_tooltip`] reads whether it is on the button.
const TOOLTIP_SETTLE: Duration = Duration::from_millis(150);

/// What the panel's widgets report. Coordinates are the panel's.
pub struct Handlers {
    /// The pointer moved over tile `index` to (`x`, `y`) (PICK-22).
    pub hover: Box<dyn Fn(usize, f64, f64)>,
    /// Tile `index` was clicked with `button`, `modifiers` held, at
    /// (`x`, `y`) (PICK-20, PICK-30, PICK-32, PICK-33).
    pub click: Box<dyn Fn(usize, u32, gdk::ModifierType, f64, f64)>,
    /// The "⋯" button (PICK-08).
    pub more: Box<dyn Fn()>,
}

/// The widgets the picker updates after building.
#[derive(Debug)]
pub struct Built {
    pub tiles: Vec<Tile>,
    pub hint: gtk::Label,
    pub more: gtk::Button,
}

/// Fill `panel` for `state`, with rows no wider than `room` (PICK-13).
pub fn build(panel: &gtk::Box, state: &PickerState, room: i32, handlers: &Rc<Handlers>) -> Built {
    while let Some(child) = panel.first_child() {
        panel.remove(&child);
    }
    let view = &state.view;
    let json = model::tiles(state);
    let layout = layout(panel, state, &json);
    let top = gtk::Box::builder()
        .spacing(4)
        .halign(gtk::Align::Center)
        .build();
    let tiles: Vec<Tile> = json
        .iter()
        .zip(&view.tiles)
        .map(|(entry, tile)| {
            let badge = tile
                .badge
                .as_ref()
                .and_then(|badge| serde_json::to_value(badge).ok());
            let name = tile::cut_name(&entry.name, layout.width - 2 * tile::PADDING, |text| {
                tile::name_width(panel, text)
            });
            tile::build(entry, &name, badge.as_ref(), &layout)
        })
        .collect();
    if tiles.is_empty() {
        top.append(&empty());
    } else {
        top.append(&grid(
            &tiles,
            &layout,
            (room, view.columns()),
            panel,
            handlers,
        ));
    }
    let more = more_button(handlers);
    top.append(&more_column(&more, !tiles.is_empty(), &layout));
    panel.append(&top);
    let hint = gtk::Label::builder()
        .halign(gtk::Align::Center)
        .wrap(true)
        .visible(false)
        .build();
    hint.add_css_class("wye-picker-hint");
    panel.append(&hint);
    if view.show_url {
        let cap = top
            .measure(gtk::Orientation::Horizontal, -1)
            .1
            .max(URL_MIN_WIDTH);
        url_line(panel, &view.url, cap);
    }
    if view.preview {
        let note = gtk::Label::builder()
            .label("Preview: choosing a browser opens nothing")
            .halign(gtk::Align::Center)
            .wrap(true)
            .build();
        note.add_css_class("wye-picker-preview");
        panel.append(&note);
    }
    Built { tiles, hint, more }
}

fn layout(panel: &gtk::Box, state: &PickerState, json: &[TileJson]) -> Layout {
    let metrics = state.view.metrics;
    let widest = json
        .iter()
        .map(|entry| tile::name_width(panel, &entry.name))
        .max()
        .unwrap_or(0);
    let badge = i32::from(metrics.badge);
    let badged = state.view.tiles.iter().any(|tile| tile.badge.is_some());
    Layout {
        icon: i32::from(metrics.icon),
        badge,
        badge_room: if badged {
            icon::picker_badge_overhang(badge)
        } else {
            0
        },
        width: tile::width(widest, i32::from(metrics.pitch), state.view.show_names),
        show_names: state.view.show_names,
        hotkey_row: json.iter().any(|entry| !entry.hotkey.is_empty()),
    }
}

/// PICK-03, PICK-13: rows of up to `columns` tiles (eight at most), fewer
/// when a row would not fit `room`.
fn grid(
    tiles: &[Tile],
    layout: &Layout,
    (room, columns): (i32, usize),
    panel: &gtk::Box,
    handlers: &Rc<Handlers>,
) -> gtk::Grid {
    let grid = gtk::Grid::builder()
        .column_spacing(COLUMN_SPACING)
        .row_spacing(ROW_SPACING)
        .build();
    let fit = (room + COLUMN_SPACING) / (layout.width + COLUMN_SPACING);
    let per_row = usize::try_from(fit.max(1)).unwrap_or(1).min(columns);
    for (index, tile) in tiles.iter().enumerate() {
        let column = i32::try_from(index % per_row).unwrap_or(0);
        let row = i32::try_from(index / per_row).unwrap_or(0);
        grid.attach(&tile.root, column, row, 1, 1);
        connect(tile, index, panel, handlers);
    }
    grid
}

fn connect(tile: &Tile, index: usize, panel: &gtk::Box, handlers: &Rc<Handlers>) {
    let motion = gtk::EventControllerMotion::new();
    let hover = Rc::clone(handlers);
    let (root, inside) = (tile.root.clone(), panel.downgrade());
    motion.connect_motion(move |_, x, y| {
        if let Some((x, y)) = inside
            .upgrade()
            .and_then(|panel| in_panel(&root, &panel, x, y))
        {
            (hover.hover)(index, x, y);
        }
    });
    tile.root.add_controller(motion);
    // Every button: left chooses, middle opens in the background, right
    // opens the tile's menu.
    let click = gtk::GestureClick::builder().button(0).build();
    let clicked = Rc::clone(handlers);
    let (root, inside) = (tile.root.clone(), panel.downgrade());
    click.connect_released(move |gesture, _, x, y| {
        let button = gesture.current_button();
        let modifiers = gesture.current_event_state();
        gesture.set_state(gtk::EventSequenceState::Claimed);
        let (x, y) = inside
            .upgrade()
            .and_then(|panel| in_panel(&root, &panel, x, y))
            .unwrap_or((x, y));
        (clicked.click)(index, button, modifiers, x, y);
    });
    tile.root.add_controller(click);
}

/// A point of `tile` in the coordinates of `panel`.
fn in_panel(tile: &gtk::Box, panel: &gtk::Box, x: f64, y: f64) -> Option<(f64, f64)> {
    #[allow(
        clippy::cast_possible_truncation,
        reason = "graphene takes f32; a point inside a window fits"
    )]
    let point = gtk::graphene::Point::new(x as f32, y as f32);
    let point = tile.compute_point(panel, &point)?;
    Some((f64::from(point.x()), f64::from(point.y())))
}

/// No tile to show: the "⋯" menu still opens the link.
fn empty() -> gtk::Box {
    let column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .valign(gtk::Align::Center)
        .margin_start(8)
        .margin_end(8)
        .margin_top(4)
        .margin_bottom(4)
        .build();
    let title = gtk::Label::builder()
        .label("No browsers to show")
        .halign(gtk::Align::Start)
        .build();
    title.add_css_class("heading");
    let body = gtk::Label::builder()
        .label("Use ⋯ to open the link another way")
        .halign(gtk::Align::Start)
        .build();
    body.add_css_class("wye-picker-dim");
    column.append(&title);
    column.append(&body);
    column
}

fn more_button(handlers: &Rc<Handlers>) -> gtk::Button {
    let button = gtk::Button::builder()
        .css_classes(["flat", "circular"])
        .focusable(false)
        .can_focus(false)
        .tooltip_text("More targets")
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    let glyph = gtk::Image::from_icon_name("view-more-horizontal-symbolic");
    glyph.set_pixel_size(MORE_ICON);
    button.set_child(Some(&glyph));
    button.add_css_class("wye-picker-more");
    button.update_property(&[gtk::accessible::Property::Label("More targets")]);
    let more = Rc::clone(handlers);
    button.connect_clicked(move |_| (more.more)());
    let motion = gtk::EventControllerMotion::new();
    let weak = button.downgrade();
    motion.connect_leave(move |_| {
        if let Some(button) = weak.upgrade() {
            button.set_has_tooltip(true);
        }
    });
    button.add_controller(motion);
    button
}

/// PICK-08: keep `button`'s tooltip back while the pointer that closed its
/// menu still rests on it; leaving the button brings the tooltip back, and
/// so does a pointer that is elsewhere once the popup has gone.
pub fn hush_tooltip(button: &gtk::Button) {
    button.set_has_tooltip(false);
    let weak = button.downgrade();
    glib::timeout_add_local_once(TOOLTIP_SETTLE, move || {
        if let Some(button) = weak.upgrade()
            && !button.state_flags().contains(gtk::StateFlags::PRELIGHT)
        {
            button.set_has_tooltip(true);
        }
    });
}

/// PICK-08: the "⋯" button follows the last tile, centred on the icons: its
/// column repeats a tile's hotkey row and icon box.
fn more_column(button: &gtk::Button, has_tiles: bool, layout: &Layout) -> gtk::Box {
    let column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .valign(if has_tiles {
            gtk::Align::Start
        } else {
            gtk::Align::Center
        })
        .build();
    column.add_css_class("wye-picker-more-column");
    if has_tiles && layout.hotkey_row {
        let spacer = gtk::Label::new(Some(" "));
        spacer.add_css_class("wye-tile-hotkey");
        spacer.set_opacity(0.0);
        column.append(&spacer);
    }
    let holder = gtk::Box::builder()
        .height_request(if has_tiles { layout.icon } else { -1 })
        .build();
    holder.append(button);
    column.append(&holder);
    column
}

/// PICK-09: "from Slack", the host in bold, the rest dimmed, cut in the
/// middle when the line is wider than `cap`; centred, at the end of `panel`.
fn url_line(panel: &gtk::Box, url: &UrlLine, cap: i32) {
    let line = gtk::Box::builder().spacing(0).build();
    line.add_css_class("wye-picker-url");
    let described = if url.source_name.is_empty() {
        url.full.clone()
    } else {
        format!("From {}: {}", url.source_name, url.full)
    };
    line.update_property(&[gtk::accessible::Property::Label(&described)]);
    line.set_tooltip_text(Some(&url.full));
    if !url.source_name.is_empty() && !url.source_icon.is_empty() {
        let source_icon = icon::image(&url.source_icon, 16);
        source_icon.set_margin_end(5);
        line.append(&source_icon);
    }
    if !url.source_name.is_empty() {
        let source = gtk::Label::builder()
            .label(format!("from {}", url.source_name))
            .margin_end(10)
            .build();
        source.add_css_class("wye-picker-url-source");
        line.append(&source);
    }
    // One label, so nothing but the text sits between the host and the rest.
    // Pango's ellipsis stays as a last resort, for a host alone too wide.
    let address = gtk::Label::builder()
        .use_markup(true)
        .label(link_markup(&url.host, &url.rest))
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .single_line_mode(true)
        .build();
    address.add_css_class("wye-picker-url-link");
    line.append(&address);
    panel.append(
        &adw::Clamp::builder()
            .maximum_size(cap)
            .tightening_threshold(cap)
            .halign(gtk::Align::Center)
            .child(&line)
            .build(),
    );
    // Rooted now, so the line's font applies to the measurements.
    let others = line.measure(gtk::Orientation::Horizontal, -1).1
        - address.measure(gtk::Orientation::Horizontal, -1).1;
    address.set_markup(&fitted_link(&address, url, cap - others));
}

/// The link's markup, its rest cut in the middle by characters until it is
/// no wider than `room` in the font of `label`, keeping the host and the end
/// of the path. Cut here rather than by Pango, whose middle ellipsis leaves
/// a gap after the "…".
fn fitted_link(label: &gtk::Label, url: &UrlLine, room: i32) -> String {
    let fits = |markup: &str| {
        let layout = label.create_pango_layout(None);
        layout.set_markup(markup);
        layout.pixel_size().0 <= room
    };
    let markup = |kept: usize| link_markup(&url.host, &middle_truncate(&url.rest, kept));
    let whole = link_markup(&url.host, &url.rest);
    if fits(&whole) {
        return whole;
    }
    // The most characters of the rest that fit, at least the "…" alone.
    let (mut low, mut high) = (1, url.rest.chars().count().saturating_sub(1));
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if fits(&markup(middle)) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    markup(low)
}

/// PICK-09: the host in bold, the rest of the link dimmed, as Pango markup.
fn link_markup(host: &str, rest: &str) -> String {
    let host = gtk::glib::markup_escape_text(host);
    if rest.is_empty() {
        return format!("<b>{host}</b>");
    }
    let rest = gtk::glib::markup_escape_text(rest);
    format!("<b>{host}</b><span alpha=\"{URL_REST_ALPHA}\">{rest}</span>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_link_is_the_bold_host_then_the_dimmed_rest_with_nothing_between() {
        // PICK-09.
        assert_eq!(
            link_markup("github.com", "/a?q=is%3Aopen&x=<1>"),
            "<b>github.com</b><span alpha=\"60%\">/a?q=is%3Aopen&amp;x=&lt;1&gt;</span>"
        );
        assert_eq!(link_markup("a&b", ""), "<b>a&amp;b</b>");
    }
}
