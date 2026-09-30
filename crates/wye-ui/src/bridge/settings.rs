//! `SettingsBackend`: the Rust side of the Settings window (03-settings-window.md). A stub so the surface's QML
//! compiles and loads; U09 fills it.
//!
//! Used from `qml/settings/`.

#[cxx_qt::bridge]
pub mod qobject {
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type SettingsBackend = super::SettingsBackendRust;
    }
}

/// No state yet.
#[derive(Default)]
pub struct SettingsBackendRust;
