//! The History window's frame (DLG-HIS-01, DLG-HIS-04): an
//! `AdwApplicationWindow` of about 560 × 480, resizable, with a header bar
//! holding the search toggle and a menu (**Clear History…**), a search bar
//! under it, an error banner, and a stack of what the window can show: the
//! list, "No History", "History Is Off" (with **Turn On**), "No Matches", or
//! a spinner until the first answer.
//!
//! Escape leaves the search first, then closes; Ctrl+W closes; Ctrl+F
//! toggles the search; typing anywhere starts it (SET-07).
//!
//! KDE counterpart: crates/wye-ui/qml/history/HistoryWindow.qml.

use adw::prelude::*;
use gtk::{gio, glib};

use crate::widgets::empty_state;

/// DLG-HIS-01: the default size.
const DEFAULT_SIZE: (i32, i32) = (560, 480);

/// The smallest size that still shows a row's link and details.
const MINIMUM_SIZE: (i32, i32) = (360, 320);

/// The pages of the window's stack.
pub mod page {
    pub const LOADING: &str = "loading";
    pub const LIST: &str = "list";
    pub const EMPTY: &str = "empty";
    pub const OFF: &str = "off";
    pub const NO_MATCHES: &str = "no-matches";
}

/// The window's widgets; the controller (`super`) fills them.
#[derive(Debug, Clone)]
pub struct Frame {
    pub window: adw::ApplicationWindow,
    pub search_button: gtk::ToggleButton,
    pub menu_button: gtk::MenuButton,
    pub search_bar: gtk::SearchBar,
    pub search_entry: gtk::SearchEntry,
    pub banner: adw::Banner,
    pub stack: gtk::Stack,
    /// The days, one group each.
    pub list: adw::PreferencesPage,
    pub turn_on: gtk::Button,
    pub no_matches: adw::StatusPage,
    /// `win.clear-history`, the menu's **Clear History…**.
    pub clear: gio::SimpleAction,
}

impl Frame {
    /// Build the window for `app`, hidden.
    pub fn new(app: &adw::Application) -> Self {
        let search_button = gtk::ToggleButton::builder()
            .icon_name("system-search-symbolic")
            .tooltip_text("Search (Ctrl+F)")
            .build();
        search_button.update_property(&[gtk::accessible::Property::Label("Search the history")]);
        let menu_button = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&main_menu())
            .tooltip_text("Main Menu")
            .primary(true)
            .build();
        let header = adw::HeaderBar::new();
        header.pack_end(&menu_button);
        header.pack_end(&search_button);

        let search_entry = gtk::SearchEntry::builder()
            .placeholder_text("Search links, apps and browsers")
            .hexpand(true)
            .build();
        search_entry.update_property(&[gtk::accessible::Property::Label("Search the history")]);
        let clamp = adw::Clamp::builder()
            .maximum_size(520)
            .child(&search_entry)
            .build();
        let search_bar = gtk::SearchBar::builder().child(&clamp).build();
        search_bar.connect_entry(&search_entry);
        search_button
            .bind_property("active", &search_bar, "search-mode-enabled")
            .bidirectional()
            .sync_create()
            .build();

        let banner = adw::Banner::builder().button_label("Dismiss").build();
        banner.connect_button_clicked(|banner| banner.set_revealed(false));

        let list = adw::PreferencesPage::new();
        list.add_css_class("wye-history-list");
        let off = empty_state::empty_state(
            "document-open-recent-symbolic",
            "History Is Off",
            "Wye does not keep the links you open.",
        );
        let turn_on = empty_state::with_action(&off, "Turn On");
        let empty = empty_state::empty_state(
            "document-open-recent-symbolic",
            "No History",
            "Links you open appear here.",
        );
        let no_matches = empty_state::empty_state("edit-find-symbolic", "No Matches", "");
        let spinner = adw::Spinner::builder()
            .width_request(32)
            .height_request(32)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();
        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .vexpand(true)
            .build();
        stack.add_named(&spinner, Some(page::LOADING));
        stack.add_named(&list, Some(page::LIST));
        stack.add_named(&empty, Some(page::EMPTY));
        stack.add_named(&off, Some(page::OFF));
        stack.add_named(&no_matches, Some(page::NO_MATCHES));

        let view = adw::ToolbarView::builder().content(&stack).build();
        view.add_top_bar(&header);
        view.add_top_bar(&search_bar);
        view.add_top_bar(&banner);
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("History")
            .default_width(DEFAULT_SIZE.0)
            .default_height(DEFAULT_SIZE.1)
            .width_request(MINIMUM_SIZE.0)
            .height_request(MINIMUM_SIZE.1)
            .hide_on_close(true)
            .content(&view)
            .build();
        window.add_css_class("wye-history");
        let clear = gio::SimpleAction::new("clear-history", None);
        window.add_action(&clear);
        let frame = Self {
            window,
            search_button,
            menu_button,
            search_bar,
            search_entry,
            banner,
            stack,
            list,
            turn_on,
            no_matches,
            clear,
        };
        frame.add_shortcuts();
        frame
    }

    /// Escape (leave the search, then close), Ctrl+W, Ctrl+F.
    fn add_shortcuts(&self) {
        let controller = gtk::ShortcutController::new();
        let escape = gtk::CallbackAction::new(glib::clone!(
            #[weak(rename_to = bar)]
            self.search_bar,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |widget, _| {
                if bar.is_search_mode() {
                    bar.set_search_mode(false);
                } else if let Some(window) = widget.downcast_ref::<gtk::Window>() {
                    window.close();
                }
                glib::Propagation::Stop
            }
        ));
        let close = gtk::CallbackAction::new(|widget, _| {
            if let Some(window) = widget.downcast_ref::<gtk::Window>() {
                window.close();
            }
            glib::Propagation::Stop
        });
        let search = gtk::CallbackAction::new(glib::clone!(
            #[weak(rename_to = button)]
            self.search_button,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, _| {
                if button.is_visible() {
                    button.set_active(!button.is_active());
                }
                glib::Propagation::Stop
            }
        ));
        for (trigger, action) in [
            ("Escape", escape),
            ("<Control>w", close),
            ("<Control>f", search),
        ] {
            controller.add_shortcut(gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string(trigger),
                Some(action),
            ));
        }
        self.window.add_controller(controller);
    }

    /// Show the page called `name` of [`page`].
    pub fn show_page(&self, name: &str) {
        self.stack.set_visible_child_name(name);
    }

    /// Searching and clearing need something to search or clear: without
    /// history the search button and the menu go (the page says why).
    pub fn set_has_history(&self, has_history: bool) {
        self.search_button.set_visible(has_history);
        self.menu_button.set_visible(has_history);
        self.clear.set_enabled(has_history);
        if has_history {
            self.search_bar.set_key_capture_widget(Some(&self.window));
        } else {
            self.search_bar.set_search_mode(false);
            self.search_bar.set_key_capture_widget(None::<&gtk::Widget>);
        }
    }

    /// Show `sentence` in the error banner; empty hides it.
    pub fn set_error(&self, sentence: &str) {
        self.banner.set_title(sentence);
        self.banner.set_revealed(!sentence.is_empty());
    }
}

/// The header bar's menu.
fn main_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some("_Clear History…"), Some("win.clear-history"));
    menu
}

/// DLG-HIS-01: Clear History asks first.
pub fn confirm_clear() -> adw::AlertDialog {
    let dialog = adw::AlertDialog::builder()
        .heading("Clear History?")
        .body("Wye forgets every link in the history. This cannot be undone.")
        .close_response("cancel")
        .default_response("cancel")
        .build();
    dialog.add_responses(&[("cancel", "_Cancel"), ("clear", "_Clear History")]);
    dialog.set_response_appearance("clear", adw::ResponseAppearance::Destructive);
    dialog
}
