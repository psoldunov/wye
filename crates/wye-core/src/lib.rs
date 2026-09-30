//! Wye's link-routing core.
//!
//! Everything here is pure: no file, process or network access. The binary
//! and the desktop integration feed it configuration, the discovered apps and
//! what they know about the link, and it decides where the link opens. The
//! behaviour follows the specification in `docs/spec/`; requirement IDs such
//! as `PIPE-06` refer to it.

pub mod catalogue;
pub mod clean;
pub mod config;
pub mod expand;
mod host;
pub mod keys;
pub mod matcher;
pub mod normalize;
pub mod pipeline;
pub mod rule;
pub mod source;
pub mod target;

pub use catalogue::ServiceCatalogue;
pub use config::{Config, ConfigWarning, Loaded};
pub use keys::{Modifier, Modifiers};
pub use matcher::{MatcherKind, UrlMatcher};
pub use pipeline::{
    Decision, EntryPoint, Force, LinkRequest, OpenOptions, Pipeline, Rejected, Resolution,
    ScriptScope, Step,
};
pub use rule::{Rule, RunPosition};
pub use source::{SourceApp, SourceAppSpec};
pub use target::{Availability, CustomApp, DesktopId, Target};
