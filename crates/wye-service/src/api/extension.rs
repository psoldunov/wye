//! The browser extension's native-messaging host manifests (BEXT-04).
//!
//! The service writes them for every detected browser, so installing the
//! extension is all a user has to do. The file watcher runs [`refresh`]
//! whenever it arms for an environment (at start, after the bus name is
//! claimed, DEF-04) and whenever a browser's top-level directory appears
//! (`~/.mozilla`, `~/.config/BraveSoftware`; see
//! `native_messaging::watched_names`), so a browser installed and first run
//! while Wye runs gets its manifest without a restart. A refresh runs on a
//! blocking thread and writes only the manifests that are missing or name
//! another host, so one that changes nothing touches no file and logs
//! nothing; a symlink in a manifest's place is left alone. `wye extension
//! remove` opts out (`extension-host-removed` in the state file) until `wye
//! extension install`.
//!
//! The directories come from the service's environment, never from the
//! process's own, so a service pointed at other files (tests) stays there.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use wye_desktop::native_messaging::{self, Manifest};
use wye_desktop::state::State;

use crate::api::link::Environment;
use crate::context::{ServiceContext, blocking};

/// Write the manifests `environment`'s browsers lack, unless the user
/// opted out; logs what it did and never fails.
pub(crate) async fn refresh(ctx: &ServiceContext, environment: Arc<Environment>) {
    let current_exe = wye_executable(ctx);
    let outcome = blocking(move || refresh_manifests(&environment, current_exe.as_deref())).await;
    match outcome {
        Ok(Ok(Outcome::Written(written))) if !written.is_empty() => {
            let browsers: Vec<&str> = written.iter().map(|manifest| manifest.browser).collect();
            tracing::info!(
                ?browsers,
                "wrote the browser extension's native-messaging host manifests"
            );
        }
        Ok(Ok(Outcome::Written(_))) => {}
        Ok(Ok(Outcome::OptedOut)) => {
            tracing::debug!(
                "the extension host manifests were removed by the user; not writing them"
            );
        }
        Ok(Ok(Outcome::NoHost)) => tracing::info!(
            "{} is not installed; the browser extension cannot reach Wye",
            native_messaging::HOST_PROGRAM
        ),
        Ok(Err(error)) => {
            tracing::warn!(%error, "cannot write the extension host manifests");
        }
        Err(error) => tracing::warn!(%error, "cannot write the extension host manifests"),
    }
}

/// The `wye` executable the host is looked for beside: the one set
/// explicitly, else this one (packages install `wye-native-host` next to
/// `wye`).
fn wye_executable(ctx: &ServiceContext) -> Option<PathBuf> {
    ctx.wye_executable_override()
        .or_else(|| std::env::current_exe().ok())
}

/// What one refresh did.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// The user removed the manifests (`wye extension remove`).
    OptedOut,
    /// No `wye-native-host` to name in them.
    NoHost,
    /// The manifests written now; empty when every one was current.
    Written(Vec<Manifest>),
}

/// Write the manifests `environment`'s browsers lack, naming the host found
/// on its search path or next to `current_exe`, unless the user opted out.
///
/// # Errors
///
/// When the state file cannot be read (the opt-out is then unknown, so
/// nothing is written) or a manifest cannot be written.
fn refresh_manifests(
    environment: &Environment,
    current_exe: Option<&Path>,
) -> Result<Outcome, String> {
    let state = State::load(&environment.state).map_err(|error| error.to_string())?;
    if state.extension_host_removed {
        return Ok(Outcome::OptedOut);
    }
    let Some(host) = native_messaging::find_host(&environment.xdg, current_exe) else {
        return Ok(Outcome::NoHost);
    };
    native_messaging::refresh(&environment.xdg, &host)
        .map(Outcome::Written)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;

    use super::*;

    /// An environment under `root`, with `root/bin` as the search path.
    fn environment(root: &Path) -> Environment {
        let vars = [
            ("HOME", root.join("home")),
            ("XDG_CONFIG_HOME", root.join("config")),
            ("XDG_STATE_HOME", root.join("state")),
            ("PATH", root.join("bin")),
        ];
        Environment::from_lookup(|name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        })
        .expect("a home directory")
    }

    fn with_firefox_and_host(root: &Path) -> Environment {
        let environment = environment(root);
        fs::create_dir_all(environment.xdg.home.join(".mozilla")).expect("firefox");
        fs::create_dir_all(root.join("bin")).expect("bin");
        fs::write(root.join("bin").join(native_messaging::HOST_PROGRAM), "").expect("host");
        environment
    }

    fn manifest(environment: &Environment) -> PathBuf {
        environment
            .xdg
            .home
            .join(".mozilla/native-messaging-hosts/dev.soldunov.wye.json")
    }

    #[test]
    fn writes_the_manifests_once_then_leaves_them_bext_04() {
        let root = tempfile::tempdir().expect("temp dir");
        let environment = with_firefox_and_host(root.path());

        let Ok(Outcome::Written(written)) = refresh_manifests(&environment, None) else {
            panic!("written");
        };
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].path, manifest(&environment));
        assert_eq!(
            refresh_manifests(&environment, None),
            Ok(Outcome::Written(Vec::new())),
            "a second start changes nothing"
        );
        assert!(!environment.state.exists(), "the state is only read");
    }

    #[test]
    fn the_opt_out_is_respected_bext_04() {
        let root = tempfile::tempdir().expect("temp dir");
        let environment = with_firefox_and_host(root.path());
        State {
            extension_host_removed: true,
            ..State::default()
        }
        .save(&environment.state)
        .expect("saved");

        assert_eq!(refresh_manifests(&environment, None), Ok(Outcome::OptedOut));
        assert!(!manifest(&environment).exists());
    }

    #[test]
    fn without_the_host_nothing_is_written_bext_04() {
        let root = tempfile::tempdir().expect("temp dir");
        let environment = environment(root.path());
        fs::create_dir_all(environment.xdg.home.join(".mozilla")).expect("firefox");

        assert_eq!(refresh_manifests(&environment, None), Ok(Outcome::NoHost));
        assert!(!manifest(&environment).exists());
    }

    #[test]
    fn an_unreadable_state_writes_nothing_bext_04() {
        let root = tempfile::tempdir().expect("temp dir");
        let environment = with_firefox_and_host(root.path());
        fs::create_dir_all(environment.state.parent().expect("dir")).expect("dir");
        fs::write(&environment.state, "extension-host-removed = 3\n").expect("written");

        assert!(refresh_manifests(&environment, None).is_err());
        assert!(!manifest(&environment).exists());
    }
}
