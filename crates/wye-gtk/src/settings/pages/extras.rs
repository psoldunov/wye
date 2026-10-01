//! The Extras page (09-extras.md): link hygiene applied to opened links
//! and, optionally, to copied ones. EXT-01 to EXT-05.
//!
//! The rows that rewrite copied text need clipboard watching (EXT-12).
//! GNOME's compositor offers apps no data-control protocol, so without
//! Wye's Shell integration those rows are dimmed, say "Not available in this
//! session" and keep a help button that explains why (BLK-10, 19-help-texts
//! "Clipboard features").
//!
//! KDE counterpart: crates/wye-ui/qml/settings/ExtrasPage.qml.

use adw::prelude::*;
use gtk::glib;

use super::{Context, Page};
use crate::settings::snapshot::Snapshot;
use crate::settings::store::SettingsStore;
use crate::widgets::{group, help, links, row, switch_row};

/// The subtitle of a clipboard row the session cannot serve.
const UNAVAILABLE: &str = "Not available in this session";

/// EXT-05, wording as on KDE; "Songlink" links to song.link (EXT-15).
const SONGLINK_TITLE: &str =
    "Convert copied music links to <a href=\"https://song.link\">Songlink</a>";
const SONGLINK_SUBTITLE: &str = "For easy sharing with anyone, regardless of the music service they use. Supports Apple Music, Spotify, TIDAL, and Deezer.";

/// The Extras page.
#[derive(Debug)]
pub struct ExtrasPage {
    page: adw::PreferencesPage,
}

impl ExtrasPage {
    /// Build the page on `context`'s store.
    #[must_use]
    pub fn new(context: &Context) -> Self {
        let store = &context.store;
        let page = adw::PreferencesPage::builder()
            .title("Extras")
            .name("extras")
            .build();

        // EXT-01: the help popover lists what is removed (EXT-10).
        let cleaning = group::group("");
        let on_open = switch_row::switch_row("Remove tracking parameters when opening links", "");
        switch_row::bind(store, &on_open, "extras.strip-tracking-on-open", true);
        help::add_help(store, &on_open, "remove-tracking");
        cleaning.add(&on_open);
        // EXT-02, EXT-03
        cleaning.add(&clipboard_row(
            store,
            "Remove tracking parameters when copying links",
            "",
            "extras.strip-tracking-on-copy",
        ));
        cleaning.add(&clipboard_row(
            store,
            "Remove leading “mailto:” when copying email addresses",
            "",
            "extras.strip-mailto-on-copy",
        ));
        page.add(&cleaning);

        // EXT-04
        let https = group::group("");
        let force = switch_row::switch_row("Force opened links to be HTTPS", "");
        switch_row::bind(store, &force, "extras.force-https", false);
        https.add(&force);
        page.add(&https);

        // EXT-05
        let music = group::group("");
        let songlink = clipboard_row(
            store,
            SONGLINK_TITLE,
            SONGLINK_SUBTITLE,
            "extras.songlink-on-copy",
        );
        music.add(&songlink);
        links::route_links(&songlink, context.link_opener());
        page.add(&music);

        Self { page }
    }
}

impl Page for ExtrasPage {
    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }
}

/// Whether the session lets Wye watch the clipboard (EXT-12).
fn watching(store: &SettingsStore) -> bool {
    store.with_snapshot(Snapshot::clipboard_watch_available)
}

/// A switch row for a feature that rewrites copied text: bound to `path`
/// (off by default), and unavailable where the clipboard cannot be watched.
fn clipboard_row(
    store: &SettingsStore,
    title: &str,
    subtitle: &'static str,
    path: &'static str,
) -> adw::SwitchRow {
    let switch = switch_row::switch_row(title, subtitle);
    switch_row::bind(store, &switch, path, false);
    let help = help::add_help(store, &switch, "clipboard-unavailable");
    if let Some(control) = switch.activatable_widget() {
        row::follow_writable(store, &control, watching);
    }
    let show = glib::clone!(
        #[weak]
        switch,
        move |store: &SettingsStore| {
            let available = watching(store);
            row::set_unavailable(&switch, !available);
            row::set_subtitle(&switch, if available { subtitle } else { UNAVAILABLE });
            help.set_visible(!available);
        }
    );
    show(store);
    store.connect_changed(show);
    switch
}
