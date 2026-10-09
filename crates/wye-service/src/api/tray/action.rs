//! What each tray item does (01-tray-menu.md menu layout, TRAY-10 to
//! TRAY-18). Every tray host sends the item's ID; this is the one place
//! that turns it into a service call.

use wye_api::Error;
use wye_api::actions::{Reopen, Window};
use wye_core::tray::ids;

/// Where "Help" leads (TRAY-15): the project's documentation.
pub(crate) const HELP_URL: &str = env!("CARGO_PKG_REPOSITORY");

/// A chosen tray item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TrayAction {
    /// TRAY-18, DEF-02.
    MakeDefault,
    /// IN-02.
    OpenClipboard,
    /// TRAY-11, TRAY-20: the radio item with this ID.
    Primary(String),
    /// A window: Settings (TRAY-16), History, Test Rules, Set Up, About.
    Show(Window),
    /// TRAY-15: open a recent link in the picker.
    Recent(u64),
    /// TRAY-15, BRW-06.
    Rescan,
    /// TRAY-15: the documentation website.
    Help,
    /// TRAY-17.
    Quit,
}

impl TrayAction {
    /// The action of the item `id`.
    ///
    /// # Errors
    ///
    /// `InvalidArgs` for an ID that names no choosable item (a header, a
    /// separator, a submenu, or nothing).
    pub(crate) fn parse(id: &str) -> Result<Self, Error> {
        let action = match id {
            ids::MAKE_DEFAULT => Self::MakeDefault,
            ids::OPEN_CLIPBOARD => Self::OpenClipboard,
            ids::SETTINGS => Self::Show(Window::Settings),
            ids::HISTORY => Self::Show(Window::History),
            ids::TEST_RULES => Self::Show(Window::TestRules),
            ids::SET_UP => Self::Show(Window::FirstRun),
            ids::ABOUT => Self::Show(Window::About),
            ids::RESCAN => Self::Rescan,
            ids::HELP => Self::Help,
            ids::QUIT => Self::Quit,
            primary if primary.starts_with("primary:") => Self::Primary(primary.to_owned()),
            recent => match recent.strip_prefix("recent:").map(str::parse) {
                Some(Ok(entry)) => Self::Recent(entry),
                _ => {
                    return Err(Error::invalid_args(format!(
                        "{id:?} is not a tray item to choose"
                    )));
                }
            },
        };
        Ok(action)
    }
}

/// How a recent link is opened again (TRAY-15: "choosing one opens it in
/// the picker").
pub(crate) const RECENT_REOPEN: Reopen = Reopen::Picker;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_choosable_item_has_an_action() {
        let cases = [
            (ids::MAKE_DEFAULT, TrayAction::MakeDefault),
            (ids::OPEN_CLIPBOARD, TrayAction::OpenClipboard),
            (
                ids::PRIMARY_PICKER,
                TrayAction::Primary(ids::PRIMARY_PICKER.to_owned()),
            ),
            ("primary:3", TrayAction::Primary("primary:3".to_owned())),
            (ids::SETTINGS, TrayAction::Show(Window::Settings)),
            (ids::HISTORY, TrayAction::Show(Window::History)),
            (ids::TEST_RULES, TrayAction::Show(Window::TestRules)),
            (ids::SET_UP, TrayAction::Show(Window::FirstRun)),
            (ids::ABOUT, TrayAction::Show(Window::About)),
            ("recent:42", TrayAction::Recent(42)),
            (ids::RESCAN, TrayAction::Rescan),
            (ids::HELP, TrayAction::Help),
            (ids::QUIT, TrayAction::Quit),
        ];
        for (id, action) in cases {
            assert_eq!(TrayAction::parse(id).ok(), Some(action), "{id}");
        }
    }

    #[test]
    fn headers_submenus_and_separators_are_not_choosable() {
        for id in [
            ids::PRIMARY_HEADER,
            ids::MORE,
            ids::RECENT,
            "separator:0",
            "recent:x",
            "",
        ] {
            assert!(TrayAction::parse(id).is_err(), "{id}");
        }
    }

    #[test]
    fn help_leads_to_the_project() {
        assert!(HELP_URL.starts_with("https://"));
    }
}
