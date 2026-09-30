//! A read-only parser for the INI-like "key file" format shared by desktop
//! entries, `mimeapps.list` and Firefox's `profiles.ini`
//! ([Desktop Entry Specification, "Basic format of the file"]).
//!
//! [Desktop Entry Specification, "Basic format of the file"]: https://specifications.freedesktop.org/desktop-entry-spec/latest/basic-format.html

use crate::xdg::Locale;

/// One `[Group]` and its `Key=Value` lines, in file order. Values are raw:
/// escapes are left for the caller, because string lists and plain strings
/// unescape differently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub name: String,
    pub entries: Vec<(String, String)>,
}

impl Group {
    /// The raw value of the first `key` in the group.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }

    /// The unescaped string value of `key`.
    #[must_use]
    pub fn string(&self, key: &str) -> Option<String> {
        self.get(key).map(unescape)
    }

    /// The unescaped string-list value of `key`; empty when the key is
    /// missing.
    #[must_use]
    pub fn list(&self, key: &str) -> Vec<String> {
        self.get(key).map(unescape_list).unwrap_or_default()
    }

    /// The unescaped string value of `key` for `locale`: `Key[lang_COUNTRY@MODIFIER]`,
    /// `Key[lang_COUNTRY]`, `Key[lang@MODIFIER]`, `Key[lang]`, then the plain
    /// `Key`, as the Desktop Entry Specification orders them (DISC-03).
    #[must_use]
    pub fn localized_string(&self, key: &str, locale: &Locale) -> Option<String> {
        self.localized_raw(key, locale).map(unescape)
    }

    /// The unescaped string-list value of `key` for `locale`, looked up like
    /// [`Group::localized_string`]; empty when no variant of the key exists.
    #[must_use]
    pub fn localized_list(&self, key: &str, locale: &Locale) -> Vec<String> {
        self.localized_raw(key, locale)
            .map(unescape_list)
            .unwrap_or_default()
    }

    fn localized_raw(&self, key: &str, locale: &Locale) -> Option<&str> {
        locale
            .candidates()
            .iter()
            .find_map(|suffix| self.get(&format!("{key}[{suffix}]")))
            .or_else(|| self.get(key))
    }

    /// A boolean value; `true`/`1` are true, anything else is false.
    #[must_use]
    pub fn bool(&self, key: &str) -> bool {
        matches!(self.get(key).map(str::trim), Some("true" | "1"))
    }
}

/// Splits `text` into groups. Blank lines, comments and lines before the
/// first group header are skipped; lines without `=` are ignored.
#[must_use]
pub fn parse(text: &str) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for line in text.lines() {
        let line = line.trim_start();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(name) = group_header(line) {
            groups.push(Group {
                name: name.to_owned(),
                entries: Vec::new(),
            });
            continue;
        }
        let (Some(group), Some((key, value))) = (groups.last_mut(), line.split_once('=')) else {
            continue;
        };
        group
            .entries
            .push((key.trim_end().to_owned(), value.trim_start().to_owned()));
    }
    groups
}

/// The name inside a `[Group Name]` header line.
#[must_use]
pub fn group_header(line: &str) -> Option<&str> {
    line.trim_end()
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
}

/// Applies the string escapes `\s`, `\n`, `\t`, `\r` and `\\`. Unknown
/// escapes are kept as written.
#[must_use]
pub fn unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        push_escape(&mut out, chars.next());
    }
    out
}

/// Splits a string list on unescaped `;`, applying [`unescape`] plus `\;`.
/// Empty items (including the conventional trailing one) are dropped.
#[must_use]
pub fn unescape_list(raw: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        match c {
            ';' => items.push(std::mem::take(&mut current)),
            '\\' => match chars.next() {
                Some(';') => current.push(';'),
                next => push_escape(&mut current, next),
            },
            _ => current.push(c),
        }
    }
    items.push(current);
    items.retain(|item| !item.is_empty());
    items
}

/// Appends the character a backslash followed by `next` stands for; unknown
/// escapes and a trailing backslash are kept as written.
fn push_escape(out: &mut String, next: Option<char>) {
    match next.map(|c| (c, simple_escape(c))) {
        Some((_, Some(decoded))) => out.push(decoded),
        Some((c, None)) => {
            out.push('\\');
            out.push(c);
        }
        None => out.push('\\'),
    }
}

fn simple_escape(c: char) -> Option<char> {
    match c {
        's' => Some(' '),
        'n' => Some('\n'),
        't' => Some('\t'),
        'r' => Some('\r'),
        '\\' => Some('\\'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_groups_and_skips_comments() {
        let groups = parse(
            "# comment\nstray=1\n[Desktop Entry]\nName = Firefox\n\n[Desktop Action new]\nName=New\nbogus\n",
        );
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name, "Desktop Entry");
        assert_eq!(groups[0].get("Name"), Some("Firefox"));
        assert_eq!(groups[1].get("Name"), Some("New"));
        assert_eq!(groups[1].entries.len(), 1);
    }

    #[test]
    fn first_duplicate_key_wins() {
        let groups = parse("[G]\nA=1\nA=2\n");
        assert_eq!(groups[0].get("A"), Some("1"));
    }

    #[test]
    fn unescapes_strings() {
        assert_eq!(unescape(r"a\sb\tc\\d\ne\r"), "a b\tc\\d\ne\r");
        assert_eq!(unescape(r#"say \"hi\""#), r#"say \"hi\""#);
        assert_eq!(unescape("trailing\\"), "trailing\\");
    }

    #[test]
    fn splits_lists() {
        assert_eq!(unescape_list("a;b;c;"), vec!["a", "b", "c"]);
        assert_eq!(unescape_list(r"a\;b;c\sd"), vec!["a;b", "c d"]);
        assert!(unescape_list("").is_empty());
        assert_eq!(unescape_list(";;x"), vec!["x"]);
    }

    #[test]
    fn looks_up_localised_keys_in_specification_order() {
        let group = &parse(
            "[G]\nName=Plain\nName[de]=Deutsch\nName[de_AT]=Oesterreich\n\
             Name[de@euro]=Euro\nName[de_AT@euro]=AT Euro\nKeywords=a;b;\nKeywords[de]=x\\;y;z;\n",
        )[0];
        let name = |locale: &str| {
            let locale = Locale::parse(locale).unwrap_or_default();
            group.localized_string("Name", &locale)
        };
        assert_eq!(name("de_AT@euro").as_deref(), Some("AT Euro"));
        assert_eq!(name("de_AT").as_deref(), Some("Oesterreich"));
        assert_eq!(name("de_CH").as_deref(), Some("Deutsch"));
        assert_eq!(name("de@euro").as_deref(), Some("Euro"));
        assert_eq!(name("de_DE.UTF-8").as_deref(), Some("Deutsch"));
        assert_eq!(name("fr_FR").as_deref(), Some("Plain"));
        assert_eq!(name("C").as_deref(), Some("Plain"));
        assert_eq!(group.localized_string("Missing", &Locale::none()), None);

        let de = Locale::parse("de").unwrap();
        assert_eq!(group.localized_list("Keywords", &de), vec!["x;y", "z"]);
        assert_eq!(
            group.localized_list("Keywords", &Locale::none()),
            vec!["a", "b"]
        );
        assert!(group.localized_list("Nothing", &de).is_empty());
    }

    #[test]
    fn reads_booleans() {
        let group = &parse("[G]\nA=true\nB=false\nC=1\n")[0];
        assert!(group.bool("A"));
        assert!(!group.bool("B"));
        assert!(group.bool("C"));
        assert!(!group.bool("D"));
    }
}
