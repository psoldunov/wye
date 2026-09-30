//! `TestLink`: how a link would be routed, step by step (IN-08, DLG-TST).

use serde::{Deserialize, Serialize};

use crate::TargetSpec;

/// The trace of one link through the pipeline.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LinkTrace {
    /// What each pipeline step did, in order.
    pub steps: Vec<TraceStep>,
    /// The outcome.
    pub decision: TraceDecision,
    /// Why the link was rejected, when it was (PIPE-02).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejected: Option<String>,
    /// The final target, when one was chosen.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<TargetSpec>,
    /// Display name of the final target.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_name: Option<String>,
    /// Options the target opens with.
    pub options: OpenOptions,
    /// The URL that would be opened.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_url: Option<String>,
    /// Index of the matching rule in the configuration, when a rule decided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_index: Option<u32>,
}

/// One pipeline step.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TraceStep {
    /// Step ID, for example `clean`, `expand`, `rule`, `web-app`.
    pub kind: String,
    /// What happened, in words.
    pub text: String,
    /// The URL after this step, when it changed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

wire_enum! {
    /// Where a traced link ends.
    #[derive(Default)]
    pub enum TraceDecision as "decision" {
        /// Opened in a target.
        #[default]
        Open = "open",
        /// Shown in the picker.
        Picker = "picker",
        /// Refused (PIPE-02).
        Rejected = "rejected",
    }
}

/// Options a target opens with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OpenOptions {
    /// Private window.
    pub private: bool,
    /// In the background.
    pub background: bool,
    /// New window.
    pub new_window: bool,
}
