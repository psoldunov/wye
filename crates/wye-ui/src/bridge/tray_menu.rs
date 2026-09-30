//! `TrayMenuBackend`: the Rust side of the tray-menu popup (TRAY-08, 01-tray-menu.md). A stub so the surface's QML
//! compiles and loads; U13 fills it.
//!
//! Used from `qml/traymenu/`.

#[cxx_qt::bridge]
pub mod qobject {
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type TrayMenuBackend = super::TrayMenuBackendRust;
    }
}

/// No state yet.
#[derive(Default)]
pub struct TrayMenuBackendRust;
