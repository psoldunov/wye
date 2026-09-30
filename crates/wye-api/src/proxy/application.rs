//! `org.freedesktop.Application` on the service (DEF-04, TRAY-05).

use std::collections::HashMap;

use zbus::proxy;
use zbus::zvariant::Value;

/// Desktop activation, as launchers call it for `DBusActivatable=true`.
#[proxy(
    interface = "org.freedesktop.Application",
    default_service = "dev.soldunov.wye",
    default_path = "/dev/soldunov/wye"
)]
pub trait Application {
    /// Started without a link.
    fn activate(&self, platform_data: HashMap<&str, Value<'_>>) -> zbus::Result<()>;

    /// Open links; each enters the pipeline (IN-01).
    fn open(&self, uris: &[&str], platform_data: HashMap<&str, Value<'_>>) -> zbus::Result<()>;

    /// A desktop action ([`crate::actions::ApplicationAction`]).
    fn activate_action(
        &self,
        action_name: &str,
        parameter: &[Value<'_>],
        platform_data: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<()>;
}
