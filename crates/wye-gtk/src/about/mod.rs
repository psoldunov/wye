//! About (17-dialogs.md): DLG-ABT-01, an `AdwAboutDialog` with Wye's name,
//! icon, version, website, issue tracker, licence and credits; DLG-ABT-02,
//! its Troubleshooting page (`debug-info`), what the service detected in
//! this session (`GetTroubleshooting`), with libadwaita's own Copy and Save
//! buttons for bug reports.
//!
//! Opened from the tray's More submenu, so it usually has no window to
//! belong to: presented without a parent, libadwaita shows it in a window of
//! its own. One instance (SET-04): showing it while open raises it. Once
//! closed, the next show builds a new dialog: libadwaita takes a closed
//! dialog's window down with it, and presenting that dialog again would
//! parent it a second time.
//!
//! KDE counterpart: crates/wye-ui/qml/about/, crates/wye-ui/src/about/.

// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
#[path = "../../../wye-ui/src/about/fixture.rs"]
pub mod fixture;
pub mod info;

use std::cell::{Cell, RefCell};

use adw::prelude::*;
use gtk::glib;
use serde_json::Value;
use wye_api::Error;

use crate::app::Presenter;
use crate::service;
use fixture::Fixture;

/// The About surface.
#[derive(Debug, Default)]
pub struct About {
    /// The dialog on screen, if any; it lets go of itself when closed.
    dialog: std::rc::Rc<RefCell<Option<adw::AboutDialog>>>,
    /// A fixture stands in for the service (self-test).
    offline: std::rc::Rc<Cell<bool>>,
}

impl About {
    /// The surface; the dialog is built when first shown.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The dialog on screen, or a new one, and whether it is new.
    fn dialog(&self) -> (adw::AboutDialog, bool) {
        if let Some(dialog) = self.dialog.borrow().as_ref() {
            return (dialog.clone(), false);
        }
        let dialog = build(&self.offline);
        let current = std::rc::Rc::downgrade(&self.dialog);
        dialog.connect_closed(move |closed| {
            // Only itself: a newer dialog may be on screen by now.
            if let Some(current) = current.upgrade()
                && current.borrow().as_ref() == Some(closed)
            {
                current.replace(None);
            }
        });
        self.dialog.replace(Some(dialog.clone()));
        (dialog, true)
    }

    /// Read the version and the troubleshooting text from the service.
    fn refresh(&self, dialog: &adw::AboutDialog) {
        if self.offline.get() {
            return;
        }
        service::request(
            |proxy| async move { proxy.version().await.map_err(Error::from) },
            glib::clone!(
                #[weak]
                dialog,
                move |result: Result<String, Error>| match result {
                    Ok(version) => dialog.set_version(&version),
                    Err(error) => tracing::info!(%error, "the service did not say its version"),
                }
            ),
        );
        service::request(
            |proxy| async move { proxy.get_troubleshooting().await },
            glib::clone!(
                #[weak]
                dialog,
                move |result: Result<String, Error>| match result {
                    Ok(text) => dialog.set_debug_info(&text),
                    Err(error) => {
                        let message = crate::error_text::describe(&error).sentence();
                        dialog.set_debug_info(&message);
                    }
                }
            ),
        );
    }

    fn load_fixture(&self, dialog: &adw::AboutDialog, fixture: &Value) {
        match Fixture::parse(&fixture.to_string()) {
            Ok(fixture) => {
                self.offline.set(true);
                dialog.set_version(&fixture.version);
                dialog.set_debug_info(&fixture.troubleshooting);
            }
            Err(error) => {
                glib::g_critical!("wye-gtk", "about: the fixture is not service data: {error}");
            }
        }
    }
}

impl Presenter for About {
    /// Show About; a self-test case may carry a `fixture`, and `reopen`
    /// to close the dialog on screen first (showing it again after a close
    /// must not warn).
    fn present(&self, _key: &str, argument: &str) {
        let request = serde_json::from_str::<Value>(argument).unwrap_or(Value::Null);
        if request.get("reopen").and_then(Value::as_bool) == Some(true) {
            let open = self.dialog.take();
            if let Some(open) = open {
                open.force_close();
            }
        }
        let (dialog, new) = self.dialog();
        if let Some(fixture) = request.get("fixture") {
            self.load_fixture(&dialog, fixture);
        }
        self.refresh(&dialog);
        if !new {
            // On screen already: raise its window (SET-04).
            if let Some(window) = dialog.root().and_downcast::<gtk::Window>() {
                window.present();
                return;
            }
        }
        dialog.present(None::<&gtk::Widget>);
    }
}

/// The dialog, with the UI host's own version until the service answers.
fn build(offline: &std::rc::Rc<Cell<bool>>) -> adw::AboutDialog {
    let dialog = adw::AboutDialog::builder()
        .application_name("Wye")
        .application_icon(info::APP_ICON)
        .version(info::UI_VERSION)
        .comments(info::DESCRIPTION)
        .developer_name(info::AUTHOR)
        .developers([format!("{} {}", info::AUTHOR, info::AUTHOR_URL)])
        .website(info::HOMEPAGE)
        .issue_url(info::ISSUE_TRACKER)
        .copyright(info::COPYRIGHT)
        .license_type(gtk::License::MitX11)
        .debug_info_filename("wye-troubleshooting.txt")
        .build();
    dialog.add_credit_section(Some("Built With"), &info::CREDITS);
    // BLK-17: the dialog's links open through Wye, like every other link.
    let offline = std::rc::Rc::clone(offline);
    dialog.connect_activate_link(move |_, uri| {
        if offline.get() {
            tracing::info!(%uri, "a link was activated in the self-test");
        } else {
            crate::links::open(uri, |error| {
                tracing::warn!(%error, "the service did not open the link");
            });
        }
        true
    });
    dialog
}
