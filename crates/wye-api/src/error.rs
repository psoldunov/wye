//! The errors the service returns (`docs/dbus-api.md`, "Errors").
//!
//! Bad input is `org.freedesktop.DBus.Error.InvalidArgs` and anything else
//! that went wrong is `org.freedesktop.DBus.Error.Failed`; the rest are Wye's
//! own names under [`ERROR_PREFIX`].
//!
//! Written by hand rather than with zbus's `DBusError` derive: the derive puts
//! every variant under one prefix, and would report `InvalidArgs` and `Failed`
//! as `org.freedesktop.zbus.Error`.
//!
//! A client that declares its proxy methods with this error type gets these
//! variants back from a method error of the same name.

use std::fmt;

use zbus::DBusError;
use zbus::message::{Header, Message};
use zbus::names::ErrorName;

/// The prefix of Wye's own error names.
pub const ERROR_PREFIX: &str = "dev.soldunov.wye.Error";

const INVALID_ARGS: &str = "org.freedesktop.DBus.Error.InvalidArgs";
const FAILED: &str = "org.freedesktop.DBus.Error.Failed";
const READ_ONLY: &str = "dev.soldunov.wye.Error.ReadOnly";
const CONFLICT: &str = "dev.soldunov.wye.Error.Conflict";
const NOT_LOSSLESS: &str = "dev.soldunov.wye.Error.NotLossless";
const NOT_FOUND: &str = "dev.soldunov.wye.Error.NotFound";
const UNAVAILABLE: &str = "dev.soldunov.wye.Error.Unavailable";
const SCRIPT_SYNTAX: &str = "dev.soldunov.wye.Error.ScriptSyntax";
const NOT_IMPLEMENTED: &str = "dev.soldunov.wye.Error.NotImplemented";

/// A D-Bus error from the service. Each variant carries the message.
#[derive(Debug)]
pub enum Error {
    /// `org.freedesktop.DBus.Error.InvalidArgs`: the caller sent something
    /// the service refuses.
    InvalidArgs(String),
    /// `org.freedesktop.DBus.Error.Failed`: the service could not do it.
    Failed(String),
    /// The configuration file cannot be written (for example a read-only
    /// file from home-manager).
    ReadOnly(String),
    /// The caller's base revision is stale (SET-06).
    Conflict(String),
    /// Saving would drop values the configuration file contains.
    NotLossless(String),
    /// The thing asked for does not exist (a history entry, a URL on the
    /// clipboard, a pending picker request).
    NotFound(String),
    /// This session cannot do this (no portal, no data-control protocol).
    Unavailable(String),
    /// A script does not compile; the message is `line:column: text`
    /// (SCR-07).
    ScriptSyntax(String),
    /// Temporary: the member exists in the contract but is not implemented
    /// yet. Removed once every member is.
    NotImplemented(String),
    /// Any other error: a name outside this contract, or a transport
    /// failure. Replied as `Failed` unless it carries a name of its own.
    Bus(zbus::Error),
}

impl Error {
    /// [`Error::InvalidArgs`].
    #[must_use]
    pub fn invalid_args(message: impl Into<String>) -> Self {
        Self::InvalidArgs(message.into())
    }

    /// [`Error::Failed`].
    #[must_use]
    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed(message.into())
    }

    /// [`Error::NotImplemented`] for one member of the contract.
    #[must_use]
    pub fn not_implemented(member: &str) -> Self {
        Self::NotImplemented(format!("{member} is not implemented yet"))
    }

    /// The variant for a known error name.
    fn from_name(name: &str, message: String) -> Option<Self> {
        let error = match name {
            INVALID_ARGS => Self::InvalidArgs(message),
            FAILED => Self::Failed(message),
            READ_ONLY => Self::ReadOnly(message),
            CONFLICT => Self::Conflict(message),
            NOT_LOSSLESS => Self::NotLossless(message),
            NOT_FOUND => Self::NotFound(message),
            UNAVAILABLE => Self::Unavailable(message),
            SCRIPT_SYNTAX => Self::ScriptSyntax(message),
            NOT_IMPLEMENTED => Self::NotImplemented(message),
            _ => return None,
        };
        Some(error)
    }

    /// The name, for a variant this crate defines.
    const fn own_name(&self) -> Option<&'static str> {
        match self {
            Self::InvalidArgs(_) => Some(INVALID_ARGS),
            Self::Failed(_) => Some(FAILED),
            Self::ReadOnly(_) => Some(READ_ONLY),
            Self::Conflict(_) => Some(CONFLICT),
            Self::NotLossless(_) => Some(NOT_LOSSLESS),
            Self::NotFound(_) => Some(NOT_FOUND),
            Self::Unavailable(_) => Some(UNAVAILABLE),
            Self::ScriptSyntax(_) => Some(SCRIPT_SYNTAX),
            Self::NotImplemented(_) => Some(NOT_IMPLEMENTED),
            Self::Bus(_) => None,
        }
    }

    /// The message, for a variant this crate defines.
    fn own_message(&self) -> Option<&str> {
        match self {
            Self::InvalidArgs(message)
            | Self::Failed(message)
            | Self::ReadOnly(message)
            | Self::Conflict(message)
            | Self::NotLossless(message)
            | Self::NotFound(message)
            | Self::Unavailable(message)
            | Self::ScriptSyntax(message)
            | Self::NotImplemented(message) => Some(message),
            Self::Bus(_) => None,
        }
    }
}

impl DBusError for Error {
    fn name(&self) -> ErrorName<'_> {
        match (self.own_name(), self) {
            (Some(name), _) => ErrorName::from_static_str_unchecked(name),
            (None, Self::Bus(zbus::Error::MethodError(name, _, _))) => name.inner().clone(),
            (None, Self::Bus(zbus::Error::FDO(error))) => error.name(),
            (None, _) => ErrorName::from_static_str_unchecked(FAILED),
        }
    }

    fn description(&self) -> Option<&str> {
        match (self.own_message(), self) {
            (Some(message), _) => Some(message),
            (None, Self::Bus(zbus::Error::MethodError(_, message, _))) => message.as_deref(),
            (None, Self::Bus(zbus::Error::FDO(error))) => error.description(),
            (None, _) => None,
        }
    }

    fn create_reply(&self, call: &Header<'_>) -> zbus::Result<Message> {
        let message = self
            .description()
            .map_or_else(|| self.to_string(), str::to_owned);
        Message::error(call, self.name())?.build(&(message,))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bus(error) => write!(formatter, "{error}"),
            _ => write!(
                formatter,
                "{}: {}",
                self.name(),
                self.description().unwrap_or_default()
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Bus(error) => Some(error),
            _ => None,
        }
    }
}

impl From<zbus::Error> for Error {
    fn from(error: zbus::Error) -> Self {
        let named = match &error {
            zbus::Error::MethodError(name, message, _) => {
                Self::from_name(name.as_str(), message.clone().unwrap_or_default())
            }
            zbus::Error::FDO(fdo) => Self::from_name(
                fdo.name().as_str(),
                fdo.description().unwrap_or_default().to_owned(),
            ),
            _ => None,
        };
        named.unwrap_or(Self::Bus(error))
    }
}

impl From<zbus::fdo::Error> for Error {
    fn from(error: zbus::fdo::Error) -> Self {
        Self::from(zbus::Error::FDO(Box::new(error)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wye_errors_carry_the_documented_names() {
        let cases = [
            (Error::ReadOnly(String::new()), "ReadOnly"),
            (Error::Conflict(String::new()), "Conflict"),
            (Error::NotLossless(String::new()), "NotLossless"),
            (Error::NotFound(String::new()), "NotFound"),
            (Error::Unavailable(String::new()), "Unavailable"),
            (Error::ScriptSyntax(String::new()), "ScriptSyntax"),
            (Error::not_implemented("OpenLink"), "NotImplemented"),
        ];
        for (error, suffix) in cases {
            assert_eq!(error.name().as_str(), format!("{ERROR_PREFIX}.{suffix}"));
        }
    }

    #[test]
    fn standard_errors_keep_their_freedesktop_names() {
        assert_eq!(Error::invalid_args("bad").name().as_str(), INVALID_ARGS);
        assert_eq!(Error::failed("disk full").name().as_str(), FAILED);
    }

    #[test]
    fn the_not_implemented_message_names_the_member() {
        assert_eq!(
            Error::not_implemented("OpenLink").description(),
            Some("OpenLink is not implemented yet")
        );
    }

    #[test]
    fn a_standard_error_from_zbus_becomes_its_variant() {
        let error = Error::from(zbus::fdo::Error::InvalidArgs("no".into()));
        assert!(matches!(error, Error::InvalidArgs(ref message) if message == "no"));
    }

    #[test]
    fn a_transport_error_is_reported_as_failed() {
        let error = Error::from(zbus::Error::InvalidReply);
        assert!(matches!(error, Error::Bus(_)));
        assert_eq!(error.name().as_str(), FAILED);
    }

    #[test]
    fn display_shows_name_and_message() {
        assert_eq!(
            Error::NotFound("no URL on the clipboard".into()).to_string(),
            "dev.soldunov.wye.Error.NotFound: no URL on the clipboard"
        );
    }
}
