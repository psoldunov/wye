//! The first-run window's steps and how the user moves between them
//! (ONB-01 to ONB-06). No Qt: QML asks [`Flow`] where it is and what the
//! buttons do, and the bridge acts on the [`Effect`] of a move.

use serde::Serialize;

/// One page of the walk-through (ONB-01 to ONB-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Step {
    /// ONB-01.
    Welcome,
    /// ONB-02.
    DefaultBrowser,
    /// ONB-03.
    Browsers,
    /// ONB-04.
    Integration,
    /// ONB-05.
    Extension,
}

impl Step {
    /// Every step, in order.
    pub const ALL: [Self; 5] = [
        Self::Welcome,
        Self::DefaultBrowser,
        Self::Browsers,
        Self::Integration,
        Self::Extension,
    ];

    /// The step's place in [`Step::ALL`]; the progress dot that is lit
    /// (ONB-06).
    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|step| *step == self)
            .unwrap_or_default()
    }

    /// The step at `index`, or `None` past the ends.
    #[must_use]
    pub fn at(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }
}

/// What the bridge does when a move lands on a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Nothing to do.
    None,
    /// The step is the browsers list: store the pre-checked browsers when
    /// the configuration has no shown list yet (ONB-03).
    SeedShownBrowsers,
    /// The walk-through is over: `onboardingDone` becomes true (ONB-06).
    Finish,
}

/// Where the user is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flow {
    step: Step,
}

impl Default for Flow {
    fn default() -> Self {
        Self::new()
    }
}

impl Flow {
    /// The start: the welcome step.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            step: Step::Welcome,
        }
    }

    #[must_use]
    pub const fn step(self) -> Step {
        self.step
    }

    /// Back is on every step after the first (ONB-06).
    #[must_use]
    pub fn can_go_back(self) -> bool {
        self.step.index() > 0
    }

    /// The last step's button is **Done** (ONB-05).
    #[must_use]
    pub fn is_last(self) -> bool {
        self.step.index() + 1 == Step::ALL.len()
    }

    /// The step after this one, and what arriving there asks for. On the
    /// last step it stays and asks to finish (ONB-05).
    #[must_use]
    pub fn next(self) -> (Self, Effect) {
        match Step::at(self.step.index() + 1) {
            Some(step) => (Self { step }, Self::arrival(step)),
            None => (self, Effect::Finish),
        }
    }

    /// The step before this one; the first stays (ONB-06).
    #[must_use]
    pub fn back(self) -> Self {
        Step::at(self.step.index().wrapping_sub(1)).map_or(self, |step| Self { step })
    }

    /// Jump to `index`, which QML reports when its page row moved on its own
    /// (an out-of-range index changes nothing).
    #[must_use]
    pub fn at(self, index: usize) -> (Self, Effect) {
        Step::at(index).map_or((self, Effect::None), |step| {
            (Self { step }, Self::arrival(step))
        })
    }

    fn arrival(step: Step) -> Effect {
        match step {
            Step::Browsers => Effect::SeedShownBrowsers,
            _ => Effect::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_walk_through_goes_welcome_to_extension() {
        // ONB-01 to ONB-05
        let mut flow = Flow::new();
        let mut seen = vec![flow.step()];
        while !flow.is_last() {
            flow = flow.next().0;
            seen.push(flow.step());
        }
        assert_eq!(seen, Step::ALL);
    }

    #[test]
    fn there_is_no_back_on_the_first_step_and_a_back_after_it() {
        // ONB-06
        let first = Flow::new();
        assert!(!first.can_go_back());
        assert_eq!(first.back(), first);
        let second = first.next().0;
        assert!(second.can_go_back());
        assert_eq!(second.back(), first);
    }

    #[test]
    fn next_on_the_last_step_finishes() {
        // ONB-05, ONB-06
        let last = Flow::new().at(Step::ALL.len() - 1).0;
        assert!(last.is_last());
        assert_eq!(last.next(), (last, Effect::Finish));
    }

    #[test]
    fn arriving_at_the_browsers_step_seeds_the_shown_list() {
        // ONB-03
        let (flow, effect) = Flow::new().next();
        assert_eq!((flow.step(), effect), (Step::DefaultBrowser, Effect::None));
        let (flow, effect) = flow.next();
        assert_eq!(
            (flow.step(), effect),
            (Step::Browsers, Effect::SeedShownBrowsers)
        );
        assert_eq!(flow.next().1, Effect::None);
    }

    #[test]
    fn an_index_outside_the_steps_changes_nothing() {
        let flow = Flow::new().next().0;
        assert_eq!(flow.at(99), (flow, Effect::None));
    }

    #[test]
    fn every_step_knows_its_dot() {
        for (index, step) in Step::ALL.into_iter().enumerate() {
            assert_eq!(step.index(), index);
        }
    }
}
