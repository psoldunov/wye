//! Calls to the session's picker and window hosts. GNOME Shell serves the
//! picker only while it owns its bus name; GTK serves windows when installed.
//! Qt remains the fallback and the sole host outside GNOME.

use std::time::Duration;

use wye_api::Error;
use wye_api::actions::Window;
use wye_api::names::{
    GNOME_BUS_NAME, GNOME_OBJECT_PATH, GTK_BUS_NAME, GTK_OBJECT_PATH, UI_BUS_NAME,
};
use wye_api::proxy::{PickerHost1Proxy, Windows1Proxy};
use zbus::fdo::DBusProxy;

use crate::context::ServiceContext;

/// Longest a call to the UI may take, bus activation of a cold `wye-ui`
/// included.
const UI_DEADLINE: Duration = Duration::from_secs(10);

pub(super) fn is_gnome(ctx: &ServiceContext) -> bool {
    ctx.environment().is_ok_and(|environment| {
        environment.xdg.current_desktops.iter().any(|desktop| {
            desktop
                .split(':')
                .any(|part| part.eq_ignore_ascii_case("GNOME"))
        })
    })
}

async fn shell_available(connection: &zbus::Connection) -> bool {
    let result = async {
        DBusProxy::new(connection)
            .await?
            .name_has_owner(GNOME_BUS_NAME.try_into().map_err(zbus::Error::from)?)
            .await
    }
    .await;
    match result {
        Ok(owned) => owned,
        Err(error) => {
            tracing::warn!(%error, "cannot check the GNOME picker; using Qt");
            false
        }
    }
}

/// Whether Qt needs prestarting before a link can reach the picker.
pub(crate) async fn needs_qt_picker(ctx: &ServiceContext) -> bool {
    if !is_gnome(ctx) {
        return true;
    }
    let Some(connection) = ctx.connection() else {
        return true;
    };
    !shell_available(connection).await
}

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
    if is_gnome(ctx) && shell_available(connection).await {
        let shell = within("ShowPicker (GNOME)", async {
            PickerHost1Proxy::builder(connection)
                .destination(GNOME_BUS_NAME)?
                .path(GNOME_OBJECT_PATH)?
                .build()
                .await?
                .show_picker(request_id, request)
                .await
        })
        .await;
        if shell.is_ok() {
            return shell;
        }
        tracing::warn!(error = %shell.unwrap_err(), "GNOME picker failed; trying Qt");
    }
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
    if is_gnome(ctx) && shell_available(connection).await {
        let shell = within("ClosePicker (GNOME)", async {
            PickerHost1Proxy::builder(connection)
                .destination(GNOME_BUS_NAME)?
                .path(GNOME_OBJECT_PATH)?
                .build()
                .await?
                .close_picker(request_id)
                .await
        })
        .await;
        // A request sent to Qt before Shell took ownership must be closed too.
        let qt_owned = DBusProxy::new(connection)
            .await?
            .name_has_owner(UI_BUS_NAME.try_into().map_err(zbus::Error::from)?)
            .await?;
        if !qt_owned {
            return shell;
        }
        let qt = within("ClosePicker (Qt)", async {
            PickerHost1Proxy::new(connection)
                .await?
                .close_picker(request_id)
                .await
        })
        .await;
        return shell.and(qt);
    }
    within("ClosePicker", async {
        PickerHost1Proxy::new(connection)
            .await?
            .close_picker(request_id)
            .await
    })
    .await
}

/// `PickerHost1.ShowMenu` on the selected picker host.
pub(crate) async fn show_menu(ctx: &ServiceContext, menu: &str) -> Result<(), Error> {
    let connection = connection(ctx)?;
    if is_gnome(ctx) && shell_available(connection).await {
        let shell = within("ShowMenu (GNOME)", async {
            PickerHost1Proxy::builder(connection)
                .destination(GNOME_BUS_NAME)?
                .path(GNOME_OBJECT_PATH)?
                .build()
                .await?
                .show_menu(menu)
                .await
        })
        .await;
        if shell.is_ok() {
            return shell;
        }
        tracing::warn!(error = %shell.unwrap_err(), "GNOME menu failed; trying Qt");
    }
    within("ShowMenu", async {
        PickerHost1Proxy::new(connection)
            .await?
            .show_menu(menu)
            .await
    })
    .await
}

/// Check both running and bus-activatable GTK hosts; an absent host is
/// the only reason to route windows to Qt on GNOME.
async fn gtk_available(connection: &zbus::Connection) -> Result<bool, Error> {
    within("checking the GTK host", async {
        let bus = DBusProxy::new(connection).await?;
        if bus
            .name_has_owner(GTK_BUS_NAME.try_into().map_err(zbus::Error::from)?)
            .await?
        {
            return Ok(true);
        }
        Ok(bus
            .list_activatable_names()
            .await?
            .iter()
            .any(|name| name.as_str() == GTK_BUS_NAME))
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
    if is_gnome(ctx) && gtk_available(connection).await? {
        return within("ShowWindow (GTK)", async {
            Windows1Proxy::builder(connection)
                .destination(GTK_BUS_NAME)?
                .path(GTK_OBJECT_PATH)?
                .build()
                .await?
                .show_window(window.as_str(), argument)
                .await
        })
        .await;
    }
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
