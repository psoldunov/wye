//! Live check of the session's clipboard (risk 9). Ignored by default: it
//! needs a real desktop session. Read-only: it never writes, and prints only
//! the mechanism and whether a link is on the clipboard, not the text.
//!
//! Run with `cargo test -p wye-service --test clipboard_live -- --ignored`.

use wye_service::platform::ClipboardProvider as _;
use wye_service::platform::clipboard::wayland::DataControlClipboard;

#[tokio::test]
#[ignore = "needs a Wayland session with data control"]
async fn risk9_data_control_reads_the_live_clipboard() {
    let clipboard = DataControlClipboard::start().await.expect("data control");
    // Give the first selection time to arrive.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let text = clipboard.read().await.expect("read");
    eprintln!(
        "mechanism: {:?}; one line of text: {}; a link: {}",
        clipboard.capabilities().read,
        text.is_some(),
        text.as_deref()
            .and_then(wye_core::clipboard::clipboard_link)
            .is_some()
    );
}
