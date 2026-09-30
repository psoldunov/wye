//! Code-area helpers (SCR-02): bracket matching and auto-indent. Positions
//! are UTF-16 units, as QML's text positions count.

/// Spaces one indent level adds.
const INDENT: &str = "  ";
const PAIRS: [(u16, u16); 3] = [
    (b'(' as u16, b')' as u16),
    (b'[' as u16, b']' as u16),
    (b'{' as u16, b'}' as u16),
];

/// The bracket matching the one at `position` or just before it, if any.
///
/// A plain depth count: brackets inside strings and comments count too,
/// which is right for almost every transform script.
#[must_use]
pub fn matching_bracket(text: &str, position: usize) -> Option<usize> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let at = [Some(position), position.checked_sub(1)]
        .into_iter()
        .flatten()
        .find(|index| {
            units
                .get(*index)
                .is_some_and(|unit| bracket(*unit).is_some())
        })?;
    let (open, close, forward) = bracket(*units.get(at)?)?;
    let mut depth = 0_usize;
    let step = |index: usize| {
        if forward {
            index.checked_add(1)
        } else {
            index.checked_sub(1)
        }
    };
    let mut index = at;
    loop {
        let unit = *units.get(index)?;
        if unit == open {
            depth += 1;
        } else if unit == close {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
        index = step(index)?;
    }
}

/// `(the bracket, its partner, search forward)` for a bracket unit; for a
/// closing bracket the roles swap so the depth count works backwards.
fn bracket(unit: u16) -> Option<(u16, u16, bool)> {
    PAIRS.iter().find_map(|(open, close)| {
        if unit == *open {
            Some((*open, *close, true))
        } else if unit == *close {
            Some((*close, *open, false))
        } else {
            None
        }
    })
}

/// What Return inserts at `position`: a newline, the current line's
/// indentation, and one more level after an opening bracket.
#[must_use]
pub fn newline_with_indent(text: &str, position: usize) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let before = units.get(..position.min(units.len())).unwrap_or_default();
    let line = before
        .iter()
        .rposition(|unit| *unit == u16::from(b'\n'))
        .and_then(|newline| before.get(newline + 1..))
        .unwrap_or(before);
    let indent: String = String::from_utf16_lossy(line)
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();
    let opens = line
        .iter()
        .rev()
        .find(|unit| **unit != u16::from(b' ') && **unit != u16::from(b'\t'))
        .is_some_and(|unit| PAIRS.iter().any(|(open, _)| open == unit));
    let extra = if opens { INDENT } else { "" };
    format!("\n{indent}{extra}")
}

/// What Tab inserts.
#[must_use]
pub const fn indent() -> &'static str {
    INDENT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scr_02_brackets_match_both_ways() {
        let text = "f(a[1], {b: 2})";
        assert_eq!(matching_bracket(text, 1), Some(14));
        assert_eq!(matching_bracket(text, 15), Some(1), "cursor after `)`");
        assert_eq!(matching_bracket(text, 8), Some(13));
        assert_eq!(matching_bracket(text, 3), Some(5));
        assert_eq!(matching_bracket("abc", 1), None);
        assert_eq!(matching_bracket("(((", 0), None, "unbalanced");
        assert_eq!(matching_bracket("", 0), None);
    }

    #[test]
    fn scr_02_brackets_count_utf16_units() {
        // The emoji takes two units, so `)` is at 4.
        assert_eq!(matching_bracket("(\u{1f600}x)", 0), Some(4));
    }

    #[test]
    fn scr_02_return_keeps_and_deepens_the_indent() {
        let text = "function f() {\n  if (x) {";
        assert_eq!(newline_with_indent(text, 14), "\n  ");
        assert_eq!(newline_with_indent(text, text.len()), "\n    ");
        assert_eq!(newline_with_indent("  a = 1;", 8), "\n  ");
        assert_eq!(newline_with_indent("", 0), "\n");
        assert_eq!(indent(), "  ");
    }
}
