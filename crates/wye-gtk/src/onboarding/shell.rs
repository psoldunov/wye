//! ONB-04 on GNOME: whether Wye's Shell extension (`wye@dev.soldunov`,
//! frontends/gnome-shell) is enabled, and **Enable**, through GNOME Shell's
//! own `org.gnome.Shell.Extensions` interface on the session bus (the one
//! the Extensions app uses). GNOME Shell is never D-Bus activatable, so
//! without it the call fails and the step says nothing about it.

use std::collections::HashMap;

use wye_api::Error;
use zbus::proxy::CacheProperties;
use zbus::zvariant::OwnedValue;

/// Wye's Shell extension (frontends/gnome-shell/metadata.json).
pub const UUID: &str = "wye@dev.soldunov";

const DESTINATION: &str = "org.gnome.Shell";
const PATH: &str = "/org/gnome/Shell";
const INTERFACE: &str = "org.gnome.Shell.Extensions";

/// GNOME Shell's `ExtensionState` values (the names since GNOME 45; the
/// numbers are older): running, or about to.
const RUNNING: [f64; 2] = [1.0, 8.0];
/// `ERROR` and `OUT_OF_DATE`: installed and enabled, but the Shell could not
/// run it (it fails, or does not list this GNOME version).
const BROKEN: [f64; 2] = [3.0, 4.0];

/// What the integration step knows about the extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShellState {
    /// Not asked yet, not GNOME, or GNOME Shell did not answer: nothing to
    /// show.
    #[default]
    Unknown,
    /// GNOME Shell does not know the extension (not installed).
    Missing,
    /// Installed, not running: the step offers **Enable**.
    Disabled,
    /// Running.
    Enabled,
    /// Enabled, but GNOME Shell could not run it: Enable would not help.
    Broken,
}

impl ShellState {
    /// The self-test's name for a state (`"shell"` in a case's argument).
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "unknown" => Some(Self::Unknown),
            "missing" => Some(Self::Missing),
            "disabled" => Some(Self::Disabled),
            "enabled" => Some(Self::Enabled),
            "broken" => Some(Self::Broken),
            _ => None,
        }
    }

    /// The state `GetExtensionInfo` describes: an empty dictionary for an
    /// extension the Shell does not know. `state` says whether it runs;
    /// `enabled` (GNOME 46 and later) only that the user switched it on, so
    /// it decides only without a `state`.
    #[must_use]
    pub fn of_info(info: &HashMap<String, OwnedValue>) -> Self {
        if info.is_empty() {
            return Self::Missing;
        }
        let is = |value: f64, states: &[f64]| {
            states
                .iter()
                .any(|state| (state - value).abs() < f64::EPSILON)
        };
        let state = info
            .get("state")
            .and_then(|value| value.downcast_ref::<f64>().ok());
        match state {
            Some(state) if is(state, &RUNNING) => Self::Enabled,
            Some(state) if is(state, &BROKEN) => Self::Broken,
            Some(_) => Self::Disabled,
            None => {
                let enabled = info
                    .get("enabled")
                    .and_then(|value| value.downcast_ref::<bool>().ok());
                if enabled.unwrap_or(false) {
                    Self::Enabled
                } else {
                    Self::Disabled
                }
            }
        }
    }
}

async fn extensions(connection: &zbus::Connection) -> Result<zbus::Proxy<'static>, Error> {
    Ok(
        zbus::proxy::Builder::<zbus::Proxy<'static>>::new(connection)
            .destination(DESTINATION)?
            .path(PATH)?
            .interface(INTERFACE)?
            .cache_properties(CacheProperties::No)
            .build()
            .await?,
    )
}

/// Ask GNOME Shell about Wye's extension.
///
/// # Errors
///
/// When GNOME Shell is not on the bus or refuses the call.
pub async fn query(connection: zbus::Connection) -> Result<ShellState, Error> {
    let proxy = extensions(&connection).await?;
    let info: HashMap<String, OwnedValue> = proxy.call("GetExtensionInfo", &(UUID,)).await?;
    Ok(ShellState::of_info(&info))
}

/// Enable Wye's extension, then report its state.
///
/// # Errors
///
/// When GNOME Shell is not on the bus or refuses the call.
pub async fn enable(connection: zbus::Connection) -> Result<ShellState, Error> {
    let proxy = extensions(&connection).await?;
    let accepted: bool = proxy.call("EnableExtension", &(UUID,)).await?;
    if !accepted {
        return Err(Error::failed("GNOME Shell did not enable Wye's extension"));
    }
    query(connection).await
}

#[cfg(test)]
mod tests {
    use zbus::zvariant::Value;

    use super::*;

    fn info(entries: &[(&str, Value<'_>)]) -> HashMap<String, OwnedValue> {
        entries
            .iter()
            .map(|(key, value)| {
                let owned = OwnedValue::try_from(value.try_clone().expect("plain value"))
                    .expect("owned value");
                ((*key).to_owned(), owned)
            })
            .collect()
    }

    #[test]
    fn an_unknown_extension_is_missing() {
        // ONB-04
        assert_eq!(ShellState::of_info(&HashMap::new()), ShellState::Missing);
    }

    #[test]
    fn the_state_says_whether_it_runs() {
        // ONB-04: running or activating is on.
        let active = info(&[("enabled", Value::from(true)), ("state", Value::from(1.0))]);
        assert_eq!(ShellState::of_info(&active), ShellState::Enabled);
        let activating = info(&[("state", Value::from(8.0))]);
        assert_eq!(ShellState::of_info(&activating), ShellState::Enabled);
        // Switched on but not running (user extensions off): Enable stays.
        let inactive = info(&[("enabled", Value::from(true)), ("state", Value::from(2.0))]);
        assert_eq!(ShellState::of_info(&inactive), ShellState::Disabled);
        // Without a state, the flag decides.
        let flag = info(&[("enabled", Value::from(true))]);
        assert_eq!(ShellState::of_info(&flag), ShellState::Enabled);
    }

    #[test]
    fn an_extension_the_shell_cannot_run_is_broken() {
        for state in [3.0, 4.0] {
            let failed = info(&[
                ("enabled", Value::from(true)),
                ("state", Value::from(state)),
            ]);
            assert_eq!(ShellState::of_info(&failed), ShellState::Broken, "{state}");
        }
    }

    #[test]
    fn an_installed_inactive_extension_is_disabled() {
        let inactive = info(&[
            ("uuid", Value::from(UUID)),
            ("enabled", Value::from(false)),
            ("state", Value::from(2.0)),
        ]);
        assert_eq!(ShellState::of_info(&inactive), ShellState::Disabled);
    }

    #[test]
    fn the_self_test_names_every_state() {
        for (name, state) in [
            ("unknown", ShellState::Unknown),
            ("missing", ShellState::Missing),
            ("disabled", ShellState::Disabled),
            ("enabled", ShellState::Enabled),
            ("broken", ShellState::Broken),
        ] {
            assert_eq!(ShellState::parse(name), Some(state));
        }
        assert_eq!(ShellState::parse("on"), None);
    }
}
