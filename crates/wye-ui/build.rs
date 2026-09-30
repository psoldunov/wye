//! Builds the QML module `dev.soldunov.wye.ui`, the cxx-qt bridges and the
//! C++ shim.
//!
//! Every input is found by listing directories, so a unit that adds a QML
//! file, a bridge or a self-test fixture never edits this file:
//!
//! - `qml/**/*.qml` and `qml/**/*.js`: the module's QML files.
//! - `src/bridge/*.rs` except `mod.rs`: one `#[cxx_qt::bridge]` each.
//! - `cpp/*.cpp`: the shim, linked with `KF6WindowSystem` (pkg-config).
//! - `fixtures/*.json`: embedded for `wye-ui --self-test`
//!   (`$OUT_DIR/fixtures.rs`).

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use cxx_qt_build::{CxxQtBuilder, QmlModule};

/// The QML module every file in `qml/` belongs to.
const QML_URI: &str = "dev.soldunov.wye.ui";

/// The KDE Frameworks library the C++ shim calls (design B).
const KF6_WINDOW_SYSTEM: &str = "KF6WindowSystem";

fn main() {
    if let Err(error) = build() {
        // A build script reports failure by panicking; the message is the
        // whole diagnostic Cargo shows.
        panic!("wye-ui build script: {error}");
    }
}

fn build() -> Result<(), String> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").map_err(|e| e.to_string())?);
    let qml_files = list(&manifest, "qml", &["qml", "js"], true)?;
    let bridges: Vec<PathBuf> = list(&manifest, "src/bridge", &["rs"], false)?
        .into_iter()
        .filter(|path| path.file_name().is_some_and(|name| name != "mod.rs"))
        .collect();
    let cpp_files = list(&manifest, "cpp", &["cpp"], false)?;
    let fixtures = list(&manifest, "fixtures", &["json"], false)?;
    for dir in ["qml", "src/bridge", "cpp", "fixtures"] {
        // A directory makes Cargo rerun the script when anything inside
        // changes, new files included.
        println!("cargo::rerun-if-changed={dir}");
    }
    println!("cargo::rerun-if-env-changed=PKG_CONFIG_PATH");

    write_fixtures(&manifest, &fixtures)?;
    let kf6_includes = pkg_config(&["--cflags-only-I", KF6_WINDOW_SYSTEM])?;
    let kf6_libs = pkg_config(&["--libs-only-L", KF6_WINDOW_SYSTEM])?;

    let builder = CxxQtBuilder::new_qml_module(QmlModule::new(QML_URI).qml_files(&qml_files))
        .qt_module("Gui")
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .qt_module("Widgets")
        .files(&bridges)
        .cpp_files(&cpp_files);
    #[allow(
        unsafe_code,
        reason = "cc_builder is unsafe only because it can override flags cxx-qt-build set; this adds include paths"
    )]
    let builder = unsafe {
        builder.cc_builder(|cc| {
            for flag in kf6_includes.split_whitespace() {
                if let Some(dir) = flag.strip_prefix("-I") {
                    cc.include(dir);
                }
            }
        })
    };
    builder.build();

    for flag in kf6_libs.split_whitespace() {
        if let Some(dir) = flag.strip_prefix("-L") {
            println!("cargo::rustc-link-search=native={dir}");
        }
    }
    println!("cargo::rustc-link-lib={KF6_WINDOW_SYSTEM}");
    Ok(())
}

/// Files under `manifest/dir` with one of `extensions`, as paths relative to
/// the crate, sorted. `recursive` descends into subdirectories.
fn list(
    manifest: &Path,
    dir: &str,
    extensions: &[&str],
    recursive: bool,
) -> Result<Vec<PathBuf>, String> {
    let mut found = Vec::new();
    walk(manifest, Path::new(dir), extensions, recursive, &mut found)
        .map_err(|error| format!("cannot list {dir}: {error}"))?;
    found.sort();
    Ok(found)
}

fn walk(
    manifest: &Path,
    relative: &Path,
    extensions: &[&str],
    recursive: bool,
    found: &mut Vec<PathBuf>,
) -> io::Result<()> {
    for entry in fs::read_dir(manifest.join(relative))? {
        let entry = entry?;
        let path = relative.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            if recursive {
                walk(manifest, &path, extensions, recursive, found)?;
            }
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extensions.contains(&extension))
        {
            found.push(path);
        }
    }
    Ok(())
}

/// `$OUT_DIR/fixtures.rs`: `FIXTURES`, each file stem with its text.
fn write_fixtures(manifest: &Path, fixtures: &[PathBuf]) -> Result<(), String> {
    let entries: String = fixtures
        .iter()
        .filter_map(|path| {
            let stem = path.file_stem()?.to_str()?;
            let absolute = manifest.join(path);
            Some(format!(
                "    ({stem:?}, include_str!({:?})),\n",
                absolute.display().to_string()
            ))
        })
        .collect();
    let source = format!(
        "/// Self-test fixtures: surface name and JSON text.\npub const FIXTURES: &[(&str, &str)] = &[\n{entries}];\n"
    );
    let out = PathBuf::from(env::var("OUT_DIR").map_err(|e| e.to_string())?);
    fs::write(out.join("fixtures.rs"), source)
        .map_err(|error| format!("cannot write fixtures.rs: {error}"))
}

/// Run `pkg-config` (honouring `$PKG_CONFIG`) and return its output.
fn pkg_config(args: &[&str]) -> Result<String, String> {
    let program = env::var("PKG_CONFIG").unwrap_or_else(|_| "pkg-config".to_owned());
    let output = Command::new(&program)
        .args(args)
        .output()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("{program} printed non-UTF-8: {error}"))
}
