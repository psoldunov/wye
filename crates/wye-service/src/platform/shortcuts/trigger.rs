//! Turning a stored binding (KEY-03, `Ctrl+Shift+o`) into the trigger the
//! `GlobalShortcuts` portal expects as `preferred_trigger`.
//!
//! The portal takes the XDG "shortcuts" format: modifiers `CTRL`, `ALT`,
//! `SHIFT` and `LOGO` joined to an XKB key name with `+`, for example
//! `CTRL+ALT+w`. Wye already stores XKB key names, so only the modifiers are
//! renamed.

use wye_core::Modifier;
use wye_core::keybinding::KeyBinding;

/// The portal's name for a modifier.
const fn portal_modifier(modifier: Modifier) -> &'static str {
    match modifier {
        Modifier::Ctrl => "CTRL",
        Modifier::Alt => "ALT",
        Modifier::Shift => "SHIFT",
        Modifier::Super => "LOGO",
    }
}

/// The modifiers in the order the portal's examples use.
const PORTAL_ORDER: [Modifier; 4] = [
    Modifier::Ctrl,
    Modifier::Alt,
    Modifier::Shift,
    Modifier::Super,
];

/// `stored` as a portal trigger, or `None` when it is empty or not a
/// binding Wye can read (the portal then asks without a preference).
#[must_use]
pub fn portal_trigger(stored: &str) -> Option<String> {
    if stored.trim().is_empty() {
        return None;
    }
    let binding: KeyBinding = stored
        .parse()
        .inspect_err(|error| tracing::warn!(%error, stored, "cannot read the shortcut"))
        .ok()?;
    let modifiers = binding.modifiers();
    let parts: Vec<&str> = PORTAL_ORDER
        .into_iter()
        .filter(|modifier| modifiers.contains(*modifier))
        .map(|modifier| -> &str { portal_modifier(modifier) })
        .chain(std::iter::once(binding.key()))
        .collect();
    Some(parts.join("+"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_take_the_portal_names_key_40() {
        assert_eq!(portal_trigger("Ctrl+Alt+w").as_deref(), Some("CTRL+ALT+w"));
        assert_eq!(
            portal_trigger("Super+Shift+o").as_deref(),
            Some("SHIFT+LOGO+o")
        );
    }

    #[test]
    fn keys_keep_their_xkb_names() {
        assert_eq!(
            portal_trigger("Ctrl+Return").as_deref(),
            Some("CTRL+Return")
        );
        assert_eq!(portal_trigger("F9").as_deref(), Some("F9"));
    }

    #[test]
    fn any_spelling_is_canonicalised_first() {
        assert_eq!(
            portal_trigger("meta+control+W").as_deref(),
            Some("CTRL+LOGO+w")
        );
    }

    #[test]
    fn empty_or_unreadable_bindings_have_no_preference() {
        assert_eq!(portal_trigger(""), None);
        assert_eq!(portal_trigger("  "), None);
        assert_eq!(portal_trigger("Hyper+w"), None);
    }
}
