//! libadwaita's success and error colours for the places CSS cannot reach:
//! Pango markup in the result line (SCR-04) and a `GtkTextTag` on the error
//! line (SCR-05). The values are libadwaita 1.9's `--success-color`,
//! `--success-bg-color`, `--error-color` and `--error-bg-color` for the light
//! and the dark style; the style manager says which one is on.

/// One style's colours, as `#rrggbb`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub success: &'static str,
    pub success_bg: &'static str,
    pub error: &'static str,
    pub error_bg: &'static str,
}

const LIGHT: Palette = Palette {
    success: "#1b8553",
    success_bg: "#2ec27e",
    error: "#c01c28",
    error_bg: "#e01b24",
};

const DARK: Palette = Palette {
    success: "#78e9ab",
    success_bg: "#26a269",
    error: "#ff938c",
    error_bg: "#c01c28",
};

/// The colours of the style now on screen.
#[must_use]
pub fn current() -> Palette {
    of(adw::StyleManager::default().is_dark())
}

/// The colours of the dark style or the light one.
#[must_use]
pub const fn of(dark: bool) -> Palette {
    if dark { DARK } else { LIGHT }
}

/// `hex` (`#rrggbb`) with `alpha`, for a text tag.
#[must_use]
pub fn rgba(hex: &str, alpha: f32) -> gtk::gdk::RGBA {
    let color = gtk::gdk::RGBA::parse(hex).unwrap_or(gtk::gdk::RGBA::BLACK);
    color.with_alpha(alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_colour_parses() {
        for palette in [of(false), of(true)] {
            for hex in [
                palette.success,
                palette.success_bg,
                palette.error,
                palette.error_bg,
            ] {
                assert!(gtk::gdk::RGBA::parse(hex).is_ok(), "{hex}");
            }
        }
        assert!((rgba("#ffffff", 0.5).alpha() - 0.5).abs() < f32::EPSILON);
    }
}
