//! What the first test run waits for (SCR-04).
//!
//! The script (`GetScript`) and the test link (`GetHistory`) arrive in
//! separate answers, in either order. The first run starts once both are in,
//! so its result always belongs to the link shown in the field.

/// Which of the first run's inputs have arrived.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Readiness {
    script: bool,
    link: bool,
}

impl Readiness {
    /// Nothing has arrived yet.
    #[must_use]
    pub const fn waiting() -> Self {
        Self {
            script: false,
            link: false,
        }
    }

    /// The script has arrived.
    #[must_use]
    pub const fn with_script(self) -> Self {
        Self {
            script: true,
            ..self
        }
    }

    /// The test link has arrived.
    #[must_use]
    pub const fn with_link(self) -> Self {
        Self { link: true, ..self }
    }

    /// Both have arrived.
    #[must_use]
    pub const fn is_ready(self) -> bool {
        self.script && self.link
    }
}

#[cfg(test)]
mod tests {
    use super::Readiness;

    #[test]
    fn waits_for_both() {
        assert!(!Readiness::waiting().is_ready());
        assert!(!Readiness::waiting().with_script().is_ready());
        assert!(!Readiness::waiting().with_link().is_ready());
    }

    #[test]
    fn ready_in_either_order() {
        assert!(Readiness::waiting().with_script().with_link().is_ready());
        assert!(Readiness::waiting().with_link().with_script().is_ready());
    }

    #[test]
    fn repeated_arrivals_stay_ready() {
        let ready = Readiness::waiting().with_script().with_link();
        assert!(ready.with_script().is_ready());
        assert!(ready.with_link().is_ready());
    }
}
