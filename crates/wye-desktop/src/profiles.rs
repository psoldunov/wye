//! Browser profiles: Chromium's `Local State` (DISC-06) and Firefox's
//! classic `profiles.ini` (DISC-07).
//!
//! Firefox 138+ can also keep profiles in a profile group: a SQLite store
//! referenced by a `StoreID` key in `profiles.ini`. Reading it is out of
//! scope here; profiles that only exist in a group store are not listed.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::keyfile;

/// A browser profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// Chromium: the profile directory (`Default`, `Profile 1`). Firefox: the
    /// `Path` value from `profiles.ini`, as written. This is the ID stored in
    /// [`wye_core::Target::Profile`].
    pub id: String,
    /// Display name.
    pub name: String,
    /// The profile's picture, when one exists on disk (DISC-08).
    pub avatar: Option<PathBuf>,
    /// The profile's absolute directory.
    pub path: PathBuf,
}

/// A profile store that exists but cannot be read.
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is not valid JSON: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

const LOCAL_STATE: &str = "Local State";
const PROFILES_INI: &str = "profiles.ini";
const CHROMIUM_AVATAR: &str = "Google Profile Picture.png";

/// Reads `profile.info_cache` from `<config_dir>/Local State`, sorted by
/// name. A missing file yields no profiles.
///
/// # Errors
///
/// Returns [`ProfileError`] when the file exists but cannot be read or
/// parsed.
pub fn read_chromium(config_dir: &Path) -> Result<Vec<Profile>, ProfileError> {
    let path = config_dir.join(LOCAL_STATE);
    let Some(text) = read_optional(&path)? else {
        return Ok(Vec::new());
    };
    let state: Value =
        serde_json::from_str(&text).map_err(|source| ProfileError::Json { path, source })?;
    let Some(cache) = state
        .pointer("/profile/info_cache")
        .and_then(Value::as_object)
    else {
        return Ok(Vec::new());
    };
    let profiles = cache
        .iter()
        .map(|(dir, info)| {
            let path = config_dir.join(dir);
            let avatar = Some(path.join(CHROMIUM_AVATAR)).filter(|file| file.is_file());
            Profile {
                id: dir.clone(),
                name: info
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|name| !name.is_empty())
                    .unwrap_or(dir)
                    .to_owned(),
                avatar,
                path,
            }
        })
        .collect();
    Ok(sorted(profiles))
}

/// Reads the `[ProfileN]` groups of `<dir>/profiles.ini`, sorted by name.
/// A missing file yields no profiles.
///
/// # Errors
///
/// Returns [`ProfileError::Io`] when the file exists but cannot be read.
pub fn read_firefox(dir: &Path) -> Result<Vec<Profile>, ProfileError> {
    let Some(text) = read_optional(&dir.join(PROFILES_INI))? else {
        return Ok(Vec::new());
    };
    let profiles = keyfile::parse(&text)
        .iter()
        .filter(|group| is_profile_group(&group.name))
        .filter_map(|group| {
            let id = group.get("Path")?.to_owned();
            let relative = match group.get("IsRelative") {
                Some(flag) => flag.trim() == "1",
                None => !Path::new(&id).is_absolute(),
            };
            let path = if relative {
                dir.join(&id)
            } else {
                PathBuf::from(&id)
            };
            Some(Profile {
                name: group.get("Name").unwrap_or(&id).to_owned(),
                avatar: None,
                path,
                id,
            })
        })
        .collect();
    Ok(sorted(profiles))
}

fn is_profile_group(name: &str) -> bool {
    name.strip_prefix("Profile")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

fn sorted(mut profiles: Vec<Profile>) -> Vec<Profile> {
    profiles.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    profiles
}

fn read_optional(path: &Path) -> Result<Option<String>, ProfileError> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ProfileError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn reads_chromium_profiles() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(LOCAL_STATE),
            r#"{"profile":{"info_cache":{
                "Profile 1":{"name":"Work"},
                "Default":{"name":"Personal"},
                "Profile 2":{}
            }}}"#,
        )
        .unwrap();
        fs::create_dir_all(dir.path().join("Profile 1")).unwrap();
        fs::write(dir.path().join("Profile 1").join(CHROMIUM_AVATAR), b"png").unwrap();

        let profiles = read_chromium(dir.path()).unwrap();
        let summary: Vec<(&str, &str)> = profiles
            .iter()
            .map(|p| (p.id.as_str(), p.name.as_str()))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("Default", "Personal"),
                ("Profile 2", "Profile 2"),
                ("Profile 1", "Work")
            ]
        );
        assert_eq!(
            profiles[2].avatar,
            Some(dir.path().join("Profile 1").join(CHROMIUM_AVATAR))
        );
        assert_eq!(profiles[0].avatar, None);
        assert_eq!(profiles[0].path, dir.path().join("Default"));
    }

    #[test]
    fn chromium_tolerates_missing_state() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_chromium(dir.path()).unwrap().is_empty());
        fs::write(dir.path().join(LOCAL_STATE), "{}").unwrap();
        assert!(read_chromium(dir.path()).unwrap().is_empty());
        fs::write(dir.path().join(LOCAL_STATE), "not json").unwrap();
        assert!(matches!(
            read_chromium(dir.path()),
            Err(ProfileError::Json { .. })
        ));
    }

    #[test]
    fn reads_firefox_profiles() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(PROFILES_INI),
            "[General]\nStartWithLastProfile=1\nVersion=2\n\n\
             [Profile1]\nName=work\nIsRelative=0\nPath=/data/ff/work\n\n\
             [Profile0]\nName=default-release\nIsRelative=1\nPath=Profiles/abcd.default-release\nDefault=1\n\n\
             [Install4F96D1932A9F858E]\nDefault=Profiles/abcd.default-release\n\n\
             [ProfileX]\nName=bogus\nPath=x\n",
        )
        .unwrap();

        let profiles = read_firefox(dir.path()).unwrap();
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].name, "default-release");
        assert_eq!(profiles[0].id, "Profiles/abcd.default-release");
        assert_eq!(
            profiles[0].path,
            dir.path().join("Profiles/abcd.default-release")
        );
        assert_eq!(profiles[1].id, "/data/ff/work");
        assert_eq!(profiles[1].path, PathBuf::from("/data/ff/work"));
    }

    #[test]
    fn firefox_tolerates_missing_ini() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_firefox(dir.path()).unwrap().is_empty());
    }
}
