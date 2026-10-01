//! Import Rules… and Export Rules… (RUL-02): a `GtkFileDialog` (the portal's
//! file chooser where there is one) picks the file; GIO reads or writes it
//! without blocking the window; the service does the rest (`ImportRules`
//! adds the file's rules at the end of the list, `ExportRules` returns all
//! rules with their scripts as one file).
//!
//! KDE counterpart: `importFrom` / `exportTo` in
//! crates/wye-ui/src/bridge/rules.rs.

use adw::prelude::*;
use gtk::{gio, glib};

/// The name an export suggests.
const EXPORT_NAME: &str = "wye-rules.toml";

/// What a transfer did, for the page's toast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// `count` rules were added.
    Imported(u32),
    Exported,
    /// The message for the user.
    Failed(String),
}

impl Outcome {
    /// The toast's text.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::Imported(1) => "Imported 1 rule".to_owned(),
            Self::Imported(count) => format!("Imported {count} rules"),
            Self::Exported => "Rules exported".to_owned(),
            Self::Failed(message) => message.clone(),
        }
    }
}

/// Wye rules files, and everything.
fn filters(all_files: bool) -> gio::ListStore {
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    let rules = gtk::FileFilter::new();
    rules.set_name(Some("Wye rules (*.toml)"));
    rules.add_suffix("toml");
    filters.append(&rules);
    if all_files {
        let all = gtk::FileFilter::new();
        all.set_name(Some("All files"));
        all.add_pattern("*");
        filters.append(&all);
    }
    filters
}

/// Ask for a rules file and import it; `done` runs unless the user
/// cancels.
pub fn import(window: &gtk::Window, done: impl FnOnce(Outcome) + 'static) {
    let dialog = gtk::FileDialog::builder()
        .title("Import Rules")
        .modal(true)
        .filters(&filters(true))
        .build();
    let window = window.clone();
    glib::spawn_future_local(async move {
        let Ok(file) = dialog.open_future(Some(&window)).await else {
            return;
        };
        let text = match file.load_contents_future().await {
            Ok((bytes, _)) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(error) => {
                done(Outcome::Failed(format!("Cannot read the file: {error}")));
                return;
            }
        };
        crate::service::request(
            move |proxy| async move { proxy.import_rules(&text).await },
            move |answer| {
                done(match answer {
                    Ok(count) => Outcome::Imported(count),
                    Err(error) => Outcome::Failed(format!("Cannot import the rules: {error}")),
                });
            },
        );
    });
}

/// Ask where to save and export every rule there.
pub fn export(window: &gtk::Window, done: impl FnOnce(Outcome) + 'static) {
    let dialog = gtk::FileDialog::builder()
        .title("Export Rules")
        .modal(true)
        .initial_name(EXPORT_NAME)
        .filters(&filters(false))
        .build();
    let window = window.clone();
    glib::spawn_future_local(async move {
        let Ok(file) = dialog.save_future(Some(&window)).await else {
            return;
        };
        crate::service::request(
            |proxy| async move { proxy.export_rules().await },
            move |answer| match answer {
                Ok(text) => {
                    glib::spawn_future_local(write(file, text, done));
                }
                Err(error) => done(Outcome::Failed(format!("Cannot export the rules: {error}"))),
            },
        );
    });
}

/// Write the exported `text` to `file`.
async fn write(file: gio::File, text: String, done: impl FnOnce(Outcome)) {
    let written = file
        .replace_contents_future(
            text.into_bytes(),
            None,
            false,
            gio::FileCreateFlags::REPLACE_DESTINATION,
        )
        .await;
    done(match written {
        Ok(_) => Outcome::Exported,
        Err((_, error)) => Outcome::Failed(format!("Cannot write the file: {error}")),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rul_02_outcomes_read_as_on_kde() {
        assert_eq!(Outcome::Imported(1).text(), "Imported 1 rule");
        assert_eq!(Outcome::Imported(3).text(), "Imported 3 rules");
        assert_eq!(Outcome::Exported.text(), "Rules exported");
        assert_eq!(Outcome::Failed("x".into()).text(), "x");
    }
}
