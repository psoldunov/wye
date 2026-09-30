//! Calls to the UI host `dev.soldunov.wye.Ui` (`PickerHost1`, `Windows1`).
//!
//! The first call bus-activates `wye-ui` (`dev.soldunov.wye.Ui.service`);
//! each call has a deadline, so a UI that never answers falls back to the
//! stand-in instead of holding the link forever.

use std::time::Duration;

use wye_api::Error;
use wye_api::actions::Window;
use wye_api::names::UI_BUS_NAME;
use wye_api::proxy::{PickerHost1Proxy, Windows1Proxy};
use zbus::fdo::DBusProxy;

use crate::context::ServiceContext;

/// Longest a call to the UI may take, bus activation of a cold `wye-ui`
/// included.
const UI_DEADLINE: Duration = Duration::from_secs(10);

fn connection(ctx: &ServiceContext) -> Result<&zbus::Connection, Error> {
    ctx.connection()
        .ok_or_else(|| Error::Unavailable("the service is not on the session bus".to_owned()))
}

/// A UI host that cannot be reached (not installed, not activatable, gone)
/// is `Unavailable`; what the UI itself answered stays as it is.
fn unreachable(error: Error) -> Error {
    match error {
        Error::Bus(error) => Error::Unavailable(format!("the UI host cannot be reached: {error}")),
        other => other,
    }
}

async fn within<T>(what: &str, call: impl Future<Output = Result<T, Error>>) -> Result<T, Error> {
    tokio::time::timeout(UI_DEADLINE, call)
        .await
        .map_err(|_| Error::Unavailable(format!("the UI did not answer {what} in time")))?
        .map_err(unreachable)
}

/// `PickerHost1.ShowPicker` (PICK-01, PICK-27).
pub(crate) async fn show_picker(
    ctx: &ServiceContext,
    request_id: &str,
    request: &str,
) -> Result<(), Error> {
    let connection = connection(ctx)?;
    within("ShowPicker", async {
        PickerHost1Proxy::new(connection)
            .await?
            .show_picker(request_id, request)
            .await
    })
    .await
}

/// `PickerHost1.ClosePicker`.
pub(crate) async fn close_picker(ctx: &ServiceContext, request_id: &str) -> Result<(), Error> {
    let connection = connection(ctx)?;
    within("ClosePicker", async {
        PickerHost1Proxy::new(connection)
            .await?
            .close_picker(request_id)
            .await
    })
    .await
}

/// `Windows1.ShowWindow`.
pub(crate) async fn show_window(
    ctx: &ServiceContext,
    window: Window,
    argument: &str,
) -> Result<(), Error> {
    let connection = connection(ctx)?;
    within("ShowWindow", async {
        Windows1Proxy::new(connection)
            .await?
            .show_window(window.as_str(), argument)
            .await
    })
    .await
}

/// Start `wye-ui` ahead of the first picker (decision 2), so the first
/// link does not wait for Qt to load.
pub(crate) async fn activate(ctx: &ServiceContext) -> Result<(), Error> {
    let connection = connection(ctx)?;
    within("StartServiceByName", async {
        DBusProxy::new(connection)
            .await?
            .start_service_by_name(UI_BUS_NAME.try_into().map_err(zbus::Error::from)?, 0)
            .await?;
        Ok(())
    })
    .await
}
