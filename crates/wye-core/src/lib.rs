//! Wye's link-routing core.
//!
//! Everything here is pure: no file, process or network access. The binary
//! and the desktop integration feed it configuration, the discovered apps and
//! what they know about the link, and it decides where the link opens. The
//! behaviour follows the specification in `docs/spec/`; requirement IDs such
//! as `PIPE-06` refer to it.

pub mod catalogue;
pub mod clean;
pub mod clipboard;
pub mod config;
pub mod expand;
pub mod history;
pub mod hooks;
mod host;
pub mod keybinding;
pub mod keys;
pub mod link_text;
pub mod matcher;
pub mod merge_patch;
pub mod normalize;
pub mod picker;
pub mod pipeline;
pub mod rule;
pub mod rules_file;
pub mod sign_in;
pub mod source;
pub mod target;
pub mod target_menu;
pub mod tray;

pub use catalogue::ServiceCatalogue;
pub use config::{Config, ConfigWarning, Loaded};
pub use hooks::{Hooks, ShortLinkResolver, Transformer};
pub use keys::{Modifier, Modifiers};
pub use matcher::{MatcherKind, UrlMatcher};
pub use pipeline::{
    Chosen, Decision, EntryPoint, Finished, Force, LinkRequest, OpenOptions, Pipeline, Rejected,
    Resolution, ScriptScope, SkipReason, Step,
};
pub use rule::{Rule, RunPosition};
pub use source::{SourceApp, SourceAppSpec};
pub use target::{Availability, CustomApp, DesktopId, Target};
