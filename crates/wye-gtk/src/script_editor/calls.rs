//! The editor's calls to the service: `GetScript` and `SetScript` (SCR-07),
//! `RunScript` (SCR-04), `GetConfig` for a rule's name (SCR-01),
//! `GetHistory` for the test link and `GetApps` for the Source App popup
//! (SCR-04), and `ScriptFileChanged` (SCR-08). Each answer is dropped when
//! the editor has opened another script (or run again) meanwhile.

use std::collections::HashMap;
use std::rc::{Rc, Weak};

use futures_lite::StreamExt as _;
use wye_api::Error;
use wye_api::actions::ScriptScope;
use wye_api::context as keys;
use wye_api::proxy::Wye1Proxy;
use wye_api::scripts::ScriptRun;
use zbus::proxy::CacheProperties;
use zbus::zvariant::Value;

use super::controller::Controller;
use super::document;
use super::opening;
use super::result::{self, ResultView};
use crate::error_text;
use crate::service;

impl Controller {
    /// `GetScript`, then the first test run.
    pub(super) fn load(&self) {
        let Some(scope) = self.state.borrow().scope.clone() else {
            return;
        };
        let generation = self.generation.get();
        let me = self.me.clone();
        service::request(
            move |proxy| async move { proxy.get_script(&scope.to_string()).await },
            move |answer| {
                let Some(this) = alive(&me, generation) else {
                    return;
                };
                match answer {
                    Ok(text) => this.loaded(&text),
                    Err(error) => this.load_failed(&error_text::describe(&error).sentence()),
                }
            },
        );
    }

    /// The rule's name for the title, the last link for the test and the
    /// apps for the popup.
    pub(super) fn fetch_context(&self, scope: &ScriptScope, named: bool) {
        if let (ScriptScope::Rule(id), false) = (scope, named) {
            let (id, scope) = (id.clone(), scope.clone());
            let me = self.me.clone();
            service::request(
                |proxy| async move { proxy.get_config().await },
                move |answer| {
                    let name = answer
                        .ok()
                        .and_then(|(json, _)| opening::rule_name(&json, &id));
                    if let (Some(this), Some(name)) = (me.upgrade(), name) {
                        this.named(&scope, name);
                    }
                },
            );
        }
        self.set_test_url(&result::test_url(None));
        let generation = self.generation.get();
        let me = self.me.clone();
        service::request(
            |proxy| async move { proxy.get_history().await },
            move |answer| {
                let Some(this) = alive(&me, generation) else {
                    return;
                };
                // A link the user already typed stays (SCR-04).
                let untouched = this.state.borrow().test_url == result::test_url(None);
                if let (Ok(json), true) = (answer, untouched) {
                    this.set_test_url(&result::test_url(Some(&json)));
                }
                this.link_arrived();
            },
        );
        let me = self.me.clone();
        service::request(
            |proxy| async move { proxy.get_apps(true).await },
            move |answer| {
                let apps = answer
                    .map(|json| document::apps_of(&json))
                    .unwrap_or_default();
                if let Some(this) = me.upgrade() {
                    this.set_choices(document::source_choices(&apps));
                }
            },
        );
    }

    /// Follow `ScriptFileChanged` for as long as the host runs, so no change
    /// falls between two openings (SCR-08).
    pub(super) fn listen(&self) {
        if self.subscription.borrow().is_some() {
            return;
        }
        let me = self.me.clone();
        let subscription = service::follow(
            |connection| async move {
                let proxy = Wye1Proxy::builder(&connection)
                    .cache_properties(CacheProperties::No)
                    .build()
                    .await?;
                let changes = proxy.receive_script_file_changed().await?;
                Ok(changes.filter_map(|signal| Some(signal.args().ok()?.scope.to_string())))
            },
            move |scope: String| {
                let Some(this) = me.upgrade() else {
                    return;
                };
                let ours = this
                    .state
                    .borrow()
                    .scope
                    .as_ref()
                    .is_some_and(|mine| mine.to_string() == scope);
                if ours {
                    this.changed_on_disk();
                }
            },
        );
        *self.subscription.borrow_mut() = Some(subscription);
    }

    /// `RunScript` on the editor's text, link and source app; numbered
    /// `run`, so only the newest answer shows.
    pub(super) fn request_run(&self, run: u64) {
        let (source, url, app, rule) = {
            let state = self.state.borrow();
            (
                state.document.text().to_owned(),
                state.test_url.clone(),
                state.source_app.clone(),
                state.rule_name.clone(),
            )
        };
        let me = self.me.clone();
        service::request(
            move |proxy| async move {
                let mut context: HashMap<&str, Value<'_>> = HashMap::new();
                if !app.is_empty() {
                    context.insert(keys::SOURCE_DESKTOP_ID, Value::from(app.as_str()));
                }
                if let Some(rule) = &rule {
                    context.insert(keys::RULE, Value::from(rule.as_str()));
                }
                proxy.run_script(&source, &url, context).await
            },
            move |answer| {
                let Some(this) = me.upgrade().filter(|this| this.run.get() == run) else {
                    return;
                };
                this.show_result(view_of(answer));
            },
        );
    }

    /// `SetScript`; a syntax error the service finds shows as the result.
    pub(super) fn request_save(&self) {
        let (scope, text) = {
            let state = self.state.borrow();
            let Some(scope) = state.scope.clone() else {
                return;
            };
            (scope, state.document.text().to_owned())
        };
        let generation = self.generation.get();
        let me = self.me.clone();
        service::request(
            move |proxy| async move { proxy.set_script(&scope.to_string(), &text).await },
            move |answer| {
                let Some(this) = alive(&me, generation) else {
                    return;
                };
                match answer {
                    Ok(()) => this.saved(),
                    Err(Error::ScriptSyntax(text)) => {
                        let (line, message) = result::parse_syntax_message(&text);
                        this.save_failed(None);
                        this.show_result(ResultView {
                            error_line: line,
                            syntax_error: true,
                            ..ResultView::failed(message)
                        });
                    }
                    Err(error) => {
                        let sentence = format!(
                            "The script was not saved. {}",
                            error_text::describe(&error).sentence()
                        );
                        this.save_failed(Some(&sentence));
                    }
                }
            },
        );
    }
}

/// The controller behind `me`, if it still shows the script of `generation`.
fn alive(me: &Weak<Controller>, generation: u64) -> Option<Rc<Controller>> {
    me.upgrade()
        .filter(|this| this.generation.get() == generation)
}

/// What a `RunScript` answer shows.
fn view_of(answer: Result<String, Error>) -> ResultView {
    match answer {
        Ok(json) => match wye_api::json::decode::<ScriptRun>("ScriptRun", &json) {
            Ok(run) => ResultView::of(&run),
            Err(error) => ResultView::failed(error.to_string()),
        },
        Err(Error::InvalidArgs(message)) => ResultView::failed(message),
        Err(error) => ResultView::failed(error.to_string()),
    }
}
