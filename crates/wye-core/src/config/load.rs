//! Lenient loading: a value that cannot be read is dropped with a warning
//! and keeps its default, while the rest of the file still applies.
//!
//! Every key is tried on its own against the whole configuration type. The
//! page sections are checked key by key, so one bad switch does not reset
//! its neighbours; rules and shown browsers are checked entry by entry, so
//! one bad rule does not drop the others. Anything else (a target, a key
//! list, a modifier set) is taken or dropped as a whole.

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

/// Tries a candidate table; returns the error message when it does not fit.
type Probe<'a> = dyn Fn(Table) -> Result<(), String> + 'a;

/// Reads `table` as `T`, dropping what cannot be read and reporting unknown
/// keys with their dotted path (`extras.force-http`).
pub(super) fn lenient<T: DeserializeOwned + Default>(
    table: Table,
    warnings: &mut Vec<ConfigWarning>,
) -> T {
    let probe = |candidate: Table| {
        candidate
            .try_into::<T>()
            .map(drop)
            .map_err(|error| error.message().to_owned())
    };
    let kept = prune(table, "", &probe, warnings);
    let read = serde_ignored::deserialize(Value::Table(kept), |path| {
        warnings.push(ConfigWarning::UnknownKey(path.to_string()));
    });
    read.unwrap_or_else(|error| {
        warnings.push(ConfigWarning::Unusable(error.message().to_owned()));
        T::default()
    })
}

fn prune(table: Table, path: &str, probe: &Probe<'_>, warnings: &mut Vec<ConfigWarning>) -> Table {
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
                let inner = prune(inner, &key_path, &nested, warnings);
                kept.insert(key, Value::Table(inner));
            }
            Value::Array(entries) if LISTS.contains(&key_path.as_str()) => {
                let entries = entries
                    .into_iter()
                    .enumerate()
                    .filter_map(|(index, entry)| {
                        match probe(single(Value::Array(vec![entry.clone()]))) {
                            Ok(()) => Some(entry),
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
                    .collect();
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
