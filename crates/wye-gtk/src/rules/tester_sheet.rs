//! The rule tester, "Test Rules" (DLG-TST-01 to DLG-TST-03): a link, an
//! optional source app and held keys go through the pipeline with
//! `TestLink` (nothing opens); the Steps card lists what changed the link or
//! decided the target, ending with the outcome. The result follows the
//! inputs (300 ms after the last change, at once on Enter); "Skip network",
//! pinned at the bottom, leaves short links unexpanded. The matched rule
//! opens in the rule editor.
//!
//! The sheet keeps itself alive until it closes: its signal handlers hold
//! it weakly, so a caller may drop the handle [`TesterSheet::open`] returns.
//!
//! Under the self-test a trace from the fixture stands in for the service,
//! and [`TesterSheet::type_link`] checks that what is typed gets an answer.
//!
//! KDE counterpart: crates/wye-ui/qml/rules/RuleTesterSheet.qml and
//! crates/wye-ui/src/bridge/tester.rs.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::time::Duration;

use adw::prelude::*;
use gtk::glib;
use serde_json::Value;
use zbus::zvariant;

use super::tester::{self, TraceView};
use super::{Apps, trace_rows};
use crate::error_text;
use crate::settings::menu::Surface;
use crate::settings::store::SettingsStore;
use crate::widgets::button_row::ButtonRow;
use crate::widgets::modifiers::ModifierChooser;
use crate::widgets::section::clear_rows;
use crate::widgets::sheet::Sheet;
use crate::widgets::{entry_row, group};

/// DLG-TST-01: how long the inputs rest before a run.
const RUN_DELAY: Duration = Duration::from_millis(300);
/// What the Steps card says before there is a result.
const NO_LINK: &str = "Enter a link to see which rule it matches and where it opens.";
const NO_RESULT: &str = "No result yet.";

/// The sheet's controls.
struct Fields {
    link: adw::EntryRow,
    source: adw::ComboRow,
    /// The desktop IDs behind `source`'s entries; the first is none.
    source_ids: Vec<String>,
    held: ModifierChooser,
    skip_network: gtk::Switch,
    steps: gtk::ListBox,
    waiting: gtk::Stack,
    waiting_text: gtk::Label,
    error: gtk::Label,
    labels: gtk::SizeGroup,
}

struct Inner {
    sheet: Sheet,
    store: SettingsStore,
    fields: Fields,
    /// Numbers runs, so a late answer never replaces a newer one.
    run: Cell<u64>,
    busy: Cell<bool>,
    view: RefCell<Option<TraceView>>,
    /// The self-test's stand-in for the service: what every run answers
    /// while the store is offline.
    answer: Option<Result<TraceView, String>>,
    pending: RefCell<Option<glib::SourceId>>,
    edit_rule: Box<dyn Fn(usize)>,
}

/// An open rule tester. Clones share it.
#[derive(Clone)]
pub struct TesterSheet {
    inner: Rc<Inner>,
}

impl std::fmt::Debug for TesterSheet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TesterSheet")
            .field("run", &self.inner.run.get())
            .finish_non_exhaustive()
    }
}

fn with(weak: &Weak<Inner>, act: impl FnOnce(&TesterSheet)) {
    if let Some(inner) = weak.upgrade() {
        act(&TesterSheet { inner });
    }
}

impl TesterSheet {
    /// Show the tester over `window` with `url`; while the store is offline
    /// (the self-test) `trace` answers instead of the service.
    /// `edit_rule(index)` opens the matched rule (DLG-TST-03).
    pub fn open(
        window: &adw::ApplicationWindow,
        store: &SettingsStore,
        apps: &Apps,
        url: &str,
        trace: Option<&Value>,
        edit_rule: impl Fn(usize) + 'static,
    ) -> Self {
        let sheet = Sheet::new("Test Rules", "Done");
        sheet.set_valid(true);
        sheet.cancel().set_visible(false);
        sheet.connect_primary(|| true);
        let fields = build(&sheet, apps);
        fields.link.set_text(url);
        let inner = Rc::new(Inner {
            sheet,
            store: store.clone(),
            fields,
            run: Cell::new(0),
            busy: Cell::new(false),
            view: RefCell::default(),
            answer: trace.map(|trace| tester::view(&trace.to_string())),
            pending: RefCell::default(),
            edit_rule: Box::new(edit_rule),
        });
        let this = Self { inner };
        this.connect();
        this.keep_until_closed();
        this.run_now();
        let inner = &this.inner;
        inner.sheet.present(window);
        // A long link shows from its start, not selected and scrolled to
        // its end.
        entry_row::focus_when_shown(inner.sheet.dialog(), &inner.fields.link, 0);
        this
    }

    /// The sheet's dialog, so the page can tell whether it is open.
    #[must_use]
    pub fn dialog(&self) -> &adw::Dialog {
        self.inner.sheet.dialog()
    }

    /// Hold the sheet while its dialog is open; closing lets it go.
    fn keep_until_closed(&self) {
        let keep = RefCell::new(Some(self.clone()));
        self.inner.sheet.dialog().connect_closed(move |_| {
            keep.take();
        });
    }

    /// Self-test: type `text` into the link once the caller has let go of
    /// the sheet and press Enter, as a user would; a critical when no
    /// answer shows (the sheet stopped listening to its own inputs).
    pub fn type_link(&self, text: &str) {
        let link = self.inner.fields.link.downgrade();
        let steps = self.inner.fields.steps.downgrade();
        let text = text.to_owned();
        // At the default priority: before the self-test's next case, which
        // an idle callback could miss while the sheet's opening animates.
        glib::timeout_add_local_once(Duration::ZERO, move || {
            let (Some(link), Some(steps)) = (link.upgrade(), steps.upgrade()) else {
                glib::g_critical!("wye-gtk", "tester: closed before the link was typed");
                return;
            };
            link.set_text(&text);
            link.emit_by_name::<()>("entry-activated", &[]);
            if steps.row_at_index(0).is_none() {
                glib::g_critical!("wye-gtk", "tester: no answer for the link typed into it");
            }
        });
    }

    fn connect(&self) {
        let fields = &self.inner.fields;
        let weak = Rc::downgrade(&self.inner);
        fields.link.connect_changed(glib::clone!(
            #[strong]
            weak,
            move |_| with(&weak, Self::run_later)
        ));
        fields.link.connect_entry_activated(glib::clone!(
            #[strong]
            weak,
            move |_| with(&weak, Self::run_now)
        ));
        fields.source.connect_selected_notify(glib::clone!(
            #[strong]
            weak,
            move |_| with(&weak, Self::run_later)
        ));
        fields.held.connect_changed(glib::clone!(
            #[strong]
            weak,
            move |_| with(&weak, Self::run_later)
        ));
        fields.skip_network.connect_active_notify(glib::clone!(
            #[strong]
            weak,
            move |_| with(&weak, Self::run_later)
        ));
        self.inner.sheet.dialog().connect_closed(glib::clone!(
            #[strong]
            weak,
            move |_| with(&weak, Self::cancel_pending)
        ));
    }

    fn cancel_pending(&self) {
        if let Some(source) = self.inner.pending.take() {
            source.remove();
        }
    }

    /// DLG-TST-01: run once the inputs rest.
    fn run_later(&self) {
        self.cancel_pending();
        let weak = Rc::downgrade(&self.inner);
        let source = glib::timeout_add_local_once(RUN_DELAY, move || {
            with(&weak, |this| {
                this.inner.pending.take();
                this.run_now();
            });
        });
        self.inner.pending.replace(Some(source));
    }

    /// Ask the service what the link does.
    fn run_now(&self) {
        self.cancel_pending();
        let inner = &self.inner;
        let fields = &inner.fields;
        let url = fields.link.text().trim().to_owned();
        if url.is_empty() {
            // No link, no result; an answer still on its way must not land.
            inner.run.set(inner.run.get() + 1);
            inner.busy.set(false);
            self.show(None, "");
            return;
        }
        if inner.store.offline() {
            // The self-test: a fixture's trace, or nothing.
            inner.run.set(inner.run.get() + 1);
            match &inner.answer {
                Some(Ok(view)) => self.show(Some(view.clone()), ""),
                Some(Err(error)) => self.show(None, error),
                None => {}
            }
            return;
        }
        let run = inner.run.get() + 1;
        inner.run.set(run);
        let source = usize::try_from(fields.source.selected())
            .ok()
            .and_then(|index| fields.source_ids.get(index))
            .cloned()
            .unwrap_or_default();
        let context = tester::context(
            &source,
            &fields.held.pressed(),
            fields.skip_network.is_active(),
        );
        inner.busy.set(true);
        self.show_waiting();
        let weak = Rc::downgrade(inner);
        crate::service::request(
            move |proxy| async move { proxy.test_link(&url, dict(&context)).await },
            move |answer| {
                with(&weak, |this| {
                    if this.inner.run.get() != run {
                        return;
                    }
                    this.inner.busy.set(false);
                    match answer
                        .map_err(|error| error_text::describe(&error).sentence())
                        .and_then(|json| tester::view(&json))
                    {
                        Ok(view) => this.show(Some(view), ""),
                        Err(error) => this.show(None, &error),
                    }
                });
            },
        );
    }

    /// The placeholder while there is no result: a spinner while the
    /// service works, else a line saying what to do.
    fn show_waiting(&self) {
        let inner = &self.inner;
        let fields = &inner.fields;
        let busy = inner.busy.get() && inner.view.borrow().is_none();
        fields
            .waiting
            .set_visible_child_name(if busy { "busy" } else { "text" });
        fields
            .waiting_text
            .set_label(if fields.link.text().is_empty() {
                NO_LINK
            } else {
                NO_RESULT
            });
    }

    /// Show `view` (none: the placeholder) and `error`.
    fn show(&self, view: Option<TraceView>, error: &str) {
        let inner = &self.inner;
        let fields = &inner.fields;
        fields.error.set_label(error);
        fields.error.set_visible(!error.is_empty());
        clear_rows(&fields.steps);
        if let Some(view) = &view {
            for step in &view.steps {
                fields
                    .steps
                    .append(&trace_rows::step(&step.label, &step.text, &fields.labels));
            }
            let target = inner.store.target_label(Surface::Rule, &view.target, "");
            fields
                .steps
                .append(&trace_rows::outcome(view, &target, &fields.labels));
            if let Ok(index) = usize::try_from(view.rule_index) {
                fields.steps.append(&self.matched_rule(index));
            }
        }
        inner.view.replace(view);
        self.show_waiting();
    }

    /// DLG-TST-03: the matched rule, which opens in the rule editor.
    fn matched_rule(&self, index: usize) -> adw::ActionRow {
        let name = super::rules(&self.inner.store)
            .and_then(|rules| rules.get(index).map(|rule| rule.name.clone()))
            .unwrap_or_default();
        let subtitle = if name.is_empty() {
            String::new()
        } else {
            format!("“{}”", glib::markup_escape_text(&name))
        };
        let row = ButtonRow::new("Matched rule", &subtitle, "Edit Rule…");
        row.activate_with_row();
        let weak = Rc::downgrade(&self.inner);
        row.button().connect_clicked(move |_| {
            with(&weak, |this| (this.inner.edit_rule)(index));
        });
        row.row().clone()
    }
}

/// `TestLink`'s context (an `a{sv}`) from its JSON form.
fn dict(context: &Value) -> HashMap<&str, zvariant::Value<'_>> {
    let mut map = HashMap::new();
    for (key, value) in context.as_object().into_iter().flatten() {
        if let Some(value) = variant(value) {
            map.insert(key.as_str(), value);
        }
    }
    map
}

/// A context value as a variant: flags, strings and string lists are all
/// the context has.
fn variant(value: &Value) -> Option<zvariant::Value<'_>> {
    match value {
        Value::Bool(flag) => Some((*flag).into()),
        Value::String(text) => Some(text.as_str().into()),
        Value::Array(items) => {
            let texts: Vec<&str> = items.iter().filter_map(Value::as_str).collect();
            Some(texts.into())
        }
        _ => None,
    }
}

/// The inputs, the Steps card and the footer.
fn build(sheet: &Sheet, apps: &Apps) -> Fields {
    let page = sheet.page();
    // DLG-TST-01
    let inputs = group::group("");
    let link = adw::EntryRow::builder()
        .title("Link")
        .input_purpose(gtk::InputPurpose::Url)
        .build();
    inputs.add(&link);
    let (source, source_ids) = source_row(apps);
    inputs.add(&source);
    let held = ModifierChooser::new();
    inputs.add(&held.add_row("Held keys", ""));
    page.add(&inputs);

    let steps = steps_group(page);
    let skip_network = skip_network_bar(sheet);
    Fields {
        link,
        source,
        source_ids,
        held,
        skip_network,
        steps: steps.list,
        waiting: steps.waiting,
        waiting_text: steps.waiting_text,
        error: steps.error,
        labels: gtk::SizeGroup::new(gtk::SizeGroupMode::Horizontal),
    }
}

/// DLG-TST-01: the Source app popup, "None" then the installed apps by
/// name, and the desktop ID of each entry.
fn source_row(apps: &Apps) -> (adw::ComboRow, Vec<String>) {
    let mut entries = vec![("None".to_owned(), String::new())];
    let mut installed: Vec<(String, String)> = apps
        .get()
        .apps
        .into_iter()
        .map(|app| (app.name, app.id))
        .collect();
    installed.sort_by_key(|(name, _)| name.to_lowercase());
    entries.extend(installed);
    let names: Vec<&str> = entries.iter().map(|(name, _)| name.as_str()).collect();
    let source = adw::ComboRow::builder()
        .title("Source app")
        .model(&gtk::StringList::new(&names))
        .enable_search(true)
        .build();
    (source, entries.into_iter().map(|(_, id)| id).collect())
}

/// The Steps card's widgets.
struct Steps {
    list: gtk::ListBox,
    waiting: gtk::Stack,
    waiting_text: gtk::Label,
    error: gtk::Label,
}

/// DLG-TST-02: the Steps card on `page`, with its placeholder (a line or a
/// spinner) and the error line under it.
fn steps_group(page: &adw::PreferencesPage) -> Steps {
    let steps_group = group::group("Steps");
    let steps = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    let waiting_text = gtk::Label::builder()
        .label(NO_LINK)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(["dimmed"])
        .build();
    let spinner = adw::Spinner::builder()
        .width_request(24)
        .height_request(24)
        .build();
    let waiting = gtk::Stack::builder()
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();
    waiting.add_named(&waiting_text, Some("text"));
    waiting.add_named(&spinner, Some("busy"));
    steps.set_placeholder(Some(&waiting));
    steps_group.add(&steps);
    let error = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .visible(false)
        .margin_top(8)
        .css_classes(["error"])
        .build();
    steps_group.add(&error);
    page.add(&steps_group);
    Steps {
        list: steps,
        waiting,
        waiting_text,
        error,
    }
}

/// DLG-TST-02: the Skip network switch in a bar pinned under the sheet.
fn skip_network_bar(sheet: &Sheet) -> gtk::Switch {
    // DLG-TST-02: Skip network sits at the bottom, beside nothing, so the
    // steps keep the room.
    let skip_network = gtk::Switch::builder().valign(gtk::Align::Center).build();
    skip_network.update_property(&[gtk::accessible::Property::Label("Skip network")]);
    let skip_label = gtk::Label::builder()
        .label("Skip network")
        .mnemonic_widget(&skip_network)
        .build();
    let bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .tooltip_text("Do not ask short-link services where a link leads.")
        .css_classes(["toolbar", "wye-sheet-footer"])
        .build();
    bar.append(&skip_network);
    bar.append(&skip_label);
    sheet.add_footer(&bar);
    skip_network
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn dlg_tst_01_the_context_goes_over_d_bus_as_is() {
        let context = tester::context("slack.desktop", &["Shift".to_owned()], true);
        let map = dict(&context);
        assert_eq!(map.len(), 4);
        assert!(dict(&json!(3)).is_empty());
        assert!(dict(&json!({"n": 1})).is_empty(), "numbers are not context");
    }
}
