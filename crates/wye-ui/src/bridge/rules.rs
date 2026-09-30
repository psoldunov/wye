//! `RulesBackend`: the Rust side of the Rules page and rule editor (08-rules.md). A stub so the surface's QML
//! compiles and loads; U11 fills it.
//!
//! Used from `qml/settings/` (Rules page and sheets).

#[cxx_qt::bridge]
pub mod qobject {
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type RulesBackend = super::RulesBackendRust;
    }
}

/// No state yet.
#[derive(Default)]
pub struct RulesBackendRust;
