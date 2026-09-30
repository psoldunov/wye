//! Corrections for values that parse but cannot apply. The loader only
//! reports them, so saving the file keeps what the user wrote; the pipeline
//! routes with the corrected copy.

use std::fmt;
use std::ops::RangeInclusive;

use super::{Advanced, Browsers, Config, ConfigWarning, ExpansionSettings, PickerKeys, ShownEntry};
use crate::rule::Rule;
use crate::target::Target;

impl Config {
    /// Returns a copy with every value that cannot apply replaced, plus a
    /// warning for each replacement.
    #[must_use]
    pub fn sanitized(&self, known_services: &[&str]) -> (Self, Vec<ConfigWarning>) {
        let mut warnings = Vec::new();
        let browsers = self.browsers.sanitized(&self.picker.keys, &mut warnings);
        check_held_modifiers(&self.picker.keys, &mut warnings);
        let apps = self
            .apps
            .iter()
            .filter(|(id, target)| {
                let known = known_services.contains(&id.as_str());
                if !known {
                    warnings.push(ConfigWarning::UnknownService((*id).clone()));
                }
                known && **target != Target::Default
            })
            .map(|(id, target)| (id.clone(), target.clone()))
            .collect();
        check_rules(&self.rules, &mut warnings);
        let advanced = Advanced {
            expansion: self.advanced.expansion.sanitized(&mut warnings),
            ..self.advanced.clone()
        };
        let config = Self {
            browsers,
            apps,
            advanced,
            ..self.clone()
        };
        (config, warnings)
    }
}

impl Browsers {
    fn sanitized(&self, keys: &PickerKeys, warnings: &mut Vec<ConfigWarning>) -> Self {
        let concrete_or_picker = |target: &Target, key: &'static str, warnings: &mut Vec<_>| {
            if *target == Target::Default {
                warnings.push(ConfigWarning::DefaultNotAllowed(key));
                Target::Picker
            } else {
                target.clone()
            }
        };
        let primary = concrete_or_picker(&self.primary, "browsers.primary", warnings);
        let alternative = concrete_or_picker(&self.alternative, "browsers.alternative", warnings);

        let action_keys: Vec<String> = keys.action_keys().map(str::to_lowercase).collect();
        let mut seen_hotkeys: Vec<String> = Vec::new();
        let shown = self
            .shown
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| {
                if !entry.target.is_concrete() {
                    warnings.push(ConfigWarning::ShownNotConcrete(i));
                    return None;
                }
                let hotkey = entry.hotkey.as_ref().and_then(|key| {
                    let lower = key.to_lowercase();
                    if !is_single_key(key) {
                        warnings.push(ConfigWarning::InvalidHotkey(key.clone()));
                        None
                    } else if action_keys.contains(&lower) {
                        warnings.push(ConfigWarning::HotkeyTakenByAction(key.clone()));
                        None
                    } else if seen_hotkeys.contains(&lower) {
                        warnings.push(ConfigWarning::DuplicateHotkey(key.clone()));
                        None
                    } else {
                        seen_hotkeys.push(lower);
                        Some(key.clone())
                    }
                });
                Some(ShownEntry {
                    target: entry.target.clone(),
                    hotkey,
                })
            })
            .collect();
        Self {
            primary,
            alternative,
            alternative_key: self.alternative_key,
            shown,
        }
    }
}

/// KEY-11: a target hotkey is one key without modifiers: a character or an
/// XKB key name, never a combination such as `Ctrl+a`.
fn is_single_key(key: &str) -> bool {
    !key.trim().is_empty() && !key.contains('+') && !key.chars().any(char::is_whitespace)
}

/// KEY-21: the three held-modifier actions need different modifier sets.
/// Reported only; the picker resolves a clash in the order listed here.
fn check_held_modifiers(keys: &PickerKeys, warnings: &mut Vec<ConfigWarning>) {
    let held = [
        ("picker.keys.private-modifier", keys.private_modifier),
        ("picker.keys.background-modifier", keys.background_modifier),
        ("picker.keys.new-window-modifier", keys.new_window_modifier),
    ];
    for (i, &(first, a)) in held.iter().enumerate() {
        for &(second, b) in held.iter().skip(i + 1) {
            if a == b {
                warnings.push(ConfigWarning::ModifierClash { first, second });
            }
        }
    }
}

impl ExpansionSettings {
    fn sanitized(&self, warnings: &mut Vec<ConfigWarning>) -> Self {
        let timeout_ms = clamp_setting(
            "advanced.expansion.timeout-ms",
            self.timeout_ms,
            &Self::TIMEOUT_RANGE,
            warnings,
        );
        let max_redirects = clamp_setting(
            "advanced.expansion.max-redirects",
            self.max_redirects,
            &Self::REDIRECT_RANGE,
            warnings,
        );
        Self {
            timeout_ms,
            max_redirects,
            ..self.clone()
        }
    }
}

fn clamp_setting<T: Copy + Ord + fmt::Display>(
    key: &'static str,
    value: T,
    range: &RangeInclusive<T>,
    warnings: &mut Vec<ConfigWarning>,
) -> T {
    let used = value.clamp(*range.start(), *range.end());
    if used != value {
        warnings.push(ConfigWarning::OutOfRange {
            key,
            value: value.to_string(),
            used: used.to_string(),
        });
    }
    used
}

fn check_rules(rules: &[Rule], warnings: &mut Vec<ConfigWarning>) {
    let mut ids: Vec<&str> = Vec::new();
    for (index, rule) in rules.iter().enumerate() {
        if let Err(errors) = rule.compile() {
            warnings.push(ConfigWarning::InvalidRule {
                index,
                name: rule.name.clone(),
                errors,
            });
        }
        if let Some(id) = rule.id.as_deref() {
            if ids.contains(&id) {
                warnings.push(ConfigWarning::DuplicateRuleId(id.to_owned()));
            }
            ids.push(id);
        }
    }
}
