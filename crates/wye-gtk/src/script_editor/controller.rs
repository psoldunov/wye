//! The editor's state and what it does: open a script (SCR-01), follow the
//! edits (SCR-07), run the test as the user types (SCR-04), save, revert and
//! reload (SCR-08), and ask before unsaved changes go (SCR-10). The calls to
//! the service are in [`super::calls`]; with a fixture none are made.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use adw::prelude::*;
use gtk::{gio, glib};
use wye_api::actions::ScriptScope;

use super::document::{self, Document, SourceChoice};
use super::frame::{Frame, action, page};
use super::opening::{self, Fixture, Opening};
use super::readiness::Readiness;
use super::result::{self, ResultView};
use super::unsaved::{self, Answer};
use crate::service::Subscription;

/// SCR-04: how long typing pauses before the test runs.
const RUN_DELAY: Duration = Duration::from_millis(300);

/// What follows a save the SCR-10 question asked for, or a Discard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum After {
    /// Close the window (its close button, Escape, Ctrl+W).
    Close,
    /// Open another script (a `ShowWindow` for a different scope).
    Open(String),
    /// Let the host quit (`Windows1.Quit`).
    Quit(Quit),
}

/// What quits the host.
type QuitHost = Box<dyn FnOnce()>;

/// The host's quit, held while the SCR-10 question waits; it runs once.
#[derive(Clone)]
pub struct Quit(Rc<Cell<Option<QuitHost>>>);

impl Quit {
    fn new(quit: QuitHost) -> Self {
        Self(Rc::new(Cell::new(Some(quit))))
    }

    fn run(&self) {
        if let Some(quit) = self.0.take() {
            quit();
        }
    }
}

impl std::fmt::Debug for Quit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Quit")
    }
}

impl PartialEq for Quit {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Quit {}

/// What the window shows, besides the widgets' own state.
#[derive(Debug, Default)]
pub struct State {
    pub scope: Option<ScriptScope>,
    pub rule_name: Option<String>,
    /// SCR-01's full title.
    pub title: String,
    pub document: Document,
    /// The self-test's data in place of the service.
    pub fixture: Option<Fixture>,
    pub result: ResultView,
    pub readiness: Readiness,
    pub test_url: String,
    /// The source app's desktop ID; empty for none.
    pub source_app: String,
    pub choices: Vec<SourceChoice>,
    /// A save is in flight.
    pub busy: bool,
    /// SCR-08: the file changed on disk.
    pub external_change: bool,
}

/// The window and its state.
pub struct Controller {
    pub(super) frame: Frame,
    pub(super) me: Weak<Self>,
    pub(super) state: RefCell<State>,
    /// Counts openings, so a late answer for an earlier script is dropped.
    pub(super) generation: Cell<u64>,
    /// Counts test runs, so a late answer never replaces a newer one.
    pub(super) run: Cell<u64>,
    /// Widgets are being filled by code, not by the user.
    filling: Cell<bool>,
    pending_run: RefCell<Option<glib::SourceId>>,
    /// SCR-10: answered, so the close that follows is not asked about again.
    close_confirmed: Cell<bool>,
    /// SCR-10: Save was chosen in the question; this follows once it went through.
    after_save: RefCell<Option<After>>,
    /// SCR-08: `ScriptFileChanged`, followed while the host runs.
    pub(super) subscription: RefCell<Option<Subscription>>,
    save_action: gio::SimpleAction,
    revert_action: gio::SimpleAction,
}

impl std::fmt::Debug for Controller {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Controller")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

/// `then` on the controller behind `me`, if it is still there.
fn on(me: &Weak<Controller>, then: impl Fn(&Controller) + 'static) -> impl Fn() + 'static {
    let me = me.clone();
    move || {
        if let Some(controller) = me.upgrade() {
            then(&controller);
        }
    }
}

impl Controller {
    pub fn new(app: &adw::Application) -> Rc<Self> {
        let controller = Rc::new_cyclic(|me: &Weak<Self>| {
            let frame = Frame::new(app);
            let save_action = frame.add_action(action::SAVE, on(me, Self::save));
            let revert_action = frame.add_action(action::REVERT, on(me, Self::reload));
            frame.add_action(action::RUN_TEST, on(me, Self::run_test));
            frame.add_action(
                action::REFERENCE,
                on(me, |this| {
                    let split = &this.frame.split;
                    split.set_show_sidebar(!split.shows_sidebar());
                }),
            );
            Self {
                frame,
                me: me.clone(),
                state: RefCell::default(),
                generation: Cell::new(0),
                run: Cell::new(0),
                filling: Cell::new(false),
                pending_run: RefCell::default(),
                close_confirmed: Cell::new(false),
                after_save: RefCell::default(),
                subscription: RefCell::default(),
                save_action,
                revert_action,
            }
        });
        controller.connect();
        controller
    }

    /// `then` on this controller later, if it is still there.
    pub(super) fn later(&self, then: impl Fn(&Self) + 'static) -> impl Fn() + 'static {
        on(&self.me, then)
    }

    fn connect(&self) {
        let frame = &self.frame;
        let edited = self.later(Self::edited);
        frame.code.buffer.connect_changed(move |_| edited());
        let link = self.later(Self::link_edited);
        frame.test.link.connect_changed(move |_| link());
        let source = self.later(Self::source_chosen);
        frame.test.source.connect_selected_notify(move |_| source());
        let reload = self.later(Self::reload);
        frame.external.connect_button_clicked(move |_| reload());
        let retry = self.later(Self::retry);
        frame.retry.connect_clicked(move |_| retry());
        let me = self.me.clone();
        frame.window.connect_close_request(move |_| {
            me.upgrade()
                .map_or(glib::Propagation::Proceed, |this| this.closing())
        });
        let me = self.me.clone();
        frame.test.follow_style(move || {
            me.upgrade()
                .map(|this| this.state.borrow().result.clone())
                .unwrap_or_default()
        });
    }

    /// `ShowWindow("script-editor", argument)`: show, raise and focus the
    /// window (SET-04) with the script `argument` names.
    pub fn show(&self, argument: &str) {
        let opening = match opening::parse(argument) {
            Ok(opening) => opening,
            Err(message) => {
                tracing::warn!(%message, "script editor: the argument is not valid");
                self.frame.window.present();
                return;
            }
        };
        let window = &self.frame.window;
        if opening.fixture.is_some() {
            // The self-test's next case: whatever the last one opened goes.
            if let Some(dialog) = window.visible_dialog() {
                dialog.force_close();
            }
        } else if window.is_visible() && self.state.borrow().scope.as_ref() == Some(&opening.scope)
        {
            // The script on screen: keep the edits, only raise it.
            window.present();
            return;
        } else if window.is_visible() && self.state.borrow().document.is_dirty() {
            window.present();
            self.ask(After::Open(argument.to_owned()));
            return;
        }
        self.open(opening);
        self.close_confirmed.set(false);
        window.present();
        self.apply_window_keys();
    }

    /// The self-test's `edit`, `reference` and `confirmClose`.
    fn apply_window_keys(&self) {
        let Some(fixture) = self.state.borrow().fixture.clone() else {
            return;
        };
        self.frame.split.set_show_sidebar(fixture.reference);
        if let Some(text) = &fixture.edit {
            // As the user would type it: an edit, with its undo step.
            self.frame.code.type_text(text);
        }
        if fixture.confirm_close {
            self.frame.window.close();
        }
    }

    fn open(&self, opening: Opening) {
        self.generation.set(self.generation.get() + 1);
        self.cancel_run();
        *self.after_save.borrow_mut() = None;
        let title = opening::title(&opening.scope, opening.rule_name.as_deref());
        {
            let mut state = self.state.borrow_mut();
            *state = State {
                scope: Some(opening.scope.clone()),
                rule_name: opening.rule_name.clone(),
                title,
                fixture: opening.fixture.clone(),
                // A changed source app stays; the link is the next one's.
                source_app: std::mem::take(&mut state.source_app),
                choices: std::mem::take(&mut state.choices),
                readiness: Readiness::waiting(),
                ..State::default()
            };
        }
        self.frame.error.set_revealed(false);
        self.show_title();
        self.show_result(ResultView::default());
        if let Some(fixture) = opening.fixture {
            self.load_fixture(&fixture);
        } else {
            self.frame.stack.set_visible_child_name(page::LOADING);
            self.load();
            self.fetch_context(&opening.scope, opening.rule_name.is_some());
            self.listen();
        }
    }

    /// The fixture in place of `GetScript`, `GetHistory` and `GetApps`.
    fn load_fixture(&self, fixture: &Fixture) {
        let url = fixture
            .test_url
            .clone()
            .unwrap_or_else(|| result::DEFAULT_TEST_URL.to_owned());
        self.set_test_url(&url);
        self.link_arrived();
        self.set_choices(document::source_choices(&fixture.apps));
        self.loaded(&fixture.source);
        self.state.borrow_mut().external_change = fixture.external_change;
        self.update_flags();
    }

    /// The script arrived: show it, and run the first test once the link is
    /// there too (SCR-04).
    pub(super) fn loaded(&self, text: &str) {
        self.state.borrow_mut().document = Document::loaded(text);
        self.fill(|| self.frame.code.load(text));
        self.frame.stack.set_visible_child_name(page::EDITOR);
        self.update_flags();
        // A reload (Revert, SCR-08) runs again on the text now shown.
        let ready = {
            let mut state = self.state.borrow_mut();
            state.readiness = state.readiness.with_script();
            state.readiness.is_ready()
        };
        if ready {
            self.run_test();
        }
    }

    /// The test link arrived (`GetHistory`, or the fixture's).
    pub(super) fn link_arrived(&self) {
        let ready = {
            let mut state = self.state.borrow_mut();
            let before = state.readiness;
            state.readiness = before.with_link();
            !before.is_ready() && state.readiness.is_ready()
        };
        if ready {
            self.run_test();
        }
    }

    /// The script could not be read: say why, with Try Again.
    pub(super) fn load_failed(&self, sentence: &str) {
        self.frame
            .failed
            .set_description(Some(&glib::markup_escape_text(sentence)));
        self.frame.stack.set_visible_child_name(page::FAILED);
    }

    fn retry(&self) {
        self.frame.stack.set_visible_child_name(page::LOADING);
        self.load();
    }

    /// Put `url` in the Link entry without it counting as typing.
    pub(super) fn set_test_url(&self, url: &str) {
        url.clone_into(&mut self.state.borrow_mut().test_url);
        self.fill(|| self.frame.test.link.set_text(url));
    }

    /// Offer `choices` in the Source App popup.
    pub(super) fn set_choices(&self, choices: Vec<SourceChoice>) {
        let chosen = self.state.borrow().source_app.clone();
        let now = self.fill(|| self.frame.test.set_choices(&choices, &chosen));
        let mut state = self.state.borrow_mut();
        state.source_app = now;
        state.choices = choices;
    }

    /// Run `work` on the widgets as code, not as the user.
    fn fill<R>(&self, work: impl FnOnce() -> R) -> R {
        let was = self.filling.replace(true);
        let result = work();
        self.filling.set(was);
        result
    }

    fn edited(&self) {
        if self.filling.get() {
            return;
        }
        let text = self.frame.code.text();
        {
            let mut state = self.state.borrow_mut();
            state.document = state.document.edited(&text);
        }
        self.update_flags();
        self.schedule_run();
    }

    fn link_edited(&self) {
        if self.filling.get() {
            return;
        }
        self.state.borrow_mut().test_url = self.frame.test.link.text().into();
        self.schedule_run();
    }

    fn source_chosen(&self) {
        if self.filling.get() {
            return;
        }
        let index = usize::try_from(self.frame.test.source.selected()).unwrap_or(0);
        {
            let mut state = self.state.borrow_mut();
            state.source_app = index
                .checked_sub(1)
                .and_then(|index| state.choices.get(index))
                .map(|choice| choice.id.clone())
                .unwrap_or_default();
        }
        self.schedule_run();
    }

    /// SCR-04: run the test once typing pauses.
    fn schedule_run(&self) {
        self.cancel_run();
        let me = self.me.clone();
        let id = glib::timeout_add_local_once(RUN_DELAY, move || {
            if let Some(this) = me.upgrade() {
                // It fired: nothing left to cancel.
                this.pending_run.borrow_mut().take();
                this.run_test();
            }
        });
        *self.pending_run.borrow_mut() = Some(id);
    }

    fn cancel_run(&self) {
        if let Some(id) = self.pending_run.borrow_mut().take() {
            id.remove();
        }
    }

    /// Run the script on the test link (SCR-04).
    pub(super) fn run_test(&self) {
        self.cancel_run();
        let run = self.run.get() + 1;
        self.run.set(run);
        let fixture_run = self.state.borrow().fixture.as_ref().map(|fixture| {
            fixture
                .run
                .as_ref()
                .map_or_else(ResultView::default, ResultView::of)
        });
        match fixture_run {
            Some(view) => self.show_result(view),
            None => self.request_run(run),
        }
    }

    /// Show a run's result, and mark its error line (SCR-05).
    pub(super) fn show_result(&self, view: ResultView) {
        self.frame.test.show(&view);
        self.frame.code.mark_error(view.error_line);
        self.state.borrow_mut().result = view;
        self.update_flags();
    }

    /// Save the script (SCR-07).
    fn save(&self) {
        if !self.can_save() {
            return;
        }
        if self.state.borrow().fixture.is_some() {
            self.saved();
            return;
        }
        self.state.borrow_mut().busy = true;
        self.update_flags();
        self.request_save();
    }

    /// The save went through.
    pub(super) fn saved(&self) {
        {
            let mut state = self.state.borrow_mut();
            state.document = state.document.saved();
            state.external_change = false;
            state.busy = false;
        }
        self.update_flags();
        let after = self.after_save.borrow_mut().take();
        if let Some(after) = after {
            self.perform(after);
        }
    }

    /// The save failed: the window stays open (SCR-10), the reason shows.
    pub(super) fn save_failed(&self, sentence: Option<&str>) {
        self.state.borrow_mut().busy = false;
        *self.after_save.borrow_mut() = None;
        if let Some(sentence) = sentence {
            self.frame.error.set_title(sentence);
            self.frame.error.set_revealed(true);
        }
        self.update_flags();
    }

    /// Read the saved script again: Revert, or take the change made on disk
    /// (SCR-08).
    fn reload(&self) {
        self.state.borrow_mut().external_change = false;
        let fixture = self.state.borrow().fixture.clone();
        match fixture {
            Some(fixture) => self.load_fixture(&Fixture {
                external_change: false,
                ..fixture
            }),
            None => self.load(),
        }
        self.update_flags();
    }

    /// SCR-08: the script's file changed on disk.
    pub(super) fn changed_on_disk(&self) {
        self.state.borrow_mut().external_change = true;
        self.update_flags();
    }

    fn can_save(&self) -> bool {
        let state = self.state.borrow();
        state.document.can_save(state.result.syntax_error) && !state.busy
    }

    /// SCR-10 for `Windows1.Quit`: with unsaved changes on screen the
    /// question comes first, and the host quits after Save (once saved) or
    /// Discard; Cancel keeps it running.
    pub fn before_quit(&self, quit: Box<dyn FnOnce()>) {
        let dirty = self.frame.window.is_visible() && self.state.borrow().document.is_dirty();
        if !dirty {
            quit();
            return;
        }
        self.frame.window.present();
        self.ask(After::Quit(Quit::new(quit)));
    }

    /// SCR-10: closing with unsaved changes asks first.
    fn closing(&self) -> glib::Propagation {
        if self.close_confirmed.replace(false) || !self.state.borrow().document.is_dirty() {
            self.cancel_run();
            return glib::Propagation::Proceed;
        }
        self.ask(After::Close);
        glib::Propagation::Stop
    }

    fn ask(&self, after: After) {
        let (title, can_save) = (self.state.borrow().title.clone(), self.can_save());
        let me = self.me.clone();
        unsaved::ask(&self.frame.window, &title, can_save, move |answer| {
            let Some(this) = me.upgrade() else {
                return;
            };
            match answer {
                Answer::Save => {
                    *this.after_save.borrow_mut() = Some(after.clone());
                    this.save();
                }
                Answer::Discard => this.perform(after.clone()),
            }
        });
    }

    fn perform(&self, after: After) {
        match after {
            After::Close => {
                self.close_confirmed.set(true);
                self.frame.window.close();
            }
            After::Quit(quit) => quit.run(),
            After::Open(argument) => {
                // Answered: the edits go, the next script opens.
                let fresh = self.state.borrow().document.saved();
                self.state.borrow_mut().document = fresh;
                self.show(&argument);
            }
        }
    }

    /// The title, Save, Revert, the banner and the tooltip from the state.
    fn update_flags(&self) {
        let can_save = self.can_save();
        let state = self.state.borrow();
        let dirty = state.document.is_dirty();
        self.save_action.set_enabled(can_save);
        self.revert_action.set_enabled(dirty && !state.busy);
        let tooltip = if state.result.syntax_error {
            "Fix the syntax error to save"
        } else {
            "Save (Ctrl+S)"
        };
        self.frame.save.set_tooltip_text(Some(tooltip));
        self.frame.external.set_title(if dirty {
            "The script changed on disk. Reloading discards your changes."
        } else {
            "The script changed on disk."
        });
        self.frame.external.set_revealed(state.external_change);
        drop(state);
        self.show_title();
    }

    /// SCR-01 in the window's title; the scope below the name in the header,
    /// with "Edited" while there are unsaved changes.
    fn show_title(&self) {
        let state = self.state.borrow();
        let title = if state.title.is_empty() {
            "Transform Script"
        } else {
            &state.title
        };
        self.frame.window.set_title(Some(title));
        let (name, scope) = title.split_once(" \u{2014} ").unwrap_or((title, ""));
        self.frame.heading.set_title(name);
        let subtitle = if state.document.is_dirty() {
            format!("{scope} \u{b7} Edited")
        } else {
            scope.to_owned()
        };
        self.frame.heading.set_subtitle(&subtitle);
    }

    /// A rule's name arrived for the title (SCR-01).
    pub(super) fn named(&self, scope: &ScriptScope, name: String) {
        {
            let mut state = self.state.borrow_mut();
            if state.scope.as_ref() != Some(scope) {
                return;
            }
            state.title = opening::title(scope, Some(&name));
            state.rule_name = Some(name);
        }
        self.show_title();
    }
}
