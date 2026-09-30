//! The help popover texts (BLK-08), from 19-help-texts.md. One place, so
//! the texts are audited against the spec in one file (U14) and every page
//! asks for them by ID.

/// What the texts with a placeholder need to know.
#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    /// The alternative-browser key as shown, for example "Shift" (BRW-03).
    pub alternative_key: &'a str,
    /// Whether the session reports held modifiers (KEY-06).
    pub held_keys_available: bool,
}

const ALTERNATIVE_BROWSER: &str = "Hold {key} while you open a link in another app, and Wye opens it in the alternative browser instead of following your rules. Use it as an escape hatch when a rule sends a link somewhere you do not want this time. Change the key in the row below.";

const ALTERNATIVE_BROWSER_NO_HELD_KEYS: &str = " This session does not tell apps which keys are held, so the key only works for links from the Wye browser extension.";

const BROWSER_PROFILES: &str = "Wye finds the profiles of Chromium-based browsers (Chrome, Chromium, Brave, Vivaldi, Edge) and Firefox-based browsers (Firefox, Zen, LibreWolf, Floorp). Profiles appear in every browser menu and can be added to the picker. If a profile is missing, click Rescan.";

const OPEN_IN_BACKGROUND: &str = "Open the link without switching to the browser, so you can keep working. Some desktops always bring the browser to the front when it was not running yet.";

const FORCE_NEW_WINDOW: &str = "Open the link in a new browser window instead of a new tab in the current window. Available when “Open in” is a browser that supports it.";

const REMOVE_TRACKING: &str = "Removes parameters that only track where you came from, such as utm_source, fbclid and gclid. Parameters a page needs to work are kept. Example: shop.example/item?id=7&utm_source=news becomes shop.example/item?id=7. Removed everywhere: utm_*, fbclid, gclid, dclid, msclkid, mc_cid, mc_eid, igshid, yclid, _hsenc and _hsmi; on some sites also their own, such as si on YouTube and Spotify links.";

const BYPASS_KEY: &str = "Hold this key while opening a link from the browser extension to follow your rules instead of showing the picker.";

const GLOBAL_SHORTCUTS: &str = "Your desktop does not let apps register shortcuts. Bind the command in your compositor's configuration instead, for example: bindsym $mod+Shift+o exec wye clipboard";

const HELD_KEYS_UNAVAILABLE: &str = "This session does not tell apps which keys are held. On GNOME, enable Wye's Shell integration. Links from the Wye browser extension still carry held keys.";

const CLIPBOARD_UNAVAILABLE: &str =
    "This session does not let apps watch the clipboard. On GNOME, enable Wye's Shell integration.";

/// The help text `id` asks for, or `None` for an unknown ID.
///
/// IDs: `alternative-browser`, `browser-profiles`, `open-in-background`,
/// `force-new-window`, `remove-tracking`, `bypass-key`, `global-shortcuts`,
/// `held-keys-unavailable`, `clipboard-unavailable`.
#[must_use]
pub fn text(id: &str, context: &Context<'_>) -> Option<String> {
    let plain = |text: &str| Some(text.to_owned());
    match id {
        "alternative-browser" => {
            let mut text = ALTERNATIVE_BROWSER.replace("{key}", context.alternative_key);
            if !context.held_keys_available {
                text.push_str(ALTERNATIVE_BROWSER_NO_HELD_KEYS);
            }
            Some(text)
        }
        "browser-profiles" => plain(BROWSER_PROFILES),
        "open-in-background" => plain(OPEN_IN_BACKGROUND),
        "force-new-window" => plain(FORCE_NEW_WINDOW),
        "remove-tracking" => plain(REMOVE_TRACKING),
        "bypass-key" => plain(BYPASS_KEY),
        "global-shortcuts" => plain(GLOBAL_SHORTCUTS),
        "held-keys-unavailable" => plain(HELD_KEYS_UNAVAILABLE),
        "clipboard-unavailable" => plain(CLIPBOARD_UNAVAILABLE),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTEXT: Context<'static> = Context {
        alternative_key: "Shift",
        held_keys_available: true,
    };

    #[test]
    fn the_alternative_browser_text_names_the_key() {
        // BRW-02
        let text = text("alternative-browser", &CONTEXT).expect("text");
        assert!(
            text.starts_with("Hold Shift while you open a link"),
            "{text}"
        );
        assert!(!text.contains("{key}"));
    }

    #[test]
    fn without_held_keys_the_text_says_only_the_extension_works() {
        // 19-help-texts.md, KEY-06
        let context = Context {
            held_keys_available: false,
            ..CONTEXT
        };
        let text = text("alternative-browser", &context).expect("text");
        assert!(
            text.ends_with("only works for links from the Wye browser extension."),
            "{text}"
        );
    }

    #[test]
    fn every_id_has_a_text_and_an_unknown_one_none() {
        for id in [
            "alternative-browser",
            "browser-profiles",
            "open-in-background",
            "force-new-window",
            "remove-tracking",
            "bypass-key",
            "global-shortcuts",
            "held-keys-unavailable",
            "clipboard-unavailable",
        ] {
            assert!(
                text(id, &CONTEXT).is_some_and(|text| !text.is_empty()),
                "{id}"
            );
        }
        assert_eq!(text("nope", &CONTEXT), None);
    }

    #[test]
    fn no_text_is_longer_than_four_sentences() {
        // 19-help-texts.md: two to four sentences
        for id in ["browser-profiles", "open-in-background", "remove-tracking"] {
            let text = text(id, &CONTEXT).expect("text");
            assert!(text.matches(". ").count() <= 4, "{id}: {text}");
        }
    }
}
