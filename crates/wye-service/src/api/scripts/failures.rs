//! SCR-22: a script that fails while opening a link is reported once, until
//! the script changes. The hashes of scripts already reported are kept in
//! the state file (`script-errors-notified`).

use crate::context::ServiceContext;
use crate::platform::Notification;

/// Hashes kept; older ones are forgotten (and may be reported again).
const MAX_REMEMBERED: usize = 64;

/// One failed run, as reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Failure {
    /// "The global transform script" or "The transform script of rule “X”".
    pub script: String,
    /// Hash of the scope and the script's text.
    pub hash: String,
    /// The error, with its line.
    pub message: String,
}

/// A stable hash of `scope` and `source` (FNV-1a, 64 bits), so the state
/// file stays valid across Rust versions.
pub(crate) fn script_hash(scope: &str, source: &str) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let hash = scope
        .bytes()
        .chain(std::iter::once(0))
        .chain(source.bytes())
        .fold(OFFSET, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(PRIME)
        });
    format!("{hash:016x}")
}

/// Notify about `failure` unless this script was already reported.
pub(crate) async fn report(ctx: &ServiceContext, failure: Failure) {
    let _one_at_a_time = ctx.scripts().reporting.lock().await;
    let hash = failure.hash.clone();
    let mut first = false;
    let updated = crate::api::state::update(ctx, |state| {
        if state.script_errors_notified.contains(&hash) {
            return Ok(state.clone());
        }
        first = true;
        Ok(wye_desktop::State {
            script_errors_notified: remember(&state.script_errors_notified, &hash),
            ..state.clone()
        })
    })
    .await;
    if let Err(error) = updated {
        // Without the state file every failure would notify; log instead.
        tracing::warn!(%error, "cannot record the script failure; not notifying");
        return;
    }
    if !first {
        return;
    }
    let notification = Notification {
        summary: "Transform script failed".to_owned(),
        body: format!(
            "{} failed, so the link opened unchanged: {}",
            failure.script, failure.message
        ),
        ..Notification::default()
    };
    if let Err(error) = ctx.platform().notifier.notify(&notification).await {
        tracing::warn!(%error, "cannot show the script failure");
    }
}

/// `hashes` plus `hash`, newest last, at most [`MAX_REMEMBERED`].
fn remember(hashes: &[String], hash: &str) -> Vec<String> {
    let skip = (hashes.len() + 1).saturating_sub(MAX_REMEMBERED);
    hashes
        .iter()
        .skip(skip)
        .cloned()
        .chain(std::iter::once(hash.to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_are_stable_and_depend_on_scope_and_text() {
        assert_eq!(script_hash("global", ""), script_hash("global", ""));
        assert_eq!(script_hash("global", "a").len(), 16);
        assert_ne!(script_hash("global", "a"), script_hash("global", "b"));
        assert_ne!(script_hash("global", "a"), script_hash("rule:x", "a"));
        // Known FNV-1a value, so a change of algorithm is noticed.
        assert_eq!(script_hash("", ""), "af63bd4c8601b7df");
    }

    #[test]
    fn old_hashes_are_forgotten() {
        let full: Vec<String> = (0..MAX_REMEMBERED).map(|n| n.to_string()).collect();
        let next = remember(&full, "new");
        assert_eq!(next.len(), MAX_REMEMBERED);
        assert_eq!(next.first().map(String::as_str), Some("1"));
        assert_eq!(next.last().map(String::as_str), Some("new"));
        assert_eq!(remember(&[], "a"), ["a"]);
    }
}
