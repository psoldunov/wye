//! Icons as QML draws them (TGT-03, SHOWN-03).
//!
//! The service reports an icon as a theme name or an absolute path.
//! `Kirigami.Icon` takes a theme name as it is and a file as a URL, so a
//! path becomes a `file://` URL here, once, for every row.

/// The Picker's glyph (TGT-03, SET-02): the bulleted list.
pub const PICKER_ICON: &str = "view-list-text";

/// What to give `Kirigami.Icon.source` for `icon`; empty when there is none.
#[must_use]
pub fn source(icon: Option<&str>) -> String {
    match icon {
        None | Some("") => String::new(),
        Some(path) if path.starts_with('/') => format!("file://{path}"),
        Some(name) => name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_theme_name_stays_a_name() {
        assert_eq!(source(Some("firefox")), "firefox");
    }

    #[test]
    fn an_absolute_path_becomes_a_file_url() {
        assert_eq!(source(Some("/tmp/a.png")), "file:///tmp/a.png");
    }

    #[test]
    fn no_icon_is_empty() {
        assert_eq!(source(None), "");
        assert_eq!(source(Some("")), "");
    }
}
