//! Icons as the service reports them (TGT-03, SHOWN-03): a theme name or an
//! absolute path. [`crate::widgets::icon::image`] draws either.
//!
//! Counterpart of crates/wye-ui/src/settings/icon.rs, which turns paths into
//! `file://` URLs for QML; GTK takes the path as it is.

/// The Picker's glyph (TGT-03, SET-02): the bulleted list.
pub const PICKER_ICON: &str = "view-list-bullet-symbolic";

/// The icon to show for `icon`; empty when there is none.
#[must_use]
pub fn source(icon: Option<&str>) -> String {
    icon.unwrap_or_default().to_owned()
}

/// Whether `source` names a file rather than a theme icon.
#[must_use]
pub fn is_file(source: &str) -> bool {
    source.starts_with('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_theme_name_stays_a_name() {
        assert_eq!(source(Some("firefox")), "firefox");
        assert!(!is_file("firefox"));
    }

    #[test]
    fn an_absolute_path_stays_a_path() {
        assert_eq!(source(Some("/tmp/a.png")), "/tmp/a.png");
        assert!(is_file("/tmp/a.png"));
    }

    #[test]
    fn no_icon_is_empty() {
        assert_eq!(source(None), "");
        assert_eq!(source(Some("")), "");
    }
}
