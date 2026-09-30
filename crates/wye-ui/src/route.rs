//! What a D-Bus call to the UI host turns into on the Qt side.
//!
//! The interfaces in `crate::host` validate their arguments and build a
//! [`UiCommand`]; [`UiCommand::into_delivery`] decides which surface gets
//! it. `qml/Main.qml` then calls the surface root's
//! `handle(action, key, argument)`:
//!
//! | Call | Surface | action | key | argument |
//! |---|---|---|---|---|
//! | `PickerHost1.ShowPicker` | `picker` | `show` | request id | `PickerRequest` JSON |
//! | `PickerHost1.ClosePicker` | `picker` | `close` | request id | empty |
//! | `PickerHost1.ShowMenu` | `tray-menu` | `toggle` | empty | `TrayMenu` JSON |
//! | `Windows1.ShowWindow` | per [`Surface::for_window`] | `show` | window name | argument |
//! | `Windows1.Quit` | — | — | — | — |

use wye_api::actions::Window;

use crate::surface::Surface;

/// A request from the service, already validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiCommand {
    /// Show or replace the picker (PICK-27).
    ShowPicker {
        request_id: String,
        request_json: String,
    },
    /// Close the picker for this request.
    ClosePicker { request_id: String },
    /// Toggle the tray-menu popup (TRAY-08).
    ShowMenu { menu_json: String },
    /// Open or raise a window (SET-04).
    ShowWindow { window: Window, argument: String },
    /// Quit the UI host.
    Quit,
}

/// What a surface is asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Show,
    Close,
    Toggle,
}

impl Action {
    /// The name QML receives.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Show => "show",
            Self::Close => "close",
            Self::Toggle => "toggle",
        }
    }
}

/// One call for one surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub surface: Surface,
    pub action: Action,
    pub key: String,
    pub argument: String,
}

/// What the Qt side receives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delivery {
    Route(Route),
    Quit,
}

impl UiCommand {
    /// The surface and action this command becomes.
    pub fn into_delivery(self) -> Delivery {
        let route = |surface, action, key, argument| {
            Delivery::Route(Route {
                surface,
                action,
                key,
                argument,
            })
        };
        match self {
            Self::ShowPicker {
                request_id,
                request_json,
            } => route(Surface::Picker, Action::Show, request_id, request_json),
            Self::ClosePicker { request_id } => {
                route(Surface::Picker, Action::Close, request_id, String::new())
            }
            Self::ShowMenu { menu_json } => {
                route(Surface::TrayMenu, Action::Toggle, String::new(), menu_json)
            }
            Self::ShowWindow { window, argument } => route(
                Surface::for_window(window),
                Action::Show,
                window.as_str().to_owned(),
                argument,
            ),
            Self::Quit => Delivery::Quit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route_of(command: UiCommand) -> Route {
        match command.into_delivery() {
            Delivery::Route(route) => route,
            Delivery::Quit => panic!("expected a route"),
        }
    }

    #[test]
    fn a_picker_request_keeps_its_id_and_json() {
        let route = route_of(UiCommand::ShowPicker {
            request_id: "7".into(),
            request_json: "{}".into(),
        });
        assert_eq!(route.surface, Surface::Picker);
        assert_eq!(route.action.as_str(), "show");
        assert_eq!((route.key.as_str(), route.argument.as_str()), ("7", "{}"));
    }

    #[test]
    fn close_and_menu_route_to_their_surfaces() {
        let close = route_of(UiCommand::ClosePicker {
            request_id: "7".into(),
        });
        assert_eq!(
            (close.surface, close.action),
            (Surface::Picker, Action::Close)
        );
        let menu = route_of(UiCommand::ShowMenu {
            menu_json: "{}".into(),
        });
        assert_eq!(
            (menu.surface, menu.action),
            (Surface::TrayMenu, Action::Toggle)
        );
    }

    #[test]
    fn a_window_carries_its_name_as_the_key() {
        let route = route_of(UiCommand::ShowWindow {
            window: Window::RuleEditor,
            argument: r#"{"match":"example.com"}"#.into(),
        });
        assert_eq!(route.surface, Surface::Settings);
        assert_eq!(route.key, "rule-editor");
        assert_eq!(route.argument, r#"{"match":"example.com"}"#);
    }

    #[test]
    fn quit_is_not_a_route() {
        assert_eq!(UiCommand::Quit.into_delivery(), Delivery::Quit);
    }
}
