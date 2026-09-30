//! `TrayMenuBackend`: the Rust side of the tray-menu popup (TRAY-08,
//! 01-tray-menu.md). It reads what `PickerHost1.ShowMenu` sent
//! ([`crate::tray_menu::MenuView`]), exposes the rows and the pointer to QML,
//! and sends the chosen item to the service (`ActivateTrayItem`).
//!
//! Used from `qml/traymenu/`.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, rows)]
        #[qproperty(bool, placed)]
        #[qproperty(QString, placement_output, cxx_name = "placementOutput")]
        #[qproperty(i32, placement_x, cxx_name = "placementX")]
        #[qproperty(i32, placement_y, cxx_name = "placementY")]
        type TrayMenuBackend = super::TrayMenuBackendRust;

        /// Show the `ShowMenu` payload `json`; false when it cannot be read.
        #[qinvokable]
        fn load(self: Pin<&mut Self>, json: &QString) -> bool;

        /// The ID of the top-level item whose fixed accelerator is `text`
        /// (KEY-51); empty when none.
        #[qinvokable]
        fn accelerator(self: &Self, text: &QString) -> QString;

        /// Send the chosen item to the service; false when `id` cannot be
        /// chosen.
        #[qinvokable]
        fn activate(self: Pin<&mut Self>, id: &QString) -> bool;
    }

    impl cxx_qt::Threading for TrayMenuBackend {}
}

use core::pin::Pin;

use cxx_qt::{CxxQtType as _, Threading as _};
use cxx_qt_lib::QString;

use crate::service;
use crate::tray_menu::MenuView;

/// The properties' values and the menu shown.
#[derive(Default)]
pub struct TrayMenuBackendRust {
    rows: QString,
    placed: bool,
    placement_output: QString,
    placement_x: i32,
    placement_y: i32,
    view: Option<MenuView>,
}

impl qobject::TrayMenuBackend {
    /// See the bridge declaration.
    pub fn load(mut self: Pin<&mut Self>, json: &QString) -> bool {
        let view = match MenuView::parse(&json.to_string()) {
            Ok(view) => view,
            Err(error) => {
                tracing::warn!(%error, "cannot show the tray menu");
                return false;
            }
        };
        let (output, x, y) = view
            .placement
            .clone()
            .map_or((String::new(), 0, 0), |at| (at.output, at.x, at.y));
        self.as_mut().set_placed(view.placement.is_some());
        self.as_mut()
            .set_placement_output(QString::from(output.as_str()));
        self.as_mut().set_placement_x(x);
        self.as_mut().set_placement_y(y);
        self.as_mut()
            .set_rows(QString::from(view.rows_json().as_str()));
        self.rust_mut().get_mut().view = Some(view);
        true
    }

    /// See the bridge declaration.
    pub fn accelerator(&self, text: &QString) -> QString {
        let text = text.to_string();
        let id = self
            .rust()
            .view
            .as_ref()
            .and_then(|view| view.accelerator(&text))
            .unwrap_or_default();
        QString::from(id)
    }

    /// See the bridge declaration.
    pub fn activate(self: Pin<&mut Self>, id: &QString) -> bool {
        let id = id.to_string();
        let selectable = self
            .rust()
            .view
            .as_ref()
            .is_some_and(|view| view.is_selectable(&id));
        if !selectable {
            return false;
        }
        service::request(
            self.qt_thread(),
            move |proxy| async move { proxy.activate_tray_item(&id).await },
            |_backend, result| {
                if let Err(error) = result {
                    tracing::warn!(%error, "the service did not run the menu item");
                }
            },
        );
        true
    }
}
