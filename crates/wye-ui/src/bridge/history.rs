//! `HistoryBackend`: the Rust side of the History window (17-dialogs.md, DLG-HIS). A stub so the surface's QML
//! compiles and loads; U14 fills it.
//!
//! Used from `qml/history/`.

#[cxx_qt::bridge]
pub mod qobject {
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type HistoryBackend = super::HistoryBackendRust;
    }
}

/// No state yet.
#[derive(Default)]
pub struct HistoryBackendRust;
