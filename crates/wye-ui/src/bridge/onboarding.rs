//! `OnboardingBackend`: the Rust side of first run (18-onboarding.md). A stub so the surface's QML
//! compiles and loads; U14 fills it.
//!
//! Used from `qml/onboarding/`.

#[cxx_qt::bridge]
pub mod qobject {
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type OnboardingBackend = super::OnboardingBackendRust;
    }
}

/// No state yet.
#[derive(Default)]
pub struct OnboardingBackendRust;
