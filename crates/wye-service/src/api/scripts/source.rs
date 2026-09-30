//! The pipeline's scripts, read from disk for every link (PIPE-05,
//! PIPE-14), with `console.log` going to Wye's log (SCR-20) and failures
//! reported once (SCR-22).

use wye_api::actions::ScriptScope as ApiScope;
use wye_core::ScriptScope;
use wye_core::hooks::{ScriptError, TransformContext};
use wye_script::{Run, ScriptSource};

use super::failures::{self, Failure};
use super::files::ScriptFiles;
use crate::context::ServiceContext;

/// Reads `transform.js` and `rules/<id>.js`.
#[derive(Debug, Clone)]
pub(crate) struct FileScripts {
    files: ScriptFiles,
    /// Where failures are reported; `None` for trial runs (`TestLink`),
    /// which must not notify.
    reporter: Option<Reporter>,
}

#[derive(Debug, Clone)]
struct Reporter {
    ctx: ServiceContext,
    runtime: tokio::runtime::Handle,
}

impl FileScripts {
    /// Scripts that report failures through `ctx`. Reporting needs a Tokio
    /// runtime; outside one, failures are only logged.
    pub(crate) fn new(ctx: &ServiceContext, files: ScriptFiles) -> Self {
        let reporter = tokio::runtime::Handle::try_current()
            .ok()
            .map(|runtime| Reporter {
                ctx: ctx.clone(),
                runtime,
            });
        Self { files, reporter }
    }

    /// Scripts that only log their failures, for routing without the
    /// service.
    pub(crate) fn offline(files: ScriptFiles) -> Self {
        Self {
            files,
            reporter: None,
        }
    }

    /// The same scripts, without notifications.
    pub(crate) fn quiet(self) -> Self {
        Self {
            reporter: None,
            ..self
        }
    }
}

/// The API scope of the script the pipeline asks for.
fn api_scope(scope: ScriptScope, context: &TransformContext<'_>) -> Result<ApiScope, ScriptError> {
    match scope {
        ScriptScope::Global => Ok(ApiScope::Global),
        ScriptScope::Rule => context
            .rule
            .and_then(|rule| rule.id)
            .map(|id| ApiScope::Rule(id.to_owned()))
            .ok_or_else(|| {
                ScriptError::new(
                    "the rule has no ID, so it has no script file; save the rule again to give it one",
                )
            }),
    }
}

/// How the user knows the script.
fn describe(scope: ScriptScope, context: &TransformContext<'_>) -> String {
    match (scope, context.rule) {
        (ScriptScope::Rule, Some(rule)) => {
            format!("The transform script of rule \u{201c}{}\u{201d}", rule.name)
        }
        _ => "The global transform script".to_owned(),
    }
}

impl ScriptSource for FileScripts {
    fn source(
        &self,
        scope: ScriptScope,
        context: &TransformContext<'_>,
    ) -> Result<Option<String>, ScriptError> {
        let scope = api_scope(scope, context)?;
        self.files
            .read(&scope)
            .map_err(|error| ScriptError::new(error.to_string()))
    }

    fn ran(&self, scope: ScriptScope, context: &TransformContext<'_>, source: &str, run: &Run) {
        let label = api_scope(scope, context)
            .map_or_else(|_| scope.label().to_owned(), |scope| scope.to_string());
        for line in &run.logs {
            tracing::info!(script = %label, "console.log: {line}");
        }
        let Err(error) = &run.result else {
            return;
        };
        tracing::warn!(script = %label, %error, "transform script failed");
        let Some(reporter) = &self.reporter else {
            return;
        };
        let failure = Failure {
            script: describe(scope, context),
            hash: failures::script_hash(&label, source),
            message: error.to_string(),
        };
        let ctx = reporter.ctx.clone();
        reporter
            .runtime
            .spawn(async move { failures::report(&ctx, failure).await });
    }
}

#[cfg(test)]
mod tests {
    use wye_core::hooks::RuleRef;
    use wye_core::pipeline::EntryPoint;
    use wye_core::{Modifiers, SourceApp};

    use super::*;

    fn context<'a>(source: &'a SourceApp, rule: Option<RuleRef<'a>>) -> TransformContext<'a> {
        TransformContext {
            entry: EntryPoint::Handler,
            source,
            held: Modifiers::NONE,
            rule,
        }
    }

    #[test]
    fn scopes_follow_the_rule_id() {
        let source = SourceApp::default();
        let rule = RuleRef {
            id: Some("gh"),
            name: "GitHub",
        };
        assert_eq!(
            api_scope(ScriptScope::Rule, &context(&source, Some(rule))),
            Ok(ApiScope::Rule("gh".into()))
        );
        assert_eq!(
            api_scope(ScriptScope::Global, &context(&source, None)),
            Ok(ApiScope::Global)
        );
        let unnamed = RuleRef { id: None, ..rule };
        assert!(api_scope(ScriptScope::Rule, &context(&source, Some(unnamed))).is_err());
        assert_eq!(
            describe(ScriptScope::Rule, &context(&source, Some(rule))),
            "The transform script of rule \u{201c}GitHub\u{201d}"
        );
    }

    /// Run the global script over a link, as the pipeline would.
    fn transform(scripts: &wye_script::ScriptTransformer<FileScripts>) {
        use wye_core::Transformer as _;
        let url = url::Url::parse("https://example.com/").expect("url");
        let source = SourceApp::default();
        let _outcome = scripts.transform(ScriptScope::Global, &url, &context(&source, None));
    }

    /// Scripts without the 50 ms limit (SCR-21), which a busy test machine
    /// can hit and which is not what these tests are about.
    fn patient(scripts: FileScripts) -> wye_script::ScriptTransformer<FileScripts> {
        let limits = wye_script::Limits {
            time: std::time::Duration::from_secs(10),
            ..wye_script::Limits::default()
        };
        wye_script::ScriptTransformer::with_limits(scripts, limits)
    }

    /// The notifications once `expected` arrived (or 5 s passed) and a
    /// quiet spell showed no more are coming.
    async fn shown_after_quiet(
        fakes: &crate::platform::fake::FakePlatform,
        expected: usize,
    ) -> usize {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        while fakes.notifier.shown().len() < expected && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        fakes.notifier.shown().len()
    }

    // SCR-22: one notification per failing script until it changes, and
    // none for trial runs.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn scr_22_failures_notify_once_per_script() {
        let home = tempfile::tempdir().expect("temp dir");
        let root = home.path().to_path_buf();
        let environment = crate::api::link::Environment::from_lookup(|name| match name {
            "HOME" => Some(root.clone().into_os_string()),
            "XDG_CONFIG_HOME" => Some(root.join("config").into_os_string()),
            "XDG_STATE_HOME" => Some(root.join("state").into_os_string()),
            _ => None,
        })
        .expect("home");
        let fakes = crate::platform::fake::FakePlatform::new();
        let ctx = ServiceContext::new(fakes.platform());
        ctx.set_environment(environment.clone());
        let files = ScriptFiles::beside(&environment.config);
        let failing = ApiScope::Global;
        files
            .write(&failing, "export default function transform() { nope(); }")
            .expect("written");
        let scripts = patient(FileScripts::new(&ctx, files.clone()));

        transform(&scripts);
        transform(&scripts);
        assert_eq!(shown_after_quiet(&fakes, 1).await, 1);
        let shown = fakes.notifier.shown();
        assert!(
            shown[0].body.contains("The global transform script failed"),
            "{:?}",
            shown[0]
        );
        assert!(shown[0].body.contains("ReferenceError"), "{:?}", shown[0]);

        let quiet = patient(FileScripts::new(&ctx, files.clone()).quiet());
        files
            .write(&failing, "export default function transform() { again(); }")
            .expect("written");
        transform(&quiet);
        assert_eq!(shown_after_quiet(&fakes, 1).await, 1);

        transform(&scripts);
        assert_eq!(shown_after_quiet(&fakes, 2).await, 2);
        let state = wye_desktop::State::load(&environment.state).expect("state");
        assert_eq!(state.script_errors_notified.len(), 2);
    }
}
