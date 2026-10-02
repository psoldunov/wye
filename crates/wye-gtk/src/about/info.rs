//! What the About dialog says about Wye itself (DLG-ABT-01).
//!
//! Mirrors the facts in crates/wye-ui/src/about/info.rs; keep the two in
//! step.

/// The project's home, also its issue tracker's parent (Cargo.toml
/// `repository`).
pub const HOMEPAGE: &str = "https://github.com/psoldunov/wye";
/// Where bugs go (DLG-ABT-01, "issue tracker").
pub const ISSUE_TRACKER: &str = "https://github.com/psoldunov/wye/issues";
/// The desktop entry's ID, which is also the application's icon name (the
/// `GResource` carries it too, `data/resources.gresource.xml`).
pub const APP_ICON: &str = "dev.soldunov.wye";
/// What Wye is, in one line.
pub const DESCRIPTION: &str = "Opens every link in the browser you want.";
/// Who wrote it.
pub const AUTHOR: &str = "Philipp Soldunov";
/// The author's page.
pub const AUTHOR_URL: &str = "https://github.com/psoldunov";
/// Who wrote it, and the year (LICENSE).
pub const COPYRIGHT: &str = "© 2026 Philipp Soldunov";
/// The toolkits this host is built with, as `AdwAboutDialog` credits
/// (`Name URL`).
pub const CREDITS: [&str; 2] = [
    "GTK https://www.gtk.org/",
    "libadwaita https://gnome.pages.gitlab.gnome.org/libadwaita/",
];

/// The GTK host's own version, for when the service does not answer.
pub const UI_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_links_are_the_repository() {
        let manifest = include_str!("../../../../Cargo.toml");
        assert!(manifest.contains(&format!("repository = \"{HOMEPAGE}\"")));
        assert!(ISSUE_TRACKER.starts_with(HOMEPAGE));
    }

    #[test]
    fn the_icon_is_the_desktop_entry() {
        let resources = include_str!("../../data/resources.gresource.xml");
        assert!(resources.contains(&format!("{APP_ICON}.svg")));
    }
}
