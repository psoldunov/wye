//! A page that is not built yet: its title and a "Coming soon" status page,
//! so the switcher, titles and routes work for all seven pages.

use adw::prelude::*;

use super::{Page, PageInfo};
use crate::widgets::empty_state;

/// A page with only a status page.
#[derive(Debug)]
pub struct PlaceholderPage {
    page: adw::PreferencesPage,
}

impl PlaceholderPage {
    /// The placeholder for `info`.
    #[must_use]
    pub fn new(info: &PageInfo) -> Self {
        let status = empty_state::empty_state(
            info.icon,
            "Coming Soon",
            &format!(
                "The {} settings are not in the GNOME version of Wye yet.",
                info.title
            ),
        );
        let group = adw::PreferencesGroup::new();
        group.add(&status);
        let page = adw::PreferencesPage::builder()
            .title(info.title)
            .name(info.id)
            .build();
        page.add(&group);
        Self { page }
    }
}

impl Page for PlaceholderPage {
    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }
}
