//! `wye extension install|remove`: write or delete the native-messaging
//! host manifests the browser extension needs (BEXT-04), with the same
//! code as `wye-native-host --install|--remove`.

use std::process::ExitCode;

use wye_native_host::install as manifests;

use super::{Console, Context};
use crate::cli::ExtensionAction;
use crate::notice;

/// Run `action`; exit 1 when a manifest cannot be written or deleted, or
/// the host program is not installed.
pub fn run(ctx: &Context, console: &mut Console<'_>, action: ExtensionAction) -> ExitCode {
    let result = match action {
        ExtensionAction::Install => {
            let exe = std::env::current_exe().ok();
            manifests::install(&mut console.out, &ctx.xdg, exe.as_deref())
        }
        ExtensionAction::Remove => manifests::remove(&mut console.out, &ctx.xdg),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            notice::write(console.err, format_args!("wye: {message}"));
            ExitCode::FAILURE
        }
    }
}
