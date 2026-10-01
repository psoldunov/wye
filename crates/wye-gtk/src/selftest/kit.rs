//! The widget kit's gallery (`--self-test kit`): one window with every
//! building block (BLK-01 to BLK-18, `crate::widgets`) on a store loaded
//! from `fixtures/kit.json`, so each block renders under the self-test's
//! warning gate and in snapshots before a page uses it. Page builders can
//! look here for how a block reads in light and dark. No window name
//! routes here; only the self-test shows it.

use std::cell::OnceCell;

use adw::prelude::*;
use gtk::{gio, glib};
use serde_json::{Value, json};

use crate::app::Presenter;
use crate::settings::menu::Surface;
use crate::settings::store::SettingsStore;
use crate::widgets::button_row::ButtonRow;
use crate::widgets::callout::Callout;
use crate::widgets::checklist::{Checklist, ChecklistItem};
use crate::widgets::choice_row::ChoiceRow;
use crate::widgets::entry_row::EntryRow;
use crate::widgets::list_toolbar::ListCard;
use crate::widgets::modifiers::ModifierChooser;
use crate::widgets::radio_row::RadioRow;
use crate::widgets::row::{self, Tint};
use crate::widgets::section::AddSection;
use crate::widgets::sheet::Sheet;
use crate::widgets::shortcut::{ShortcutChips, ShortcutRecorder};
use crate::widgets::target_row::TargetRow;
use crate::widgets::{empty_state, group, help, links, switch_row};

/// The gallery surface.
#[derive(Debug)]
pub struct Kit {
    app: adw::Application,
    store: SettingsStore,
    built: OnceCell<Gallery>,
}

#[derive(Debug)]
struct Gallery {
    window: adw::ApplicationWindow,
    primary: TargetRow,
}

impl Kit {
    /// The gallery for `app`.
    #[must_use]
    pub fn new(app: &adw::Application) -> Self {
        Self {
            app: app.clone(),
            store: SettingsStore::new(),
            built: OnceCell::new(),
        }
    }
}

impl Presenter for Kit {
    fn present(&self, _key: &str, argument: &str) {
        let request: Value = serde_json::from_str(argument).unwrap_or(Value::Null);
        if let Some(fixture) = request.get("fixture")
            && let Err(error) = self.store.load_fixture(fixture)
        {
            glib::g_critical!("wye-gtk", "kit: the fixture is not service data: {error}");
        }
        let gallery = self.built.get_or_init(|| build(&self.app, &self.store));
        gallery.window.present();
        // Each case shows only what it asks for: a sheet an earlier case
        // opened would cover this one's.
        if let Some(dialog) = gallery.window.visible_dialog() {
            dialog.force_close();
        }
        match request.get("sheet").and_then(Value::as_str) {
            Some("sheet") => sheet(&gallery.window),
            Some("target-menu") => {
                WidgetExt::activate(gallery.primary.row());
            }
            Some(other) => glib::g_warning!("wye-gtk", "kit: no sheet {other:?}"),
            None => {}
        }
    }
}

fn build(app: &adw::Application, store: &SettingsStore) -> Gallery {
    let page = adw::PreferencesPage::new();
    let primary = targets(&page, store);
    rows(&page, store);
    choices(&page, store);
    lists(&page);
    let callout = Callout::new(
        store,
        "kit-callout",
        "Please Read",
        "A callout with a title, <b>bold</b> text, <tt>inline code</tt> and <a href=\"https://example.com\">a link</a>.",
    );
    page.add(callout.group());
    let status = empty_state::empty_state(
        "wye-rules-symbolic",
        "No Rules",
        "Rules send links to the browser you choose.",
    );
    let _ = empty_state::with_action(&status, "Add Rule…");
    let empty = group::group("Empty State");
    empty.add(&status);
    page.add(&empty);

    let view = adw::ToolbarView::builder().content(&page).build();
    view.add_top_bar(&adw::HeaderBar::new());
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Widget Kit")
        .default_width(720)
        .default_height(2300)
        .content(&view)
        .build();
    Gallery { window, primary }
}

/// BLK-04.
fn targets(page: &adw::PreferencesPage, store: &SettingsStore) -> TargetRow {
    let section = group::group("Target Popup Rows");
    let primary = TargetRow::new("Primary browser", "", Surface::Browsers);
    primary.bind(store, "browsers.primary", &json!({"picker": true}));
    primary.connect_other(|| tracing::info!("kit: Other…"));
    section.add(primary.row());
    let service = TargetRow::new("Discord", "discord.com", Surface::Apps);
    service.bind_service(store, "discord");
    section.add(service.row());
    let missing = TargetRow::new(
        "Linear",
        "A target whose app is gone (APP-10)",
        Surface::Apps,
    );
    missing.bind_service(store, "linear");
    section.add(missing.row());
    let draft = TargetRow::new("Open with", "A rule editor's draft (RUL-12)", Surface::Rule);
    draft.show(store, &json!({"private": "firefox.desktop"}), "");
    draft.connect_chosen(|target| tracing::info!(%target, "kit: draft target"));
    section.add(draft.row());
    page.add(&section);
    primary
}

/// BLK-02, BLK-03, BLK-05, BLK-08, BLK-10, BLK-17.
fn rows(page: &adw::PreferencesPage, store: &SettingsStore) {
    let section = group::group_with_description(
        "Rows",
        "A group description with <a href=\"https://example.com\">a link</a>.",
    );
    let status = ButtonRow::new("Wye is your default browser", "", "Stop Being Default");
    row::add_leading_icon(status.row()).set("object-select-symbolic", Tint::Success);
    section.add(status.row());
    let configure =
        ButtonRow::new("Remove tracking", "Strips utm_* and friends", "Configure…").add_switch();
    configure.bind_switch(store, "extras.strip-tracking-on-open", false, true);
    help::add_help(store, configure.row(), "remove-tracking");
    section.add(configure.row());
    let switch = switch_row::switch_row("Force new window", "Not available for the Picker");
    help::add_help(store, &switch, "force-new-window");
    row::set_disabled(&switch, true);
    section.add(&switch);
    let link = row::action_row(
        "Inline links",
        "Links in a subtitle open <a href=\"https://example.com\">through Wye</a>.",
    );
    section.add(&link);
    links::route_links(
        &section,
        glib::clone!(
            #[weak]
            store,
            move |url| store.open_link(url)
        ),
    );
    page.add(&section);
}

/// BLK-06, BLK-07, BLK-16, BLK-18 and the choice row.
fn choices(page: &adw::PreferencesPage, store: &SettingsStore) {
    let section = group::group("Choices");
    let size = RadioRow::new(
        "Icon size",
        "",
        &[("small", "Small"), ("medium", "Medium"), ("large", "Large")],
    );
    size.bind(store, "picker.icon-size", "medium");
    section.add(size.row());
    let tray = ChoiceRow::new(
        "Tray icon",
        "",
        &[("primary-browser", "Primary Browser"), ("wye", "Wye")],
    );
    tray.bind(store, "general.tray-icon", "primary-browser");
    section.add(tray.row());
    let modifiers = ModifierChooser::new();
    let alternative = modifiers.add_row("Alternative browser key", "");
    modifiers.bind(store, "browsers.alternative-key");
    section.add(&alternative);
    let name = EntryRow::new("Name");
    name.set_validator(name_problem);
    section.add(name.row());
    let recorder = ShortcutRecorder::new("Record Shortcut");
    recorder.set_binding(Some("Ctrl+Alt+o"));
    recorder.connect_recorded(|recorded| tracing::info!(?recorded, "kit: recorded"));
    let toggle = row::action_row("Toggle menu", "");
    toggle.add_suffix(recorder.widget());
    section.add(&toggle);
    let unset = ShortcutRecorder::new("Record Shortcut");
    let open = row::action_row("Open URL from clipboard", "");
    open.add_suffix(unset.widget());
    section.add(&open);
    let chips = ShortcutChips::new();
    chips.set_bindings(&[
        "Return".to_owned(),
        "KP_Enter".to_owned(),
        "space".to_owned(),
    ]);
    chips.connect_added(|binding| tracing::info!(%binding, "kit: key added"));
    chips.connect_removed(|index| tracing::info!(index, "kit: key removed"));
    let keys = row::action_row("Open", "");
    keys.add_suffix(chips.widget());
    section.add(&keys);
    page.add(&section);
}

/// BLK-13, BLK-14, BLK-15.
fn lists(page: &adw::PreferencesPage) {
    let matchers = AddSection::new("Matchers", "Every matcher must match.", "No Matchers");
    matchers.add_button().connect_clicked(glib::clone!(
        #[strong]
        matchers,
        move |_| add_matcher(&matchers)
    ));
    page.add(matchers.group());

    let card = ListCard::new();
    card.set_placeholder(&gtk::Label::new(Some("No Rules")));
    for name in ["Work links", "Meetings"] {
        card.list().append(&row::action_row(name, "example.com"));
    }
    let _ = card.add_button("Add Rule…", "list-add-symbolic");
    let menu = gio::Menu::new();
    menu.append(Some("Import…"), Some("app.import"));
    menu.append(Some("Export…"), Some("app.export"));
    card.set_menu(&menu);
    let rules = group::group("List Toolbar");
    rules.add(card.widget());
    page.add(&rules);

    let checklist = Checklist::new();
    let hotkey = gtk::DropDown::from_strings(&["F", "C", "None"]);
    checklist.set_items(&[
        ChecklistItem {
            key: "firefox".to_owned(),
            title: "Firefox".to_owned(),
            subtitle: String::new(),
            icon: "firefox".to_owned(),
            badge: None,
            checked: true,
            extra: Some(hotkey.upcast()),
        },
        ChecklistItem {
            key: "work".to_owned(),
            title: "Work (Google Chrome)".to_owned(),
            subtitle: String::new(),
            icon: "google-chrome".to_owned(),
            badge: Some(json!({"initial": "W", "color": "#336699"})),
            checked: true,
            extra: None,
        },
        ChecklistItem {
            key: "konsole".to_owned(),
            title: "Konsole".to_owned(),
            subtitle: "Added with +".to_owned(),
            icon: "utilities-terminal".to_owned(),
            badge: None,
            checked: false,
            extra: None,
        },
    ]);
    checklist.connect_toggled(|key, on| tracing::info!(%key, on, "kit: toggled"));
    checklist.connect_moved(|from, to| tracing::info!(from, to, "kit: moved"));
    let shown = group::group("Reorderable Checklist");
    shown.add(checklist.widget());
    page.add(&shown);
}

/// BLK-14: a matcher row whose remove button takes it out again.
fn add_matcher(matchers: &AddSection) {
    let matcher = row::action_row("Domain", "example.com");
    let remove = gtk::Button::builder()
        .icon_name("user-trash-symbolic")
        .tooltip_text("Remove")
        .valign(gtk::Align::Center)
        .build();
    remove.add_css_class("flat");
    remove.connect_clicked(glib::clone!(
        #[strong]
        matchers,
        #[weak]
        matcher,
        move |_| matchers.remove(&matcher)
    ));
    matcher.add_suffix(&remove);
    matchers.add(&matcher);
}

/// BLK-11 over the gallery: Save is offered once the name is valid.
fn sheet(window: &adw::ApplicationWindow) {
    let sheet = Sheet::new("New Rule", "Save");
    let fields = group::group("");
    let name = EntryRow::new("Name");
    name.set_validator(name_problem);
    fields.add(name.row());
    sheet.page().add(&fields);
    sheet.set_note("Rules apply from top to bottom.");
    sheet.set_content_size(520, 420);
    sheet.set_valid(name.is_valid());
    // Weak both ways: the row and the button live in the sheet.
    name.row().connect_changed(glib::clone!(
        #[weak(rename_to = primary)]
        sheet.primary(),
        move |row| primary.set_sensitive(name_problem(&row.text()).is_none())
    ));
    sheet.connect_primary(glib::clone!(
        #[weak(rename_to = row)]
        name.row(),
        #[upgrade_or]
        false,
        move || name_problem(&row.text()).is_none()
    ));
    sheet.present(window);
}

/// The gallery's check for a name field (BLK-07).
fn name_problem(text: &str) -> Option<String> {
    text.trim()
        .is_empty()
        .then(|| "A name is needed".to_owned())
}
