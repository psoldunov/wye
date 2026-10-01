//! The rule editor's form (RUL-11 to RUL-28): the sheet's body and footer
//! built for a draft, and the controls [`super::RuleEditor`] reads and
//! updates. Split from `editor.rs` so each file stays one concern: this one
//! lays the sheet out, that one reacts to the user.

use std::cell::RefCell;

use adw::prelude::*;
use serde_json::{Value, json};

use super::super::matcher_row::MatcherRow;
use crate::settings::menu::Surface;
use crate::settings::store::SettingsStore;
use crate::widgets::button_row::ButtonRow;
use crate::widgets::callout::Callout;
use crate::widgets::choice_row::ChoiceRow;
use crate::widgets::help::add_help;
use crate::widgets::modifiers::ModifierChooser;
use crate::widgets::section::AddSection;
use crate::widgets::sheet::Sheet;
use crate::widgets::target_row::TargetRow;
use crate::widgets::{group, switch_row};

/// RUL-11, three paragraphs.
const ORDER_CALLOUT: &str = "Rules are matched in order from top to bottom of the list.\n\nThe scheme (<tt>https://</tt>) and <tt>www.</tt> are removed from the URL before matching, so you do not need to include those in the “Match” field.\n\nClick the (?) button for more info.";
/// RUL-24.
const RUN_CHOICES: [(&str, &str); 2] = [
    ("before", "before built-in rules"),
    ("after", "after built-in rules"),
];

/// The sheet's controls.
pub(super) struct Fields {
    pub(super) name: adw::EntryRow,
    pub(super) target: TargetRow,
    pub(super) blocker: adw::PreferencesGroup,
    pub(super) blocker_text: gtk::Label,
    pub(super) matchers: AddSection,
    pub(super) matcher_rows: RefCell<Vec<MatcherRow>>,
    pub(super) sources: AddSection,
    pub(super) background: adw::SwitchRow,
    pub(super) new_window: adw::SwitchRow,
    pub(super) held: ModifierChooser,
    pub(super) held_row: adw::ActionRow,
    pub(super) run: ChoiceRow,
    pub(super) transform: ButtonRow,
    pub(super) delete: Option<adw::ButtonRow>,
    pub(super) help: gtk::Button,
    pub(super) help_arrow: gtk::Image,
    pub(super) test: gtk::Button,
    /// The groups a read-only file makes insensitive (the body still
    /// scrolls).
    pub(super) editable: Vec<adw::PreferencesGroup>,
}

/// The string at `key` of a JSON object, or empty.
pub(super) fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// The strings in the array at `key`.
pub(super) fn strings(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Build the sheet's body and footer, showing `draft`.
pub(super) fn build(sheet: &Sheet, store: &SettingsStore, draft: &Value, editing: bool) -> Fields {
    let page = sheet.page();
    // RUL-11
    page.add(Callout::new(store, "rules-order", "", ORDER_CALLOUT).group());
    // RUL-18
    let (blocker, blocker_text) = blocker_group();
    page.add(&blocker);

    let (card, name, target) = name_card(store, draft);
    page.add(&card);
    let (matchers, sources) = lists(page);
    let advanced = Advanced::build(store, draft);
    page.add(&advanced.group);
    let (delete, delete_group) = if editing {
        let (row, group) = delete_group();
        page.add(&group);
        (Some(row), Some(group))
    } else {
        (None, None)
    };

    let (help, help_arrow, test) = footer(sheet);
    let mut editable = vec![
        card,
        matchers.group().clone(),
        sources.group().clone(),
        advanced.group.clone(),
    ];
    editable.extend(delete_group);
    Fields {
        name,
        target,
        blocker,
        blocker_text,
        matchers,
        matcher_rows: RefCell::default(),
        sources,
        background: advanced.background,
        new_window: advanced.new_window,
        held: advanced.held,
        held_row: advanced.held_row,
        run: advanced.run,
        transform: advanced.transform,
        delete,
        help,
        help_arrow,
        test,
        editable,
    }
}

/// RUL-12: Name and Open in, showing `draft`.
fn name_card(
    store: &SettingsStore,
    draft: &Value,
) -> (adw::PreferencesGroup, adw::EntryRow, TargetRow) {
    let card = group::group("");
    let name = adw::EntryRow::builder().title("Name").build();
    name.set_text(&text(draft, "name"));
    card.add(&name);
    let target = TargetRow::new("Open in", "", Surface::Rule);
    target.show(
        store,
        draft.get("target").unwrap_or(&json!({"default": true})),
        "",
    );
    card.add(target.row());
    (card, name, target)
}

/// RUL-13, RUL-16: the URL Matchers and Source Apps sections on `page`.
fn lists(page: &adw::PreferencesPage) -> (AddSection, AddSection) {
    let matchers = AddSection::new(
        "URL Matchers",
        "The link must match one of these patterns",
        "No Matchers",
    );
    matchers.add_button().set_tooltip_text(Some("Add Matcher"));
    page.add(matchers.group());
    let sources = AddSection::new(
        "Source Apps",
        "The link must have been clicked in one of these apps",
        "No Source Apps",
    );
    sources
        .add_button()
        .set_tooltip_text(Some("Add Source App"));
    page.add(sources.group());
    (matchers, sources)
}

/// RUL-28: Delete Rule, in a card of its own, when editing a rule.
fn delete_group() -> (adw::ButtonRow, adw::PreferencesGroup) {
    let row = adw::ButtonRow::builder()
        .title("Delete Rule")
        .start_icon_name("user-trash-symbolic")
        .build();
    row.add_css_class("destructive-action");
    let group = group::group("");
    group.add(&row);
    (row, group)
}

/// RUL-18: the line that says why Save is disabled.
fn blocker_group() -> (adw::PreferencesGroup, gtk::Label) {
    let text = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .hexpand(true)
        .build();
    let glyph = gtk::Image::builder()
        .icon_name("dialog-warning-symbolic")
        .valign(gtk::Align::Start)
        .build();
    let line = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .css_classes(["wye-rule-blocker"])
        .build();
    line.append(&glyph);
    line.append(&text);
    let group = group::group("");
    group.add(&line);
    group.set_visible(false);
    (group, text)
}

/// RUL-21: the Advanced card's rows.
struct Advanced {
    group: adw::PreferencesGroup,
    background: adw::SwitchRow,
    new_window: adw::SwitchRow,
    held: ModifierChooser,
    held_row: adw::ActionRow,
    run: ChoiceRow,
    transform: ButtonRow,
}

impl Advanced {
    /// The rows in RUL-21's order, showing `draft`.
    fn build(store: &SettingsStore, draft: &Value) -> Self {
        let flag = |key: &str| draft.get(key).and_then(Value::as_bool).unwrap_or(false);
        let group = group::group("Advanced");
        // RUL-22
        let background = switch_row::switch_row("Open in background", "");
        background.set_active(flag("open-in-background"));
        add_help(store, &background, "open-in-background");
        group.add(&background);
        // RUL-23
        let new_window = switch_row::switch_row("Force new window", "");
        new_window.set_active(flag("force-new-window"));
        add_help(store, &new_window, "force-new-window");
        group.add(&new_window);
        // RUL-27
        let held = ModifierChooser::new();
        held.set_pressed(&strings(draft, "held-keys"));
        let held_row = held.add_row("Only when keys are held", "");
        group.add(&held_row);
        // RUL-24
        let run = ChoiceRow::new("Run", "", &RUN_CHOICES);
        run.set_value(draft.get("run").and_then(Value::as_str).unwrap_or("before"));
        group.add(run.row());
        // RUL-25
        let transform = ButtonRow::new("Transform URL", "", "Edit Script…").add_switch();
        if let Some(switch) = transform.switch() {
            switch.set_active(flag("transform"));
            switch.update_property(&[gtk::accessible::Property::Label("Transform URL")]);
        }
        group.add(transform.row());
        Self {
            group,
            background,
            new_window,
            held,
            held_row,
            run,
            transform,
        }
    }
}

/// RUL-19: the bar under the note: the help button, the arrow pointing at
/// it until help was opened once, and Test….
fn footer(sheet: &Sheet) -> (gtk::Button, gtk::Image, gtk::Button) {
    let help = gtk::Button::builder()
        .css_classes(["flat", "circular", "wye-help-button"])
        .icon_name("wye-help-symbolic")
        .tooltip_text("How Rules Work")
        .valign(gtk::Align::Center)
        .build();
    help.update_property(&[gtk::accessible::Property::Label("Help: How rules work")]);
    let arrow = gtk::Image::builder()
        .icon_name("go-previous-symbolic")
        .css_classes(["warning", "wye-help-arrow"])
        .build();
    // DLG-TST: the play mark, as the Rules page's Test Rules… and KDE's.
    let test_content = adw::ButtonContent::builder()
        .icon_name("media-playback-start-symbolic")
        .label("Test…")
        .build();
    let test = gtk::Button::builder()
        .child(&test_content)
        .tooltip_text("Open the rule tester with a link this rule's first URL matcher matches")
        .valign(gtk::Align::Center)
        .build();
    test.update_property(&[gtk::accessible::Property::Label("Test…")]);
    let bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .css_classes(["toolbar", "wye-sheet-footer"])
        .build();
    bar.append(&help);
    bar.append(&arrow);
    let spacer = gtk::Box::builder().hexpand(true).build();
    bar.append(&spacer);
    bar.append(&test);
    sheet.add_footer(&bar);
    (help, arrow, test)
}
