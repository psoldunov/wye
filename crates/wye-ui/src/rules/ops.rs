//! Changes to the rule list, each a new list (RUL-02 to RUL-07, RUL-20,
//! RUL-28), and the merge patch that saves one: rules are an array, so the
//! patch carries the whole list (RFC 7386 replaces arrays).

use serde_json::{Value, json};
use wye_core::Rule;

/// Rules taken out, with their positions, for the undo toast (RUL-06).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Removed {
    entries: Vec<(usize, Rule)>,
}

impl Removed {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// "Deleted “GitHub”" or "Deleted 3 rules".
    #[must_use]
    pub fn describe(&self) -> String {
        match self.entries.as_slice() {
            [] => String::new(),
            [(_, rule)] => format!("Deleted \u{201c}{}\u{201d}", rule.name),
            entries => format!("Deleted {} rules", entries.len()),
        }
    }

    /// `rules` with the removed rules back where they were, as far as the
    /// list still allows.
    #[must_use]
    pub fn restored(&self, rules: &[Rule]) -> Vec<Rule> {
        let mut restored = rules.to_vec();
        for (index, rule) in &self.entries {
            let at = (*index).min(restored.len());
            restored.insert(at, rule.clone());
        }
        restored
    }
}

/// The patch that saves `rules`.
#[must_use]
pub fn patch(rules: &[Rule]) -> Value {
    json!({ "rules": serde_json::to_value(rules).unwrap_or_else(|_| json!([])) })
}

/// RUL-07: rule `index` turned on or off.
#[must_use]
pub fn toggled(rules: &[Rule], index: usize, enabled: bool) -> Vec<Rule> {
    rules
        .iter()
        .enumerate()
        .map(|(at, rule)| {
            if at == index {
                Rule {
                    enabled,
                    ..rule.clone()
                }
            } else {
                rule.clone()
            }
        })
        .collect()
}

/// RUL-04: rule `from` moved to position `to`.
#[must_use]
pub fn moved(rules: &[Rule], from: usize, to: usize) -> Vec<Rule> {
    let mut moved = rules.to_vec();
    if from < moved.len() && from != to {
        let rule = moved.remove(from);
        moved.insert(to.min(moved.len()), rule);
    }
    moved
}

/// RUL-06, RUL-28: rule `index` removed, and what undo needs.
#[must_use]
pub fn removed(rules: &[Rule], index: usize) -> (Vec<Rule>, Removed) {
    let entries = rules
        .get(index)
        .map(|rule| vec![(index, rule.clone())])
        .unwrap_or_default();
    let rest = rules
        .iter()
        .enumerate()
        .filter(|(at, _)| *at != index)
        .map(|(_, rule)| rule.clone())
        .collect();
    (rest, Removed { entries })
}

/// RUL-02 "Delete All Rules…".
#[must_use]
pub fn all_removed(rules: &[Rule]) -> Removed {
    Removed {
        entries: rules.iter().cloned().enumerate().collect(),
    }
}

/// A copy of rule `index` right below it, named "<name> copy", without the
/// original's ID (and so without its script file).
#[must_use]
pub fn duplicated(rules: &[Rule], index: usize, id: &str) -> Vec<Rule> {
    let Some(original) = rules.get(index) else {
        return rules.to_vec();
    };
    let copy = Rule {
        id: Some(id.to_owned()),
        name: format!("{} copy", original.name),
        transform: false,
        ..original.clone()
    };
    let mut list = rules.to_vec();
    list.insert(index + 1, copy);
    list
}

/// RUL-20: `rule` saved at `index`, or added at the bottom for a new rule.
#[must_use]
pub fn saved(rules: &[Rule], index: Option<usize>, rule: Rule) -> Vec<Rule> {
    match index.filter(|index| *index < rules.len()) {
        Some(index) => rules
            .iter()
            .enumerate()
            .map(|(at, existing)| {
                if at == index {
                    rule.clone()
                } else {
                    existing.clone()
                }
            })
            .collect(),
        None => rules.iter().cloned().chain(std::iter::once(rule)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(name: &str) -> Rule {
        serde_json::from_value(json!({"name": name, "id": name})).expect("a rule")
    }

    fn names(rules: &[Rule]) -> Vec<&str> {
        rules.iter().map(|rule| rule.name.as_str()).collect()
    }

    fn abc() -> Vec<Rule> {
        vec![rule("a"), rule("b"), rule("c")]
    }

    #[test]
    fn rul_04_moving_reorders() {
        assert_eq!(names(&moved(&abc(), 0, 2)), ["b", "c", "a"]);
        assert_eq!(names(&moved(&abc(), 2, 0)), ["c", "a", "b"]);
        assert_eq!(names(&moved(&abc(), 5, 0)), ["a", "b", "c"]);
    }

    #[test]
    fn rul_06_removal_can_be_undone() {
        let (rest, removed) = removed(&abc(), 1);
        assert_eq!(names(&rest), ["a", "c"]);
        assert_eq!(removed.describe(), "Deleted \u{201c}b\u{201d}");
        assert_eq!(names(&removed.restored(&rest)), ["a", "b", "c"]);
        let all = all_removed(&abc());
        assert_eq!(all.describe(), "Deleted 3 rules");
        assert_eq!(names(&all.restored(&[])), ["a", "b", "c"]);
        assert!(super::removed(&abc(), 9).1.is_empty());
    }

    #[test]
    fn rul_07_toggling_touches_one_rule() {
        let off = toggled(&abc(), 1, false);
        assert!(off[0].enabled && !off[1].enabled && off[2].enabled);
    }

    #[test]
    fn duplicates_sit_below_and_own_no_script() {
        let mut original = abc();
        original[0].transform = true;
        let list = duplicated(&original, 0, "rule-9");
        assert_eq!(names(&list), ["a", "a copy", "b", "c"]);
        assert_eq!(list[1].id.as_deref(), Some("rule-9"));
        assert!(!list[1].transform);
    }

    #[test]
    fn rul_20_saving_replaces_or_appends() {
        assert_eq!(names(&saved(&abc(), Some(1), rule("B"))), ["a", "B", "c"]);
        assert_eq!(names(&saved(&abc(), None, rule("d"))), ["a", "b", "c", "d"]);
        let patch = patch(&[rule("a")]);
        assert_eq!(patch["rules"][0]["name"], "a");
    }
}
