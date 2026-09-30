//! Talking to the Wye service (`dev.soldunov.wye`) from the command line.
//!
//! Each call runs on a small single-threaded runtime and is bounded by
//! [`TIMEOUT`], which includes starting the service through D-Bus
//! activation. A caller tells an absent service ([`CallError::Unreachable`],
//! where the command may do the work itself) from a refusal
//! ([`CallError::Refused`], where the service already told the user) and
//! from a call that was sent but not answered ([`CallError::NoAnswer`]: the
//! service may still do it, so the work is never repeated here).

use std::future::Future;
use std::time::Duration;

use wye_api::Error;
use wye_api::proxy::{ApplicationProxy, Wye1Proxy};

/// How long one call may take, bus activation included.
pub const TIMEOUT: Duration = Duration::from_secs(3);

/// D-Bus errors that mean the call reached no service: nothing owns the
/// name and nothing could be started for it, or the service lacks the
/// member (an older one).
const ABSENT: [&str; 5] = [
    "org.freedesktop.DBus.Error.ServiceUnknown",
    "org.freedesktop.DBus.Error.NameHasNoOwner",
    "org.freedesktop.DBus.Error.UnknownMethod",
    "org.freedesktop.DBus.Error.UnknownObject",
    "org.freedesktop.DBus.Error.UnknownInterface",
];

/// Bus-activation failures (`Spawn.ChildExited`, `Spawn.ExecFailed`, …):
/// the service could not be started, so it never saw the call.
const SPAWN_ERRORS: &str = "org.freedesktop.DBus.Error.Spawn.";

/// Activation through systemd that failed (`NoSuchUnit`, `UnitMasked`, …),
/// which the bus passes on: the service never started.
const SYSTEMD_ERRORS: &str = "org.freedesktop.systemd1.";

/// The bus daemon itself: an error it sends means the call was never
/// delivered, except when it gave up waiting for the reply.
const BUS_DAEMON: &str = "org.freedesktop.DBus";

/// Bus-daemon errors that come after the call was delivered.
const DELIVERED: [&str; 2] = [
    "org.freedesktop.DBus.Error.NoReply",
    "org.freedesktop.DBus.Error.TimedOut",
];

/// Why a call to the service did not succeed.
#[derive(Debug)]
pub enum CallError {
    /// No service took the call: no session bus, no service installed or
    /// startable, or a member it does not implement.
    Unreachable(String),
    /// The call was sent, but no answer came in time, or the connection
    /// broke: the service may still act on it, so it must not be repeated.
    NoAnswer(String),
    /// The service answered with an error it stands by.
    Refused(Error),
}

impl std::fmt::Display for CallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable(reason) => {
                write!(formatter, "the Wye service is not reachable: {reason}")
            }
            Self::NoAnswer(reason) => write!(
                formatter,
                "the Wye service did not answer ({reason}); it may still do it"
            ),
            Self::Refused(error) => write!(formatter, "{}", message(error)),
        }
    }
}

/// A connection to the service's public interface.
pub struct Client {
    runtime: tokio::runtime::Runtime,
    wye: Wye1Proxy<'static>,
    application: ApplicationProxy<'static>,
}

impl Client {
    /// Connect to the session bus.
    ///
    /// # Errors
    ///
    /// [`CallError::Unreachable`] without a session bus.
    pub fn connect() -> Result<Self, CallError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| CallError::Unreachable(error.to_string()))?;
        let (wye, application) = runtime.block_on(bounded(async {
            let connection = zbus::Connection::session().await?;
            let wye = Wye1Proxy::new(&connection).await?;
            let application = ApplicationProxy::new(&connection).await?;
            Ok::<_, zbus::Error>((wye, application))
        }))??;
        Ok(Self {
            runtime,
            wye,
            application,
        })
    }

    /// Run `call` on the public interface.
    ///
    /// # Errors
    ///
    /// [`CallError`] as described there.
    pub fn call<T, F, Fut>(&self, call: F) -> Result<T, CallError>
    where
        F: FnOnce(Wye1Proxy<'static>) -> Fut,
        Fut: Future<Output = Result<T, Error>>,
    {
        let reply = self
            .runtime
            .block_on(async { tokio::time::timeout(TIMEOUT, call(self.wye.clone())).await })
            .map_err(|_| CallError::NoAnswer(format!("no answer within {TIMEOUT:?}")))?;
        reply.map_err(classify)
    }

    /// `org.freedesktop.Application.Activate`: Wye started without a link
    /// (TRAY-05).
    ///
    /// # Errors
    ///
    /// [`CallError`] as described there.
    pub fn activate(&self) -> Result<(), CallError> {
        let application = self.application.clone();
        let reply = self.runtime.block_on(bounded(async move {
            application
                .activate(std::collections::HashMap::default())
                .await
        }))?;
        reply.map_err(|error| classify(Error::from(error)))
    }
}

async fn bounded<T>(future: impl Future<Output = T>) -> Result<T, CallError> {
    tokio::time::timeout(TIMEOUT, future)
        .await
        .map_err(|_| CallError::Unreachable(format!("no answer within {TIMEOUT:?}")))
}

impl From<zbus::Error> for CallError {
    fn from(error: zbus::Error) -> Self {
        Self::Unreachable(error.to_string())
    }
}

/// Errors that mean "nobody here to do it" become [`CallError::Unreachable`];
/// other bus errors leave the outcome unknown ([`CallError::NoAnswer`]); the
/// rest are the service's answer.
fn classify(error: Error) -> CallError {
    match error {
        Error::NotImplemented(reason) | Error::Unavailable(reason) => {
            CallError::Unreachable(reason)
        }
        Error::Bus(error) if is_absent(&error) => CallError::Unreachable(error.to_string()),
        Error::Bus(error) => CallError::NoAnswer(error.to_string()),
        other => CallError::Refused(other),
    }
}

/// Whether `error` says the call reached no service.
fn is_absent(error: &zbus::Error) -> bool {
    use zbus::DBusError as _;
    let absent = |name: &str| {
        ABSENT.contains(&name) || name.starts_with(SPAWN_ERRORS) || name.starts_with(SYSTEMD_ERRORS)
    };
    match error {
        zbus::Error::MethodError(name, _, reply) => {
            let from_bus = reply
                .header()
                .sender()
                .is_some_and(|sender| sender.as_str() == BUS_DAEMON);
            absent(name.as_str()) || (from_bus && !DELIVERED.contains(&name.as_str()))
        }
        zbus::Error::FDO(fdo) => absent(fdo.name().as_str()),
        _ => false,
    }
}

/// The message of a service error, without the D-Bus error name.
pub fn message(error: &Error) -> String {
    use zbus::DBusError as _;
    error
        .description()
        .map_or_else(|| error.to_string(), str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn method_error(name: &str) -> Error {
        error_from(name, None)
    }

    /// A D-Bus error named `name`, sent by `sender`.
    fn error_from(name: &str, sender: Option<&str>) -> Error {
        let builder = zbus::message::Message::method_call("/", "Ping").expect("builder");
        let builder = match sender {
            Some(sender) => builder.sender(sender).expect("sender"),
            None => builder,
        };
        Error::from(zbus::Error::MethodError(
            zbus::names::OwnedErrorName::try_from(name).expect("error name"),
            Some("x".to_owned()),
            builder.build(&()).expect("message"),
        ))
    }

    /// Activation that failed in systemd or the bus daemon never reached a
    /// service: `wye open` routes the link itself (DEF-04).
    #[test]
    fn a_failed_activation_is_unreachable() {
        for error in [
            method_error("org.freedesktop.systemd1.NoSuchUnit"),
            method_error("org.freedesktop.systemd1.UnitMasked"),
            error_from("org.freedesktop.DBus.Error.AccessDenied", Some(BUS_DAEMON)),
            error_from(
                "org.freedesktop.DBus.Error.LimitsExceeded",
                Some(BUS_DAEMON),
            ),
        ] {
            let shown = format!("{error:?}");
            let classified = classify(error);
            assert!(
                matches!(classified, CallError::Unreachable(_)),
                "{shown}: {classified:?}"
            );
        }
        assert!(matches!(
            classify(error_from(
                "org.freedesktop.DBus.Error.NoReply",
                Some(BUS_DAEMON)
            )),
            CallError::NoAnswer(_)
        ));
        assert!(matches!(
            classify(error_from(
                "org.freedesktop.DBus.Error.LimitsExceeded",
                Some(":1.7")
            )),
            CallError::NoAnswer(_)
        ));
    }

    #[test]
    fn a_missing_member_or_service_is_unreachable() {
        assert!(matches!(
            classify(Error::not_implemented("OpenLink")),
            CallError::Unreachable(_)
        ));
        for name in [
            "org.freedesktop.DBus.Error.ServiceUnknown",
            "org.freedesktop.DBus.Error.NameHasNoOwner",
            "org.freedesktop.DBus.Error.UnknownMethod",
            "org.freedesktop.DBus.Error.Spawn.ChildExited",
        ] {
            assert!(
                matches!(classify(method_error(name)), CallError::Unreachable(_)),
                "{name}"
            );
        }
    }

    /// Finding #1: a call the service may have acted on is never repeated
    /// here, so a link cannot open twice (DEF-04, IN-01).
    #[test]
    fn a_call_sent_but_unanswered_is_not_unreachable() {
        for error in [
            Error::from(zbus::Error::InvalidReply),
            method_error("org.freedesktop.DBus.Error.NoReply"),
            method_error("org.freedesktop.DBus.Error.TimedOut"),
        ] {
            assert!(matches!(classify(error), CallError::NoAnswer(_)), "retried");
        }
    }

    #[test]
    fn a_rejected_link_is_the_services_answer() {
        let error = classify(Error::invalid_args(
            "Wye can't open this kind of link (ftp:)",
        ));
        assert!(matches!(error, CallError::Refused(Error::InvalidArgs(_))));
        assert_eq!(error.to_string(), "Wye can't open this kind of link (ftp:)");
    }
}
