//! Profile badges (DISC-08, PICK-06): the picture of a profile, or a coloured
//! circle with its initial when it has none. The result is a
//! [`wye_core::target_menu::Badge`], which the picker and the target menus
//! draw.

use std::path::Path;

use wye_core::target_menu::Badge;

/// Colours for profiles that have none of their own, as `0xRRGGBB`. The
/// profile's name picks one, so a profile keeps its colour from run to run.
const PALETTE: [u32; 12] = [
    0x1a_73_e8, 0xd9_30_25, 0x18_80_38, 0xf2_99_00, 0x93_34_e6, 0x00_96_a6, 0xe5_2c_83, 0x5f_63_68,
    0x4e_34_2e, 0x00_61_a4, 0xc2_41_0c, 0x5b_7c_2e,
];

/// Used if the palette index cannot be formed, which it always can.
const FALLBACK_COLOR: u32 = 0x5f_63_68;

/// The badge of a profile: its picture when `image` names a file that
/// exists, else an initial on `color`, else an initial on a colour picked
/// from the name.
#[must_use]
pub fn badge(name: &str, image: Option<&Path>, color: Option<u32>) -> Badge {
    match image.filter(|path| path.is_file()) {
        Some(path) => Badge::Image(path.to_string_lossy().into_owned()),
        None => Badge::Initial {
            text: initial(name),
            color: color.unwrap_or_else(|| hashed_color(name)),
        },
    }
}

/// The first letter or digit of `name`, upper-cased; `?` for a name without
/// one.
#[must_use]
pub fn initial(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map_or_else(|| "?".to_owned(), |c| c.to_uppercase().collect())
}

/// A stable colour for `name` from a fixed palette (FNV-1a over the
/// lower-cased name).
#[must_use]
pub fn hashed_color(name: &str) -> u32 {
    let hash = name
        .to_lowercase()
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        });
    let index = usize::try_from(hash % PALETTE.len() as u64).unwrap_or(0);
    PALETTE.get(index).copied().unwrap_or(FALLBACK_COLOR)
}

/// The colour of an ARGB integer as Chromium stores it in `Local State`
/// (`profile_highlight_color`, `default_avatar_fill_color`): a signed 32-bit
/// number, for example `-14671840`. The alpha byte is dropped.
#[must_use]
pub fn rgb_from_argb(value: i64) -> u32 {
    u32::try_from(value & 0x00ff_ffff).unwrap_or(0)
}

/// True for colours too light to carry a white initial.
#[must_use]
pub fn is_light(rgb: u32) -> bool {
    let [_, r, g, b] = rgb.to_be_bytes();
    // Rec. 601 luma, scaled to 0..=255_000.
    let luma = u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114;
    luma > 217_000
}

/// A CSS colour as Firefox stores a profile theme (`#rgb`, `#rrggbb`,
/// `#rrggbbaa`, `rgb(…)`, `rgba(…)`), as `0xRRGGBB`. `None` for anything
/// else, including fully transparent colours and theme variables.
#[must_use]
pub fn parse_css_color(text: &str) -> Option<u32> {
    let text = text.trim();
    if let Some(hex) = text.strip_prefix('#') {
        return parse_hex(hex);
    }
    let lower = text.to_ascii_lowercase();
    let body = lower
        .strip_prefix("rgba(")
        .or_else(|| lower.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    parse_rgb_function(body)
}

fn parse_hex(hex: &str) -> Option<u32> {
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let value = |digits: &str| u32::from_str_radix(digits, 16).ok();
    match hex.len() {
        3 => {
            let mut rgb = 0;
            for digit in hex.chars() {
                rgb = (rgb << 8) | (value(&digit.to_string())? * 0x11);
            }
            Some(rgb)
        }
        6 => value(hex),
        8 if value(hex.get(6..)?)? != 0 => value(hex.get(..6)?),
        _ => None,
    }
}

fn parse_rgb_function(body: &str) -> Option<u32> {
    let parts: Vec<&str> = body
        .split([',', ' ', '/'])
        .filter(|p| !p.is_empty())
        .collect();
    if !(3..=4).contains(&parts.len()) {
        return None;
    }
    if let Some(alpha) = parts.get(3)
        && alpha.trim_end_matches('%').parse::<f64>().ok()? <= 0.0
    {
        return None;
    }
    let mut rgb = 0_u32;
    for channel in parts.iter().take(3) {
        let value: u8 = channel.parse().ok()?;
        rgb = (rgb << 8) | u32::from(value);
    }
    Some(rgb)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn a_picture_that_exists_wins() {
        let dir = tempfile::tempdir().unwrap();
        let picture = dir.path().join("avatar.png");
        fs::write(&picture, b"png").unwrap();
        assert_eq!(
            badge("Work", Some(&picture), Some(0xff_00_00)),
            Badge::Image(picture.to_string_lossy().into_owned())
        );
    }

    #[test]
    fn without_a_picture_the_badge_is_an_initial_on_the_given_colour() {
        let missing = Path::new("/nonexistent/avatar.png");
        for image in [None, Some(missing)] {
            assert_eq!(
                badge("work", image, Some(0x12_34_56)),
                Badge::Initial {
                    text: "W".into(),
                    color: 0x12_34_56
                }
            );
        }
    }

    #[test]
    fn without_any_colour_the_name_picks_one() {
        let first = badge("Personal", None, None);
        assert_eq!(first, badge("personal", None, None), "case does not matter");
        let Badge::Initial { text, color } = first else {
            panic!("expected an initial");
        };
        assert_eq!(text, "P");
        assert!(PALETTE.contains(&color));
        let colors: std::collections::HashSet<u32> =
            ["Work", "Home", "School", "Dev", "Tax", "Kids", "Travel"]
                .iter()
                .map(|name| hashed_color(name))
                .collect();
        assert!(
            colors.len() >= 3,
            "names spread over the palette: {colors:?}"
        );
    }

    #[test]
    fn initials() {
        assert_eq!(initial("work"), "W");
        assert_eq!(initial("  (Work)"), "W");
        assert_eq!(initial("ärger"), "Ä");
        assert_eq!(initial("ßeta"), "SS");
        assert_eq!(initial("2fa"), "2");
        assert_eq!(initial("Яндекс"), "Я");
        assert_eq!(initial(""), "?");
        assert_eq!(initial("()"), "?");
    }

    #[test]
    fn chromium_argb_integers() {
        // -14671840 is 0xFF202020 as a signed 32-bit number.
        assert_eq!(rgb_from_argb(-14_671_840), 0x20_20_20);
        assert_eq!(rgb_from_argb(0xff_1a_73_e8), 0x1a_73_e8);
        assert_eq!(rgb_from_argb(-1), 0xff_ff_ff);
        assert_eq!(rgb_from_argb(0), 0);
    }

    #[test]
    fn light_colours_are_recognised() {
        assert!(is_light(0xff_ff_ff));
        assert!(is_light(0xf0_f0_f4));
        assert!(!is_light(0x1a_73_e8));
        assert!(!is_light(0x00_00_00));
        assert!(!is_light(0xf2_99_00));
    }

    #[test]
    fn css_colours() {
        for (text, expected) in [
            ("#1a73e8", Some(0x1a_73_e8)),
            ("#FFF", Some(0xff_ff_ff)),
            ("#f80", Some(0xff_88_00)),
            ("#1a73e8ff", Some(0x1a_73_e8)),
            ("#1a73e800", None),
            ("rgb(26, 115, 232)", Some(0x1a_73_e8)),
            ("rgba(26, 115, 232, 0.5)", Some(0x1a_73_e8)),
            ("rgb(26 115 232 / 80%)", Some(0x1a_73_e8)),
            ("rgb(26 115 232 / 0%)", None),
            ("rgba(0, 0, 0, 0)", None),
            ("rgb(300, 0, 0)", None),
            ("rgb(1, 2)", None),
            ("#12345", None),
            ("#ggg", None),
            ("var(--toolbar-bgcolor)", None),
            ("red", None),
            ("", None),
        ] {
            assert_eq!(parse_css_color(text), expected, "{text}");
        }
    }
}
