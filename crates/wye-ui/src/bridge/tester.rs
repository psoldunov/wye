//! `TesterBackend`: the Rust side of the rule tester sheet (17-dialogs.md, DLG-TST). A stub so the surface's QML
//! compiles and loads; U11 fills it.
//!
//! Used from `qml/settings/` (sheets).

#[cxx_qt::bridge]
pub mod qobject {
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type TesterBackend = super::TesterBackendRust;
    }
}

/// No state yet.
#[derive(Default)]
pub struct TesterBackendRust;
