//! `ToggleMenu` (TRAY-08): the tray menu as a popup of the UI host at the
//! pointer (spec decision #14, decision 4).
//!
//! The service sends `PickerHost1.ShowMenu` with the `Tray` model plus a
//! `placement` key (the pointer, as for the picker; absent when unknown, and
//! the popup is centred). The UI host shows the popup, or closes it when it
//! is already shown, and sends the chosen item back with
//! `ActivateTrayItem`. `MenuRequested` goes out first, for a tray host that
//! can open its own menu.

use std::time::Duration;

use serde_json::Value;
use wye_api::Error;
use wye_api::picker::Placement;
use wye_api::proxy::PickerHost1Proxy;

use crate::api::Result;
use crate::context::ServiceContext;

/// Longest the UI host may take, bus activation of a cold `wye-ui`
/// included.
const UI_DEADLINE: Duration = Duration::from_secs(10);

/// The key of the pointer in the `ShowMenu` payload.
const PLACEMENT: &str = "placement";

/// `dev.soldunov.wye1.ToggleMenu` (TRAY-08).
///
/// # Errors
///
/// `Unavailable` when the UI host cannot be reached; the tray model's
/// errors when the menu cannot be built.
pub async fn toggle_menu(ctx: &ServiceContext) -> Result<()> {
    if let Err(error) = crate::bus::menu_requested(ctx).await {
        tracing::debug!(%error, "cannot announce MenuRequested");
    }
    let tray = super::super::tray::tray_json(ctx).await?;
    let placement = ctx.platform().pointer.pointer().await;
    let payload = payload(&tray, placement.as_ref())?;
    show_menu(ctx, &payload).await
}

/// The `Tray` JSON with the pointer added as `placement`.
fn payload(tray: &str, placement: Option<&Placement>) -> Result<String> {
    let menu: Value = serde_json::from_str(tray)
        .map_err(|error| Error::failed(format!("the tray menu is not JSON: {error}")))?;
    let Value::Object(fields) = menu else {
        return Err(Error::failed("the tray menu is not a JSON object"));
    };
    let placement = placement
        .map(serde_json::to_value)
        .transpose()
        .map_err(|error| Error::failed(format!("cannot encode the pointer: {error}")))?;
    let payload: serde_json::Map<String, Value> = fields
        .into_iter()
        .filter(|(key, _)| key != PLACEMENT)
        .chain(placement.map(|placement| (PLACEMENT.to_owned(), placement)))
        .collect();
    Ok(Value::Object(payload).to_string())
}

/// `PickerHost1.ShowMenu` on the UI host.
async fn show_menu(ctx: &ServiceContext, payload: &str) -> Result<()> {
    let connection = ctx
        .connection()
        .ok_or_else(|| Error::Unavailable("the service is not on the session bus".to_owned()))?;
    let call = async {
        PickerHost1Proxy::new(connection)
            .await?
            .show_menu(payload)
            .await
    };
    match tokio::time::timeout(UI_DEADLINE, call).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(Error::Bus(error))) => Err(Error::Unavailable(format!(
            "the UI host cannot be reached: {error}"
        ))),
        Ok(Err(error)) => Err(error),
        Err(_) => Err(Error::Unavailable(format!(
            "the UI host did not answer ShowMenu within {UI_DEADLINE:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use wye_api::tray::TrayMenu;

    use super::*;

    #[test]
    fn the_payload_is_the_tray_menu_plus_the_pointer_tray_08() {
        let tray = wye_api::json::encode(&TrayMenu::default()).expect("encodes");
        let placement = Placement {
            output: "DP-1".into(),
            x: 10,
            y: 20,
        };
        let text = payload(&tray, Some(&placement)).expect("payload");
        let menu: TrayMenu = wye_api::json::decode("menu", &text).expect("still a menu");
        assert_eq!(menu, TrayMenu::default());
        let value: Value = serde_json::from_str(&text).expect("JSON");
        assert_eq!(
            value[PLACEMENT],
            serde_json::json!({"output": "DP-1", "x": 10, "y": 20})
        );
    }

    #[test]
    fn without_a_pointer_there_is_no_placement() {
        let tray = r#"{"icon":{"kind":"app"},"visible":true,"items":[],"placement":1}"#;
        let text = payload(tray, None).expect("payload");
        let value: Value = serde_json::from_str(&text).expect("JSON");
        assert!(value.get(PLACEMENT).is_none());
    }

    #[test]
    fn a_menu_that_is_not_an_object_is_the_services_fault() {
        assert!(matches!(payload("[]", None), Err(Error::Failed(_))));
    }
}
