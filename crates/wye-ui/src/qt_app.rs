//! The Qt side of the process: the application object, the QML engine and
//! the event loop.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cxx::UniquePtr;
use cxx_qt_lib::{QMap, QMapPair_QString_QVariant, QQmlApplicationEngine, QString, QUrl, QVariant};
use wye_api::names::BUS_NAME;

use crate::bridge::shim::ffi;
use crate::selftest::{fixtures, log, snapshot};
use crate::surface::{self, Surface};

/// Shown in window titles after the window's own title.
const DISPLAY_NAME: &str = "Wye";

/// How `Main.qml` starts.
pub enum Launch {
    /// Stay resident and serve the D-Bus interfaces (decision 2).
    Resident,
    /// Load one surface with its fixtures, print the pass line, exit. With
    /// `snapshots`, one case at a time, saving its windows there.
    SelfTest {
        surface: Surface,
        snapshots: Option<PathBuf>,
    },
}

/// Run the event loop until `Qt.quit()` or `Windows1.Quit`.
///
/// # Errors
///
/// When the application cannot be created, the fixtures are invalid, or
/// `Main.qml` fails to load.
pub fn run(launch: &Launch) -> anyhow::Result<ExitCode> {
    let properties = initial_properties(launch)?;
    let args: Vec<String> = std::env::args().collect();
    // The desktop file name doubles as the Wayland app id: the service's
    // application ID, so the compositor matches the windows to Wye's entry.
    let app: UniquePtr<ffi::QApplication> = ffi::application_new(
        &args,
        &QString::from(BUS_NAME),
        &QString::from(DISPLAY_NAME),
    );
    anyhow::ensure!(!app.is_null(), "cannot create the Qt application");

    let mut engine = QQmlApplicationEngine::new();
    let Some(mut engine_ref) = engine.as_mut() else {
        anyhow::bail!("cannot create the QML engine");
    };
    engine_ref.as_mut().set_initial_properties(&properties);
    let failed = Arc::new(AtomicBool::new(false));
    let failed_flag = Arc::clone(&failed);
    // Loading a qrc: file is synchronous: the signal, if any, fires inside
    // `load`. The guard disconnects when dropped at the end of this scope.
    let _guard = engine_ref
        .as_mut()
        .on_object_creation_failed(move |_, _| failed_flag.store(true, Ordering::SeqCst));
    engine_ref.load(&QUrl::from(
        surface::resource_url(surface::MAIN_QML).as_str(),
    ));
    anyhow::ensure!(
        !failed.load(Ordering::SeqCst),
        "{} failed to load; the QML warnings above say why",
        surface::MAIN_QML
    );

    let code = ffi::application_exec();
    drop(engine);
    drop(app);
    Ok(u8::try_from(code).map_or(ExitCode::FAILURE, ExitCode::from))
}

/// The root object's `selfTest…` properties; empty for a resident start.
fn initial_properties(launch: &Launch) -> anyhow::Result<QMap<QMapPair_QString_QVariant>> {
    let mut properties = QMap::<QMapPair_QString_QVariant>::default();
    if let Launch::SelfTest { surface, snapshots } = launch {
        let cases = fixtures::cases(*surface)?;
        // Empty for a plain self-test: `Main.qml` then delivers every case
        // at once, as before.
        let prefixes = match snapshots {
            Some(dir) => serde_json::to_string(&snapshot::prefixes(dir, *surface, &cases))?,
            None => String::new(),
        };
        let cases = serde_json::to_string(&cases)?;
        for (name, value) in [
            ("selfTestSurface", surface.name()),
            ("selfTestCases", cases.as_str()),
            ("selfTestPassLine", log::PASS_LINE),
            ("selfTestSnapshots", prefixes.as_str()),
        ] {
            properties.insert(QString::from(name), QVariant::from(&QString::from(value)));
        }
    }
    Ok(properties)
}
