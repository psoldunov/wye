//! `AppChooserBackend`: the Rust side of the app chooser sheet (17-dialogs.md, DLG-APP). A stub so the surface's QML
//! compiles and loads; U09 fills it.
//!
//! Used from `qml/settings/` (sheets).

#[cxx_qt::bridge]
pub mod qobject {
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type AppChooserBackend = super::AppChooserBackendRust;
    }
}

/// No state yet.
#[derive(Default)]
pub struct AppChooserBackendRust;
