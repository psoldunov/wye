//! BLK-11 Sheet: a modal `AdwDialog` over the Settings window (the window
//! behind is dimmed and inert), with its own header bar: **Cancel** at the
//! start, the primary button at the end in the accent colour
//! (`suggested-action`), insensitive until the content is valid. The body
//! scrolls (an `AdwPreferencesPage`); an optional note is pinned under it.
//! Escape cancels.
//!
//! This is the GNOME arrangement of the spec's "pinned footer with the
//! buttons": libadwaita dialogs put Cancel and the primary action in the
//! header bar.
//!
//! API:
//! - [`Sheet::new`]`(title, primary)`: the dialog. Add groups to
//!   [`Sheet::page`].
//! - [`Sheet::set_valid`]: enable the primary button.
//! - [`Sheet::set_note`]: the pinned note (markup); empty hides it.
//! - [`Sheet::add_footer`]: a bar pinned under the note.
//! - [`Sheet::connect_primary`]: what the primary button does; the sheet
//!   closes after it unless the callback returns `false` (keep it open to
//!   show an error).
//! - [`Sheet::present`]`(parent)`: show it over `parent`'s window.
//! - [`Sheet::set_content_size`]: the dialog's size. Every Settings sheet
//!   keeps the one default (640 × 680 logical pixels), so they open alike,
//!   as KDE's do; only the stacked choosers are smaller.

use adw::prelude::*;
use gtk::glib;

/// Width and height of every Settings sheet, in logical pixels: room for
/// three key chips beside an action's name (KEY-20), and nearly the
/// Settings window's height.
const DEFAULT_SIZE: (i32, i32) = (640, 680);

/// A sheet. Clones share the dialog.
#[derive(Debug, Clone)]
pub struct Sheet {
    dialog: adw::Dialog,
    page: adw::PreferencesPage,
    primary: gtk::Button,
    cancel: gtk::Button,
    note: gtk::Label,
}

impl Sheet {
    /// A sheet titled `title` whose primary button reads `primary`.
    #[must_use]
    pub fn new(title: &str, primary: &str) -> Self {
        let cancel = gtk::Button::builder().label("Cancel").build();
        let primary = gtk::Button::builder()
            .css_classes(["suggested-action"])
            .label(primary)
            .sensitive(false)
            .build();
        let header = adw::HeaderBar::builder()
            .show_start_title_buttons(false)
            .show_end_title_buttons(false)
            .build();
        header.pack_start(&cancel);
        header.pack_end(&primary);
        let page = adw::PreferencesPage::new();
        let note = gtk::Label::builder()
            .use_markup(true)
            .wrap(true)
            .xalign(0.0)
            .visible(false)
            .css_classes(["dimmed", "caption", "wye-sheet-note"])
            .build();
        let view = adw::ToolbarView::builder().content(&page).build();
        view.add_top_bar(&header);
        view.add_bottom_bar(&note);
        let dialog = adw::Dialog::builder()
            .title(title)
            .child(&view)
            .content_width(DEFAULT_SIZE.0)
            .content_height(DEFAULT_SIZE.1)
            .default_widget(&primary)
            .build();
        cancel.connect_clicked(glib::clone!(
            #[weak]
            dialog,
            move |_| {
                dialog.close();
            }
        ));
        Self {
            dialog,
            page,
            primary,
            cancel,
            note,
        }
    }

    /// The dialog.
    #[must_use]
    pub fn dialog(&self) -> &adw::Dialog {
        &self.dialog
    }

    /// The scrolling body: add `AdwPreferencesGroup`s to it.
    #[must_use]
    pub fn page(&self) -> &adw::PreferencesPage {
        &self.page
    }

    /// The primary button.
    #[must_use]
    pub fn primary(&self) -> &gtk::Button {
        &self.primary
    }

    /// The Cancel button; hide it on a sheet whose changes apply at once.
    #[must_use]
    pub fn cancel(&self) -> &gtk::Button {
        &self.cancel
    }

    /// Whether the content is valid: the primary button works only then.
    pub fn set_valid(&self, valid: bool) {
        self.primary.set_sensitive(valid);
    }

    /// The note pinned under the body; empty hides it.
    pub fn set_note(&self, markup: &str) {
        self.note.set_markup(markup);
        self.note.set_visible(!markup.is_empty());
    }

    /// Pin `bar` under the body and the note: controls that belong to the
    /// sheet rather than to its content (the rule editor's help and Test…,
    /// the rule tester's Skip network).
    pub fn add_footer(&self, bar: &impl IsA<gtk::Widget>) {
        if let Some(view) = self.dialog.child().and_downcast::<adw::ToolbarView>() {
            view.add_bottom_bar(bar);
        }
    }

    /// The dialog's size in logical pixels.
    pub fn set_content_size(&self, width: i32, height: i32) {
        self.dialog.set_content_width(width);
        self.dialog.set_content_height(height);
    }

    /// Run `activated` when the primary button is pressed; the sheet closes
    /// when it returns `true`.
    pub fn connect_primary(&self, activated: impl Fn() -> bool + 'static) {
        self.primary.connect_clicked(glib::clone!(
            #[weak(rename_to = dialog)]
            self.dialog,
            move |_| {
                if activated() {
                    dialog.close();
                }
            }
        ));
    }

    /// Show the sheet over the window `parent` belongs to.
    pub fn present(&self, parent: &impl IsA<gtk::Widget>) {
        self.dialog.present(Some(parent));
    }

    /// Close the sheet.
    pub fn close(&self) {
        self.dialog.close();
    }
}
