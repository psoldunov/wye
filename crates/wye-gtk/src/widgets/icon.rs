//! Icons the service reports for targets and apps (TGT-03, SHOWN-03): a
//! theme name or an absolute path, with an optional profile badge (a coloured
//! initial, or a small picture) at the bottom end.
//!
//! API:
//! - [`image`]`(source, size)`: a `GtkImage` for a theme name or a file; an
//!   empty source gives an empty image of the same size, so rows line up.
//! - [`target_icon`]`(source, badge, size)`: the image with the badge over
//!   it, as the target menu draws a profile.
//! - [`picker_icon`]`(source, badge, size, badge_size)`: the same with the
//!   picker's own badge size (PICK-06, PICK-11), overlapping the icon's
//!   lower-start corner by [`picker_badge_overhang`].

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt::Write as _;

use adw::prelude::*;
use gtk::{gdk, pango};
use serde_json::Value;

use crate::settings::icon;

/// The icon of an app whose own icon the theme does not have.
pub const FALLBACK_APP_ICON: &str = "application-x-executable";

/// A `GtkImage` of `size` pixels for `source` (theme name or path). An app
/// icon the current theme lacks (a Flatpak whose icon is not exported, an
/// uninstalled app) shows the generic program icon instead of the "missing
/// image" glyph.
#[must_use]
pub fn image(source: &str, size: i32) -> gtk::Image {
    let image = if icon::is_file(source) {
        gtk::Image::from_file(source)
    } else if source.is_empty() {
        gtk::Image::new()
    } else {
        gtk::Image::from_icon_name(themed(source))
    };
    image.set_pixel_size(size);
    image
}

/// `name`, or [`FALLBACK_APP_ICON`] for a full-colour icon the theme does
/// not have. Symbolic icons (Wye's own come from the `GResource`) are left
/// alone.
fn themed(name: &str) -> &str {
    if name.ends_with("-symbolic") {
        return name;
    }
    // GTK also draws a name through its symbolic variant
    // (`utilities-terminal` as `utilities-terminal-symbolic`).
    let known = gdk::Display::default().is_none_or(|display| {
        let theme = gtk::IconTheme::for_display(&display);
        theme.has_icon(name) || theme.has_icon(&format!("{name}-symbolic"))
    });
    if known { name } else { FALLBACK_APP_ICON }
}

/// `source` with `badge` (`{"initial", "color"}` or `{"image"}`, the
/// service's `Badge`) over its lower-start corner, as the KDE picker draws it
/// (`PickerBadge.qml`). The widget is exactly `size` square: the badge
/// overhangs it a little instead of growing it.
#[must_use]
pub fn target_icon(source: &str, badge: Option<&Value>, size: i32) -> gtk::Widget {
    let badge_size = badge_size(size);
    badged(source, badge, size, badge_size, badge_size / 8)
}

/// A picker tile's icon (PICK-06): `source` at `size` with `badge` at the
/// picker's `badge_size` (PICK-11's metrics, 24 px on a 40 px icon), over
/// the icon's lower-start corner and reaching [`picker_badge_overhang`]
/// past it on both sides, as the Shell extension and the KDE picker draw
/// it. The widget stays `size` square; the tile keeps room for the part
/// below.
#[must_use]
pub fn picker_icon(source: &str, badge: Option<&Value>, size: i32, badge_size: i32) -> gtk::Widget {
    badged(
        source,
        badge,
        size,
        badge_size,
        picker_badge_overhang(badge_size),
    )
}

/// How far a picker badge of `badge_size` reaches past the icon's start
/// and bottom edges: a quarter of it, rounded (picker-view.js, KDE's
/// `PickerTile.qml`).
#[must_use]
pub const fn picker_badge_overhang(badge_size: i32) -> i32 {
    (badge_size + 2) / 4
}

/// `source` with `badge` of `badge_size` reaching `overhang` past the
/// lower-start corner.
fn badged(
    source: &str,
    badge: Option<&Value>,
    size: i32,
    badge_size: i32,
    overhang: i32,
) -> gtk::Widget {
    let base = image(source, size);
    let Some(badge) = badge.and_then(|badge| badge_widget(badge, badge_size)) else {
        return base.upcast();
    };
    let overlay = gtk::Overlay::builder()
        .child(&base)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    badge.set_halign(gtk::Align::Start);
    badge.set_valign(gtk::Align::End);
    badge.set_margin_start(-overhang);
    badge.set_margin_bottom(-overhang);
    overlay.add_overlay(&badge);
    overlay.upcast()
}

/// Smallest badge that still shows an initial.
const MIN_BADGE: i32 = 11;
/// Icons above this are picker-sized: the badge is a third of them.
const SMALL_ICON_MAX: i32 = 32;

/// The badge's diameter for an icon of `icon` pixels: about 55% of a small
/// icon (16 to 32 px), a third of a large one (KDE's 16 px on 48 px).
fn badge_size(icon: i32) -> i32 {
    if icon > SMALL_ICON_MAX {
        (icon + 1) / 3
    } else {
        ((icon * 11 + 10) / 20).max(MIN_BADGE)
    }
}

/// The ring cutting the badge out of the icon: `size / 16`, at least 1 px.
fn ring_width(size: i32) -> i32 {
    ((size + 8) / 16).max(1)
}

fn badge_widget(badge: &Value, size: i32) -> Option<gtk::Widget> {
    let ring = ring_width(size);
    let ring_class = format!("wye-badge-ring-{}", ring.min(3));
    if let Some(path) = badge
        .get("image")
        .and_then(Value::as_str)
        .filter(|path| icon::is_file(path))
    {
        let picture = gtk::Picture::for_filename(path);
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_size_request(size, size);
        // Clips the picture to the badge's round corners (CSS has no overflow).
        picture.set_overflow(gtk::Overflow::Hidden);
        picture.add_css_class("wye-badge");
        picture.add_css_class(&ring_class);
        return Some(picture.upcast());
    }
    let initial = badge.get("initial").and_then(Value::as_str)?;
    let initial = initial.chars().next()?;
    // A label sets the glyph on its line box (ascent and descent) and snaps
    // it to whole pixels, which pushes a capital off the middle. `Initial`
    // draws the glyph centred on its ink instead.
    let circle = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    circle.set_size_request(size, size);
    circle.append(&initial::Initial::new(&initial.to_string(), size));
    circle.add_css_class("wye-badge");
    circle.add_css_class(&ring_class);
    if let Some((class, light)) = badge
        .get("color")
        .and_then(Value::as_str)
        .and_then(colour_class)
    {
        circle.add_css_class(&class);
        circle.add_css_class(if light {
            "wye-badge-on-light"
        } else {
            "wye-badge-on-dark"
        });
    }
    Some(circle.upcast())
}

/// The initial of a badge, drawn so the ink of the glyph is centred in the
/// widget on both axes.
mod initial {
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;

    mod imp {
        use std::cell::RefCell;

        use gtk::glib;
        use gtk::graphene;
        use gtk::prelude::*;
        use gtk::subclass::prelude::*;

        #[derive(Default)]
        pub struct Initial {
            pub layout: RefCell<Option<gtk::pango::Layout>>,
        }

        #[glib::object_subclass]
        impl ObjectSubclass for Initial {
            const NAME: &'static str = "WyeBadgeInitial";
            type Type = super::Initial;
            type ParentType = gtk::Widget;
        }

        impl ObjectImpl for Initial {}

        impl WidgetImpl for Initial {
            fn snapshot(&self, snapshot: &gtk::Snapshot) {
                let Some(layout) = self.layout.borrow().clone() else {
                    return;
                };
                let widget = self.obj();
                let (ink, _) = layout.extents();
                let scale = f64::from(gtk::pango::SCALE);
                let x = f64::from(widget.width()) / 2.0
                    - (f64::from(ink.x()) + f64::from(ink.width()) / 2.0) / scale;
                let y = f64::from(widget.height()) / 2.0
                    - (f64::from(ink.y()) + f64::from(ink.height()) / 2.0) / scale;
                snapshot.save();
                #[expect(clippy::cast_possible_truncation, reason = "a sub-pixel offset")]
                snapshot.translate(&graphene::Point::new(x as f32, y as f32));
                snapshot.append_layout(&layout, &widget.color());
                snapshot.restore();
            }
        }
    }

    glib::wrapper! {
        pub struct Initial(ObjectSubclass<imp::Initial>) @extends gtk::Widget, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
    }

    impl Initial {
        /// `text` for a badge of `size` pixels.
        pub fn new(text: &str, size: i32) -> Self {
            let widget: Self = glib::Object::new();
            widget.set_hexpand(true);
            widget.set_vexpand(true);
            let layout = widget.create_pango_layout(Some(text));
            layout.set_attributes(Some(&super::initial_attributes(size)));
            widget.imp().layout.replace(Some(layout));
            widget
        }
    }
}

/// The initial: bold, half the badge's size, in pixels so the badge scales
/// with the icon and not with the font setting.
fn initial_attributes(size: i32) -> pango::AttrList {
    let attributes = pango::AttrList::new();
    attributes.insert(pango::AttrSize::new_size_absolute(
        (size / 2).max(1) * pango::SCALE,
    ));
    attributes.insert(pango::AttrInt::new_weight(pango::Weight::Heavy));
    attributes
}

/// Whether `rgb` (`rrggbb` or `rgb`, any alpha already removed) is a light
/// fill, by relative luminance, so the initial goes dark on it.
fn is_light(hex: &str) -> bool {
    let channel = |i: usize| -> f64 {
        let digits = if hex.len() >= 6 {
            hex.get(2 * i..2 * i + 2).map(str::to_owned)
        } else {
            hex.get(i..=i).map(|digit| digit.repeat(2))
        };
        let value = digits
            .and_then(|digits| u8::from_str_radix(&digits, 16).ok())
            .unwrap_or(0);
        let value = f64::from(value) / 255.0;
        if value <= 0.039_28 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(0) + 0.7152 * channel(1) + 0.0722 * channel(2) > 0.35
}

thread_local! {
    /// The badge colours already given a CSS class, and the provider holding
    /// their rules.
    static BADGE_COLOURS: RefCell<(BTreeSet<String>, Option<gtk::CssProvider>)> =
        const { RefCell::new((BTreeSet::new(), None)) };
}

/// A CSS class that paints a badge in `colour` (`#rrggbb`), registered once
/// per colour, and whether that fill is light. `None` for anything that is not
/// a hex colour, so a service value never reaches the style sheet unchecked.
fn colour_class(colour: &str) -> Option<(String, bool)> {
    let hex = colour.strip_prefix('#')?;
    let valid = matches!(hex.len(), 3 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit());
    if !valid {
        return None;
    }
    let hex = hex.to_ascii_lowercase();
    let light = is_light(&hex);
    let class = format!("wye-badge-{hex}");
    let display = gdk::Display::default()?;
    BADGE_COLOURS.with(|cell| {
        let mut cell = cell.borrow_mut();
        let (known, provider) = &mut *cell;
        if known.insert(hex.clone()) {
            let provider = provider.get_or_insert_with(|| {
                let provider = gtk::CssProvider::new();
                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
                provider
            });
            let rules = known.iter().fold(String::new(), |mut rules, hex| {
                // Writing to a String cannot fail.
                let _ = writeln!(
                    rules,
                    "box.wye-badge.wye-badge-{hex} {{ background-color: #{hex}; }}"
                );
                rules
            });
            provider.load_from_string(&rules);
        }
    });
    Some((class, light))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_hex_colours_become_classes() {
        // No display in unit tests: valid colours still fail at the display,
        // invalid ones before it.
        assert_eq!(colour_class("red; } * { color: red"), None);
        assert_eq!(colour_class("#12345"), None);
        assert_eq!(colour_class("336699"), None);
    }

    #[test]
    fn badge_geometry_follows_the_icon() {
        assert_eq!(badge_size(16), 11);
        assert_eq!(badge_size(24), 13);
        assert_eq!(badge_size(32), 18);
        assert_eq!(badge_size(48), 16);
        assert_eq!(ring_width(11), 1);
        assert_eq!(ring_width(18), 1);
        assert_eq!(ring_width(32), 2);
    }

    #[test]
    fn picker_badges_overlap_the_icon_by_a_quarter() {
        // PICK-06, PICK-11: the badges of the three picker sizes reach a
        // quarter of their size past the icon, rounded as the Shell
        // extension rounds it (`Math.round(badge / 4)`).
        assert_eq!(picker_badge_overhang(14), 4);
        assert_eq!(picker_badge_overhang(18), 5);
        assert_eq!(picker_badge_overhang(24), 6);
        // The ring of a large picker's badge, as KDE's `PickerBadge.qml`.
        assert_eq!(ring_width(24), 2);
    }

    #[test]
    fn the_initial_goes_dark_on_light_fills() {
        assert!(is_light("ffcc00"));
        assert!(is_light("fff"));
        assert!(!is_light("336699"));
        assert!(!is_light("000"));
    }
}
