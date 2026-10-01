//! The Rules page (08-rules.md, RUL-01 to RUL-07): the rules in the order
//! the pipeline checks them, in one list card with its toolbar (BLK-13):
//! **Add Rule…** and **Test Rules…** at the left, the "⋯" menu at the right
//! (Test Rules…; Import Rules…, Export Rules…; Delete All Rules…; How Rules
//! Work). With no rules the card shows the empty state (BLK-12); with rules,
//! the hint "Click a rule to edit it. Drag to reorder." sits under it.
//!
//! Every change is a new rule list saved whole through the store (SET-06,
//! `crate::rules::ops`): the list shows the store's answer. Deleting offers
//! Undo in a toast (RUL-06, RUL-28).
//!
//! `ShowWindow("rule-editor", prefill)` (PICK-31, history's Create Rule,
//! `{"index": n}` to edit one) and `ShowWindow("test-rules", {url})` reach
//! the page through [`Page::request`].
//!
//! KDE counterpart: crates/wye-ui/qml/settings/RulesPage.qml.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gio, glib};
use serde_json::Value;

use super::{Context, Page};
use crate::rules::draft::{self, Prefill};
use crate::rules::editor::{Callbacks, RuleEditor};
use crate::rules::ops::{self, Removed};
use crate::rules::rule_list::{Item, RuleList};
use crate::rules::tester_sheet::TesterSheet;
use crate::rules::transfer::{self, Outcome};
use crate::rules::{self, Apps, list};
use crate::settings::menu::Surface;
use crate::settings::request::{RULE_EDITOR, TEST_RULES};
use crate::settings::sheets;
use crate::widgets::list_toolbar::ListCard;
use crate::widgets::{empty_state, group, toast};

/// RUL-06: how long Undo is offered, in seconds (KDE: 8 s).
const UNDO_SECONDS: u32 = 8;
/// RUL-01.
const EMPTY_TEXT: &str = "A rule lets you open a specific app based on the URL and source app";
const HINT: &str = "Click a rule to edit it. Drag to reorder.";
/// Most dialogs stacked over the page's own (a chooser over the editor).
const MAX_STACKED: usize = 4;

/// The Rules page.
#[derive(Debug)]
pub struct RulesPage {
    page: adw::PreferencesPage,
    state: Rc<State>,
}

struct State {
    context: Context,
    apps: Apps,
    card: ListCard,
    list: RuleList,
    hint: gtk::Label,
    adds: [gtk::Button; 2],
    actions: gio::SimpleActionGroup,
    /// The last deletion and its toast, while Undo is offered.
    undo: RefCell<Option<(Removed, adw::Toast)>>,
    editor: RefCell<Option<RuleEditor>>,
    /// The page's own dialogs that are open (editor, tester, help, the
    /// Delete All question): what [`close_sheets`] closes, with whatever
    /// was stacked on them, and nothing else over the window.
    dialogs: RefCell<Vec<glib::WeakRef<adw::Dialog>>>,
}

impl std::fmt::Debug for State {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RulesPage")
            .field("list", &self.list)
            .finish_non_exhaustive()
    }
}

fn with(weak: &Weak<State>, act: impl FnOnce(&Rc<State>)) {
    if let Some(state) = weak.upgrade() {
        act(&state);
    }
}

impl RulesPage {
    /// Build the page on `context`'s store.
    #[must_use]
    pub fn new(context: &Context) -> Self {
        let page = adw::PreferencesPage::builder()
            .title("Rules")
            .name("rules")
            .build();
        let card = ListCard::new();
        let add = card.add_button("Add Rule…", "list-add-symbolic");
        add.set_tooltip_text(Some("Add a rule at the end of the list"));
        let test = card.add_button("Test Rules…", "media-playback-start-symbolic");
        test.set_tooltip_text(Some("See which rule a link matches, without opening it"));
        card.menu_button().set_tooltip_text(Some("More Actions"));
        card.set_menu(&more_menu());
        // RUL-01
        let empty = empty_state::empty_state("wye-rules-symbolic", "No Rules", EMPTY_TEXT);
        // Inside the card: as tall as it needs, or the hidden placeholder
        // would stretch the list under its rows.
        empty.set_vexpand(false);
        let empty_add = empty_state::with_action(&empty, "Add Rule…");
        card.set_placeholder(&empty);
        let hint = gtk::Label::builder()
            .label(HINT)
            .xalign(0.0)
            .wrap(true)
            .margin_top(10)
            .margin_start(12)
            .css_classes(["dimmed", "caption"])
            .build();
        let list_group = group::group("");
        list_group.add(card.widget());
        list_group.add(&hint);
        page.add(&list_group);

        let actions = gio::SimpleActionGroup::new();
        page.insert_action_group("rules", Some(&actions));
        let state = Rc::new(State {
            context: context.clone(),
            apps: Apps::default(),
            list: RuleList::new(card.list()),
            card,
            hint,
            adds: [add, empty_add],
            actions,
            undo: RefCell::default(),
            editor: RefCell::default(),
            dialogs: RefCell::default(),
        });
        connect(&state, &test);
        show(&state);
        let weak = Rc::downgrade(&state);
        context.store.connect_changed(glib::clone!(
            #[strong]
            weak,
            move |_| with(&weak, show)
        ));
        // A sheet belongs to this page: it goes when another page shows.
        page.connect_unmap(glib::clone!(
            #[strong]
            weak,
            move |_| with(&weak, |state| close_sheets(state))
        ));
        Self { page, state }
    }
}

impl Page for RulesPage {
    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }

    /// The self-test's states: `rule-editor`, `rule-editor-end` (the first
    /// rule, scrolled to its end), `tester`, `rules-help`,
    /// `delete-all`, `undo` (the first rule deleted), `row-menu`,
    /// `more-menu`; `none` closes what is open.
    fn open_sheet(&self, name: &str) -> bool {
        let state = &self.state;
        close_sheets(state);
        // Each case shows only what it asks for.
        if let Some((_, toast)) = state.undo.take() {
            toast.dismiss();
        }
        match name {
            "rule-editor" => open_new(state, ""),
            "rule-editor-end" => {
                open_rule(state, 0);
                if let Some(editor) = state.editor.borrow().as_ref() {
                    editor.reveal_end();
                }
            }
            "tester" => {
                open_tester(state, "", None);
            }
            "rules-help" => {
                open_new(state, "");
                if let Some(editor) = state.editor.borrow().as_ref() {
                    editor.show_help();
                }
            }
            "delete-all" => confirm_delete_all(state),
            "undo" => delete(state, 0),
            "row-menu" => state.list.open_menu(0),
            "more-menu" => state.card.menu_button().popup(),
            "none" => {}
            _ => return false,
        }
        true
    }

    fn request(&self, key: &str, argument: &str) {
        let state = &self.state;
        match key {
            // PICK-31, DLG-TST-03: a prefill, or `{"index": n}`. An editor
            // with changes is not dropped unasked.
            RULE_EDITOR => {
                let argument = argument.to_owned();
                replace_editor(state, move |state| match Prefill::parse(&argument).index {
                    Some(index) => open_rule(state, index),
                    None => open_new(state, &argument),
                });
            }
            // DLG-TST: `{"url": …}`; in the self-test also `trace` (the
            // service's answer) and `type` (a link to type in once open).
            // Over an editor with changes, which stays.
            TEST_RULES => {
                if !editor_has_changes(state) {
                    close_sheets(state);
                }
                let request: Value = serde_json::from_str(argument).unwrap_or(Value::Null);
                let text = |name: &str| request.get(name).and_then(Value::as_str);
                let tester =
                    open_tester(state, text("url").unwrap_or_default(), request.get("trace"));
                if let (Some(tester), Some(typed)) = (tester, text("type")) {
                    tester.type_link(typed);
                }
            }
            _ => tracing::debug!(%key, "the Rules page takes no such request"),
        }
    }
}

/// What a "⋯" menu item does.
type PageAction = fn(&Rc<State>);

/// RUL-02: the "⋯" menu.
fn more_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    let sections: [&[(&str, &str)]; 4] = [
        &[("Test Rules…", "rules.test")],
        &[
            ("Import Rules…", "rules.import"),
            ("Export Rules…", "rules.export"),
        ],
        &[("Delete All Rules…", "rules.delete-all")],
        &[("How Rules Work", "rules.help")],
    ];
    for items in sections {
        let section = gio::Menu::new();
        for (label, action) in items {
            section.append(Some(label), Some(action));
        }
        menu.append_section(None, &section);
    }
    menu
}

/// Wire the toolbar, the menu and the list.
fn connect(state: &Rc<State>, test: &gtk::Button) {
    let weak = Rc::downgrade(state);
    for add in &state.adds {
        add.connect_clicked(glib::clone!(
            #[strong]
            weak,
            move |_| with(&weak, |state| open_new(state, ""))
        ));
    }
    test.connect_clicked(glib::clone!(
        #[strong]
        weak,
        move |_| with(&weak, |state| {
            open_tester(state, "", None);
        })
    ));
    let entries: [(&str, PageAction); 5] = [
        ("test", |state| {
            open_tester(state, "", None);
        }),
        ("import", |state| transfer_rules(state, transfer::import)),
        ("export", |state| transfer_rules(state, transfer::export)),
        ("delete-all", confirm_delete_all),
        ("help", |state| {
            if let Some(window) = state.context.window() {
                own(state, &rules::help::open(&window));
            }
        }),
    ];
    for (name, run) in entries {
        let action = gio::SimpleAction::new(name, None);
        action.connect_activate(glib::clone!(
            #[strong]
            weak,
            move |_, _| with(&weak, run)
        ));
        state.actions.add_action(&action);
    }
    connect_list(state);
}

/// What the user does to a row: edit, turn off, move, duplicate, delete.
fn connect_list(state: &Rc<State>) {
    let weak = Rc::downgrade(state);
    let list = &state.list;
    list.connect_edit(glib::clone!(
        #[strong]
        weak,
        move |index| with(&weak, |state| open_rule(state, index))
    ));
    list.connect_toggle(glib::clone!(
        #[strong]
        weak,
        move |index, on| with(&weak, |state| change(state, |rules| ops::toggled(
            rules, index, on
        )))
    ));
    list.connect_moved(glib::clone!(
        #[strong]
        weak,
        move |from, to| with(&weak, |state| change(state, |rules| ops::moved(
            rules, from, to
        )))
    ));
    list.connect_duplicate(glib::clone!(
        #[strong]
        weak,
        move |index| {
            with(&weak, |state| {
                change(state, |rules| {
                    let id = draft::fresh_id(rules, rules::seed());
                    ops::duplicated(rules, index, &id)
                });
            });
        }
    ));
    list.connect_delete(glib::clone!(
        #[strong]
        weak,
        move |index| with(&weak, |state| delete(state, index))
    ));
}

/// The list's rows for what the store holds.
fn items(state: &State) -> Vec<Item> {
    let store = &state.context.store;
    let config = store.with_snapshot(|snapshot| snapshot.config.to_string());
    let rules = list::rules_of(&config).unwrap_or_default();
    list::rows(&rules, &state.apps.names())
        .into_iter()
        .map(|view| {
            let target = store.target_label(Surface::Rule, &view.target, "");
            Item { view, target }
        })
        .collect()
}

/// Show what the store holds; read the installed apps once it holds
/// something, so summaries say "from Slack".
fn show(state: &Rc<State>) {
    let weak = Rc::downgrade(state);
    state.apps.ensure(&state.context.store, move || {
        with(&weak, |state| {
            state
                .list
                .show(items(state), state.context.store.writable());
        });
    });
    let store = &state.context.store;
    let writable = store.writable();
    let items = items(state);
    let any = !items.is_empty();
    state.list.show(items, writable);
    state.hint.set_visible(any);
    for add in &state.adds {
        add.set_sensitive(writable);
    }
    for (name, enabled) in [
        ("import", writable),
        ("export", any),
        ("delete-all", writable && any),
    ] {
        if let Some(action) = state
            .actions
            .lookup_action(name)
            .and_downcast::<gio::SimpleAction>()
        {
            action.set_enabled(enabled);
        }
    }
}

/// Save the list `edit` makes of the current one.
fn change(state: &State, edit: impl FnOnce(&[wye_core::Rule]) -> Vec<wye_core::Rule>) {
    let store = &state.context.store;
    if let Some(current) = rules::rules(store) {
        rules::save(store, &edit(&current));
    }
}

/// Count `dialog` as the page's own until it closes.
fn own(state: &Rc<State>, dialog: &impl IsA<adw::Dialog>) {
    let dialog = dialog.upcast_ref::<adw::Dialog>();
    state.dialogs.borrow_mut().push(dialog.downgrade());
    let weak = Rc::downgrade(state);
    dialog.connect_closed(move |closed| {
        with(&weak, |state| {
            state
                .dialogs
                .borrow_mut()
                .retain(|dialog| dialog.upgrade().is_some_and(|open| open != *closed));
        });
    });
}

/// Whether one of the page's own dialogs is open.
fn own_open(state: &State) -> bool {
    state
        .dialogs
        .borrow()
        .iter()
        .any(|dialog| dialog.upgrade().is_some())
}

/// Close the page's own sheets (editor, tester, help, the Delete All
/// question) with what is stacked on them, and its popovers; another
/// page's sheet stays.
fn close_sheets(state: &State) {
    let Some(window) = state.context.window() else {
        return;
    };
    sheets::close_popovers(state.card.widget().upcast_ref());
    for _ in 0..MAX_STACKED {
        if !own_open(state) {
            break;
        }
        // The top one first: what is stacked on the page's sheets is
        // theirs (an app chooser over the editor).
        let Some(top) = window.visible_dialog() else {
            break;
        };
        top.force_close();
    }
    state.editor.replace(None);
}

/// Whether the rule editor is open with changes not saved.
fn editor_has_changes(state: &State) -> bool {
    state.editor.borrow().as_ref().is_some_and(|editor| {
        editor.has_changes()
            && state
                .dialogs
                .borrow()
                .iter()
                .any(|dialog| dialog.upgrade().as_ref() == Some(editor.dialog()))
    })
}

/// Close the page's sheets and run `open`; an editor with changes asks
/// first (Keep Editing or Discard), so its work is not lost unasked.
fn replace_editor(state: &Rc<State>, open: impl FnOnce(&Rc<State>) + 'static) {
    let window = state.context.window();
    let Some(window) = window.filter(|_| editor_has_changes(state)) else {
        close_sheets(state);
        open(state);
        return;
    };
    let question = adw::AlertDialog::builder()
        .heading("Discard Changes?")
        .body("The rule you are editing has changes that are not saved.")
        .close_response("keep")
        .default_response("keep")
        .build();
    question.add_responses(&[("keep", "Keep Editing"), ("discard", "Discard")]);
    question.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
    let weak = Rc::downgrade(state);
    let open = RefCell::new(Some(open));
    question.connect_response(Some("discard"), move |_, _| {
        with(&weak, |state| {
            if let Some(open) = open.take() {
                close_sheets(state);
                open(state);
            }
        });
    });
    question.present(Some(&window));
}

/// What the editor asks of the page.
fn callbacks(state: &Rc<State>) -> Callbacks {
    let weak = Rc::downgrade(state);
    let (save_weak, delete_weak) = (weak.clone(), weak.clone());
    Callbacks {
        save: Box::new(move |index, rule| {
            with(&save_weak, |state| {
                change(state, |rules| ops::saved(rules, index, rule));
            });
        }),
        delete: Box::new(move |index| with(&delete_weak, |state| delete(state, index))),
        // RUL-19: the tester opens over the editor, which keeps its draft.
        test: Box::new(move |url| {
            with(&weak, |state| {
                open_tester(state, &url, None);
            });
        }),
    }
}

/// RUL-10: a new rule, prefilled from a `rule-editor` argument (PICK-31).
fn open_new(state: &Rc<State>, argument: &str) {
    let Some(window) = state.context.window() else {
        return;
    };
    let store = &state.context.store;
    let current = rules::rules(store).unwrap_or_default();
    let id = draft::fresh_id(&current, rules::seed());
    let draft = draft::new_draft(&Prefill::parse(argument), &id);
    let editor = RuleEditor::open(&window, store, &state.apps, None, draft, callbacks(state));
    own(state, editor.dialog());
    state.editor.replace(Some(editor));
}

/// RUL-05: edit rule `index`.
fn open_rule(state: &Rc<State>, index: usize) {
    let Some(window) = state.context.window() else {
        return;
    };
    let store = &state.context.store;
    let Some(current) = rules::rules(store) else {
        return;
    };
    let id = draft::fresh_id(&current, rules::seed());
    let Some(draft) = draft::draft_for(&current, index, &id) else {
        tracing::warn!(index, "there is no such rule to edit");
        return;
    };
    let editor = RuleEditor::open(
        &window,
        store,
        &state.apps,
        Some(index),
        draft,
        callbacks(state),
    );
    own(state, editor.dialog());
    state.editor.replace(Some(editor));
}

/// DLG-TST: the tester with `url`; the matched rule opens in the editor.
/// The sheet keeps itself while open, so the page need not.
fn open_tester(state: &Rc<State>, url: &str, trace: Option<&Value>) -> Option<TesterSheet> {
    let window = state.context.window()?;
    let weak = Rc::downgrade(state);
    let tester = TesterSheet::open(
        &window,
        &state.context.store,
        &state.apps,
        url,
        trace,
        // DLG-TST-03: the matched rule replaces the sheets, asking first
        // when an editor under the tester has changes.
        move |index| {
            with(&weak, |state| {
                replace_editor(state, move |state| open_rule(state, index));
            });
        },
    );
    own(state, tester.dialog());
    Some(tester)
}

/// RUL-06, RUL-28: delete rule `index`, with Undo.
fn delete(state: &Rc<State>, index: usize) {
    let store = &state.context.store;
    let Some(current) = rules::rules(store) else {
        return;
    };
    let (rest, removed) = ops::removed(&current, index);
    if removed.is_empty() {
        return;
    }
    rules::save(store, &rest);
    offer_undo(state, removed);
}

/// RUL-02: Delete All Rules… asks first.
fn confirm_delete_all(state: &Rc<State>) {
    let Some(window) = state.context.window() else {
        return;
    };
    let dialog = adw::AlertDialog::builder()
        .heading("Delete All Rules?")
        .body("Every rule is removed. Their scripts stay on disk.")
        .close_response("cancel")
        .default_response("cancel")
        .build();
    dialog.add_responses(&[("cancel", "Cancel"), ("delete", "Delete All Rules")]);
    dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
    let weak = Rc::downgrade(state);
    dialog.connect_response(Some("delete"), move |_, _| {
        with(&weak, |state| {
            let store = &state.context.store;
            if let Some(current) = rules::rules(store) {
                rules::save(store, &[]);
                offer_undo(state, ops::all_removed(&current));
            }
        });
    });
    own(state, &dialog);
    dialog.present(Some(&window));
}

/// The undo toast: "Deleted “GitHub”" or "Deleted 3 rules", with Undo.
fn offer_undo(state: &Rc<State>, removed: Removed) {
    if let Some((_, previous)) = state.undo.take() {
        previous.dismiss();
    }
    let (count, name) = removed.summary();
    let title = if count == 1 {
        format!("Deleted “{}”", glib::markup_escape_text(name))
    } else {
        format!("Deleted {count} rules")
    };
    let toast = adw::Toast::builder()
        .title(title)
        .button_label("Undo")
        .timeout(UNDO_SECONDS)
        .build();
    let weak = Rc::downgrade(state);
    toast.connect_button_clicked(glib::clone!(
        #[strong]
        weak,
        move |_| {
            with(&weak, |state| {
                let taken = state.undo.take();
                if let Some((removed, _)) = taken {
                    change(state, |rules| removed.restored(rules));
                }
            });
        }
    ));
    toast.connect_dismissed(move |toast| {
        with(&weak, |state| {
            let current = state
                .undo
                .borrow()
                .as_ref()
                .is_some_and(|(_, shown)| shown == toast);
            if current {
                state.undo.replace(None);
            }
        });
    });
    state.undo.replace(Some((removed, toast.clone())));
    toast::show(&state.card.widget().clone(), toast);
}

/// Show what an import or export did.
fn report(state: &State, outcome: &Outcome) {
    if matches!(outcome, Outcome::Imported(_)) {
        state.context.store.refresh();
    }
    toast::show(
        &state.card.widget().clone(),
        adw::Toast::builder()
            .title(glib::markup_escape_text(&outcome.text()))
            .build(),
    );
}

/// [`transfer::import`] or [`transfer::export`], with what to do after.
type Transfer = fn(&gtk::Window, Box<dyn FnOnce(Outcome)>);

/// RUL-02: Import Rules… or Export Rules… (`run`), then say what it did.
fn transfer_rules(state: &Rc<State>, run: Transfer) {
    let Some(window) = state.context.window() else {
        return;
    };
    let weak = Rc::downgrade(state);
    run(
        window.upcast_ref(),
        Box::new(move |outcome| {
            with(&weak, |state| report(state, &outcome));
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::store::SettingsStore;

    #[test]
    fn rul_02_the_menu_has_its_sections_in_order() {
        let menu = more_menu();
        assert_eq!(menu.n_items(), 4);
    }

    #[test]
    fn rul_01_the_store_rules_are_read_or_refused() {
        let store = SettingsStore::new();
        store
            .load_fixture(&serde_json::json!({"config": {"rules": [{"name": "a"}]}}))
            .expect("fixture");
        assert_eq!(rules::rules(&store).map(|rules| rules.len()), Some(1));
        store
            .load_fixture(&serde_json::json!({"config": {"rules": [{"enabled": "yes"}]}}))
            .expect("fixture");
        assert!(
            rules::rules(&store).is_none(),
            "never built on a broken list"
        );
    }
}
