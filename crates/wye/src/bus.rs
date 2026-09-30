//! Talking to the Wye service (`dev.soldunov.wye`) from the command line.
//!
//! Each call runs on a small single-threaded runtime and is bounded by
//! [`TIMEOUT`], which includes starting the service through D-Bus
//! activation. A caller tells an unreachable service ([`CallError::Unreachable`],
//! where the command does the work itself) from a refusal
//! ([`CallError::Refused`], where the service already told the user).

use std::future::Future;
use std::time::Duration;

use wye_api::Error;
use wye_api::proxy::{ApplicationProxy, Wye1Proxy};

/// How long one call may take, bus activation included.
pub const TIMEOUT: Duration = Duration::from_secs(3);

/// Why a call to the service did not succeed.
#[derive(Debug)]
pub enum CallError {
    /// No service answered: no session bus, no service installed, a
    /// timeout, or a member it does not implement yet.
    Unreachable(String),
    /// The service answered with an error it stands by.
    Refused(Error),
}

impl std::fmt::Display for CallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable(reason) => {
                write!(formatter, "the Wye service is not reachable: {reason}")
            }
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
        let reply = self.runtime.block_on(bounded(call(self.wye.clone())))?;
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
/// the rest are the service's answer.
fn classify(error: Error) -> CallError {
    match error {
        Error::NotImplemented(reason) | Error::Unavailable(reason) => {
            CallError::Unreachable(reason)
        }
        Error::Bus(error) => CallError::Unreachable(error.to_string()),
        other => CallError::Refused(other),
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

    #[test]
    fn a_missing_member_or_service_is_unreachable() {
        assert!(matches!(
            classify(Error::not_implemented("OpenLink")),
            CallError::Unreachable(_)
        ));
        assert!(matches!(
            classify(Error::from(zbus::Error::InvalidReply)),
            CallError::Unreachable(_)
        ));
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
