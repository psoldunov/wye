//! Reading X11 replies: modifier masks, window properties, monitors. Pure,
//! so it is tested without an X server.

use wye_api::context::Modifier;
use wye_api::picker::Placement;

/// Core protocol modifier bits (`KeyButMask`) for the keys Wye knows, with
/// the usual mapping: Mod1 is Alt, Mod4 is Super.
const MASK_BITS: [(u16, Modifier); 4] = [
    (1 << 0, Modifier::Shift),
    (1 << 2, Modifier::Ctrl),
    (1 << 3, Modifier::Alt),
    (1 << 6, Modifier::Super),
];

/// The modifiers held in a `QueryPointer` mask; Lock, Mod2 (Num Lock) and
/// mouse buttons do not count.
#[must_use]
pub fn modifiers(mask: u16) -> Vec<Modifier> {
    MASK_BITS
        .iter()
        .filter(|(bit, _)| mask & bit != 0)
        .map(|(_, modifier)| *modifier)
        .collect()
}

/// A `STRING` or `UTF8_STRING` property: its text up to the first NUL;
/// `None` when empty or not UTF-8.
#[must_use]
pub fn text(bytes: &[u8]) -> Option<String> {
    let text = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
    std::str::from_utf8(text)
        .ok()
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// The class of `WM_CLASS` (`instance\0class\0`), or the instance when the
/// class is missing.
#[must_use]
pub fn wm_class(bytes: &[u8]) -> Option<String> {
    let mut parts = bytes.split(|byte| *byte == 0);
    let instance = parts.next().and_then(text);
    parts.next().and_then(text).or(instance)
}

/// One `RandR` monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monitor {
    /// Output name, such as `DP-1`.
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Where the pointer at root position (`x`, `y`) is, relative to the
/// monitor that contains it; `None` when no monitor does.
#[must_use]
pub fn placement(monitors: &[Monitor], x: i32, y: i32) -> Option<Placement> {
    monitors
        .iter()
        .find(|monitor| {
            (monitor.x..monitor.x + monitor.width).contains(&x)
                && (monitor.y..monitor.y + monitor.height).contains(&y)
        })
        .map(|monitor| Placement {
            output: monitor.name.clone(),
            x: x - monitor.x,
            y: y - monitor.y,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_bits_map_to_modifiers_key_06() {
        assert_eq!(modifiers(0), Vec::<Modifier>::new());
        assert_eq!(modifiers(1), vec![Modifier::Shift]);
        assert_eq!(modifiers(4 | 8), vec![Modifier::Ctrl, Modifier::Alt]);
        assert_eq!(modifiers(64), vec![Modifier::Super]);
        // Lock, Num Lock (Mod2) and button 1 are not held keys.
        assert_eq!(modifiers(2 | 16 | 256), Vec::<Modifier>::new());
    }

    #[test]
    fn text_properties_stop_at_the_first_nul() {
        assert_eq!(
            text(b"org.kde.dolphin\0"),
            Some("org.kde.dolphin".to_owned())
        );
        assert_eq!(text(b"firefox"), Some("firefox".to_owned()));
        assert_eq!(text(b""), None);
        assert_eq!(text(b"\0"), None);
        assert_eq!(text(&[0xff, 0xfe]), None);
    }

    #[test]
    fn wm_class_prefers_the_class() {
        assert_eq!(
            wm_class(b"navigator\0firefox\0"),
            Some("firefox".to_owned())
        );
        assert_eq!(wm_class(b"xterm\0"), Some("xterm".to_owned()));
        assert_eq!(wm_class(b""), None);
    }

    #[test]
    fn the_pointer_is_placed_on_the_monitor_under_it_pick_02() {
        let monitors = [
            Monitor {
                name: "eDP-1".to_owned(),
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            Monitor {
                name: "DP-1".to_owned(),
                x: 1920,
                y: 0,
                width: 2560,
                height: 1440,
            },
        ];
        assert_eq!(
            placement(&monitors, 2000, 100),
            Some(Placement {
                output: "DP-1".to_owned(),
                x: 80,
                y: 100,
            })
        );
        assert_eq!(
            placement(&monitors, 1919, 1079).map(|at| at.output),
            Some("eDP-1".to_owned())
        );
        assert_eq!(placement(&monitors, 100, 1200), None);
    }
}
