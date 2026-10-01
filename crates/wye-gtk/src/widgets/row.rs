//! BLK-02 Row and BLK-10 Disabled row.
//!
//! A row is an `AdwActionRow`: title on the left, an optional dimmed
//! subtitle under it (markup: `<b>`, `<tt>` for inline code, `<a>` links,
//! which [`super::links::route_links`] sends through Wye), and its controls
//! as suffixes, centred on the title.
//!
//! API:
//! - [`action_row`]`(title, subtitle)`: the row; an empty subtitle is none.
//! - [`titled`]`(row, title, subtitle)`: the same for a subclass (switch,
//!   combo row), so every row reads its title and subtitle alike.
//! - [`add_leading_icon`]`(row)` then [`LeadingIcon::set`]`(icon, tint)`: an
//!   icon before the title, for example a status glyph tinted with
//!   [`Tint::Success`] (GEN-05).
//! - [`set_disabled`]`(row, disabled)`: dim the title, help button and
//!   control when the setting does not apply (BLK-10).
//! - [`set_unavailable`]`(row, unavailable)`: dim a row the session cannot
//!   offer while its help button keeps working (KEY-06).
//! - [`follow_writable`]`(store, widget, applies)`: keep a configuration
//!   control sensitive only while the file is writable and `applies` holds.

use adw::prelude::*;

use crate::settings::store::SettingsStore;

/// An `AdwActionRow` with `title` and, unless empty, `subtitle` (markup).
#[must_use]
pub fn action_row(title: &str, subtitle: &str) -> adw::ActionRow {
    titled(adw::ActionRow::new(), title, subtitle)
}

/// `row` (an action row or a subclass: switch, combo) with `title` and,
/// unless empty, `subtitle` (both markup).
pub fn titled<R>(row: R, title: &str, subtitle: &str) -> R
where
    R: IsA<adw::ActionRow> + IsA<adw::PreferencesRow>,
{
    row.set_use_markup(true);
    row.set_title(title);
    set_subtitle(&row, subtitle);
    row
}

/// Set or clear the subtitle of any `AdwActionRow` (switch and combo rows
/// included).
pub fn set_subtitle(row: &impl IsA<adw::ActionRow>, subtitle: &str) {
    row.set_subtitle(subtitle);
}

/// How a leading icon is tinted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tint {
    /// The success colour: a check that reads as "all good".
    Success,
    /// The warning colour.
    Warning,
    /// The error colour.
    Error,
    /// The accent colour.
    Accent,
}

impl Tint {
    const fn class(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Accent => "accent",
        }
    }
}

/// Classes a leading icon may carry; cleared before a new tint is set.
const TINTS: [Tint; 4] = [Tint::Success, Tint::Warning, Tint::Error, Tint::Accent];

/// An icon before a row's title, for example the General page's status glyph
/// (GEN-05): a plain mark, never a box that could pass for a check box.
#[derive(Debug, Clone)]
pub struct LeadingIcon {
    image: gtk::Image,
}

impl LeadingIcon {
    /// Show `icon` (a symbolic icon name) tinted with `tint`; an empty
    /// `icon` hides it.
    pub fn set(&self, icon: &str, tint: Tint) {
        for other in TINTS {
            self.image.remove_css_class(other.class());
        }
        self.image.add_css_class(tint.class());
        self.image
            .set_icon_name(Some(icon).filter(|icon| !icon.is_empty()));
        self.image.set_visible(!icon.is_empty());
    }
}

/// Add a leading icon slot before the title of `row`; set it with
/// [`LeadingIcon::set`].
pub fn add_leading_icon(row: &impl IsA<adw::ActionRow>) -> LeadingIcon {
    let image = gtk::Image::builder()
        .pixel_size(16)
        .valign(gtk::Align::Center)
        .css_classes(["wye-status-glyph"])
        .visible(false)
        .build();
    row.add_prefix(&image);
    LeadingIcon { image }
}

/// BLK-10: dim `row` (title, help button and control) when the setting does
/// not apply. libadwaita draws an insensitive row dimmed and inert.
pub fn set_disabled(row: &impl IsA<gtk::Widget>, disabled: bool) {
    row.set_sensitive(!disabled);
}

/// BLK-10, KEY-06: a setting the session cannot offer ("Not available in
/// this session"). Unlike [`set_disabled`] the row itself stays sensitive,
/// so its help button still opens and says why: the title and subtitle are
/// dimmed (`wye-unavailable`) and a row that toggles its control when
/// clicked no longer does. The control itself is made insensitive by
/// [`follow_writable`] with an `applies` that checks availability (an
/// `AdwSwitchRow`'s switch is its `activatable_widget`).
pub fn set_unavailable<R>(row: &R, unavailable: bool)
where
    R: IsA<adw::ActionRow> + IsA<gtk::Widget> + IsA<gtk::ListBoxRow>,
{
    if unavailable {
        row.add_css_class("wye-unavailable");
    } else {
        row.remove_css_class("wye-unavailable");
    }
    if row.activatable_widget().is_some() {
        row.set_activatable(!unavailable);
    }
}

/// Keep `widget` sensitive while the configuration file is writable and
/// `applies(store)` holds (SET-06: a read-only file keeps the window usable,
/// but nothing that changes the file can be used). Runs now and after every
/// store change.
pub fn follow_writable<W, F>(store: &SettingsStore, widget: &W, applies: F)
where
    W: IsA<gtk::Widget>,
    F: Fn(&SettingsStore) -> bool + 'static,
{
    let widget_ref = widget.upcast_ref::<gtk::Widget>();
    let widget = widget_ref.downgrade();
    let update = move |store: &SettingsStore| {
        if let Some(widget) = widget.upgrade() {
            widget.set_sensitive(store.writable() && applies(store));
        }
    };
    update(store);
    // The handler goes with the widget: rows built again are not followed.
    store.connect_changed_while(widget_ref, update);
}
