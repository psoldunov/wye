//! The rule editor (RUL-10 to RUL-28): a sheet over the Settings window,
//! "New Rule" or "Edit Rule". A new rule starts from [`draft::new_draft`]
//! (PICK-31 prefills a Domain matcher and the source app); an existing one
//! from [`draft::draft_for`]. **Save** stays insensitive until
//! [`draft::check`] says the rule is valid (RUL-18), and a line above the
//! fields says why once the user has changed something.
//!
//! Top to bottom: the order callout (RUL-11); Name and Open in (RUL-12);
//! URL Matchers (RUL-13, RUL-14); Source Apps (RUL-16); Advanced (RUL-21 to
//! RUL-27); Delete Rule when editing (RUL-28). Pinned under the body: the
//! match-logic note (RUL-17) and a bar with the help button, its first-use
//! arrow and **Test…** (RUL-19). Cancel and Save are in the header bar
//! (BLK-11).
//!
//! KDE counterpart: crates/wye-ui/qml/rules/RuleEditorSheet.qml.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::glib;
use serde_json::{Value, json};
use wye_core::Rule;

mod form;

use form::{Fields, build, strings, text};

use super::matcher_row::MatcherRow;
use super::{Apps, draft, help, list, script};
use crate::settings::sheets::app_chooser::{AppChooser, Choice};
use crate::settings::store::SettingsStore;
use crate::widgets::sheet::Sheet;
use crate::widgets::{entry_row, icon, row};

/// Source app icons.
const ICON_SIZE: i32 = 32;
/// RUL-17.
const MATCH_NOTE: &str = "If you specify both types, at least one of the URL matchers <b>AND</b> one of the source apps must match.";
/// RUL-27.
const ALTERNATIVE_KEY_WINS: &str =
    "These are the alternative-browser keys, which take precedence over rules.";

/// What the editor asks of the page.
pub struct Callbacks {
    /// Save: the rule's position, `None` for a new rule (RUL-20).
    pub save: Box<dyn Fn(Option<usize>, Rule)>,
    /// Delete Rule (RUL-28).
    pub delete: Box<dyn Fn(usize)>,
    /// Test…: open the tester with this link (RUL-19).
    pub test: Box<dyn Fn(String)>,
}

struct Inner {
    sheet: Sheet,
    store: SettingsStore,
    apps: Apps,
    index: Option<usize>,
    draft: RefCell<Value>,
    /// The draft as the sheet opened, to tell whether it has changes.
    initial: Value,
    /// The user changed something since the sheet opened (RUL-18). An
    /// existing rule counts as changed: its problems show at once.
    touched: Cell<bool>,
    fields: Fields,
    callbacks: Callbacks,
}

/// An open rule editor. Clones share it.
#[derive(Clone)]
pub struct RuleEditor {
    inner: Rc<Inner>,
}

impl std::fmt::Debug for RuleEditor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuleEditor")
            .field("index", &self.inner.index)
            .finish_non_exhaustive()
    }
}

/// Run `act` on the editor `weak` points to, if it is still open.
fn with(weak: &Weak<Inner>, act: impl FnOnce(&RuleEditor)) {
    if let Some(inner) = weak.upgrade() {
        act(&RuleEditor { inner });
    }
}

impl RuleEditor {
    /// Show `draft` over `window`: rule `index`, or a new rule for `None`.
    pub fn open(
        window: &adw::ApplicationWindow,
        store: &SettingsStore,
        apps: &Apps,
        index: Option<usize>,
        draft: Value,
        callbacks: Callbacks,
    ) -> Self {
        let title = if index.is_some() {
            "Edit Rule"
        } else {
            "New Rule"
        };
        let sheet = Sheet::new(title, "Save");
        sheet.set_note(MATCH_NOTE);
        let fields = build(&sheet, store, &draft, index.is_some());
        let inner = Rc::new(Inner {
            sheet,
            store: store.clone(),
            apps: apps.clone(),
            index,
            initial: draft.clone(),
            draft: RefCell::new(draft),
            touched: Cell::new(index.is_some()),
            fields,
            callbacks,
        });
        let this = Self { inner };
        this.connect();
        this.connect_advanced();
        this.show_matchers(None);
        this.show_sources();
        this.refresh();
        let inner = &this.inner;
        inner.sheet.present(window);
        // As on KDE: the caret at the end of the name, nothing selected.
        entry_row::focus_when_shown(inner.sheet.dialog(), &inner.fields.name, -1);
        this
    }

    /// The sheet's dialog, so the page can tell whether it is open.
    #[must_use]
    pub fn dialog(&self) -> &adw::Dialog {
        self.inner.sheet.dialog()
    }

    /// Whether the draft differs from the rule as the sheet opened: closing
    /// it then loses work, so the page asks first (RUL-20).
    #[must_use]
    pub fn has_changes(&self) -> bool {
        *self.inner.draft.borrow() != self.inner.initial
    }

    /// Open the rules help (RUL-19), as the "?" does; the arrow pointing at
    /// it goes for good.
    pub fn show_help(&self) {
        help::open(self.inner.sheet.dialog());
        if !self.help_seen() {
            self.inner
                .store
                .update_ui_state(&json!({"helpArrowSeen": true}));
        }
    }

    /// Scroll to the end of the body (the Advanced card), as focusing its
    /// last control does.
    pub fn reveal_end(&self) {
        let fields = &self.inner.fields;
        let last: gtk::Widget = match &fields.delete {
            Some(delete) => delete.clone().upcast(),
            None => fields.transform.button().clone().upcast(),
        };
        // Once the sheet is on screen: before, it takes no focus.
        let dialog = self.inner.sheet.dialog().downgrade();
        glib::idle_add_local_once(move || {
            if let Some(dialog) = dialog.upgrade() {
                dialog.set_focus(Some(&last));
            }
        });
    }

    fn help_seen(&self) -> bool {
        self.inner
            .store
            .with_snapshot(|snapshot| snapshot.status.ui_state.help_arrow_seen)
    }

    /// Set the draft's `key` to `value`, as the user's change.
    fn edit(&self, key: &str, value: Value) {
        if let Some(object) = self.inner.draft.borrow_mut().as_object_mut() {
            object.insert(key.to_owned(), value);
        }
        self.inner.touched.set(true);
        self.refresh();
    }

    fn text(&self, key: &str) -> String {
        text(&self.inner.draft.borrow(), key)
    }

    /// Show what the draft's check says (RUL-18, RUL-23, RUL-27).
    fn refresh(&self) {
        let inner = &self.inner;
        let fields = &inner.fields;
        let draft = inner.draft.borrow().clone();
        let check = draft::check(&draft, super::alternative_key(&inner.store));
        let writable = inner.store.writable();
        inner.sheet.set_valid(writable && check.valid);
        let blocker = if writable && inner.touched.get() && !check.valid {
            blocker(&check, &draft)
        } else {
            String::new()
        };
        fields.blocker_text.set_label(&blocker);
        fields.blocker.set_visible(!blocker.is_empty());
        for (index, row) in fields.matcher_rows.borrow().iter().enumerate() {
            row.set_error(check.matcher_errors.get(index).map_or("", String::as_str));
        }
        // RUL-23: Default and the picker open no particular browser.
        let target = draft.get("target").cloned().unwrap_or(Value::Null);
        let by_default = target.get("default").is_some() || target.get("picker").is_some();
        row::set_disabled(&fields.new_window, by_default);
        fields.held_row.set_subtitle(if check.alternative_key_wins {
            ALTERNATIVE_KEY_WINS
        } else {
            ""
        });
        fields.help_arrow.set_visible(!self.help_seen());
        for group in &fields.editable {
            group.set_sensitive(writable);
        }
        if let Some(delete) = &fields.delete {
            delete.set_sensitive(writable);
        }
    }

    fn connect(&self) {
        let inner = &self.inner;
        let fields = &inner.fields;
        let weak = Rc::downgrade(inner);
        fields.name.connect_changed(glib::clone!(
            #[strong]
            weak,
            move |entry| with(&weak, |this| this
                .edit("name", json!(entry.text().as_str())))
        ));
        fields.target.connect_chosen(glib::clone!(
            #[strong]
            weak,
            move |target| with(&weak, |this| this.edit("target", target.clone()))
        ));
        fields.target.connect_other(glib::clone!(
            #[strong]
            weak,
            move || with(&weak, Self::choose_other_target)
        ));
        fields.matchers.add_button().connect_clicked(glib::clone!(
            #[strong]
            weak,
            move |_| with(&weak, Self::add_matcher)
        ));
        fields.sources.add_button().connect_clicked(glib::clone!(
            #[strong]
            weak,
            move |_| with(&weak, Self::add_sources)
        ));
        inner.sheet.connect_primary(glib::clone!(
            #[strong]
            weak,
            move || weak.upgrade().is_some_and(|inner| Self { inner }.save())
        ));
        fields.help.connect_clicked(glib::clone!(
            #[strong]
            weak,
            move |_| with(&weak, Self::show_help)
        ));
        fields.test.connect_clicked(glib::clone!(
            #[strong]
            weak,
            move |_| {
                with(&weak, |this| {
                    let url = draft::test_url(&this.inner.draft.borrow());
                    (this.inner.callbacks.test)(url);
                });
            }
        ));
        if let Some(delete) = &fields.delete {
            delete.connect_activated(glib::clone!(
                #[strong]
                weak,
                move |_| with(&weak, Self::delete)
            ));
        }
        self.follow_store();
    }

    /// The file's writability, the alternative key and the help arrow can
    /// change while the sheet is open; the handler goes with it.
    fn follow_store(&self) {
        let inner = &self.inner;
        let weak = Rc::downgrade(inner);
        let handler = inner.store.connect_changed(glib::clone!(
            #[strong]
            weak,
            move |_| with(&weak, Self::refresh)
        ));
        let store = inner.store.downgrade();
        let handler = RefCell::new(Some(handler));
        inner.sheet.dialog().connect_closed(move |_| {
            if let (Some(store), Some(handler)) = (store.upgrade(), handler.take()) {
                store.disconnect(handler);
            }
        });
    }

    /// RUL-21 to RUL-25, RUL-27.
    fn connect_advanced(&self) {
        let fields = &self.inner.fields;
        let weak = Rc::downgrade(&self.inner);
        fields.background.connect_active_notify(glib::clone!(
            #[strong]
            weak,
            move |row| with(&weak, |this| this
                .edit("open-in-background", json!(row.is_active())))
        ));
        fields.new_window.connect_active_notify(glib::clone!(
            #[strong]
            weak,
            move |row| with(&weak, |this| this
                .edit("force-new-window", json!(row.is_active())))
        ));
        fields.held.connect_changed(glib::clone!(
            #[strong]
            weak,
            move |names| with(&weak, |this| this.edit("held-keys", json!(names)))
        ));
        fields.run.connect_chosen(glib::clone!(
            #[strong]
            weak,
            move |value| with(&weak, |this| this.edit("run", json!(value)))
        ));
        if let Some(switch) = fields.transform.switch() {
            switch.connect_active_notify(glib::clone!(
                #[strong]
                weak,
                move |switch| with(&weak, |this| this.set_transform(switch.is_active()))
            ));
        }
        fields.transform.button().connect_clicked(glib::clone!(
            #[strong]
            weak,
            move |_| {
                with(&weak, |this| {
                    let scope = script::scope(&this.text("id"));
                    script::open(&this.inner.store, &scope, &this.text("name"));
                });
            }
        ));
    }

    /// TGT-06: "Other…" picks any app as the target.
    fn choose_other_target(&self) {
        let weak = Rc::downgrade(&self.inner);
        AppChooser::open(
            self.inner.sheet.dialog(),
            &self.inner.store,
            Choice::Single,
            move |targets| {
                if let Some(target) = targets.first() {
                    with(&weak, |this| {
                        this.edit("target", target.clone());
                        this.inner.fields.target.show(&this.inner.store, target, "");
                    });
                }
            },
        );
    }

    /// SCR-09: turning Transform URL on for a rule without a script opens
    /// the script editor.
    fn set_transform(&self, on: bool) {
        self.edit("transform", json!(on));
        if on {
            let scope = script::scope(&self.text("id"));
            script::open_if_missing(&self.inner.store, &scope, &self.text("name"));
        }
    }

    /// The draft's matchers.
    fn matchers(&self) -> Vec<Value> {
        self.inner
            .draft
            .borrow()
            .get("url-matchers")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    }

    /// RUL-14: "+" adds an empty Domain matcher and focuses its entry.
    fn add_matcher(&self) {
        let mut matchers = self.matchers();
        matchers.push(json!({"kind": "domain", "pattern": ""}));
        let added = matchers.len() - 1;
        self.edit("url-matchers", Value::Array(matchers));
        self.show_matchers(Some(added));
        self.refresh();
    }

    fn remove_matcher(&self, index: usize) {
        let matchers: Vec<Value> = self
            .matchers()
            .into_iter()
            .enumerate()
            .filter(|(at, _)| *at != index)
            .map(|(_, matcher)| matcher)
            .collect();
        self.edit("url-matchers", Value::Array(matchers));
        self.show_matchers(None);
        self.refresh();
    }

    /// The user typed in matcher `index` or chose its kind; the rows stay,
    /// so the entry keeps the focus.
    fn set_matcher(&self, index: usize, kind: &str, pattern: &str) {
        let matchers: Vec<Value> = self
            .matchers()
            .into_iter()
            .enumerate()
            .map(|(at, matcher)| {
                if at == index {
                    json!({"kind": kind, "pattern": pattern})
                } else {
                    matcher
                }
            })
            .collect();
        self.edit("url-matchers", Value::Array(matchers));
    }

    /// The matcher rows for the draft; `focus` puts the cursor in one.
    fn show_matchers(&self, focus: Option<usize>) {
        let fields = &self.inner.fields;
        fields.matchers.clear();
        let rows: Vec<MatcherRow> = self
            .matchers()
            .iter()
            .enumerate()
            .map(|(index, matcher)| {
                let row = MatcherRow::new(&text(matcher, "kind"), &text(matcher, "pattern"));
                let weak = Rc::downgrade(&self.inner);
                row.connect_edited(move |kind, pattern| {
                    with(&weak, |this| this.set_matcher(index, kind, &pattern));
                });
                let weak = Rc::downgrade(&self.inner);
                row.connect_remove(move || with(&weak, |this| this.remove_matcher(index)));
                fields.matchers.add(row.row());
                row
            })
            .collect();
        if let Some(row) = focus.and_then(|index| rows.get(index)) {
            row.focus_entry();
        }
        fields.matcher_rows.replace(rows);
    }

    /// RUL-16: the source apps, each with its icon, name and a remove
    /// button.
    fn show_sources(&self) {
        let fields = &self.inner.fields;
        fields.sources.clear();
        let specs = strings(&self.inner.draft.borrow(), "source-apps");
        for source in list::source_rows(&specs, &self.inner.apps.json()) {
            let row = row::action_row(&glib::markup_escape_text(&source.name), "");
            let glyph = if source.icon.is_empty() {
                "application-x-executable"
            } else {
                source.icon.as_str()
            };
            row.add_prefix(&icon::image(glyph, ICON_SIZE));
            let remove = gtk::Button::builder()
                .css_classes(["flat", "circular"])
                .icon_name("list-remove-symbolic")
                .tooltip_text("Remove Source App")
                .valign(gtk::Align::Center)
                .build();
            remove.update_property(&[gtk::accessible::Property::Label(&format!(
                "Remove “{}”",
                source.name
            ))]);
            let weak = Rc::downgrade(&self.inner);
            let spec = source.spec.clone();
            remove.connect_clicked(move |_| with(&weak, |this| this.remove_source(&spec)));
            row.add_suffix(&remove);
            fields.sources.add(&row);
        }
    }

    /// "+": the app chooser, several at once, recent sources first.
    fn add_sources(&self) {
        let weak = Rc::downgrade(&self.inner);
        AppChooser::open(
            self.inner.sheet.dialog(),
            &self.inner.store,
            Choice::Multiple { recent: true },
            move |targets| with(&weak, |this| this.add_source_targets(targets)),
        );
    }

    /// Add the apps the chooser returned (`{"custom": "slack.desktop"}`),
    /// skipping those already listed.
    fn add_source_targets(&self, targets: &[Value]) {
        let mut specs = strings(&self.inner.draft.borrow(), "source-apps");
        let chosen = targets
            .iter()
            .filter_map(|target| target.as_object()?.values().next()?.as_str())
            .map(str::to_owned);
        for spec in chosen {
            if !specs.contains(&spec) {
                specs.push(spec);
            }
        }
        self.edit("source-apps", json!(specs));
        self.show_sources();
    }

    fn remove_source(&self, spec: &str) {
        let specs: Vec<String> = strings(&self.inner.draft.borrow(), "source-apps")
            .into_iter()
            .filter(|candidate| candidate != spec)
            .collect();
        self.edit("source-apps", json!(specs));
        self.show_sources();
    }

    /// Save (RUL-20); false keeps the sheet open.
    fn save(&self) -> bool {
        let inner = &self.inner;
        let draft = inner.draft.borrow().clone();
        if !inner.store.writable()
            || !draft::check(&draft, super::alternative_key(&inner.store)).valid
        {
            return false;
        }
        match draft::rule_of(&draft) {
            Ok(rule) => {
                (inner.callbacks.save)(inner.index, rule);
                true
            }
            Err(error) => {
                tracing::warn!(%error, "the rule draft is not a rule");
                false
            }
        }
    }

    /// RUL-28: Delete Rule closes the sheet; the page offers Undo.
    fn delete(&self) {
        if let Some(index) = self.inner.index {
            self.inner.sheet.close();
            (self.inner.callbacks.delete)(index);
        }
    }
}

/// Why Save is disabled, in words (RUL-18); a matcher's own error shows
/// under it.
fn blocker(check: &draft::Check, draft: &Value) -> String {
    if !check.error.is_empty() {
        return check.error.clone();
    }
    let mut parts = Vec::new();
    if check.name_missing {
        parts.push("Give the rule a name.");
    }
    if check.no_condition {
        parts.push("Add a URL matcher, a source app or held keys.");
    }
    let patterns: Vec<String> = draft
        .get("url-matchers")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|matcher| text(matcher, "pattern"))
                .collect()
        })
        .unwrap_or_default();
    let failing: Vec<usize> = check
        .matcher_errors
        .iter()
        .enumerate()
        .filter(|(_, error)| !error.is_empty())
        .map(|(index, _)| index)
        .collect();
    if failing
        .iter()
        .any(|index| patterns.get(*index).is_some_and(String::is_empty))
    {
        parts.push("Fill in or remove the empty URL matcher.");
    } else if !failing.is_empty() {
        parts.push("Correct the URL matcher marked below.");
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(draft: &Value) -> draft::Check {
        draft::check(draft, wye_core::Modifiers::default())
    }

    #[test]
    fn rul_18_the_blocker_says_what_is_missing() {
        let empty = json!({"name": "", "url-matchers": []});
        assert_eq!(
            blocker(&check(&empty), &empty),
            "Give the rule a name. Add a URL matcher, a source app or held keys."
        );
        let blank = json!({"name": "x", "url-matchers": [{"kind": "domain", "pattern": ""}]});
        assert_eq!(
            blocker(&check(&blank), &blank),
            "Fill in or remove the empty URL matcher."
        );
        let broken = json!({"name": "x", "url-matchers": [{"kind": "regex", "pattern": "a("}]});
        assert_eq!(
            blocker(&check(&broken), &broken),
            "Correct the URL matcher marked below."
        );
        let fine = json!({"name": "x", "url-matchers": [{"kind": "domain", "pattern": "a.b"}]});
        assert!(check(&fine).valid);
        assert_eq!(blocker(&check(&fine), &fine), "");
    }

    #[test]
    fn drafts_are_read_safely() {
        let draft = json!({"name": "GitHub", "source-apps": ["a.desktop", 3, "b"]});
        assert_eq!(text(&draft, "name"), "GitHub");
        assert_eq!(text(&draft, "missing"), "");
        assert_eq!(strings(&draft, "source-apps"), ["a.desktop", "b"]);
        assert!(strings(&draft, "held-keys").is_empty());
    }
}
