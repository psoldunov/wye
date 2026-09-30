//! What travels with a link: the keys of the `a{sv}` context of `OpenLink`,
//! `TestLink` and `RunScript`, the options of `PickerChose`, and the string
//! values they take (IN-01, IN-05, IN-07, PIPE-01).
//!
//! Every key is optional. A missing `entry` means [`Entry::Handler`], a missing
//! `force` means [`Force::None`], a missing `held-known` means `false`.

/// `s`: desktop ID of the app the link came from, when the caller knows it.
pub const SOURCE_DESKTOP_ID: &str = "source-desktop-id";
/// `s`: executable of the source app, when there is no desktop ID.
pub const SOURCE_EXECUTABLE: &str = "source-executable";
/// `u`: process to start source-app detection at; the service detects.
pub const SOURCE_PID: &str = "source-pid";
/// `s`: `XDG_ACTIVATION_TOKEN` handed to the launched app (LAUNCH-03).
pub const ACTIVATION_TOKEN: &str = "activation-token";
/// `s`: X11 `DESKTOP_STARTUP_ID` handed to the launched app (LAUNCH-03).
pub const STARTUP_ID: &str = "startup-id";
/// `as`: modifiers held when the link was opened, as [`Modifier`] names.
pub const HELD: &str = "held";
/// `b`: whether `held` is known; `false` makes the service probe.
pub const HELD_KNOWN: &str = "held-known";
/// `s`: an [`Entry`] value.
pub const ENTRY: &str = "entry";
/// `s`: a [`Force`] value.
pub const FORCE: &str = "force";
/// `b`, `TestLink` only: do not contact short-link services.
pub const SKIP_NETWORK: &str = "skip-network";

/// `PickerChose` option `b`: open in a private window (PICK-20).
pub const OPTION_PRIVATE: &str = "private";
/// `PickerChose` option `b`: open in the background (PICK-21).
pub const OPTION_BACKGROUND: &str = "background";
/// `PickerChose` option `b`: force a new window (PICK-32).
pub const OPTION_NEW_WINDOW: &str = "new-window";
/// `PickerChose` option `s`: activation token from the picker's input event
/// (PICK-33); same key as [`ACTIVATION_TOKEN`].
pub const OPTION_ACTIVATION_TOKEN: &str = ACTIVATION_TOKEN;

/// `platform_data` key of `org.freedesktop.Application` carrying the
/// Wayland activation token.
pub const PLATFORM_ACTIVATION_TOKEN: &str = "activation-token";
/// `platform_data` key carrying the X11 startup ID.
pub const PLATFORM_STARTUP_ID: &str = "desktop-startup-id";

wire_enum! {
    /// How a link reached Wye (11-url-pipeline.md, "Entry points").
    #[derive(Default)]
    pub enum Entry as "entry" {
        /// Opened in another app while Wye is the default browser.
        #[default]
        Handler = "handler",
        /// Tray item or global shortcut reading the clipboard.
        Clipboard = "clipboard",
        /// The browser extension, over native messaging.
        Extension = "extension",
        /// `wye open` and friends.
        Cli = "cli",
    }
}

wire_enum! {
    /// A caller's override of the normal decision.
    #[derive(Default)]
    pub enum Force as "force" {
        /// Decide normally.
        #[default]
        None = "none",
        /// Always show the picker (PIPE-11).
        Picker = "picker",
        /// As if the alternative-browser key were held (PIPE-06, IN-04).
        Alternative = "alternative",
    }
}

wire_enum! {
    /// A modifier key. Left and right count as the same key.
    pub enum Modifier as "modifier" {
        Shift = "Shift",
        Ctrl = "Ctrl",
        Alt = "Alt",
        Super = "Super",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_value_parses_its_own_spelling() {
        for entry in Entry::ALL {
            assert_eq!(entry.as_str().parse::<Entry>(), Ok(*entry));
        }
        for force in Force::ALL {
            assert_eq!(force.as_str().parse::<Force>(), Ok(*force));
        }
        for modifier in Modifier::ALL {
            assert_eq!(modifier.as_str().parse::<Modifier>(), Ok(*modifier));
        }
    }

    #[test]
    fn an_unknown_value_names_what_it_was_meant_to_be() {
        let error = "sideways".parse::<Force>().expect_err("rejected");
        assert_eq!(error.to_string(), "unknown force `sideways`");
    }

    #[test]
    fn defaults_are_a_plain_handler_link() {
        assert_eq!(Entry::default(), Entry::Handler);
        assert_eq!(Force::default(), Force::None);
    }

    #[test]
    fn modifiers_serialise_with_their_key_names() {
        assert_eq!(
            serde_json::to_string(&[Modifier::Shift, Modifier::Ctrl]).expect("encodes"),
            r#"["Shift","Ctrl"]"#
        );
    }
}
