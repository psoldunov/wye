//! Writing and deleting the host manifests of every detected browser
//! (BEXT-04), through `wye_desktop::native_messaging`: `wye extension
//! install|remove` and `wye-native-host --install|--remove` both run it.
//!
//! The service writes the manifests itself at every start; `remove`
//! records `extension-host-removed` in the state file so it stops, and
//! `install` clears it again.

use std::io::{self, Write};
use std::path::Path;

use wye_desktop::native_messaging::{self, Manifest, Outcome};
use wye_desktop::state::State;
use wye_desktop::xdg::XdgDirs;

const UNSUPPORTED: &str =
    "Flatpak and Snap browsers are not supported: their sandbox cannot start the host.";

const OPTED_OUT: &str =
    "Wye will not write them again when it starts, until `wye extension install`.";

/// Write a manifest naming the host for every browser found, list them on
/// `out`, and let the service keep them current again (the state file at
/// `state`).
///
/// # Errors
///
/// When the host cannot be found, a manifest cannot be written, or the
/// state file cannot be updated.
pub fn install(
    out: &mut impl Write,
    xdg: &XdgDirs,
    state: &Path,
    current_exe: Option<&Path>,
) -> Result<(), String> {
    let host = native_messaging::find_host(xdg, current_exe).ok_or_else(|| {
        format!(
            "cannot find {} on PATH or next to this program",
            native_messaging::HOST_PROGRAM
        )
    })?;
    let written = native_messaging::install(xdg, &host).map_err(|error| error.to_string())?;
    set_removed(state, false)?;
    report(out, &written, "No supported browser was found.").map_err(|error| io_error(&error))?;
    writeln!(out, "{UNSUPPORTED}").map_err(|error| io_error(&error))
}

/// Delete every manifest, list them on `out`, and record in the state file
/// at `state` that the service must not write them again.
///
/// # Errors
///
/// When the state file cannot be updated or a manifest cannot be deleted.
pub fn remove(out: &mut impl Write, xdg: &XdgDirs, state: &Path) -> Result<(), String> {
    // Recorded first: a service starting meanwhile must not put them back.
    set_removed(state, true)?;
    let removed = native_messaging::remove(xdg).map_err(|error| error.to_string())?;
    report(out, &removed, "No manifest was installed.")
        .and_then(|()| writeln!(out, "{OPTED_OUT}"))
        .map_err(|error| io_error(&error))
}

/// Record whether the user removed the manifests (BEXT-04). The state file
/// is shared with the service and the CLI, so it is loaded, changed and
/// saved, and only written when the flag changes.
fn set_removed(state: &Path, removed: bool) -> Result<(), String> {
    let current = State::load(state).map_err(|error| error.to_string())?;
    if current.extension_host_removed == removed {
        return Ok(());
    }
    State {
        extension_host_removed: removed,
        ..current
    }
    .save(state)
    .map_err(|error| error.to_string())
}

/// One line per manifest: what happened to it, for which browser, where.
fn report(out: &mut impl Write, manifests: &[Manifest], none: &str) -> io::Result<()> {
    if manifests.is_empty() {
        return writeln!(out, "{none}");
    }
    manifests.iter().try_for_each(|manifest| {
        let browser = manifest.browser;
        let path = manifest.path.display();
        match manifest.outcome {
            Outcome::Written | Outcome::Current => writeln!(out, "Installed for {browser}: {path}"),
            Outcome::Removed => writeln!(out, "Removed for {browser}: {path}"),
            Outcome::Symlink => writeln!(
                out,
                "Left alone for {browser} (a symlink, managed elsewhere): {path}"
            ),
        }
    })
}

fn io_error(error: &io::Error) -> String {
    format!("cannot write the report: {error}")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use wye_desktop::xdg::Locale;

    use super::*;

    fn xdg(root: &Path) -> XdgDirs {
        let home = root.join("home");
        XdgDirs {
            config_home: home.join(".config"),
            config_dirs: Vec::new(),
            data_home: home.join(".local/share"),
            data_dirs: Vec::new(),
            current_desktops: Vec::new(),
            search_path: vec![root.join("bin")],
            locale: Locale::none(),
            home,
        }
    }

    #[test]
    fn install_and_remove_report_each_browser_bext_04() {
        let root = tempfile::tempdir().expect("temp dir");
        let xdg = xdg(root.path());
        fs::create_dir_all(xdg.home.join(".mozilla/firefox")).expect("firefox");
        fs::create_dir_all(root.path().join("bin")).expect("bin");
        fs::write(root.path().join("bin/wye-native-host"), "").expect("host");

        let state = root.path().join("state/wye/state.toml");

        let mut out = Vec::new();
        install(&mut out, &xdg, &state, None).expect("installed");
        let text = String::from_utf8(out).expect("UTF-8");
        assert!(text.starts_with("Installed for Firefox: "), "{text}");
        assert!(text.contains("Flatpak"));
        assert!(!state.exists(), "nothing to record");
        let manifest = xdg
            .home
            .join(".mozilla/native-messaging-hosts/dev.soldunov.wye.json");
        let written = fs::read_to_string(&manifest).expect("manifest");
        assert!(
            written.contains(
                &root
                    .path()
                    .join("bin/wye-native-host")
                    .display()
                    .to_string()
            )
        );

        let mut out = Vec::new();
        remove(&mut out, &xdg, &state).expect("removed");
        let text = String::from_utf8(out).expect("UTF-8");
        assert!(text.starts_with("Removed for Firefox"), "{text}");
        assert!(text.contains("wye extension install"), "{text}");
        assert!(!manifest.exists());
    }

    /// A symlinked manifest is reported as left alone, not as installed or
    /// removed, and survives both (BEXT-04).
    #[test]
    fn a_symlinked_manifest_is_reported_left_alone_bext_04() {
        let root = tempfile::tempdir().expect("temp dir");
        let xdg = xdg(root.path());
        fs::create_dir_all(root.path().join("bin")).expect("bin");
        fs::write(root.path().join("bin/wye-native-host"), "").expect("host");
        let hosts = xdg.home.join(".mozilla/native-messaging-hosts");
        fs::create_dir_all(&hosts).expect("hosts");
        let target = root.path().join("managed.json");
        fs::write(&target, "{}").expect("target");
        let link = hosts.join("dev.soldunov.wye.json");
        std::os::unix::fs::symlink(&target, &link).expect("linked");
        let state = root.path().join("state/wye/state.toml");
        let expected = format!(
            "Left alone for Firefox (a symlink, managed elsewhere): {}\n",
            link.display()
        );

        let mut out = Vec::new();
        install(&mut out, &xdg, &state, None).expect("installed");
        let text = String::from_utf8(out).expect("UTF-8");
        assert!(text.starts_with(&expected), "{text}");

        let mut out = Vec::new();
        remove(&mut out, &xdg, &state).expect("removed");
        let text = String::from_utf8(out).expect("UTF-8");
        assert!(text.starts_with(&expected), "{text}");
        assert!(
            fs::symlink_metadata(&link)
                .expect("still there")
                .file_type()
                .is_symlink()
        );
    }

    /// `remove` opts out of the manifests the service writes at start, and
    /// the opt-out survives restarts until `install` (BEXT-04).
    #[test]
    fn remove_opts_out_and_install_opts_back_in_bext_04() {
        let root = tempfile::tempdir().expect("temp dir");
        let xdg = xdg(root.path());
        fs::create_dir_all(root.path().join("bin")).expect("bin");
        fs::write(root.path().join("bin/wye-native-host"), "").expect("host");
        let state = root.path().join("state/wye/state.toml");
        let kept = State {
            onboarding_done: true,
            ..State::default()
        };
        kept.save(&state).expect("saved");

        remove(&mut Vec::new(), &xdg, &state).expect("removed");
        let removed = State::load(&state).expect("loads");
        assert!(removed.extension_host_removed);
        assert!(removed.onboarding_done, "other keys are kept");

        install(&mut Vec::new(), &xdg, &state, None).expect("installed");
        assert_eq!(State::load(&state).expect("loads"), kept);
    }

    #[test]
    fn without_the_host_nothing_is_written() {
        let root = tempfile::tempdir().expect("temp dir");
        let xdg = xdg(root.path());
        let state = root.path().join("state.toml");
        State {
            extension_host_removed: true,
            ..State::default()
        }
        .save(&state)
        .expect("saved");
        let error = install(&mut Vec::new(), &xdg, &state, None).expect_err("no host");
        assert!(error.contains("wye-native-host"));
        assert!(
            State::load(&state).expect("loads").extension_host_removed,
            "a failed install keeps the opt-out"
        );
    }
}
