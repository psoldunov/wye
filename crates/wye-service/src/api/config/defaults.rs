//! `GetDefaults(section)`: the default values of one part of the
//! configuration, for "Reset to Defaults" (KEY-04).

use serde_json::Value;
use wye_api::Error;
use wye_core::Config;

/// The defaults at `section`, a dotted key path in the file's spelling
/// (`picker.keys`, `shortcuts`); an empty section is the whole file.
///
/// # Errors
///
/// `InvalidArgs` for a section the configuration does not have.
pub(crate) fn section(section: &str) -> Result<Value, Error> {
    let defaults = serde_json::to_value(Config::default())
        .map_err(|error| Error::failed(format!("cannot encode the defaults: {error}")))?;
    let trimmed = section.trim();
    if trimmed.is_empty() {
        return Ok(defaults);
    }
    trimmed
        .split('.')
        .try_fold(&defaults, |value, key| value.get(key))
        .cloned()
        .ok_or_else(|| Error::invalid_args(format!("the configuration has no section `{section}`")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nested_section_is_found() {
        let keys = section("picker.keys").expect("exists");
        assert!(keys.get("open").is_some(), "{keys}");
    }

    #[test]
    fn the_empty_section_is_everything() {
        assert!(section("").expect("exists").get("general").is_some());
    }

    #[test]
    fn an_unknown_section_is_invalid() {
        assert!(matches!(section("nope.keys"), Err(Error::InvalidArgs(_))));
    }
}
