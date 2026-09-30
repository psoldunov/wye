//! `PickerBackend`: the Rust side of the picker (02-picker.md). A stub so the surface's QML
//! compiles and loads; U05 fills it.
//!
//! Used from `qml/picker/`.

#[cxx_qt::bridge]
pub mod qobject {
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type PickerBackend = super::PickerBackendRust;
    }
}

/// No state yet.
#[derive(Default)]
pub struct PickerBackendRust;
