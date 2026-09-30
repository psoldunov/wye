//! `ScriptEditorBackend`: the Rust side of the script editor (16-script-editor.md). A stub so the surface's QML
//! compiles and loads; U12 fills it.
//!
//! Used from `qml/script/`.

#[cxx_qt::bridge]
pub mod qobject {
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type ScriptEditorBackend = super::ScriptEditorBackendRust;
    }
}

/// No state yet.
#[derive(Default)]
pub struct ScriptEditorBackendRust;
