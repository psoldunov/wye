//! BLK-01 Group card: an `AdwPreferencesGroup`, a rounded card a step
//! lighter than the window with its rows separated by inset hairlines, an
//! optional bold title above it ("Startup", "Tray") and vertical space
//! between groups. Add rows with `group.add(&row)`.
//!
//! API:
//! - [`group`]`(title)`: a group; an empty title is none.
//! - [`group_with_description`]`(title, description)`: with a dimmed
//!   description under the title (markup, links go through Wye with
//!   [`super::links::route_links`]).

use adw::prelude::*;

/// A preferences group titled `title` (none when empty).
#[must_use]
pub fn group(title: &str) -> adw::PreferencesGroup {
    adw::PreferencesGroup::builder().title(title).build()
}

/// A preferences group with `title` and a `description` under it.
#[must_use]
pub fn group_with_description(title: &str, description: &str) -> adw::PreferencesGroup {
    let group = group(title);
    group.set_description(Some(description).filter(|text| !text.is_empty()));
    group
}
