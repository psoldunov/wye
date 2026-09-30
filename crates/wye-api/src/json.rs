//! Encoding and decoding the JSON carried in `s` values.

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::Error;

/// Encode a payload for the bus.
///
/// # Errors
///
/// [`Error::failed`] when the value cannot be represented as JSON, which for
/// this crate's types means a bug in the service.
pub fn encode<T: Serialize>(value: &T) -> Result<String, Error> {
    serde_json::to_string(value)
        .map_err(|error| Error::failed(format!("cannot encode JSON: {error}")))
}

/// Decode a payload received from the bus.
///
/// # Errors
///
/// [`Error::invalid_args`] naming `what` when the text is not valid JSON of
/// the expected shape.
pub fn decode<T: DeserializeOwned>(what: &str, text: &str) -> Result<T, Error> {
    serde_json::from_str(text).map_err(|error| Error::invalid_args(format!("{what}: {error}")))
}

#[cfg(test)]
mod tests {
    use zbus::DBusError as _;

    use super::*;
    use crate::tray::TrayMenu;

    #[test]
    fn a_payload_survives_the_round_trip() {
        let menu = TrayMenu::default();
        let text = encode(&menu).expect("encodes");
        assert_eq!(decode::<TrayMenu>("tray", &text).expect("decodes"), menu);
    }

    #[test]
    fn malformed_input_is_the_callers_mistake() {
        let error = decode::<TrayMenu>("tray menu", "{").expect_err("rejected");
        assert_eq!(
            error.name().as_str(),
            "org.freedesktop.DBus.Error.InvalidArgs"
        );
        assert!(
            error
                .description()
                .is_some_and(|text| text.starts_with("tray menu:")),
            "{error:?}"
        );
    }
}
