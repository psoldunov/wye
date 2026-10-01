//! SCR-10: the question before unsaved changes are closed away, an
//! `AdwAlertDialog` with **Cancel**, **Discard** and **Save** (the default,
//! off while the script has a syntax error, SCR-07). Escape and Cancel go
//! back to the editor.
//!
//! Same copy as crates/wye-ui/qml/script/ScriptUnsavedDialog.qml.

use adw::prelude::*;

/// The answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Save,
    Discard,
}

const CANCEL: &str = "cancel";
const DISCARD: &str = "discard";
const SAVE: &str = "save";

/// Ask about the changes to `name` over `parent`; `answered` gets Save or
/// Discard, nothing for Cancel.
pub fn ask(
    parent: &impl IsA<gtk::Widget>,
    name: &str,
    can_save: bool,
    answered: impl Fn(Answer) + 'static,
) {
    let dialog = adw::AlertDialog::new(
        Some("Discard unsaved changes?"),
        Some(&body(name, can_save)),
    );
    dialog.add_response(CANCEL, "_Cancel");
    dialog.add_response(DISCARD, "_Discard");
    dialog.add_response(SAVE, "_Save");
    dialog.set_response_appearance(DISCARD, adw::ResponseAppearance::Destructive);
    dialog.set_response_appearance(SAVE, adw::ResponseAppearance::Suggested);
    dialog.set_response_enabled(SAVE, can_save);
    dialog.set_default_response(Some(if can_save { SAVE } else { CANCEL }));
    dialog.set_close_response(CANCEL);
    dialog.connect_response(None, move |_, response| match response {
        SAVE => answered(Answer::Save),
        DISCARD => answered(Answer::Discard),
        _ => {}
    });
    dialog.present(Some(parent));
}

fn body(name: &str, can_save: bool) -> String {
    if can_save {
        format!(
            "“{name}” has changes that are not saved. Save them, or discard them and close the editor."
        )
    } else {
        format!(
            "“{name}” has changes that are not saved, and the script has a syntax error, so it cannot be saved. Discard the changes and close the editor, or go back and fix it."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::body;

    #[test]
    fn scr_10_the_question_says_why_save_is_off() {
        assert!(
            body("Transform Script — Global", true)
                .starts_with("“Transform Script — Global” has changes")
        );
        assert!(body("x", false).contains("syntax error"));
    }
}
