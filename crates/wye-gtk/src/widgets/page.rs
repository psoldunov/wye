//! The Settings pages' column: one width for every page, wider than
//! libadwaita's default (600) so a rule's summary and its target fit side by
//! side and a callout runs over fewer lines, as on KDE, where the pages use
//! the window's width.
//!
//! `AdwPreferencesPage` has no property for its column's width: its content
//! sits in an `AdwClamp` (its documented node tree, `preferencespage >
//! scrolledwindow > viewport > clamp`), whose maximum size this sets. When a
//! later libadwaita builds the page differently, the page keeps its own
//! width.
//!
//! API:
//! - [`widen`]`(page)`: give `page` the Settings column width.

use adw::prelude::*;

/// The widest the Settings pages' column grows, in logical pixels.
pub const COLUMN_WIDTH: i32 = 700;

/// Give `page`'s column [`COLUMN_WIDTH`].
pub fn widen(page: &adw::PreferencesPage) {
    if let Some(clamp) = clamp_of(page.upcast_ref()) {
        clamp.set_maximum_size(COLUMN_WIDTH);
        // The whole width up to the maximum, not a column that narrows
        // gradually from libadwaita's default threshold.
        clamp.set_tightening_threshold(COLUMN_WIDTH);
    } else {
        tracing::debug!("this libadwaita's preferences page has no clamp to widen");
    }
}

/// The first `AdwClamp` under `widget`, depth first.
fn clamp_of(widget: &gtk::Widget) -> Option<adw::Clamp> {
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(clamp) = current.downcast_ref::<adw::Clamp>() {
            return Some(clamp.clone());
        }
        if let Some(found) = clamp_of(&current) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}
