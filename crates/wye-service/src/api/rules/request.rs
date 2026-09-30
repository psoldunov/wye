//! The link `TestLink` traces, from its `a{sv}` context (IN-08). The keys
//! are `OpenLink`'s (`docs/dbus-api.md`); nothing is probed or detected,
//! since the tester describes the situation it wants to see.

use std::str::FromStr;

use wye_api::context as keys;
use wye_api::{Error, UnknownValue};
use wye_core::pipeline::{EntryPoint, Force};
use wye_core::{DesktopId, LinkRequest, Modifier, Modifiers, SourceApp};
use zbus::zvariant::Value;

use crate::api::Dict;

/// `skip-network` (`b`): trace without contacting short-link services.
///
/// # Errors
///
/// `InvalidArgs` when the value is not a boolean.
pub(crate) fn skip_network(context: &Dict) -> Result<bool, Error> {
    Ok(flag(context, keys::SKIP_NETWORK)?.unwrap_or(false))
}

/// The request `TestLink` routes.
///
/// # Errors
///
/// `InvalidArgs` for a value of the wrong type or an unknown spelling.
pub(crate) fn from_context(url: &str, context: &Dict) -> Result<LinkRequest, Error> {
    let entry = match text(context, keys::ENTRY)? {
        Some(value) => entry_point(parse(keys::ENTRY, value)?),
        None => EntryPoint::Handler,
    };
    let force = match text(context, keys::FORCE)? {
        Some(value) => force(parse(keys::FORCE, value)?),
        None => Force::None,
    };
    let desktop_id = text(context, keys::SOURCE_DESKTOP_ID)?
        .map(|id| {
            DesktopId::new(id).map_err(|error| {
                Error::invalid_args(format!("{}: {error}", keys::SOURCE_DESKTOP_ID))
            })
        })
        .transpose()?;
    Ok(LinkRequest {
        source: SourceApp {
            desktop_id,
            executable: text(context, keys::SOURCE_EXECUTABLE)?.map(str::to_owned),
        },
        held: held(context)?,
        force,
        ..LinkRequest::new(url, entry)
    })
}

fn held(context: &Dict) -> Result<Modifiers, Error> {
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

fn unwrap<'a>(value: &'a Value<'a>) -> &'a Value<'a> {
    match value {
        Value::Value(inner) => unwrap(inner),
        other => other,
    }
}

fn parse<T: FromStr<Err = UnknownValue>>(key: &str, value: &str) -> Result<T, Error> {
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

const fn modifier(modifier: keys::Modifier) -> Modifier {
    match modifier {
        keys::Modifier::Shift => Modifier::Shift,
        keys::Modifier::Ctrl => Modifier::Ctrl,
        keys::Modifier::Alt => Modifier::Alt,
        keys::Modifier::Super => Modifier::Super,
    }
}
