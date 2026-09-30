//! Wye's transform-script engine (16-script-editor.md).
//!
//! Scripts are JavaScript ES modules run by `QuickJS` through `rquickjs`.
//! The module's `default` export receives a mutable `URL` and a context
//! object and returns a new link, or nothing to keep it.
//!
//! - [`run`] / [`run_with_limits`]: one run, with its `console.log` output
//!   and run time (SCR-04, SCR-20, SCR-21, SCR-23).
//! - [`check`]: compile without running, for `SetScript` (SCR-07).
//! - [`ScriptTransformer`]: the core's
//!   [`Transformer`](wye_core::hooks::Transformer) hook (PIPE-05, PIPE-14).
//! - [`changed_ranges`]: what a script changed, for the editor (SCR-04).
//! - [`TEMPLATE`]: a new script's starting text (SCR-03).
//!
//! The engine has no IO: the caller reads script files.

mod diff;
mod engine;
mod error;
mod limits;
mod prelude;
mod transformer;

pub use diff::changed_ranges;
pub use engine::{Run, ScriptInput, check, run, run_with_limits};
pub use error::SyntaxError;
pub use limits::{Limits, MAX_LOG_CHARS, MAX_LOG_LINES, MEMORY_LIMIT, STACK_LIMIT, TIME_LIMIT};
pub use transformer::{ScriptSource, ScriptTransformer};

/// A new script's text (SCR-03): the signature and one example.
pub const TEMPLATE: &str = include_str!("template.js");
