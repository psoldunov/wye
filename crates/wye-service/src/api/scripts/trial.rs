//! `RunScript`: the editor's test run (SCR-04). The context keys are
//! `OpenLink`'s (`docs/dbus-api.md`) plus `rule`, the rule name a per-rule
//! script sees as `context.rule`.

use std::str::FromStr;

use url::Url;
use wye_api::Error;
use wye_api::context as keys;
use wye_api::scripts::ScriptRun;
use wye_core::hooks::{RuleRef, TransformContext};
use wye_core::pipeline::EntryPoint;
use wye_core::{DesktopId, Modifier, Modifiers, SourceApp};
use wye_script::{Run, ScriptInput};
use zbus::zvariant::Value;

use crate::api::Dict;

/// `s`, `RunScript` only: the rule name `context.rule` shows.
pub(crate) const RULE: &str = "rule";

/// The link and context of one trial run.
///
/// # Errors
///
/// `InvalidArgs` for a link that does not parse, or a context value of the
/// wrong type or with an unknown spelling.
pub(crate) fn input(url: &str, context: &Dict) -> Result<(Url, ScriptInput), Error> {
    let url = Url::parse(url).map_err(|error| {
        Error::invalid_args(format!("\u{201c}{url}\u{201d} is not a link: {error}"))
    })?;
    let entry = match text(context, keys::ENTRY)? {
        Some(value) => entry_point(parse::<keys::Entry>(keys::ENTRY, value)?),
        None => EntryPoint::Handler,
    };
    let desktop_id = text(context, keys::SOURCE_DESKTOP_ID)?
        .map(|id| {
            DesktopId::new(id).map_err(|error| {
                Error::invalid_args(format!("{}: {error}", keys::SOURCE_DESKTOP_ID))
            })
        })
        .transpose()?;
    let source = SourceApp {
        desktop_id,
        executable: text(context, keys::SOURCE_EXECUTABLE)?.map(str::to_owned),
    };
    let rule_name = text(context, RULE)?;
    let transform = TransformContext {
        entry,
        source: &source,
        held: held(context)?,
        rule: rule_name.map(|name| RuleRef { id: None, name }),
    };
    Ok((url, ScriptInput::from_context(&transform)))
}

/// The run as `RunScript` answers it.
pub(crate) fn answer(url: &Url, run: Run) -> ScriptRun {
    let micros = u64::try_from(run.elapsed.as_micros()).unwrap_or(u64::MAX);
    match run.result {
        Ok(Some(returned)) => ScriptRun {
            ok: true,
            changed: wye_script::changed_ranges(url.as_str(), returned.as_str()),
            url: Some(returned.into()),
            micros,
            logs: run.logs,
            ..ScriptRun::default()
        },
        Ok(None) => ScriptRun {
            ok: true,
            micros,
            logs: run.logs,
            ..ScriptRun::default()
        },
        Err(error) => ScriptRun {
            ok: false,
            error: Some(error.message),
            line: error.line,
            micros,
            logs: run.logs,
            ..ScriptRun::default()
        },
    }
}

fn held(context: &Dict) -> Result<Modifiers, Error> {
    let Some(value) = context.get(keys::HELD) else {
        return Ok(Modifiers::NONE);
    };
    let Value::Array(names) = unwrap(value) else {
        return Err(wrong_type(keys::HELD, "as"));
    };
    let held: Vec<Modifier> = names
        .iter()
        .map(|name| match unwrap(name) {
            Value::Str(name) => parse::<Modifier>(keys::HELD, name.as_str()),
            _ => Err(wrong_type(keys::HELD, "as")),
        })
        .collect::<Result<_, _>>()?;
    Ok(Modifiers::from_slice(&held))
}

const fn entry_point(entry: keys::Entry) -> EntryPoint {
    match entry {
        keys::Entry::Handler => EntryPoint::Handler,
        keys::Entry::Clipboard => EntryPoint::Clipboard,
        keys::Entry::Extension => EntryPoint::Extension,
        keys::Entry::Cli => EntryPoint::Cli,
    }
}

fn text<'a>(context: &'a Dict, key: &str) -> Result<Option<&'a str>, Error> {
    match context.get(key).map(|value| unwrap(value)) {
        None => Ok(None),
        Some(Value::Str(text)) => Ok(Some(text.as_str())),
        Some(_) => Err(wrong_type(key, "s")),
    }
}

fn parse<T: FromStr>(key: &str, value: &str) -> Result<T, Error>
where
    T::Err: std::fmt::Display,
{
    value
        .parse()
        .map_err(|error| Error::invalid_args(format!("{key}: {error}")))
}

/// A value, looking through a variant wrapper.
fn unwrap<'a>(value: &'a Value<'a>) -> &'a Value<'a> {
    match value {
        Value::Value(inner) => unwrap(inner),
        other => other,
    }
}

fn wrong_type(key: &str, signature: &str) -> Error {
    Error::invalid_args(format!("{key} must be of type {signature}"))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use wye_core::hooks::ScriptError;
    use zbus::zvariant::OwnedValue;

    use super::*;

    fn dict(pairs: Vec<(&str, Value<'_>)>) -> Dict {
        pairs
            .into_iter()
            .map(|(key, value)| (key.to_owned(), OwnedValue::try_from(value).expect("owned")))
            .collect()
    }

    #[test]
    fn scr_04_the_context_reaches_the_script() {
        let context = dict(vec![
            (keys::ENTRY, Value::from("clipboard")),
            (keys::SOURCE_DESKTOP_ID, Value::from("org.kde.dolphin")),
            (keys::HELD, Value::from(vec!["Ctrl", "Shift"])),
            (RULE, Value::from("GitHub")),
        ]);
        let (url, input) = input("https://example.com/", &context).expect("valid");
        assert_eq!(url.as_str(), "https://example.com/");
        assert_eq!(input.entry_point, "clipboard");
        assert_eq!(input.source_app.as_deref(), Some("org.kde.dolphin.desktop"));
        assert_eq!(input.held_keys, ["Shift", "Ctrl"]);
        assert_eq!(input.rule.as_deref(), Some("GitHub"));
    }

    #[test]
    fn bad_input_is_invalid_args() {
        let empty = Dict::new();
        assert!(matches!(input("nope", &empty), Err(Error::InvalidArgs(_))));
        let wrong = dict(vec![(keys::ENTRY, Value::from(3_u32))]);
        assert!(matches!(
            input("https://a.example/", &wrong),
            Err(Error::InvalidArgs(_))
        ));
        let unknown = dict(vec![(keys::HELD, Value::from(vec!["Hyper"]))]);
        assert!(matches!(
            input("https://a.example/", &unknown),
            Err(Error::InvalidArgs(_))
        ));
    }

    #[test]
    fn scr_04_answers_carry_ranges_errors_and_time() {
        let url = Url::parse("https://twitter.com/a").expect("url");
        let changed = answer(
            &url,
            Run {
                result: Ok(Some(Url::parse("https://x.com/a").expect("url"))),
                logs: vec!["hi".into()],
                elapsed: Duration::from_micros(420),
            },
        );
        assert!(changed.ok);
        assert_eq!(changed.url.as_deref(), Some("https://x.com/a"));
        assert_eq!(changed.changed, [[8, 9]]);
        assert_eq!(changed.micros, 420);
        assert_eq!(changed.logs, ["hi"]);

        let failed = answer(
            &url,
            Run {
                result: Err(ScriptError::at_line("ReferenceError: x", 3)),
                logs: Vec::new(),
                elapsed: Duration::ZERO,
            },
        );
        assert!(!failed.ok);
        assert_eq!(failed.error.as_deref(), Some("ReferenceError: x"));
        assert_eq!(failed.line, Some(3));
        assert_eq!(failed.url, None);
    }
}
