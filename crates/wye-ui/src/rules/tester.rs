//! The rule tester's result (DLG-TST-02): `TestLink`'s trace as the rows
//! the sheet shows, and the context the tester sends.

use serde::Serialize;
use serde_json::{Map, Value, json};
use wye_api::context as keys;
use wye_api::trace::{LinkTrace, OpenOptions};

/// One step row: a short label and what happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepView {
    pub label: String,
    pub text: String,
}

/// What the sheet shows below the inputs.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceView {
    pub steps: Vec<StepView>,
    /// `open`, `picker` or `rejected`.
    pub decision: String,
    pub rejected: String,
    /// The final target in its configuration shape, or null.
    pub target: Value,
    pub target_name: String,
    /// "Private window · In the background", or empty.
    pub options: String,
    pub final_url: String,
    /// The matched rule's position, or -1 (DLG-TST-03).
    pub rule_index: i64,
}

/// The view of `TestLink`'s JSON.
///
/// # Errors
///
/// When the text is not a trace.
pub fn view(trace_json: &str) -> Result<TraceView, String> {
    let trace: LinkTrace =
        wye_api::json::decode("LinkTrace", trace_json).map_err(|error| error.to_string())?;
    Ok(TraceView {
        steps: trace
            .steps
            .iter()
            .map(|step| StepView {
                label: label(&step.kind).to_owned(),
                text: step.text.clone(),
            })
            .collect(),
        decision: trace.decision.to_string(),
        rejected: trace.rejected.unwrap_or_default(),
        target: trace
            .target
            .and_then(|target| serde_json::to_value(target).ok())
            .unwrap_or(Value::Null),
        target_name: trace.target_name.unwrap_or_default(),
        options: options(trace.options),
        final_url: trace.final_url.unwrap_or_default(),
        rule_index: trace.rule_index.map_or(-1, i64::from),
    })
}

/// The row label of a step kind (17-dialogs.md, "Rule tester").
fn label(kind: &str) -> &'static str {
    match kind {
        "unwrap" | "expand" => "Expanded",
        "clean" => "Cleaned",
        "https" => "HTTPS",
        "script" => "Transformed",
        "alternative-key" => "Held keys",
        "rule" | "web-app" => "Matched",
        "fallback" | "default" => "Fallback",
        "missing" => "Missing",
        "forced-picker" | "picker" => "Picker",
        "locked" => "Locked",
        _ => "Step",
    }
}

/// The options a target opens with, in words.
fn options(options: OpenOptions) -> String {
    [
        (options.private, "Private window"),
        (options.background, "In the background"),
        (options.new_window, "New window"),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .map(|(_, text)| text)
    .collect::<Vec<_>>()
    .join(" \u{b7} ")
}

/// `TestLink`'s context (DLG-TST-01) as JSON: the source app, the held keys
/// (known, so nothing is probed) and `skip-network`.
#[must_use]
pub fn context(source_app: &str, held: &[String], skip_network: bool) -> Value {
    let mut context = Map::new();
    if !source_app.is_empty() {
        context.insert(keys::SOURCE_DESKTOP_ID.to_owned(), json!(source_app));
    }
    context.insert(keys::HELD.to_owned(), json!(held));
    context.insert(keys::HELD_KNOWN.to_owned(), json!(true));
    context.insert(keys::SKIP_NETWORK.to_owned(), json!(skip_network));
    Value::Object(context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dlg_tst_02_steps_read_as_the_spec_shows() {
        let trace = json!({
            "steps": [
                {"kind": "expand", "text": "Expanded bit.ly", "url": "https://github.com/x?utm_source=y"},
                {"kind": "clean", "text": "Removed utm_source"},
                {"kind": "rule", "text": "Matched rule 1 \"GitHub\""}
            ],
            "decision": "open",
            "target": {"app": "firefox.desktop"},
            "targetName": "Firefox",
            "options": {"private": true, "newWindow": true},
            "finalUrl": "https://github.com/x",
            "ruleIndex": 0
        })
        .to_string();
        let view = view(&trace).expect("a trace");
        let labels: Vec<_> = view.steps.iter().map(|step| step.label.as_str()).collect();
        assert_eq!(labels, ["Expanded", "Cleaned", "Matched"]);
        assert_eq!(view.options, "Private window \u{b7} New window");
        assert_eq!(view.rule_index, 0);
        assert_eq!(view.target, json!({"app": "firefox.desktop"}));
        assert!(super::view("nope").is_err());
    }

    #[test]
    fn dlg_tst_01_the_context_says_the_keys_are_known() {
        let context = context("com.slack.Slack.desktop", &["Shift".to_owned()], true);
        assert_eq!(context[keys::SOURCE_DESKTOP_ID], "com.slack.Slack.desktop");
        assert_eq!(context[keys::HELD], json!(["Shift"]));
        assert_eq!(context[keys::HELD_KNOWN], true);
        assert_eq!(context[keys::SKIP_NETWORK], true);
        assert!(
            super::context("", &[], false)
                .get(keys::SOURCE_DESKTOP_ID)
                .is_none()
        );
    }
}
