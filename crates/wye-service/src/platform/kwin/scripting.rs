//! Loading scripts into `KWin`: `org.kde.kwin.Scripting` on `/Scripting`
//! (`KWin` 6: `loadScript(s path, s name) → i`, `start()`,
//! `unloadScript(s name) → b`).

use std::path::Path;

use async_trait::async_trait;
use zbus::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::proxy::CacheProperties;

use crate::platform::PlatformError;

/// `KWin`'s well-known bus name.
pub const KWIN_BUS_NAME: &str = "org.kde.KWin";

/// `KWin`'s scripting object, as far as Wye uses it.
#[zbus::proxy(
    interface = "org.kde.kwin.Scripting",
    default_service = "org.kde.KWin",
    default_path = "/Scripting",
    gen_blocking = false
)]
trait Scripting {
    /// Load a script; its ID, or -1 when `plugin_name` is taken.
    #[zbus(name = "loadScript")]
    fn load_script(&self, file_path: &str, plugin_name: &str) -> zbus::Result<i32>;

    /// Run every loaded script that is not running yet.
    #[zbus(name = "start")]
    fn start(&self) -> zbus::Result<()>;

    /// Unload the script loaded as `plugin_name`; whether there was one.
    #[zbus(name = "unloadScript")]
    fn unload_script(&self, plugin_name: &str) -> zbus::Result<bool>;
}

/// What the `KWin` helper needs from the compositor; a fake in tests.
#[async_trait]
pub trait ScriptHost: Send + Sync + std::fmt::Debug {
    /// Unique bus name of the compositor, which a script's `callDBus`
    /// comes from; `None` when it is not running.
    async fn owner(&self) -> Option<String>;
    /// Load the script at `path` under `name`.
    async fn load(&self, path: &Path, name: &str) -> Result<(), PlatformError>;
    /// Run every loaded script that is not running yet.
    async fn start(&self) -> Result<(), PlatformError>;
    /// Unload the script loaded under `name`.
    async fn unload(&self, name: &str) -> Result<(), PlatformError>;
}

/// `KWin` over the session bus.
#[derive(Debug, Clone)]
pub struct KWinScripting {
    connection: zbus::Connection,
}

impl KWinScripting {
    /// Talk to `KWin` over `connection`.
    #[must_use]
    pub fn new(connection: zbus::Connection) -> Self {
        Self { connection }
    }

    async fn proxy(&self) -> Result<ScriptingProxy<'static>, PlatformError> {
        ScriptingProxy::builder(&self.connection)
            .cache_properties(CacheProperties::No)
            .build()
            .await
            .map_err(|error| failed("reach", &error))
    }
}

#[async_trait]
impl ScriptHost for KWinScripting {
    async fn owner(&self) -> Option<String> {
        let bus = DBusProxy::new(&self.connection).await.ok()?;
        let name = BusName::try_from(KWIN_BUS_NAME).ok()?;
        bus.get_name_owner(name)
            .await
            .ok()
            .map(|owner| owner.to_string())
    }

    async fn load(&self, path: &Path, name: &str) -> Result<(), PlatformError> {
        let path = path
            .to_str()
            .ok_or_else(|| PlatformError::Failed(format!("{} is not UTF-8", path.display())))?;
        let id = self
            .proxy()
            .await?
            .load_script(path, name)
            .await
            .map_err(|error| failed("loadScript", &error))?;
        if id < 0 {
            return Err(PlatformError::Failed(format!(
                "KWin refused to load {name}: the name is taken"
            )));
        }
        Ok(())
    }

    async fn start(&self) -> Result<(), PlatformError> {
        self.proxy()
            .await?
            .start()
            .await
            .map_err(|error| failed("start", &error))
    }

    async fn unload(&self, name: &str) -> Result<(), PlatformError> {
        let unloaded = self
            .proxy()
            .await?
            .unload_script(name)
            .await
            .map_err(|error| failed("unloadScript", &error))?;
        if !unloaded {
            tracing::debug!(name, "KWin had no script to unload");
        }
        Ok(())
    }
}

fn failed(what: &str, error: &zbus::Error) -> PlatformError {
    PlatformError::Failed(format!("KWin {what}: {error}"))
}
