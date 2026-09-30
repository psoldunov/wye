//! zbus proxies for every interface in `docs/dbus-api.md`.
//!
//! Methods of Wye's own interfaces return [`crate::Error`], so a caller can
//! match `NotFound`, `Conflict` and the rest directly. Structured values are
//! JSON strings; decode them with [`crate::json::decode`] into the types of
//! this crate.

mod application;
mod kwin;
mod ui;
mod wye1;

use std::collections::HashMap;

use zbus::zvariant::Value;

pub use application::ApplicationProxy;
pub use kwin::KWin1Proxy;
pub use ui::{PickerHost1Proxy, Windows1Proxy};
pub use wye1::Wye1Proxy;

/// An `a{sv}` argument as a caller builds it.
pub type Dict<'a> = HashMap<&'a str, Value<'a>>;

#[cfg(test)]
mod tests {
    use zbus::names::{BusName, InterfaceName};
    use zbus::proxy::Defaults;
    use zbus::zvariant::ObjectPath;

    use super::*;
    use crate::names;

    type Names = (
        Option<&'static str>,
        Option<&'static str>,
        Option<&'static str>,
    );

    fn defaults<P: Defaults>() -> Names {
        (
            P::INTERFACE.as_ref().map(InterfaceName::as_str),
            P::DESTINATION.as_ref().map(BusName::as_str),
            P::PATH.as_ref().map(ObjectPath::as_str),
        )
    }

    #[test]
    fn service_proxies_use_the_documented_names() {
        let service = (Some(names::BUS_NAME), Some(names::OBJECT_PATH));
        let wye1 = defaults::<Wye1Proxy<'_>>();
        assert_eq!(wye1, (Some(names::INTERFACE), service.0, service.1));
        let application = defaults::<ApplicationProxy<'_>>();
        assert_eq!(
            application,
            (Some(names::APPLICATION_INTERFACE), service.0, service.1)
        );
        let kwin = defaults::<KWin1Proxy<'_>>();
        assert_eq!(kwin, (Some(names::KWIN_INTERFACE), service.0, service.1));
    }

    #[test]
    fn ui_proxies_use_the_documented_names() {
        let ui = (Some(names::UI_BUS_NAME), Some(names::UI_OBJECT_PATH));
        assert_eq!(
            defaults::<PickerHost1Proxy<'_>>(),
            (Some(names::PICKER_HOST_INTERFACE), ui.0, ui.1)
        );
        assert_eq!(
            defaults::<Windows1Proxy<'_>>(),
            (Some(names::WINDOWS_INTERFACE), ui.0, ui.1)
        );
    }
}
