//! The UI host on the session bus: `dev.soldunov.wye.PickerHost1` and
//! `dev.soldunov.wye.Windows1` at `/dev/soldunov/wye/Ui`, and single
//! instance by owning `dev.soldunov.wye.Ui` (SET-04, `docs/dbus-api.md`
//! "UI host").
//!
//! The interfaces only validate and forward: each call becomes a
//! [`UiCommand`] for the [`Dispatcher`], which the Qt thread turns into a
//! call on the surface's QML (see `crate::route`).

use wye_api::Error;
use wye_api::actions::Window;
use wye_api::names::{UI_BUS_NAME, UI_OBJECT_PATH};
use wye_api::picker::PickerRequest;
use wye_api::proxy::Windows1Proxy;
use wye_api::tray::TrayMenu;
use zbus::fdo::{DBusProxy, RequestNameFlags, RequestNameReply};

use crate::dispatch::Dispatcher;
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

fn require_id(request_id: &str) -> Result<(), Error> {
    if request_id.is_empty() {
        Err(Error::invalid_args("request_id is empty"))
    } else {
        Ok(())
    }
}

#[zbus::interface(name = "dev.soldunov.wye.PickerHost1")]
impl PickerHost {
    /// Show the picker, or replace the request it shows (PICK-27).
    fn show_picker(&self, request_id: &str, request: &str) -> Result<(), Error> {
        require_id(request_id)?;
        wye_api::json::decode::<PickerRequest>("picker request", request)?;
        send(
            self.dispatcher,
            UiCommand::ShowPicker {
                request_id: request_id.to_owned(),
                request_json: request.to_owned(),
            },
        )
    }

    /// Close the picker showing `request_id` (superseded, or the screen
    /// locked).
    fn close_picker(&self, request_id: &str) -> Result<(), Error> {
        require_id(request_id)?;
        send(
            self.dispatcher,
            UiCommand::ClosePicker {
                request_id: request_id.to_owned(),
            },
        )
    }

    /// Toggle the tray-menu popup (TRAY-08).
    fn show_menu(&self, menu: &str) -> Result<(), Error> {
        wye_api::json::decode::<TrayMenu>("tray menu", menu)?;
        send(
            self.dispatcher,
            UiCommand::ShowMenu {
                menu_json: menu.to_owned(),
            },
        )
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

    /// Quit the UI host.
    fn quit(&self) -> Result<(), Error> {
        send(self.dispatcher, UiCommand::Quit)
    }
}

/// How the start went.
#[derive(Debug)]
pub enum Claim {
    /// This process owns [`UI_BUS_NAME`] and serves the interfaces.
    Owner(zbus::Connection),
    /// Another `wye-ui` owns it and got this start's window, if any.
    Forwarded,
}

/// Serve the interfaces on `builder`'s bus, then take [`UI_BUS_NAME`]. When
/// another process owns it, hand it `forward` with `Windows1.ShowWindow`.
///
/// The objects are served first, so the call that bus-activated this
/// process is answered; until QML attaches, the dispatcher keeps it.
///
/// # Errors
///
/// When the bus is unreachable, the objects cannot be served, or the
/// forwarded call fails.
pub async fn claim(
    builder: zbus::connection::Builder<'_>,
    dispatcher: &'static Dispatcher,
    forward: Option<(Window, &str)>,
) -> anyhow::Result<Claim> {
    let connection = builder
        .serve_at(UI_OBJECT_PATH, PickerHost { dispatcher })?
        .serve_at(UI_OBJECT_PATH, Windows { dispatcher })?
        .build()
        .await?;
    let reply = DBusProxy::new(&connection)
        .await?
        .request_name(UI_BUS_NAME.try_into()?, RequestNameFlags::DoNotQueue.into())
        .await?;
    match reply {
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => {
            Ok(Claim::Owner(connection))
        }
        RequestNameReply::Exists | RequestNameReply::InQueue => {
            if let Some((window, argument)) = forward {
                Windows1Proxy::new(&connection)
                    .await?
                    .show_window(window.as_str(), argument)
                    .await?;
            }
            Ok(Claim::Forwarded)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead as _, BufReader};
    use std::process::{Child, Command, Stdio};

    use super::*;
    use crate::route::Delivery;

    /// A private `dbus-daemon`, killed on drop; `None` (the test skips)
    /// when the program is not on `PATH`.
    struct PrivateBus {
        daemon: Child,
        address: String,
    }

    impl PrivateBus {
        fn start() -> Option<Self> {
            let spawned = Command::new("dbus-daemon")
                .args(["--session", "--nofork", "--print-address=1"])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .spawn();
            let Ok(mut daemon) = spawned else {
                eprintln!("skipping: dbus-daemon is not on PATH");
                return None;
            };
            let stdout = daemon.stdout.take().expect("stdout is piped");
            let mut address = String::new();
            BufReader::new(stdout)
                .read_line(&mut address)
                .expect("dbus-daemon prints its address");
            Some(Self {
                daemon,
                address: address.trim().to_owned(),
            })
        }

        fn builder(&self) -> zbus::connection::Builder<'_> {
            zbus::connection::Builder::address(self.address.as_str()).expect("address parses")
        }
    }

    impl Drop for PrivateBus {
        fn drop(&mut self) {
            // Already gone is fine; a leaked daemon is what this prevents.
            let _ = self.daemon.kill();
            let _ = self.daemon.wait();
        }
    }

    fn leak() -> &'static Dispatcher {
        Box::leak(Box::default())
    }

    #[tokio::test]
    async fn a_second_start_forwards_its_window_to_the_first() {
        // SET-04: one UI host; starting it again raises the window asked for.
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
        let picker = wye_api::proxy::PickerHost1Proxy::new(&client)
            .await
            .expect("proxy");
        let error = picker.show_picker("1", "[").await.expect_err("rejected");
        assert!(matches!(error, Error::InvalidArgs(_)), "{error:?}");
        let error = picker.show_picker("", "{}").await.expect_err("rejected");
        assert!(matches!(error, Error::InvalidArgs(_)), "{error:?}");
        let windows = Windows1Proxy::new(&client).await.expect("proxy");
        let error = windows.show_window("nope", "").await.expect_err("rejected");
        assert!(matches!(error, Error::InvalidArgs(_)), "{error:?}");
        picker
            .show_picker("1", "{}")
            .await
            .expect("a valid request");
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
