//! Firefox profile groups (DISC-07, Firefox 138+): profiles kept in a SQLite
//! store next to `profiles.ini`.
//!
//! `profiles.ini` names the store with a `StoreID` key, and the store is
//! `<firefox dir>/Profile Groups/<StoreID>.sqlite`. Its `Profiles` table has
//! one row per profile: `path`, `name`, `avatar` and the theme colours
//! `themeId`, `themeFg`, `themeBg`. The format is Firefox-internal and
//! undocumented, so every column except `path` and `name` is optional here
//! and any trouble with the store gives a [`GroupError`] that the caller
//! turns into a warning, leaving the classic profiles in place.
//!
//! The store is opened read-only with `immutable=1`: Firefox keeps it open
//! and locked, and Wye only wants a snapshot.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use super::badge::{badge, is_light, parse_css_color};
use super::{Profile, avatar_file};
use crate::keyfile;

const STORE_DIR: &str = "Profile Groups";
const TABLE: &str = "Profiles";
/// Firefox's name for an avatar the user picked a picture for.
const CUSTOM_AVATAR: &str = "custom";

/// The profile group store could not be used.
#[derive(Debug, thiserror::Error)]
pub enum GroupError {
    #[error("the profile group store {path} is missing")]
    Missing { path: PathBuf },
    #[error("{id:?} is not a profile group store ID")]
    BadStoreId { id: String },
    #[error("cannot read the profile group store {path}: {source}")]
    Sqlite {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },
    #[error("the profile group store {path} has no usable `Profiles` table")]
    NoTable { path: PathBuf },
}

/// The store ID `profiles.ini` names: `StoreID` in `[General]`, else in any
/// other group. Empty values count as none.
#[must_use]
pub fn store_id(profiles_ini: &str) -> Option<String> {
    let groups = keyfile::parse(profiles_ini);
    let in_group = |general: bool| {
        groups
            .iter()
            .filter(|group| (group.name == "General") == general)
            .find_map(|group| group.get("StoreID"))
    };
    in_group(true)
        .or_else(|| in_group(false))
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
}

/// One row of the store, before it is resolved to a directory.
struct Row {
    path: String,
    name: String,
    avatar: Option<String>,
    theme_bg: Option<String>,
    theme_fg: Option<String>,
}

/// Reads the profiles of the group store `store_id` under the Firefox
/// directory `dir`, sorted as the store lists them.
///
/// A row's `path` is relative to `dir` (or to `dir/Profiles`); absolute
/// paths are kept. Rows whose directory does not exist are skipped and
/// named in `warnings`. The profile's ID is its `path` as stored.
///
/// # Errors
///
/// Returns [`GroupError`] when the store ID is not plain, the file is
/// missing or is not a database, or the `Profiles` table lacks `path` or
/// `name`.
pub fn read(
    dir: &Path,
    store_id: &str,
    warnings: &mut Vec<String>,
) -> Result<Vec<Profile>, GroupError> {
    if store_id.is_empty()
        || !store_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(GroupError::BadStoreId {
            id: store_id.to_owned(),
        });
    }
    let store = dir.join(STORE_DIR).join(format!("{store_id}.sqlite"));
    if !store.is_file() {
        return Err(GroupError::Missing { path: store });
    }
    let rows = read_rows(&store)?;
    Ok(rows
        .into_iter()
        .filter_map(|row| resolve(dir, row, warnings))
        .collect())
}

fn sqlite_error(path: &Path) -> impl Fn(rusqlite::Error) -> GroupError + '_ {
    move |source| GroupError::Sqlite {
        path: path.to_path_buf(),
        source,
    }
}

fn read_rows(store: &Path) -> Result<Vec<Row>, GroupError> {
    let fail = sqlite_error(store);
    let connection = Connection::open_with_flags(
        immutable_uri(store),
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(&fail)?;
    let columns = columns(&connection).map_err(&fail)?;
    if !["path", "name"]
        .iter()
        .all(|c| columns.iter().any(|x| x == c))
    {
        return Err(GroupError::NoTable {
            path: store.to_path_buf(),
        });
    }
    // Columns the store may not have are read as NULL.
    let optional = |column: &str| {
        if columns.iter().any(|c| c == column) {
            format!("\"{column}\"")
        } else {
            "NULL".to_owned()
        }
    };
    let sql = format!(
        "SELECT path, name, {}, {}, {} FROM {TABLE} ORDER BY id",
        optional("avatar"),
        optional("themeBg"),
        optional("themeFg"),
    );
    let sql = if columns.iter().any(|c| c == "id") {
        sql
    } else {
        sql.replace(" ORDER BY id", "")
    };
    let mut statement = connection.prepare(&sql).map_err(&fail)?;
    let rows = statement
        .query_map([], |row| {
            Ok(Row {
                path: row.get(0)?,
                name: row.get(1)?,
                avatar: row.get(2)?,
                theme_bg: row.get(3)?,
                theme_fg: row.get(4)?,
            })
        })
        .map_err(&fail)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(&fail)
}

fn columns(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({TABLE})"))?;
    let names = statement.query_map([], |row| row.get::<_, String>(1))?;
    names.collect()
}

/// `file:<path>?immutable=1` with the characters SQLite's URI syntax
/// reserves escaped.
fn immutable_uri(path: &Path) -> String {
    let mut uri = String::from("file:");
    for byte in path.to_string_lossy().bytes() {
        match byte {
            b'%' | b'?' | b'#' | b' ' | b'"' | b'<' | b'>' | b'`' | b'{' | b'}' => {
                uri.push('%');
                for nibble in [byte >> 4, byte & 0x0f] {
                    let digit = char::from_digit(u32::from(nibble), 16).unwrap_or('0');
                    uri.push(digit.to_ascii_uppercase());
                }
            }
            _ => uri.push(char::from(byte)),
        }
    }
    uri.push_str("?immutable=1");
    uri
}

fn resolve(dir: &Path, row: Row, warnings: &mut Vec<String>) -> Option<Profile> {
    let stored = Path::new(&row.path);
    let candidates = [dir.join(stored), dir.join("Profiles").join(stored)];
    let Some(path) = candidates.into_iter().find(|candidate| candidate.is_dir()) else {
        warnings.push(format!(
            "the Firefox profile {:?} has no directory at {}",
            row.name, row.path
        ));
        return None;
    };
    let image = (row.avatar.as_deref() == Some(CUSTOM_AVATAR))
        .then(|| avatar_file(&path))
        .flatten();
    // A very light theme colour would hide a white initial; such a profile
    // gets the colour its name picks instead.
    let color = [&row.theme_bg, &row.theme_fg]
        .into_iter()
        .flatten()
        .filter_map(|text| parse_css_color(text))
        .find(|color| !is_light(*color));
    Some(Profile {
        badge: Some(badge(&row.name, image.as_deref(), color)),
        avatar: image,
        name: row.name,
        id: row.path,
        path,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use wye_core::target_menu::Badge;

    use super::*;

    const STORE: &str = "ab12cd34";

    /// Writes `<dir>/Profile Groups/<STORE>.sqlite` with the given `Profiles`
    /// table schema and rows, each row being the values in column order.
    fn store(dir: &Path, columns: &str, rows: &[&str]) {
        let folder = dir.join(STORE_DIR);
        fs::create_dir_all(&folder).unwrap();
        let connection = Connection::open(folder.join(format!("{STORE}.sqlite"))).unwrap();
        connection
            .execute_batch(&format!("CREATE TABLE {TABLE} ({columns});"))
            .unwrap();
        for row in rows {
            connection
                .execute_batch(&format!("INSERT INTO {TABLE} VALUES ({row});"))
                .unwrap();
        }
    }

    const FULL: &str = "id INTEGER PRIMARY KEY, path TEXT NOT NULL, name TEXT NOT NULL, \
                        avatar TEXT NOT NULL, themeId TEXT NOT NULL, themeFg TEXT NOT NULL, \
                        themeBg TEXT NOT NULL";

    fn names(profiles: &[Profile]) -> Vec<&str> {
        profiles.iter().map(|p| p.name.as_str()).collect()
    }

    #[test]
    fn reads_profiles_and_resolves_their_directories() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("Profiles/xyz.Work")).unwrap();
        fs::create_dir_all(dir.path().join("abc.Home")).unwrap();
        store(
            dir.path(),
            FULL,
            &[
                "1, 'Profiles/xyz.Work', 'Work', 'briefcase', 'default', '#ffffff', '#1a73e8'",
                "2, 'abc.Home', 'Home', 'flower', 'default', 'rgb(255, 255, 255)', 'rgba(0, 0, 0, 0)'",
                "3, 'xyz.Work', 'Same dir under Profiles', 'star', 'default', '#fff', '#ffffff'",
            ],
        );
        let mut warnings = Vec::new();
        let profiles = read(dir.path(), STORE, &mut warnings).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            names(&profiles),
            ["Work", "Home", "Same dir under Profiles"]
        );
        assert_eq!(
            profiles[0].id, "Profiles/xyz.Work",
            "the ID is the stored path"
        );
        assert_eq!(profiles[0].path, dir.path().join("Profiles/xyz.Work"));
        assert_eq!(profiles[1].path, dir.path().join("abc.Home"));
        assert_eq!(
            profiles[2].path,
            dir.path().join("Profiles/xyz.Work"),
            "a path relative to the Profiles folder resolves too"
        );
        assert_eq!(
            profiles[0].badge,
            Some(Badge::Initial {
                text: "W".into(),
                color: 0x1a_73_e8
            })
        );
        // Transparent background and a white foreground: the name picks a colour.
        assert_eq!(
            profiles[1].badge,
            Some(Badge::Initial {
                text: "H".into(),
                color: crate::profiles::badge::hashed_color("Home")
            })
        );
        assert_eq!(profiles[0].avatar, None);
    }

    #[test]
    fn a_custom_avatar_is_the_picture_in_the_profile_directory() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("p1.custom");
        fs::create_dir_all(&profile).unwrap();
        fs::write(profile.join("avatar.png"), b"png").unwrap();
        store(
            dir.path(),
            FULL,
            &["1, 'p1.custom', 'Mine', 'custom', 'default', '#fff', '#fff'"],
        );
        let profiles = read(dir.path(), STORE, &mut Vec::new()).unwrap();
        assert_eq!(profiles[0].avatar, Some(profile.join("avatar.png")));
        assert_eq!(
            profiles[0].badge,
            Some(Badge::Image(
                profile.join("avatar.png").to_string_lossy().into_owned()
            ))
        );
    }

    #[test]
    fn a_row_without_a_directory_is_skipped_with_a_warning() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("here")).unwrap();
        store(
            dir.path(),
            FULL,
            &[
                "1, 'gone', 'Gone', 'a', 't', '#fff', '#000'",
                "2, 'here', 'Here', 'a', 't', '#fff', '#000'",
            ],
        );
        let mut warnings = Vec::new();
        let profiles = read(dir.path(), STORE, &mut warnings).unwrap();
        assert_eq!(names(&profiles), ["Here"]);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Gone"), "{warnings:?}");
    }

    #[test]
    fn absolute_paths_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let row = format!(
            "1, '{}', 'Abs', 'a', 't', '#fff', '#000'",
            elsewhere.path().display()
        );
        store(dir.path(), FULL, &[&row]);
        let profiles = read(dir.path(), STORE, &mut Vec::new()).unwrap();
        assert_eq!(profiles[0].path, elsewhere.path());
    }

    #[test]
    fn columns_beyond_path_and_name_are_optional() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("p")).unwrap();
        store(dir.path(), "path TEXT, name TEXT", &["'p', 'Minimal'"]);
        let profiles = read(dir.path(), STORE, &mut Vec::new()).unwrap();
        assert_eq!(names(&profiles), ["Minimal"]);
        assert_eq!(
            profiles[0].badge,
            Some(Badge::Initial {
                text: "M".into(),
                color: crate::profiles::badge::hashed_color("Minimal")
            })
        );
    }

    #[test]
    fn a_store_without_the_expected_table_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        store(dir.path(), "only INTEGER", &[]);
        assert!(matches!(
            read(dir.path(), STORE, &mut Vec::new()),
            Err(GroupError::NoTable { .. })
        ));

        let other = tempfile::tempdir().unwrap();
        let folder = other.path().join(STORE_DIR);
        fs::create_dir_all(&folder).unwrap();
        Connection::open(folder.join(format!("{STORE}.sqlite")))
            .unwrap()
            .execute_batch("CREATE TABLE Other (x INTEGER);")
            .unwrap();
        assert!(matches!(
            read(other.path(), STORE, &mut Vec::new()),
            Err(GroupError::NoTable { .. })
        ));
    }

    #[test]
    fn bad_stores_are_errors_not_panics() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            read(dir.path(), STORE, &mut Vec::new()),
            Err(GroupError::Missing { .. })
        ));
        for id in ["", "../evil", "a/b", "a.b", "a b"] {
            assert!(
                matches!(
                    read(dir.path(), id, &mut Vec::new()),
                    Err(GroupError::BadStoreId { .. })
                ),
                "{id:?}"
            );
        }
        let folder = dir.path().join(STORE_DIR);
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join(format!("{STORE}.sqlite")),
            b"this is not a database at all",
        )
        .unwrap();
        assert!(matches!(
            read(dir.path(), STORE, &mut Vec::new()),
            Err(GroupError::Sqlite { .. })
        ));
    }

    #[test]
    fn the_store_is_opened_read_only_and_left_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("p")).unwrap();
        store(dir.path(), FULL, &["1, 'p', 'P', 'a', 't', '#fff', '#000'"]);
        let file = dir.path().join(STORE_DIR).join(format!("{STORE}.sqlite"));
        let before = fs::read(&file).unwrap();
        read(dir.path(), STORE, &mut Vec::new()).unwrap();
        assert_eq!(fs::read(&file).unwrap(), before);
        assert_eq!(
            fs::read_dir(dir.path().join(STORE_DIR)).unwrap().count(),
            1,
            "no journal or WAL file is left behind"
        );
    }

    #[test]
    fn a_store_under_a_path_with_uri_characters_opens() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("odd #1 %20 ?dir");
        fs::create_dir_all(dir.join("p")).unwrap();
        store(&dir, FULL, &["1, 'p', 'P', 'a', 't', '#fff', '#000'"]);
        let profiles = read(&dir, STORE, &mut Vec::new()).unwrap();
        assert_eq!(names(&profiles), ["P"]);
    }

    #[test]
    fn a_locked_store_can_still_be_read() {
        // Firefox keeps the store open with a write transaction running.
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("p")).unwrap();
        store(dir.path(), FULL, &["1, 'p', 'P', 'a', 't', '#fff', '#000'"]);
        let file = dir.path().join(STORE_DIR).join(format!("{STORE}.sqlite"));
        let writer = Connection::open(&file).unwrap();
        writer.execute_batch("BEGIN EXCLUSIVE;").unwrap();
        let profiles = read(dir.path(), STORE, &mut Vec::new()).unwrap();
        assert_eq!(names(&profiles), ["P"]);
        writer.execute_batch("ROLLBACK;").unwrap();
    }

    #[test]
    fn finds_the_store_id_in_profiles_ini() {
        assert_eq!(
            store_id("[General]\nStartWithLastProfile=1\nStoreID=ab12cd34\n").as_deref(),
            Some("ab12cd34")
        );
        assert_eq!(
            store_id("[Install1]\nStoreID=zz\n[General]\nStoreID=first\n").as_deref(),
            Some("first"),
            "General wins"
        );
        assert_eq!(store_id("[Install1]\nStoreID=zz\n").as_deref(), Some("zz"));
        assert_eq!(store_id("[General]\nStoreID=\n"), None);
        assert_eq!(store_id("[General]\nVersion=2\n"), None);
        assert_eq!(store_id(""), None);
    }
}
