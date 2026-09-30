//! The script being edited: what is on disk and what is in the editor
//! (SCR-07, SCR-08), and the source apps the test can pretend to come from
//! (SCR-04).

use serde::Serialize;
use wye_api::apps::AppList;

/// The editor's text against the saved text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Document {
    saved: String,
    current: String,
    loaded: bool,
}

impl Document {
    /// A script as `GetScript` answered it.
    #[must_use]
    pub fn loaded(text: &str) -> Self {
        Self {
            saved: text.to_owned(),
            current: text.to_owned(),
            loaded: true,
        }
    }

    /// The same script with the editor showing `text`.
    #[must_use]
    pub fn edited(&self, text: &str) -> Self {
        Self {
            current: text.to_owned(),
            ..self.clone()
        }
    }

    /// The same script after the editor's text was saved.
    #[must_use]
    pub fn saved(&self) -> Self {
        Self {
            saved: self.current.clone(),
            ..self.clone()
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.current
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.saved != self.current
    }

    /// SCR-07: Save needs a loaded, changed script without a syntax error.
    #[must_use]
    pub fn can_save(&self, syntax_error: bool) -> bool {
        self.loaded && self.is_dirty() && !syntax_error
    }
}

/// One entry of the source-app popup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceChoice {
    /// Desktop ID; empty for "none".
    pub id: String,
    pub name: String,
}

/// The popup's entries: the fixture's or `GetApps(true)`'s apps by name.
#[must_use]
pub fn source_choices(apps: &[(String, String)]) -> Vec<SourceChoice> {
    let mut choices: Vec<SourceChoice> = apps
        .iter()
        .map(|(id, name)| SourceChoice {
            id: id.clone(),
            name: name.clone(),
        })
        .collect();
    choices.sort_by_cached_key(|choice| choice.name.to_lowercase());
    choices
}

/// `GetApps(true)`'s JSON as `(desktop ID, name)` pairs; empty when it does
/// not parse.
#[must_use]
pub fn apps_of(json: &str) -> Vec<(String, String)> {
    wye_api::json::decode::<AppList>("AppList", json)
        .map(|list| {
            list.apps
                .into_iter()
                .map(|app| (app.id, app.name))
                .collect()
        })
        .unwrap_or_default()
}

/// The entries as JSON for QML.
#[must_use]
pub fn choices_json(choices: &[SourceChoice]) -> String {
    serde_json::to_string(choices).unwrap_or_else(|_| "[]".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scr_07_save_needs_changes_and_no_syntax_error() {
        let fresh = Document::default();
        assert!(!fresh.can_save(false), "nothing loaded yet");
        let loaded = Document::loaded("a");
        assert!(!loaded.is_dirty());
        assert!(!loaded.can_save(false));
        let edited = loaded.edited("b");
        assert!(edited.is_dirty());
        assert!(edited.can_save(false));
        assert!(!edited.can_save(true));
        let saved = edited.saved();
        assert!(!saved.is_dirty());
        assert_eq!(saved.text(), "b");
        assert_eq!(loaded.text(), "a", "the old document is unchanged");
    }

    #[test]
    fn scr_04_source_apps_are_sorted_by_name() {
        let choices = source_choices(&[
            ("b.desktop".into(), "beta".into()),
            ("a.desktop".into(), "Alpha".into()),
        ]);
        let names: Vec<_> = choices.iter().map(|choice| choice.name.as_str()).collect();
        assert_eq!(names, ["Alpha", "beta"]);
        assert!(choices_json(&choices).starts_with("[{\"id\":\"a.desktop\""));
        assert!(apps_of("not json").is_empty());
    }
}
