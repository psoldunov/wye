//! XKB key names (KEY-03): the canonical spelling stored in the
//! configuration and the label shown in the interface.

/// `(canonical XKB name, label in the interface)`.
const NAMED: &[(&str, &str)] = &[
    ("Return", "Return"),
    ("KP_Enter", "Enter"),
    ("space", "Space"),
    ("Escape", "Esc"),
    ("Tab", "Tab"),
    ("BackSpace", "Backspace"),
    ("Delete", "Del"),
    ("Insert", "Ins"),
    ("Home", "Home"),
    ("End", "End"),
    ("Page_Up", "PgUp"),
    ("Page_Down", "PgDown"),
    ("Left", "Left"),
    ("Right", "Right"),
    ("Up", "Up"),
    ("Down", "Down"),
    ("Menu", "Menu"),
    ("Print", "Print"),
    ("Pause", "Pause"),
    ("KP_Add", "+ (Num)"),
    ("KP_Subtract", "- (Num)"),
    ("KP_Multiply", "* (Num)"),
    ("KP_Divide", "/ (Num)"),
    ("KP_Decimal", ". (Num)"),
];

/// `(XKB name, character)` for the punctuation keys. The configuration
/// stores the name; the interface shows the character.
const PUNCTUATION: &[(&str, char)] = &[
    ("comma", ','),
    ("period", '.'),
    ("slash", '/'),
    ("backslash", '\\'),
    ("semicolon", ';'),
    ("apostrophe", '\''),
    ("minus", '-'),
    ("equal", '='),
    ("plus", '+'),
    ("bracketleft", '['),
    ("bracketright", ']'),
    ("grave", '`'),
];

/// Other spellings people type or other toolkits report.
const ALIASES: &[(&str, &str)] = &[
    ("enter", "Return"),
    ("esc", "Escape"),
    ("backtab", "Tab"),
    ("iso_left_tab", "Tab"),
    ("del", "Delete"),
    ("ins", "Insert"),
    ("pgup", "Page_Up"),
    ("prior", "Page_Up"),
    ("pgdn", "Page_Down"),
    ("next", "Page_Down"),
    ("pgdown", "Page_Down"),
    ("backspace", "BackSpace"),
    ("spacebar", "space"),
];

const MODIFIER_NAMES: &[&str] = &[
    "shift",
    "shift_l",
    "shift_r",
    "ctrl",
    "control",
    "control_l",
    "control_r",
    "alt",
    "alt_l",
    "alt_r",
    "meta",
    "meta_l",
    "meta_r",
    "super",
    "super_l",
    "super_r",
    "hyper_l",
    "hyper_r",
    "iso_level3_shift",
    "caps_lock",
    "num_lock",
];

/// A key name that cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeyNameError {
    #[error("no key given")]
    Empty,
    #[error("{0:?} is a modifier, not a key")]
    Modifier(String),
    #[error("{0:?} is not a key name")]
    Unknown(String),
}

/// The canonical XKB spelling of a key: lower-case letters (`o`), digits,
/// the XKB names of punctuation (`comma`), and the usual spelling of named
/// keys (`Return`, `KP_Enter`, `space`, `F5`).
///
/// # Errors
///
/// Returns an error for an empty name, a modifier, or text that cannot be a
/// key name.
pub fn canonical_key(name: &str) -> Result<String, KeyNameError> {
    if name == " " {
        return Ok("space".to_owned());
    }
    let name = name.trim();
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err(KeyNameError::Empty);
    };
    if chars.next().is_none() {
        return Ok(single_char(first));
    }
    let lower = name.to_ascii_lowercase();
    if MODIFIER_NAMES.contains(&lower.as_str()) {
        return Err(KeyNameError::Modifier(name.to_owned()));
    }
    if let Some(&(canonical, _)) = NAMED.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
        return Ok(canonical.to_owned());
    }
    if let Some(&(name, _)) = PUNCTUATION.iter().find(|(n, _)| *n == lower) {
        return Ok(name.to_owned());
    }
    if let Some(&(_, canonical)) = ALIASES.iter().find(|(alias, _)| *alias == lower) {
        return Ok(canonical.to_owned());
    }
    if let Some(function) = function_key(&lower) {
        return Ok(function);
    }
    if let Some(keypad) = keypad_key(&lower) {
        return Ok(keypad);
    }
    if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        // An XKB name we do not know; keep its spelling.
        return Ok(name.to_owned());
    }
    Err(KeyNameError::Unknown(name.to_owned()))
}

fn single_char(c: char) -> String {
    if c == ' ' {
        return "space".to_owned();
    }
    if let Some(&(name, _)) = PUNCTUATION.iter().find(|(_, p)| *p == c) {
        return name.to_owned();
    }
    c.to_lowercase().collect()
}

fn function_key(lower: &str) -> Option<String> {
    let number: u8 = lower.strip_prefix('f')?.parse().ok()?;
    (1..=35).contains(&number).then(|| format!("F{number}"))
}

fn keypad_key(lower: &str) -> Option<String> {
    let digit = lower.strip_prefix("kp_")?;
    (digit.len() == 1 && digit.chars().all(|c| c.is_ascii_digit())).then(|| format!("KP_{digit}"))
}

/// The label of a canonical key name in the interface (KEY-03): `o` is `O`,
/// `comma` is `,`, `Escape` is `Esc`.
#[must_use]
pub fn display_key(canonical: &str) -> String {
    if let Some(&(_, label)) = NAMED.iter().find(|(n, _)| *n == canonical) {
        return label.to_owned();
    }
    if let Some(&(_, c)) = PUNCTUATION.iter().find(|(n, _)| *n == canonical) {
        return c.to_string();
    }
    if canonical.chars().count() == 1 {
        return canonical.to_uppercase();
    }
    canonical.replace("KP_", "").replace('_', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalises_single_characters() {
        assert_eq!(canonical_key("O").unwrap(), "o");
        assert_eq!(canonical_key("o").unwrap(), "o");
        assert_eq!(canonical_key("7").unwrap(), "7");
        assert_eq!(canonical_key(",").unwrap(), "comma");
        assert_eq!(canonical_key("+").unwrap(), "plus");
        assert_eq!(canonical_key(" ").unwrap(), "space");
        assert_eq!(canonical_key("Я").unwrap(), "я");
    }

    #[test]
    fn canonicalises_named_keys_case_insensitively() {
        for (input, expected) in [
            ("Return", "Return"),
            ("return", "Return"),
            ("ENTER", "Return"),
            ("kp_enter", "KP_Enter"),
            ("Space", "space"),
            ("esc", "Escape"),
            ("escape", "Escape"),
            ("pgup", "Page_Up"),
            ("Page_Down", "Page_Down"),
            ("backspace", "BackSpace"),
            ("ISO_Left_Tab", "Tab"),
            ("Backtab", "Tab"),
            ("f5", "F5"),
            ("F12", "F12"),
            ("kp_7", "KP_7"),
            ("Comma", "comma"),
            ("Menu", "Menu"),
        ] {
            assert_eq!(canonical_key(input).unwrap(), expected, "{input}");
        }
    }

    #[test]
    fn keeps_unknown_xkb_names() {
        assert_eq!(canonical_key("Cyrillic_ef").unwrap(), "Cyrillic_ef");
        assert_eq!(canonical_key("XF86AudioPlay").unwrap(), "XF86AudioPlay");
    }

    #[test]
    fn rejects_empty_modifier_and_garbage_names() {
        assert_eq!(canonical_key(""), Err(KeyNameError::Empty));
        assert_eq!(canonical_key("   "), Err(KeyNameError::Empty));
        assert!(matches!(
            canonical_key("Shift"),
            Err(KeyNameError::Modifier(_))
        ));
        assert!(matches!(
            canonical_key("Control_L"),
            Err(KeyNameError::Modifier(_))
        ));
        assert!(matches!(
            canonical_key("a b"),
            Err(KeyNameError::Unknown(_))
        ));
        assert!(matches!(
            canonical_key("ctrl-x"),
            Err(KeyNameError::Unknown(_))
        ));
    }

    #[test]
    fn labels_for_the_interface() {
        for (canonical, label) in [
            ("o", "O"),
            ("7", "7"),
            ("comma", ","),
            ("Return", "Return"),
            ("KP_Enter", "Enter"),
            ("Escape", "Esc"),
            ("space", "Space"),
            ("Page_Up", "PgUp"),
            ("F5", "F5"),
            ("KP_7", "7"),
        ] {
            assert_eq!(display_key(canonical), label, "{canonical}");
        }
    }

    #[test]
    fn canonical_names_survive_a_second_pass() {
        for (name, _) in NAMED {
            assert_eq!(canonical_key(name).unwrap(), *name);
        }
        for (name, _) in PUNCTUATION {
            assert_eq!(canonical_key(name).unwrap(), *name);
        }
    }
}
