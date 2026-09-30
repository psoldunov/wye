//! Hooks the pipeline calls out to (PIPE-03, PIPE-05, PIPE-14).
//!
//! The core is pure, so the two things that need more than data are traits
//! the service implements: running a transform script and asking a short-link
//! service where a link leads. [`Hooks`] bundles them for
//! [`Pipeline::resolve_with`](crate::Pipeline::resolve_with) and
//! [`Pipeline::finish`](crate::Pipeline::finish). With no hooks, the pipeline
//! behaves as before: it records that scripts and network expansion did not
//! run and carries on.

use std::fmt;

use url::Url;

use crate::keys::{Modifier, Modifiers};
use crate::pipeline::{EntryPoint, ScriptScope};
use crate::source::SourceApp;

impl EntryPoint {
    /// The name the script API's `context.entryPoint` uses (SCR API).
    #[must_use]
    pub const fn script_name(self) -> &'static str {
        match self {
            Self::Handler => "handler",
            Self::Clipboard => "clipboard",
            Self::Extension => "extension",
            Self::Cli => "cli",
        }
    }
}

/// The rule a per-rule script belongs to (RUL-25).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleRef<'a> {
    /// The rule's stable ID, which names `rules/<id>.js`. `None` when the
    /// rule has not been given one yet.
    pub id: Option<&'a str>,
    pub name: &'a str,
}

/// What a script is told about the link besides the link itself
/// (16-script-editor.md, "Script API").
#[derive(Debug, Clone, Copy)]
pub struct TransformContext<'a> {
    pub entry: EntryPoint,
    pub source: &'a SourceApp,
    pub held: Modifiers,
    /// Set for per-rule scripts only.
    pub rule: Option<RuleRef<'a>>,
}

impl TransformContext<'_> {
    /// `context.sourceApp`: the source app's desktop ID, else its executable
    /// name, else nothing.
    #[must_use]
    pub fn source_app(&self) -> Option<String> {
        self.source
            .desktop_id
            .as_ref()
            .map(ToString::to_string)
            .or_else(|| self.source.executable.clone())
    }

    /// `context.heldKeys`, for example `["Ctrl", "Shift"]`.
    #[must_use]
    pub fn held_keys(&self) -> Vec<&'static str> {
        self.held.iter().map(Modifier::name).collect()
    }
}

/// A script failed (SCR-21 to SCR-23). The link continues unchanged and the
/// service notifies once per script (SCR-22).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptError {
    pub message: String,
    /// The line the error points at, counted from 1, when the engine knows.
    pub line: Option<u32>,
}

impl ScriptError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            line: None,
        }
    }

    #[must_use]
    pub fn at_line(message: impl Into<String>, line: u32) -> Self {
        Self {
            message: message.into(),
            line: Some(line),
        }
    }
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for ScriptError {}

/// Runs a transform script (PIPE-05 global, PIPE-14 per rule).
pub trait Transformer {
    /// Runs the script for `scope` over `url`.
    ///
    /// `Ok(None)` keeps the link. `Ok(Some(new))` replaces it; the pipeline
    /// rejects a replacement that is not an `http`/`https` link (SCR-23).
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when the script cannot run or fails.
    fn transform(
        &self,
        scope: ScriptScope,
        url: &Url,
        context: &TransformContext<'_>,
    ) -> Result<Option<Url>, ScriptError>;
}

/// The raw value of a `Location` header. It may be relative; the core
/// resolves it against the link that answered (DLG-EXP-03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location(String);

impl Location {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why one network hop failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    #[error("the short-link service did not answer in time")]
    Timeout,
    #[error("{0}")]
    Failed(String),
}

/// Sends the one request of a network expansion (PIPE-03, DLG-EXP-03).
///
/// An implementation does exactly one hop: `HEAD` (falling back to `GET`
/// without reading the body), no cookies, redirects off. The redirect loop,
/// its limits and what may be contacted belong to the core
/// ([`ExpansionCatalogue::expand_short_link`](crate::expand::ExpansionCatalogue::expand_short_link)).
/// The timeout is the service's: build the resolver per link with a deadline
/// for the whole chain.
pub trait ShortLinkResolver {
    /// Asks where `url` redirects to.
    ///
    /// `Ok(None)` means the answer was not a redirect, so `url` is the
    /// destination.
    ///
    /// # Errors
    ///
    /// Returns [`ResolveError`] on a timeout or a network failure.
    fn resolve(&self, url: &Url) -> Result<Option<Location>, ResolveError>;
}

/// The hooks a caller plugs into the pipeline. Every hook is optional; a
/// missing one is a no-op that the trace reports.
#[derive(Clone, Copy, Default)]
pub struct Hooks<'a> {
    transformer: Option<&'a dyn Transformer>,
    short_links: Option<&'a dyn ShortLinkResolver>,
}

impl<'a> Hooks<'a> {
    /// No scripts and no network expansion.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            transformer: None,
            short_links: None,
        }
    }

    #[must_use]
    pub const fn with_transformer(self, transformer: &'a dyn Transformer) -> Self {
        Self {
            transformer: Some(transformer),
            ..self
        }
    }

    #[must_use]
    pub const fn with_short_links(self, resolver: &'a dyn ShortLinkResolver) -> Self {
        Self {
            short_links: Some(resolver),
            ..self
        }
    }

    #[must_use]
    pub const fn transformer(&self) -> Option<&'a dyn Transformer> {
        self.transformer
    }

    #[must_use]
    pub const fn short_links(&self) -> Option<&'a dyn ShortLinkResolver> {
        self.short_links
    }
}

impl fmt::Debug for Hooks<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hooks")
            .field("transformer", &self.transformer.is_some())
            .field("short_links", &self.short_links.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::target::DesktopId;

    struct Keep;

    impl Transformer for Keep {
        fn transform(
            &self,
            _: ScriptScope,
            _: &Url,
            _: &TransformContext<'_>,
        ) -> Result<Option<Url>, ScriptError> {
            Ok(None)
        }
    }

    #[test]
    fn default_hooks_are_empty() {
        let hooks = Hooks::default();
        assert!(hooks.transformer().is_none());
        assert!(hooks.short_links().is_none());
        assert_eq!(
            format!("{:?}", Hooks::none()),
            "Hooks { transformer: false, short_links: false }"
        );
    }

    #[test]
    fn hooks_are_attached_independently() {
        let keep = Keep;
        let hooks = Hooks::none().with_transformer(&keep);
        assert!(hooks.transformer().is_some());
        assert!(hooks.short_links().is_none());
    }

    #[test]
    fn context_describes_the_link_for_scripts() {
        let source = SourceApp {
            desktop_id: Some(DesktopId::new("com.slack.Slack").unwrap()),
            executable: Some("slack".into()),
        };
        let context = TransformContext {
            entry: EntryPoint::Extension,
            source: &source,
            held: Modifiers::from_slice(&[Modifier::Ctrl, Modifier::Shift]),
            rule: None,
        };
        assert_eq!(
            context.source_app().as_deref(),
            Some("com.slack.Slack.desktop")
        );
        assert_eq!(context.held_keys(), ["Shift", "Ctrl"]);
        assert_eq!(context.entry.script_name(), "extension");

        let exe_only = SourceApp {
            desktop_id: None,
            executable: Some("slack".into()),
        };
        let context = TransformContext {
            source: &exe_only,
            ..context
        };
        assert_eq!(context.source_app().as_deref(), Some("slack"));
        let unknown = SourceApp::default();
        let context = TransformContext {
            source: &unknown,
            ..context
        };
        assert_eq!(context.source_app(), None);
    }

    #[test]
    fn entry_point_names_match_the_script_api() {
        let names: Vec<_> = [
            EntryPoint::Handler,
            EntryPoint::Clipboard,
            EntryPoint::Extension,
            EntryPoint::Cli,
        ]
        .into_iter()
        .map(EntryPoint::script_name)
        .collect();
        assert_eq!(names, ["handler", "clipboard", "extension", "cli"]);
    }

    #[test]
    fn script_errors_show_their_line() {
        assert_eq!(ScriptError::new("boom").to_string(), "boom");
        assert_eq!(
            ScriptError::at_line("unexpected token", 3).to_string(),
            "line 3: unexpected token"
        );
    }
}
