//! One tile (PICK-04 to PICK-07), drawn as the GNOME Shell extension draws
//! it (frontends/gnome-shell/picker-view.js): the hotkey row, the icon with
//! its profile badge (PICK-06, the widget kit's `target_icon`, BLK-04), the
//! name cut at the end, all tiles one width.

use adw::prelude::*;
use serde_json::Value;

use super::model::TileJson;
use crate::widgets::icon;

/// Padding inside a tile (02-picker.md, "At every size").
pub const PADDING: i32 = 6;

/// The tile name's font size relative to the panel's (`.wye-tile-name` in
/// style.css, KDE's small font).
const NAME_SCALE: f64 = 0.9;

/// What every tile of one request shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub icon: i32,
    /// Every tile's width (PICK-05).
    pub width: i32,
    pub show_names: bool,
    /// Whether any tile has a hotkey: then every tile keeps the row.
    pub hotkey_row: bool,
}

/// A tile's widgets the picker updates.
#[derive(Debug, Clone)]
pub struct Tile {
    pub root: gtk::Box,
}

impl Tile {
    /// The selected tile is tinted with the accent (PICK-07).
    pub fn set_selected(&self, selected: bool) {
        if selected {
            self.root.add_css_class("wye-selected");
        } else {
            self.root.remove_css_class("wye-selected");
        }
    }

    /// A tile that cannot open the held way is dimmed (PICK-14).
    pub fn set_dimmed(&self, dimmed: bool) {
        self.root.set_opacity(if dimmed { 0.35 } else { 1.0 });
    }
}

/// The width every tile gets (PICK-05): from the size's pitch up to twice
/// that, as wide as the longest name needs.
pub fn width(widest_name: i32, pitch: i32, show_names: bool) -> i32 {
    if !show_names {
        return pitch;
    }
    (widest_name + 2 * PADDING + 2).clamp(pitch, pitch * 2)
}

/// The natural width of `name` in the tile's name style, measured with the
/// font of `panel` (rooted, so its style applies).
pub fn name_width(panel: &impl IsA<gtk::Widget>, name: &str) -> i32 {
    let (width, _) = panel.as_ref().create_pango_layout(Some(name)).pixel_size();
    let scaled = (f64::from(width) * NAME_SCALE).ceil();
    #[allow(
        clippy::cast_possible_truncation,
        reason = "clamped to i32's range just before"
    )]
    let width = scaled.clamp(0.0, f64::from(i32::MAX)) as i32;
    width
}

/// The tile for `json` with the service's `badge` (`None` when PKS-04 is
/// off or the target has none).
pub fn build(json: &TileJson, badge: Option<&Value>, layout: &Layout) -> Tile {
    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .width_request(layout.width)
        .build();
    root.add_css_class("wye-tile");
    root.update_property(&[gtk::accessible::Property::Label(&json.name)]);
    let hotkey = gtk::Label::builder()
        .label(if json.hotkey.is_empty() {
            " "
        } else {
            &json.hotkey
        })
        .visible(layout.hotkey_row)
        .build();
    hotkey.add_css_class("wye-tile-hotkey");
    root.append(&hotkey);
    let image = icon::target_icon(&json.icon, badge, layout.icon);
    image.set_halign(gtk::Align::Center);
    root.append(&image);
    let name = gtk::Label::builder()
        .label(&json.name)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .single_line_mode(true)
        .build();
    name.add_css_class("wye-tile-name");
    // PICK-05, PICK-10: one line, as wide as the tile, cut at the end.
    let clamp = adw::Clamp::builder()
        .maximum_size(layout.width - 2 * PADDING)
        .tightening_threshold(layout.width - 2 * PADDING)
        .child(&name)
        .visible(layout.show_names)
        .build();
    root.append(&clamp);
    Tile { root }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_are_as_wide_as_the_longest_name_within_two_pitches() {
        // PICK-05.
        assert_eq!(width(10, 48, true), 48);
        assert_eq!(width(60, 48, true), 60 + 2 * PADDING + 2);
        assert_eq!(width(400, 48, true), 96);
        assert_eq!(width(400, 48, false), 48);
    }
}
