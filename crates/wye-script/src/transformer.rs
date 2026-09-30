//! The core's [`Transformer`] hook over this engine (PIPE-05, PIPE-14).

use url::Url;
use wye_core::ScriptScope;
use wye_core::hooks::{ScriptError, TransformContext, Transformer};

use crate::engine::{Run, ScriptInput, run_with_limits};
use crate::limits::Limits;

/// Where a scope's script comes from, and who hears how it went.
pub trait ScriptSource {
    /// The script for `scope`. `Ok(None)` when there is none (a missing or
    /// empty file): the link is kept.
    ///
    /// # Errors
    ///
    /// When the script exists but cannot be read.
    fn source(
        &self,
        scope: ScriptScope,
        context: &TransformContext<'_>,
    ) -> Result<Option<String>, ScriptError>;

    /// Called after every run, for logging `console.log` output and
    /// reporting failures (SCR-20, SCR-22). Does nothing by default.
    fn ran(&self, scope: ScriptScope, context: &TransformContext<'_>, source: &str, run: &Run) {
        let _ = (scope, context, source, run);
    }
}

/// Runs the scripts `S` provides.
#[derive(Debug, Clone)]
pub struct ScriptTransformer<S> {
    sources: S,
    limits: Limits,
}

impl<S: ScriptSource> ScriptTransformer<S> {
    /// With the default limits (SCR-21).
    #[must_use]
    pub fn new(sources: S) -> Self {
        Self::with_limits(sources, Limits::default())
    }

    #[must_use]
    pub const fn with_limits(sources: S, limits: Limits) -> Self {
        Self { sources, limits }
    }

    /// The script source.
    #[must_use]
    pub const fn sources(&self) -> &S {
        &self.sources
    }
}

impl<S: ScriptSource> Transformer for ScriptTransformer<S> {
    fn transform(
        &self,
        scope: ScriptScope,
        url: &Url,
        context: &TransformContext<'_>,
    ) -> Result<Option<Url>, ScriptError> {
        let Some(source) = self.sources.source(scope, context)? else {
            return Ok(None);
        };
        if source.trim().is_empty() {
            return Ok(None);
        }
        let run = run_with_limits(
            &source,
            url,
            &ScriptInput::from_context(context),
            self.limits,
        );
        self.sources.ran(scope, context, &source, &run);
        run.result
    }
}
