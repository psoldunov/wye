//! JSON Merge Patch ([RFC 7386](https://www.rfc-editor.org/rfc/rfc7386)):
//! how every settings control changes the configuration (`UpdateConfig`).
//!
//! A patch is a JSON document shaped like the part of the configuration it
//! changes. Objects merge key by key, `null` deletes a key (the value falls
//! back to its default), and everything else, arrays included, replaces the
//! old value. A list such as `browsers.shown` or `rules` is therefore sent
//! whole.

use serde_json::{Map, Value};

use crate::config::Config;
use crate::target::Target;

/// Applies `patch` to `target` and returns the result; `target` is not
/// changed (RFC 7386, section 2).
#[must_use]
pub fn apply(target: &Value, patch: &Value) -> Value {
    let Value::Object(patch) = patch else {
        return patch.clone();
    };
    let mut merged = match target {
        Value::Object(members) => members.clone(),
        _ => Map::new(),
    };
    for (key, value) in patch {
        if value.is_null() {
            merged.remove(key);
        } else {
            let current = merged.get(key).unwrap_or(&Value::Null);
            let next = apply(current, value);
            merged.insert(key.clone(), next);
        }
    }
    Value::Object(merged)
}

/// Why a configuration patch was refused.
#[derive(Debug, thiserror::Error)]
pub enum PatchError {
    /// A configuration patch is an object; anything else would replace the
    /// whole configuration.
    #[error("a configuration patch must be a JSON object")]
    NotAnObject,
    #[error("the configuration cannot be written as JSON: {0}")]
    Serialize(serde_json::Error),
    /// The patched document is not a configuration: a value of the wrong
    /// type, or a target with two variants.
    #[error("the patch does not make a valid configuration: {0}")]
    Invalid(serde_json::Error),
}

/// Applies a merge patch to a configuration.
///
/// The configuration is written as JSON with the keys of the TOML file
/// (`picker.show-url`), patched, and read back, so the patched result must be
/// a valid configuration; the old one is returned untouched when it is not.
///
/// # Errors
///
/// Returns [`PatchError`] when the patch is not an object or the result is
/// not a configuration.
pub fn apply_to_config(config: &Config, patch: &Value) -> Result<Config, PatchError> {
    if !patch.is_object() {
        return Err(PatchError::NotAnObject);
    }
    let current = serde_json::to_value(config).map_err(PatchError::Serialize)?;
    serde_json::from_value(apply(&current, patch)).map_err(PatchError::Invalid)
}

/// The patch value that sets a target field to `target`.
///
/// A target is a table with one key naming its kind (`{ "app": "x" }`), and
/// a merge patch merges tables, so setting `{ "picker": true }` over
/// `{ "app": "x" }` would leave both keys. This patch names every kind and
/// clears the others, so it replaces whatever was there:
/// `{ "app": null, "picker": true, … }`.
#[must_use]
pub fn target_patch(target: &Target) -> Value {
    let mut patch: Map<String, Value> = TARGET_KINDS
        .iter()
        .map(|kind| ((*kind).to_owned(), Value::Null))
        .collect();
    if let Ok(Value::Object(set)) = serde_json::to_value(target) {
        patch.extend(set);
    }
    Value::Object(patch)
}

/// The keys of a target's table (`Target`'s serialised form).
const TARGET_KINDS: [&str; 6] = ["picker", "default", "app", "private", "profile", "custom"];

#[cfg(test)]
mod tests;
