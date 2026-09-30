//! Which parts of a link a script changed, for the editor's result line
//! (SCR-04).
//!
//! Links are split into tokens at URL delimiters, the tokens are compared
//! with a longest-common-subsequence table, and every run of tokens in the
//! new link that is not shared with the old one is reported. Ranges are in
//! UTF-16 code units, which is what QML's text positions count.

/// Delimiters that end a token and are tokens of their own.
const DELIMITERS: &[char] = &['/', '?', '#', '&', '=', '.', ':', '@', ';', ','];
/// Above this many tokens per side the table is not built and the changed
/// part is taken as everything between the common prefix and suffix.
const MAX_TOKENS: usize = 400;

/// Changed ranges of `after`, as `[start, end)` in UTF-16 units, sorted
/// and non-overlapping. Empty when the links are equal.
#[must_use]
pub fn changed_ranges(before: &str, after: &str) -> Vec<[u32; 2]> {
    let old = tokens(before);
    let new = tokens(after);
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let old_rest = old.get(prefix..).unwrap_or_default();
    let new_rest = new.get(prefix..).unwrap_or_default();
    let suffix = old_rest
        .iter()
        .rev()
        .zip(new_rest.iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let old_middle = old_rest.get(..old_rest.len() - suffix).unwrap_or_default();
    let new_middle = new_rest.get(..new_rest.len() - suffix).unwrap_or_default();
    let changed = if old_middle.len() > MAX_TOKENS || new_middle.len() > MAX_TOKENS {
        vec![true; new_middle.len()]
    } else {
        bridge_delimiters(new_middle, &not_in_common(old_middle, new_middle))
    };
    let offsets = utf16_offsets(&new);
    let mut ranges: Vec<[u32; 2]> = Vec::new();
    let spans = changed
        .iter()
        .enumerate()
        .filter(|(_, changed)| **changed)
        .filter_map(|(index, _)| {
            let token = prefix + index;
            Some((*offsets.get(token)?, *offsets.get(token + 1)?))
        });
    for (start, end) in spans {
        match ranges.last_mut() {
            Some(last) if last[1] == start => last[1] = end,
            _ => ranges.push([start, end]),
        }
    }
    ranges
}

/// `rows[i][j]`: length of the longest common subsequence of `old[i..]`
/// and `new[j..]`.
fn lcs_rows(old: &[String], new: &[String]) -> Vec<Vec<u16>> {
    let mut rows = vec![vec![0_u16; new.len() + 1]];
    for token in old.iter().rev() {
        let below = rows.last().cloned().unwrap_or_default();
        let mut reversed = vec![0_u16];
        for (j, candidate) in new.iter().enumerate().rev() {
            let right = reversed.last().copied().unwrap_or(0);
            let value = if token == candidate {
                below.get(j + 1).copied().unwrap_or(0).saturating_add(1)
            } else {
                below.get(j).copied().unwrap_or(0).max(right)
            };
            reversed.push(value);
        }
        reversed.reverse();
        rows.push(reversed);
    }
    rows.reverse();
    rows
}

/// For each token of `new`, whether it is outside one longest common
/// subsequence with `old`.
fn not_in_common(old: &[String], new: &[String]) -> Vec<bool> {
    let rows = lcs_rows(old, new);
    let length = |i: usize, j: usize| rows.get(i).and_then(|row| row.get(j)).copied().unwrap_or(0);
    let mut changed = vec![true; new.len()];
    let (mut i, mut j) = (0, 0);
    while let (Some(a), Some(b)) = (old.get(i), new.get(j)) {
        if a == b {
            if let Some(flag) = changed.get_mut(j) {
                *flag = false;
            }
            i += 1;
            j += 1;
        } else if length(i + 1, j) >= length(i, j + 1) {
            i += 1;
        } else {
            j += 1;
        }
    }
    changed
}

/// A lone delimiter between two changed tokens counts as changed, so
/// `invidious.example.org` is one range rather than three.
fn bridge_delimiters(tokens: &[String], changed: &[bool]) -> Vec<bool> {
    let flag =
        |index: Option<usize>| index.and_then(|index| changed.get(index)).copied() == Some(true);
    changed
        .iter()
        .zip(tokens)
        .enumerate()
        .map(|(index, (changed, token))| {
            *changed
                || (flag(index.checked_sub(1)) && flag(index.checked_add(1)) && is_delimiter(token))
        })
        .collect()
}

fn is_delimiter(token: &str) -> bool {
    let mut chars = token.chars();
    chars.next().is_some_and(|c| DELIMITERS.contains(&c)) && chars.next().is_none()
}

/// Runs of non-delimiters, and each delimiter on its own.
fn tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        if DELIMITERS.contains(&c) {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            tokens.push(c.to_string());
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Start offset of each token and the end of the last one, in UTF-16 units.
fn utf16_offsets(tokens: &[String]) -> Vec<u32> {
    std::iter::once(0)
        .chain(tokens.iter().scan(0_u32, |offset, token| {
            let units = u32::try_from(token.encode_utf16().count()).unwrap_or(u32::MAX);
            *offset = offset.saturating_add(units);
            Some(*offset)
        }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marked(after: &str, ranges: &[[u32; 2]]) -> Vec<String> {
        let units: Vec<u16> = after.encode_utf16().collect();
        ranges
            .iter()
            .map(|[start, end]| String::from_utf16_lossy(&units[*start as usize..*end as usize]))
            .collect()
    }

    #[test]
    fn equal_links_have_no_changes() {
        assert!(changed_ranges("https://a.example/", "https://a.example/").is_empty());
    }

    #[test]
    fn a_new_host_is_highlighted() {
        let after = "https://x.com/example/status/1";
        let ranges = changed_ranges("https://twitter.com/example/status/1", after);
        assert_eq!(marked(after, &ranges), ["x"]);
    }

    #[test]
    fn separate_changes_are_separate_ranges() {
        let after = "https://old.reddit.com/r/rust?sort=new";
        let ranges = changed_ranges("https://www.reddit.com/r/rust?sort=top", after);
        assert_eq!(marked(after, &ranges), ["old", "new"]);
    }

    #[test]
    fn a_new_link_is_highlighted_where_it_differs() {
        let after = "https://invidious.example.org/watch?v=abc";
        let ranges = changed_ranges("https://youtu.be/abc", after);
        assert_eq!(marked(after, &ranges), ["invidious.example.org/watch?v="]);
    }

    #[test]
    fn ranges_count_utf16_units() {
        let after = "https://example.com/\u{1f600}/b";
        let ranges = changed_ranges("https://example.com/a/b", after);
        assert_eq!(ranges, [[20, 22]]);
        assert_eq!(marked(after, &ranges), ["\u{1f600}"]);
    }

    #[test]
    fn removals_leave_nothing_to_highlight() {
        let ranges = changed_ranges(
            "https://example.com/?utm_source=test&id=1",
            "https://example.com/?id=1",
        );
        assert!(ranges.is_empty());
    }
}
