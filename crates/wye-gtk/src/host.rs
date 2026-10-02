//! The GTK host on the session bus: `dev.soldunov.wye.PickerHost1` and
//! `dev.soldunov.wye.Windows1` at `/dev/soldunov/wye/Gtk`, and single
//! instance by owning `dev.soldunov.wye.Gtk` (SET-04, `docs/dbus-api.md` "UI
//! host"). The service routes `ShowWindow` here for the GNOME frontend, and
//! the picker and the tray-menu popup when the Shell extension does not
//! serve them (ADV-12, `crates/wye-service/src/api/picker/host.rs`).
//!
//! The interface only validates and forwards: each call becomes a
//! [`UiCommand`] for the [`Dispatcher`], which the GTK main thread hands to the
//! surface (see `crate::route`). Mirrors crates/wye-ui/src/host.rs; keep the
//! two in step.

use wye_api::Error;
use wye_api::actions::Window;
use wye_api::names::{GTK_BUS_NAME, GTK_OBJECT_PATH};
use wye_api::proxy::Windows1Proxy;
use wye_api::tray::TrayMenu;
use zbus::fdo::{DBusProxy, RequestNameFlags, RequestNameReply};

use crate::dispatch::Dispatcher;
use crate::picker::view::PickerView;
use crate::route::UiCommand;

/// `dev.soldunov.wye.PickerHost1`.
pub struct PickerHost {
    dispatcher: &'static Dispatcher,
}

/// `dev.soldunov.wye.Windows1`.
pub struct Windows {
    dispatcher: &'static Dispatcher,
}

fn send(dispatcher: &Dispatcher, command: UiCommand) -> Result<(), Error> {
    dispatcher
        .send(command.into_delivery())
        .map_err(|error| Error::failed(error.to_string()))
}

/// A request ID the picker can answer with: never empty.
fn request_id(id: &str) -> Result<String, Error> {
    (!id.is_empty())
        .then(|| id.to_owned())
        .ok_or_else(|| Error::invalid_args("request_id is empty"))
}

#[zbus::interface(name = "dev.soldunov.wye.PickerHost1")]
impl PickerHost {
    /// Show the picker, or replace the request it shows (PICK-27).
    ///
    /// The request is read here as the picker reads it: one it cannot show
    /// is refused at once, so the service opens the link through its
    /// stand-in (PICK-23) instead of waiting for an answer.
    fn show_picker(&self, request_id: &str, request: &str) -> Result<(), Error> {
        let request_id = self::request_id(request_id)?;
        if let Err(error) = PickerView::parse(request) {
            return Err(Error::invalid_args(error.to_string()));
        }
        let request_json = request.to_owned();
        send(
            self.dispatcher,
            UiCommand::ShowPicker {
                request_id,
                request_json,
            },
        )
    }

    /// Close the picker showing `request_id` (superseded, or the screen
    /// locked); no answer follows.
    fn close_picker(&self, request_id: &str) -> Result<(), Error> {
        let request_id = self::request_id(request_id)?;
        send(self.dispatcher, UiCommand::ClosePicker { request_id })
    }

    /// Toggle the tray-menu popup (TRAY-08).
    fn show_menu(&self, menu: &str) -> Result<(), Error> {
        let menu_json =
            wye_api::json::decode::<TrayMenu>("tray menu", menu).map(|_| menu.to_owned())?;
        send(self.dispatcher, UiCommand::ShowMenu { menu_json })
    }
}

#[zbus::interface(name = "dev.soldunov.wye.Windows1")]
impl Windows {
    /// Open or raise a window (SET-04: one of each).
    fn show_window(&self, window: &str, argument: &str) -> Result<(), Error> {
        let window = window
            .parse::<Window>()
            .map_err(|error| Error::invalid_args(error.to_string()))?;
        send(
            self.dispatcher,
            UiCommand::ShowWindow {
                window,
                argument: argument.to_owned(),
            },
        )
    }

    /// Quit the GTK host.
    fn quit(&self) -> Result<(), Error> {
        send(self.dispatcher, UiCommand::Quit)
    }
}

/// How the start went.
#[derive(Debug)]
pub enum Claim {
    /// This process owns [`GTK_BUS_NAME`] and serves the interfaces.
    Owner(zbus::Connection),
    /// Another `wye-gtk` owns it and got this start's window, if any.
    Forwarded,
}

/// Serve the interfaces on `builder`'s bus, then take [`GTK_BUS_NAME`].
/// When another process owns it, hand it `forward` with
/// `Windows1.ShowWindow`.
///
/// The objects are served first, so the call that bus-activated this
/// process is answered; until the application attaches, the dispatcher
/// keeps it.
///
/// # Errors
///
/// When the bus is unreachable, the object cannot be served, or the
/// forwarded call fails.
pub async fn claim(
    builder: zbus::connection::Builder<'_>,
    dispatcher: &'static Dispatcher,
    forward: Option<(Window, &str)>,
) -> anyhow::Result<Claim> {
    let connection = builder
        .serve_at(GTK_OBJECT_PATH, PickerHost { dispatcher })?
        .serve_at(GTK_OBJECT_PATH, Windows { dispatcher })?
        .build()
        .await?;
    let reply = DBusProxy::new(&connection)
        .await?
        .request_name(
            GTK_BUS_NAME.try_into()?,
            RequestNameFlags::DoNotQueue.into(),
        )
        .await?;
    match reply {
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => {
            Ok(Claim::Owner(connection))
        }
        RequestNameReply::Exists | RequestNameReply::InQueue => {
            if let Some((window, argument)) = forward {
                windows_proxy(&connection)
                    .await?
                    .show_window(window.as_str(), argument)
                    .await?;
            }
            Ok(Claim::Forwarded)
        }
    }
}

/// `Windows1` of the running GTK host (the proxy's defaults name `wye-ui`).
async fn windows_proxy(connection: &zbus::Connection) -> zbus::Result<Windows1Proxy<'static>> {
    Windows1Proxy::builder(connection)
        .destination(GTK_BUS_NAME)?
        .path(GTK_OBJECT_PATH)?
        .build()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::{Action, Delivery};
    use crate::surface::Surface;
    use crate::test_bus::PrivateBus;

    fn leak() -> &'static Dispatcher {
        Box::leak(Box::default())
    }

    #[tokio::test]
    async fn a_second_start_forwards_its_window_to_the_first() {
        // SET-04: one GTK host; starting it again raises the window asked for.
        let Some(bus) = PrivateBus::start() else {
            return;
        };
        let first = leak();
        let owner = claim(bus.builder(), first, None).await.expect("first");
        assert!(matches!(owner, Claim::Owner(_)));

        let second = leak();
        let forwarded = claim(bus.builder(), second, Some((Window::History, "")))
            .await
            .expect("second");
        assert!(matches!(forwarded, Claim::Forwarded));

        let recorder = Recorder::default();
        first.attach(Box::new(recorder.clone())).expect("attached");
        let seen = recorder.seen();
        let [Delivery::Route(route)] = seen.as_slice() else {
            panic!("expected one route, got {seen:?}");
        };
        assert_eq!(route.key, "history");
    }

    #[tokio::test]
    async fn malformed_requests_are_the_callers_mistake() {
        let Some(bus) = PrivateBus::start() else {
            return;
        };
        let Claim::Owner(_owner) = claim(bus.builder(), leak(), None).await.expect("claim") else {
            panic!("expected to own the name");
        };
        let client = bus.builder().build().await.expect("client");
        let windows = windows_proxy(&client).await.expect("proxy");
        let error = windows.show_window("nope", "").await.expect_err("rejected");
        assert!(matches!(error, Error::InvalidArgs(_)), "{error:?}");
        windows
            .show_window("about", "")
            .await
            .expect("a valid request");
        windows.quit().await.expect("quit is accepted");
    }

    #[tokio::test]
    async fn picker_calls_are_validated_like_the_qt_hosts() {
        // docs/dbus-api.md, PickerHost1: what the picker cannot show is
        // refused now, so the service opens the link through its stand-in
        // (PICK-23); what it can show reaches the picker surface.
        let Some(bus) = PrivateBus::start() else {
            return;
        };
        let dispatcher = leak();
        let Claim::Owner(_owner) = claim(bus.builder(), dispatcher, None).await.expect("claim")
        else {
            panic!("expected to own the name");
        };
        let client = bus.builder().build().await.expect("client");
        let picker = wye_api::proxy::PickerHost1Proxy::builder(&client)
            .destination(GTK_BUS_NAME)
            .and_then(|builder| builder.path(GTK_OBJECT_PATH))
            .expect("address")
            .build()
            .await
            .expect("proxy");
        for (id, request) in [
            ("1", "["),
            ("", "{}"),
            (
                "1",
                r#"{"keys": {"actions": {"private-modifier": ["Return"]}}}"#,
            ),
        ] {
            let error = picker.show_picker(id, request).await.expect_err("rejected");
            assert!(
                matches!(error, Error::InvalidArgs(_)),
                "{id:?} {request}: {error:?}"
            );
        }
        let error = picker.close_picker("").await.expect_err("rejected");
        assert!(matches!(error, Error::InvalidArgs(_)), "{error:?}");
        let error = picker.show_menu("{").await.expect_err("rejected");
        assert!(matches!(error, Error::InvalidArgs(_)), "{error:?}");

        picker
            .show_picker("7", "{}")
            .await
            .expect("a valid request");
        picker.close_picker("7").await.expect("a valid close");
        picker
            .show_menu(r#"{"icon": {"kind": "app"}, "visible": true, "items": []}"#)
            .await
            .expect("a valid menu");
        let recorder = Recorder::default();
        dispatcher
            .attach(Box::new(recorder.clone()))
            .expect("attached");
        let routes: Vec<_> = recorder
            .seen()
            .into_iter()
            .filter_map(|delivery| match delivery {
                Delivery::Route(route) => Some((route.surface, route.action, route.key)),
                Delivery::Quit => None,
            })
            .collect();
        assert_eq!(
            routes,
            [
                (Surface::Picker, Action::Show, "7".to_owned()),
                (Surface::Picker, Action::Close, "7".to_owned()),
                (Surface::TrayMenu, Action::Toggle, String::new()),
            ]
        );
    }

    #[derive(Clone, Default)]
    struct Recorder(std::sync::Arc<std::sync::Mutex<Vec<Delivery>>>);

    impl Recorder {
        fn seen(&self) -> Vec<Delivery> {
            self.0.lock().expect("not poisoned").clone()
        }
    }

    impl crate::dispatch::Sink for Recorder {
        fn deliver(&self, delivery: Delivery) -> Result<(), crate::dispatch::SinkClosed> {
            self.0.lock().expect("not poisoned").push(delivery);
            Ok(())
        }
    }
}
