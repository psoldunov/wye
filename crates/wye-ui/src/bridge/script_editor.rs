//! `ScriptEditorBackend`: the Rust side of the script editor
//! (16-script-editor.md, SCR-01 to SCR-09). It loads and saves the script
//! through `GetScript`/`SetScript`, test-runs it with `RunScript`, and
//! listens for `ScriptFileChanged`. The logic lives in
//! [`crate::script_editor`]; this file converts types and calls the service.
//!
//! Used from `qml/script/`.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, title)]
        #[qproperty(bool, busy)]
        #[qproperty(QString, error)]
        #[qproperty(bool, dirty)]
        #[qproperty(bool, can_save, cxx_name = "canSave")]
        #[qproperty(bool, external_change, cxx_name = "externalChange")]
        #[qproperty(QString, status)]
        #[qproperty(QString, segments_json, cxx_name = "segmentsJson")]
        #[qproperty(QString, result_message, cxx_name = "resultMessage")]
        #[qproperty(i32, error_line, cxx_name = "errorLine")]
        #[qproperty(bool, syntax_error, cxx_name = "syntaxError")]
        #[qproperty(QString, result_time, cxx_name = "resultTime")]
        #[qproperty(QString, logs)]
        #[qproperty(QString, test_url, cxx_name = "testUrl")]
        #[qproperty(QString, source_app, cxx_name = "sourceApp")]
        #[qproperty(QString, sources_json, cxx_name = "sourcesJson")]
        type ScriptEditorBackend = super::ScriptEditorBackendRust;

        /// Open the script `argument` names (`ShowWindow`'s argument).
        #[qinvokable]
        fn open(self: Pin<&mut Self>, argument: &QString);

        /// The editor's text changed.
        #[qinvokable]
        fn edit(self: Pin<&mut Self>, text: &QString);

        /// Run the script on the test link (SCR-04).
        #[qinvokable]
        #[cxx_name = "runTest"]
        fn run_test(self: Pin<&mut Self>);

        /// Save the script (SCR-07).
        #[qinvokable]
        fn save(self: Pin<&mut Self>);

        /// Read the file again: Revert, or accept a change made elsewhere
        /// (SCR-08).
        #[qinvokable]
        fn reload(self: Pin<&mut Self>);

        /// The bracket matching the one at `position` (or just before it),
        /// or -1 (SCR-02).
        #[qinvokable]
        #[cxx_name = "matchingBracket"]
        fn matching_bracket(self: &Self, text: &QString, position: i32) -> i32;

        /// What Return inserts at `position`: newline and indentation
        /// (SCR-02).
        #[qinvokable]
        #[cxx_name = "newlineAt"]
        fn newline_at(self: &Self, text: &QString, position: i32) -> QString;

        /// What Tab inserts.
        #[qinvokable]
        #[cxx_name = "indentText"]
        fn indent_text(self: &Self) -> QString;

        /// The editor must show `text` (after a load or a reload).
        #[qsignal]
        #[cxx_name = "sourceLoaded"]
        fn source_loaded(self: Pin<&mut Self>, text: QString);
    }

    impl cxx_qt::Threading for ScriptEditorBackend {}
}

use core::pin::Pin;
use std::collections::HashMap;

use cxx_qt::{CxxQtType as _, Threading as _};
use cxx_qt_lib::QString;
use futures_lite::StreamExt as _;
use wye_api::Error;
use wye_api::actions::ScriptScope;
use wye_api::context as keys;
use wye_api::proxy::Wye1Proxy;
use wye_api::scripts::ScriptRun;
use zbus::proxy::CacheProperties;
use zbus::zvariant::Value;

use crate::script_editor::document::{self, Document};
use crate::script_editor::editing;
use crate::script_editor::opening::{self, Fixture};
use crate::script_editor::readiness::Readiness;
use crate::script_editor::result::{self, ResultView};
use crate::service;

/// What the editor keeps besides its properties.
#[derive(Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one field per boolean Q_PROPERTY QML binds to"
)]
pub struct ScriptEditorBackendRust {
    title: QString,
    busy: bool,
    error: QString,
    dirty: bool,
    can_save: bool,
    external_change: bool,
    status: QString,
    segments_json: QString,
    result_message: QString,
    error_line: i32,
    syntax_error: bool,
    result_time: QString,
    logs: QString,
    test_url: QString,
    source_app: QString,
    sources_json: QString,
    scope: Option<ScriptScope>,
    rule_name: Option<String>,
    document: Document,
    /// Set while the self-test's data stands in for the service.
    fixture: Option<Fixture>,
    /// Numbers test runs, so a late answer never overwrites a newer one.
    run: u64,
    /// Whether the script and the test link have arrived for the first run.
    readiness: Readiness,
    /// A `ScriptFileChanged` subscription is running.
    listening: bool,
}

fn qs(text: &str) -> QString {
    QString::from(text)
}

impl qobject::ScriptEditorBackend {
    /// See the bridge declaration.
    pub fn open(mut self: Pin<&mut Self>, argument: &QString) {
        let opening = match opening::parse(&argument.to_string()) {
            Ok(opening) => opening,
            Err(message) => {
                self.set_error(qs(&message));
                return;
            }
        };
        let title = opening::title(&opening.scope, opening.rule_name.as_deref());
        self.as_mut().set_title(qs(&title));
        self.as_mut().set_error(QString::default());
        self.as_mut().set_external_change(false);
        self.as_mut().show_result(&ResultView::default());
        {
            let mut rust = self.as_mut().rust_mut();
            let rust = rust.as_mut().get_mut();
            rust.scope = Some(opening.scope.clone());
            rust.rule_name.clone_from(&opening.rule_name);
            rust.document = Document::default();
            rust.fixture.clone_from(&opening.fixture);
            rust.readiness = Readiness::waiting();
        }
        if let Some(fixture) = opening.fixture {
            self.load_fixture(&fixture);
        } else {
            self.as_mut().load();
            self.as_mut()
                .fetch_context(&opening.scope, opening.rule_name.is_some());
            self.listen();
        }
    }

    /// See the bridge declaration.
    pub fn edit(mut self: Pin<&mut Self>, text: &QString) {
        let edited = self.document.edited(&text.to_string());
        self.as_mut().rust_mut().get_mut().document = edited;
        self.update_flags();
    }

    /// See the bridge declaration.
    pub fn run_test(mut self: Pin<&mut Self>) {
        let run = self.run + 1;
        self.as_mut().rust_mut().get_mut().run = run;
        if let Some(fixture) = &self.fixture {
            let view = fixture
                .run
                .as_ref()
                .map_or_else(ResultView::default, ResultView::of);
            self.show_result(&view);
            return;
        }
        let source = self.document.text().to_owned();
        let url = self.test_url.to_string();
        let app = self.source_app.to_string();
        let rule = self.rule_name.clone();
        service::request(
            self.qt_thread(),
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
            move |backend, answer| {
                if backend.run != run {
                    return;
                }
                let view = match answer {
                    Ok(json) => match wye_api::json::decode::<ScriptRun>("ScriptRun", &json) {
                        Ok(run) => ResultView::of(&run),
                        Err(error) => ResultView::failed(error.to_string()),
                    },
                    Err(Error::InvalidArgs(message)) => ResultView::failed(message),
                    Err(error) => ResultView::failed(error.to_string()),
                };
                backend.show_result(&view);
            },
        );
    }

    /// See the bridge declaration.
    pub fn save(mut self: Pin<&mut Self>) {
        if !self.can_save {
            return;
        }
        let Some(scope) = self.scope.clone() else {
            return;
        };
        if self.fixture.is_some() {
            self.saved();
            return;
        }
        self.as_mut().set_busy(true);
        let text = self.document.text().to_owned();
        service::request(
            self.qt_thread(),
            move |proxy| async move { proxy.set_script(&scope.to_string(), &text).await },
            |mut backend, answer| {
                backend.as_mut().set_busy(false);
                match answer {
                    Ok(()) => backend.saved(),
                    Err(Error::ScriptSyntax(text)) => {
                        let (line, message) = result::parse_syntax_message(&text);
                        backend.show_result(&ResultView {
                            error_line: line,
                            syntax_error: true,
                            ..ResultView::failed(message)
                        });
                    }
                    Err(error) => backend.set_error(qs(&error.to_string())),
                }
            },
        );
    }

    /// See the bridge declaration.
    pub fn reload(mut self: Pin<&mut Self>) {
        self.as_mut().set_external_change(false);
        match self.fixture.clone() {
            Some(fixture) => self.load_fixture(&Fixture {
                external_change: false,
                ..fixture
            }),
            None => self.load(),
        }
    }

    /// See the bridge declaration.
    #[allow(
        clippy::unused_self,
        reason = "a QML invokable belongs to the object although the answer does not depend on its state"
    )]
    pub fn matching_bracket(&self, text: &QString, position: i32) -> i32 {
        usize::try_from(position)
            .ok()
            .and_then(|position| editing::matching_bracket(&text.to_string(), position))
            .and_then(|partner| i32::try_from(partner).ok())
            .unwrap_or(-1)
    }

    /// See the bridge declaration.
    #[allow(
        clippy::unused_self,
        reason = "a QML invokable belongs to the object although the answer does not depend on its state"
    )]
    pub fn newline_at(&self, text: &QString, position: i32) -> QString {
        let position = usize::try_from(position).unwrap_or(0);
        qs(&editing::newline_with_indent(&text.to_string(), position))
    }

    /// See the bridge declaration.
    #[allow(
        clippy::unused_self,
        reason = "a QML invokable belongs to the object although the answer does not depend on its state"
    )]
    pub fn indent_text(&self) -> QString {
        qs(editing::indent())
    }

    fn saved(mut self: Pin<&mut Self>) {
        let saved = self.document.saved();
        self.as_mut().rust_mut().get_mut().document = saved;
        self.as_mut().set_external_change(false);
        self.update_flags();
    }

    fn load_fixture(mut self: Pin<&mut Self>, fixture: &Fixture) {
        let url = fixture
            .test_url
            .clone()
            .unwrap_or_else(|| result::DEFAULT_TEST_URL.to_owned());
        self.as_mut().set_test_url(qs(&url));
        // The fixture sets its link synchronously.
        let ready = self.readiness.with_link();
        self.as_mut().rust_mut().get_mut().readiness = ready;
        let choices = document::source_choices(&fixture.apps);
        self.as_mut()
            .set_sources_json(qs(&document::choices_json(&choices)));
        self.as_mut().loaded(&fixture.source);
        self.as_mut().set_external_change(fixture.external_change);
    }

    /// `GetScript`, then a first test run.
    fn load(mut self: Pin<&mut Self>) {
        let Some(scope) = self.scope.clone() else {
            return;
        };
        self.as_mut().set_busy(true);
        service::request(
            self.qt_thread(),
            move |proxy| async move { proxy.get_script(&scope.to_string()).await },
            |mut backend, answer| {
                backend.as_mut().set_busy(false);
                match answer {
                    Ok(text) => backend.loaded(&text),
                    Err(error) => backend.set_error(qs(&error.to_string())),
                }
            },
        );
    }

    fn loaded(mut self: Pin<&mut Self>, text: &str) {
        self.as_mut().rust_mut().get_mut().document = Document::loaded(text);
        self.as_mut().source_loaded(qs(text));
        self.as_mut().update_flags();
        let ready = self.readiness.with_script();
        self.as_mut().rust_mut().get_mut().readiness = ready;
        // The first run waits for the test link too (SCR-04).
        if ready.is_ready() {
            self.run_test();
        }
    }

    /// The rule's name for the title, the last link for the test and the
    /// apps for the source popup (SCR-01, SCR-04).
    fn fetch_context(mut self: Pin<&mut Self>, scope: &ScriptScope, named: bool) {
        if let (ScriptScope::Rule(id), false) = (scope, named) {
            let id = id.clone();
            let scope = scope.clone();
            service::request(
                self.qt_thread(),
                |proxy| async move { proxy.get_config().await },
                move |backend, answer| {
                    let name = answer
                        .ok()
                        .and_then(|(json, _)| opening::rule_name(&json, &id));
                    if name.is_some() && backend.scope.as_ref() == Some(&scope) {
                        let title = opening::title(&scope, name.as_deref());
                        let mut backend = backend;
                        backend.as_mut().rust_mut().get_mut().rule_name = name;
                        backend.set_title(qs(&title));
                    }
                },
            );
        }
        self.as_mut().set_test_url(qs(&result::test_url(None)));
        service::request(
            self.qt_thread(),
            |proxy| async move { proxy.get_history().await },
            |mut backend, answer| {
                // A link the user already typed stays (SCR-04).
                if let Ok(json) = answer
                    && backend.test_url.to_string() == result::test_url(None)
                {
                    backend
                        .as_mut()
                        .set_test_url(qs(&result::test_url(Some(&json))));
                }
                let before = backend.readiness;
                let after = before.with_link();
                backend.as_mut().rust_mut().get_mut().readiness = after;
                if after.is_ready() && !before.is_ready() {
                    backend.run_test();
                }
            },
        );
        service::request(
            self.qt_thread(),
            |proxy| async move { proxy.get_apps(true).await },
            |backend, answer| {
                let apps = answer
                    .map(|json| document::apps_of(&json))
                    .unwrap_or_default();
                let choices = document::source_choices(&apps);
                backend.set_sources_json(qs(&document::choices_json(&choices)));
            },
        );
    }

    /// Follow `ScriptFileChanged` for the editor's lifetime and offer to
    /// reload when it names this editor's script (SCR-08). One subscription
    /// for the whole time, so no change falls between two.
    fn listen(mut self: Pin<&mut Self>) {
        if self.listening {
            return;
        }
        self.as_mut().rust_mut().get_mut().listening = true;
        service::follow(
            self.qt_thread(),
            |connection| async move {
                let proxy = Wye1Proxy::builder(&connection)
                    .cache_properties(CacheProperties::No)
                    .build()
                    .await?;
                let changes = proxy.receive_script_file_changed().await?;
                Ok(changes.filter_map(|signal| Some(signal.args().ok()?.scope.to_string())))
            },
            |mut backend, scope: String| {
                let ours = backend
                    .scope
                    .as_ref()
                    .is_some_and(|mine| mine.to_string() == scope);
                if ours {
                    backend.as_mut().set_external_change(true);
                }
            },
        );
    }

    fn show_result(mut self: Pin<&mut Self>, view: &ResultView) {
        self.as_mut().set_status(qs(view.status.name()));
        self.as_mut().set_segments_json(qs(&view.segments_json()));
        self.as_mut().set_result_message(qs(&view.message));
        self.as_mut()
            .set_error_line(i32::try_from(view.error_line).unwrap_or(0));
        self.as_mut().set_syntax_error(view.syntax_error);
        self.as_mut().set_result_time(qs(&view.time));
        self.as_mut().set_logs(qs(&view.logs));
        self.update_flags();
    }

    fn update_flags(mut self: Pin<&mut Self>) {
        let dirty = self.document.is_dirty();
        let can_save = self.document.can_save(self.syntax_error) && !self.busy;
        self.as_mut().set_dirty(dirty);
        self.set_can_save(can_save);
    }
}
