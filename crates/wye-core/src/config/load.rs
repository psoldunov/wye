//! Lenient loading: a value that cannot be read is dropped with a warning
//! and keeps its default, while the rest of the file still applies.
//!
//! Every key is tried on its own against the whole configuration type. The
//! page sections are checked key by key, so one bad switch does not reset
//! its neighbours; rules and shown browsers are checked entry by entry, so
//! one bad rule does not drop the others. Anything else (a target, a key
//! list, a modifier set) is taken or dropped as a whole.
//!
//! Every warning names a position in the file, not in the list left after
//! unreadable entries were dropped: [`Kept`] remembers where each kept entry
//! came from.

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use toml::{Table, Value};

use super::ConfigWarning;

/// Tables whose keys are checked one by one.
const SECTIONS: &[&str] = &[
    "general",
    "browsers",
    "apps",
    "picker",
    "picker.keys",
    "extras",
    "advanced",
    "advanced.expansion",
    "shortcuts",
];

/// Arrays whose entries are checked one by one.
const LISTS: &[&str] = &["rules", "browsers.shown"];

/// File positions of the entries kept in a list, by dotted list key. A list
/// without an entry lost nothing, so its positions are the loaded ones.
#[derive(Debug, Default)]
pub(super) struct Kept(BTreeMap<String, Vec<usize>>);

impl Kept {
    /// The file position of entry `loaded` of the list `key`.
    fn file_index(&self, key: &str, loaded: usize) -> usize {
        self.0
            .get(key)
            .and_then(|positions| positions.get(loaded))
            .copied()
            .unwrap_or(loaded)
    }

    /// Rewrites the list position in an unknown-key path such as
    /// `rules.0.colour`.
    fn file_path(&self, path: &str) -> String {
        for key in self.0.keys() {
            let Some((segment, tail)) = path
                .strip_prefix(key.as_str())
                .and_then(|rest| rest.strip_prefix('.'))
                .map(|rest| rest.split_once('.').unwrap_or((rest, "")))
            else {
                continue;
            };
            if let Ok(loaded) = segment.parse::<usize>() {
                let tail = if tail.is_empty() {
                    String::new()
                } else {
                    format!(".{tail}")
                };
                return format!("{key}.{}{tail}", self.file_index(key, loaded));
            }
        }
        path.to_owned()
    }

    /// Rewrites the list position a sanitizing warning carries, from the
    /// position in the loaded lists to the one in the file.
    pub(super) fn locate(&self, warning: ConfigWarning) -> ConfigWarning {
        match warning {
            ConfigWarning::ShownNotConcrete(index) => {
                ConfigWarning::ShownNotConcrete(self.file_index("browsers.shown", index))
            }
            ConfigWarning::InvalidRule {
                index,
                name,
                errors,
            } => ConfigWarning::InvalidRule {
                index: self.file_index("rules", index),
                name,
                errors,
            },
            other => other,
        }
    }
}

/// Tries a candidate table; returns the error message when it does not fit.
type Probe<'a> = dyn Fn(Table) -> Result<(), String> + 'a;

/// Reads `table` as `T`, dropping what cannot be read and reporting unknown
/// keys with their dotted path (`extras.force-http`). Also returns where the
/// entries of each list came from in the file.
pub(super) fn lenient<T: DeserializeOwned + Default>(
    table: Table,
    warnings: &mut Vec<ConfigWarning>,
) -> (T, Kept) {
    let probe = |candidate: Table| {
        candidate
            .try_into::<T>()
            .map(drop)
            .map_err(|error| error.message().to_owned())
    };
    let mut positions = Kept::default();
    let kept = prune(table, "", &probe, warnings, &mut positions);
    let read = serde_ignored::deserialize(Value::Table(kept), |path| {
        warnings.push(ConfigWarning::UnknownKey(
            positions.file_path(&path.to_string()),
        ));
    });
    let config = read.unwrap_or_else(|error| {
        warnings.push(ConfigWarning::Unusable(error.message().to_owned()));
        T::default()
    });
    (config, positions)
}

fn prune(
    table: Table,
    path: &str,
    probe: &Probe<'_>,
    warnings: &mut Vec<ConfigWarning>,
    positions: &mut Kept,
) -> Table {
    let mut kept = Table::new();
    for (key, value) in table {
        let key_path = if path.is_empty() {
            key.clone()
        } else {
            format!("{path}.{key}")
        };
        let single = |value: Value| Table::from_iter([(key.clone(), value)]);
        let Err(error) = probe(single(value.clone())) else {
            kept.insert(key, value);
            continue;
        };
        match value {
            Value::Table(inner) if SECTIONS.contains(&key_path.as_str()) => {
                let nested = |candidate: Table| probe(single(Value::Table(candidate)));
                let inner = prune(inner, &key_path, &nested, warnings, positions);
                kept.insert(key, Value::Table(inner));
            }
            Value::Array(entries) if LISTS.contains(&key_path.as_str()) => {
                let (entries, file_positions): (Vec<_>, Vec<_>) = entries
                    .into_iter()
                    .enumerate()
                    .filter_map(|(index, entry)| {
                        match probe(single(Value::Array(vec![entry.clone()]))) {
                            Ok(()) => Some((entry, index)),
                            Err(error) => {
                                warnings.push(ConfigWarning::InvalidEntry {
                                    key: key_path.clone(),
                                    index,
                                    error,
                                });
                                None
                            }
                        }
                    })
                    .unzip();
                positions.0.insert(key_path, file_positions);
                kept.insert(key, Value::Array(entries));
            }
            _ => warnings.push(ConfigWarning::InvalidValue {
                key: key_path,
                error,
            }),
        }
    }
    kept
}
