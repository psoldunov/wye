//! What arrives with a link: the `a{sv}` context of `OpenLink` and the
//! `platform_data` of `org.freedesktop.Application.Open` (IN-01, IN-05,
//! IN-07, PIPE-01, LAUNCH-03).

use wye_api::Error;
use wye_api::context as keys;
use wye_core::{DesktopId, EntryPoint, Force, Modifier, Modifiers, SourceApp};
use zbus::zvariant::Value;

use crate::api::Dict;

/// Activation data for the launched app (LAUNCH-03).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Activation {
    /// Wayland `XDG_ACTIVATION_TOKEN`.
    pub token: Option<String>,
    /// X11 `DESKTOP_STARTUP_ID`.
    pub startup_id: Option<String>,
}

/// Everything a caller said about a link besides the URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Incoming {
    pub entry: EntryPoint,
    pub force: Force,
    /// The source app, when the caller named it.
    pub source: Option<SourceApp>,
    /// Where to start source-app detection, when the caller asked for it.
    pub source_pid: Option<u32>,
    /// Held modifiers, when the caller knows them.
    pub held: Option<Modifiers>,
    pub activation: Activation,
}

impl Default for Incoming {
    /// A plain handler link: nothing forced, nothing known.
    fn default() -> Self {
        Self {
            entry: EntryPoint::Handler,
            force: Force::None,
            source: None,
            source_pid: None,
            held: None,
            activation: Activation::default(),
        }
    }
}

impl Incoming {
    /// Parse `OpenLink`'s context ([`wye_api::context`]).
    ///
    /// # Errors
    ///
    /// `InvalidArgs` for a key with the wrong type or an unknown value.
    pub fn from_context(context: &Dict) -> Result<Self, Error> {
        let entry = match text(context, keys::ENTRY)? {
            Some(value) => entry_point(parse(keys::ENTRY, value)?),
            None => EntryPoint::Handler,
        };
        let force = match text(context, keys::FORCE)? {
            Some(value) => force(parse(keys::FORCE, value)?),
            None => Force::None,
        };
        let held = if flag(context, keys::HELD_KNOWN)?.unwrap_or(false) {
            Some(modifiers(context)?)
        } else {
            None
        };
        Ok(Self {
            entry,
            force,
            source: source(context)?,
            source_pid: number(context, keys::SOURCE_PID)?,
            held,
            activation: Activation {
                token: text(context, keys::ACTIVATION_TOKEN)?.map(str::to_owned),
                startup_id: text(context, keys::STARTUP_ID)?.map(str::to_owned),
            },
        })
    }

    /// `platform_data` of `org.freedesktop.Application`: only the
    /// activation data; everything else about the link is detected. A value
    /// of the wrong type is ignored, since launchers are not Wye's clients.
    pub fn from_platform_data(data: &Dict) -> Self {
        let owned = |key| text(data, key).ok().flatten().map(str::to_owned);
        Self {
            activation: Activation {
                token: owned(keys::PLATFORM_ACTIVATION_TOKEN),
                startup_id: owned(keys::PLATFORM_STARTUP_ID),
            },
            ..Self::default()
        }
    }
}

fn source(context: &Dict) -> Result<Option<SourceApp>, Error> {
    let desktop_id = text(context, keys::SOURCE_DESKTOP_ID)?
        .map(|id| {
            DesktopId::new(id).map_err(|error| {
                Error::invalid_args(format!("{}: {error}", keys::SOURCE_DESKTOP_ID))
            })
        })
        .transpose()?;
    let executable = text(context, keys::SOURCE_EXECUTABLE)?.map(str::to_owned);
    Ok(
        (desktop_id.is_some() || executable.is_some()).then_some(SourceApp {
            desktop_id,
            executable,
        }),
    )
}

fn modifiers(context: &Dict) -> Result<Modifiers, Error> {
    let Some(value) = context.get(keys::HELD) else {
        return Ok(Modifiers::default());
    };
    let Value::Array(names) = unwrap(value) else {
        return Err(wrong_type(keys::HELD, "as"));
    };
    let held: Vec<Modifier> = names
        .iter()
        .map(|name| match unwrap(name) {
            Value::Str(name) => parse::<keys::Modifier>(keys::HELD, name.as_str()).map(modifier),
            _ => Err(wrong_type(keys::HELD, "as")),
        })
        .collect::<Result<_, _>>()?;
    Ok(Modifiers::from_slice(&held))
}

fn text<'a>(context: &'a Dict, key: &str) -> Result<Option<&'a str>, Error> {
    context
        .get(key)
        .map(|value| match unwrap(value) {
            Value::Str(text) => Ok(text.as_str()),
            _ => Err(wrong_type(key, "s")),
        })
        .transpose()
}

fn flag(context: &Dict, key: &str) -> Result<Option<bool>, Error> {
    context
        .get(key)
        .map(|value| match unwrap(value) {
            Value::Bool(flag) => Ok(*flag),
            _ => Err(wrong_type(key, "b")),
        })
        .transpose()
}

fn number(context: &Dict, key: &str) -> Result<Option<u32>, Error> {
    context
        .get(key)
        .map(|value| match unwrap(value) {
            Value::U32(number) => Ok(*number),
            _ => Err(wrong_type(key, "u")),
        })
        .transpose()
}

/// A variant inside a variant, as some bindings send them, read as the
/// inner value.
fn unwrap<'a>(value: &'a Value<'a>) -> &'a Value<'a> {
    match value {
        Value::Value(inner) => unwrap(inner),
        other => other,
    }
}

fn parse<T: std::str::FromStr<Err = wye_api::UnknownValue>>(
    key: &str,
    value: &str,
) -> Result<T, Error> {
    value
        .parse()
        .map_err(|error| Error::invalid_args(format!("{key}: {error}")))
}

fn wrong_type(key: &str, signature: &str) -> Error {
    Error::invalid_args(format!("{key} must be of type {signature}"))
}

const fn entry_point(entry: keys::Entry) -> EntryPoint {
    match entry {
        keys::Entry::Handler => EntryPoint::Handler,
        keys::Entry::Clipboard => EntryPoint::Clipboard,
        keys::Entry::Extension => EntryPoint::Extension,
        keys::Entry::Cli => EntryPoint::Cli,
    }
}

const fn force(force: keys::Force) -> Force {
    match force {
        keys::Force::None => Force::None,
        keys::Force::Picker => Force::Picker,
        keys::Force::Alternative => Force::Alternative,
    }
}

pub(crate) const fn modifier(modifier: keys::Modifier) -> Modifier {
    match modifier {
        keys::Modifier::Shift => Modifier::Shift,
        keys::Modifier::Ctrl => Modifier::Ctrl,
        keys::Modifier::Alt => Modifier::Alt,
        keys::Modifier::Super => Modifier::Super,
    }
}

#[cfg(test)]
mod tests {
    use zbus::zvariant::OwnedValue;

    use super::*;

    fn dict(entries: Vec<(&str, Value<'_>)>) -> Dict {
        entries
            .into_iter()
            .map(|(key, value)| {
                (
                    key.to_owned(),
                    OwnedValue::try_from(value).expect("owned value"),
                )
            })
            .collect()
    }

    #[test]
    fn an_empty_context_is_a_plain_handler_link() {
        assert_eq!(
            Incoming::from_context(&Dict::new()).expect("parsed"),
            Incoming::default()
        );
    }

    #[test]
    fn every_key_is_read() {
        let context = dict(vec![
            (keys::ENTRY, Value::from("extension")),
            (keys::FORCE, Value::from("alternative")),
            (
                keys::SOURCE_DESKTOP_ID,
                Value::from("org.example.Chat.desktop"),
            ),
            (keys::SOURCE_PID, Value::from(42_u32)),
            (keys::HELD, Value::from(vec!["Shift", "Ctrl"])),
            (keys::HELD_KNOWN, Value::from(true)),
            (keys::ACTIVATION_TOKEN, Value::from("token")),
            (keys::STARTUP_ID, Value::from("startup")),
        ]);
        let incoming = Incoming::from_context(&context).expect("parsed");
        assert_eq!(incoming.entry, EntryPoint::Extension);
        assert_eq!(incoming.force, Force::Alternative);
        assert_eq!(
            incoming.source.and_then(|source| source.desktop_id),
            DesktopId::new("org.example.Chat.desktop").ok()
        );
        assert_eq!(incoming.source_pid, Some(42));
        assert_eq!(
            incoming.held,
            Some(Modifiers::from_slice(&[Modifier::Shift, Modifier::Ctrl]))
        );
        assert_eq!(incoming.activation.token.as_deref(), Some("token"));
        assert_eq!(incoming.activation.startup_id.as_deref(), Some("startup"));
    }

    #[test]
    fn held_keys_count_only_when_known() {
        let context = dict(vec![(keys::HELD, Value::from(vec!["Shift"]))]);
        assert_eq!(Incoming::from_context(&context).expect("parsed").held, None);
    }

    #[test]
    fn bad_values_are_the_callers_mistake() {
        for context in [
            dict(vec![(keys::ENTRY, Value::from("sideways"))]),
            dict(vec![(keys::FORCE, Value::from(true))]),
            dict(vec![(keys::SOURCE_PID, Value::from("1"))]),
            dict(vec![
                (keys::HELD_KNOWN, Value::from(true)),
                (keys::HELD, Value::from(vec!["Hyper"])),
            ]),
            dict(vec![(
                keys::SOURCE_DESKTOP_ID,
                Value::from("not a desktop id"),
            )]),
        ] {
            let error = Incoming::from_context(&context).expect_err("rejected");
            assert!(matches!(error, Error::InvalidArgs(_)), "{error}");
        }
    }

    #[test]
    fn platform_data_carries_the_activation_token() {
        let data = dict(vec![
            (keys::PLATFORM_ACTIVATION_TOKEN, Value::from("wayland")),
            (keys::PLATFORM_STARTUP_ID, Value::from("x11")),
            ("unrelated", Value::from(1_u32)),
        ]);
        let incoming = Incoming::from_platform_data(&data);
        assert_eq!(
            incoming.activation,
            Activation {
                token: Some("wayland".into()),
                startup_id: Some("x11".into()),
            }
        );
    }
}
