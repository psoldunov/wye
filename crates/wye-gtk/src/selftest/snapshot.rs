//! `wye-gtk --self-test [SURFACE] --snapshots DIR`: after each case, a PNG
//! of every visible window, rendered by the window's own GSK renderer from
//! the window's render node (`GtkWidgetPaintable`), at the surface's scale
//! (`--scale`, `GDK_SCALE`), so text and icons are crisp.
//!
//! Each case's files are named `<surface>-<NN>-<slug>.png`, plus `-w<M>`
//! for the second and later windows and `-p<K>` for each open popover of a
//! window (a popover is a surface of its own). A dev tool: the real gallery is
//! captured on the GNOME stage (docs/media/gnome/stage/). Windows have no
//! frame or shadow here (the private X server has no compositor).
//!
//! The naming mirrors crates/wye-ui/src/selftest/snapshot.rs; keep the two
//! in step.

use std::path::Path;

use adw::prelude::*;
use anyhow::Context as _;
use gtk::{graphene, gsk};
use serde_json::Value;

use super::fixtures::Case;
use crate::surface::Surface;

/// How long a snapshot child may take: every case waits for its render.
pub const TIMEOUT_SECS: u64 = 300;

/// Longest slug in a file name.
const MAX_SLUG: usize = 48;

/// Fields of a JSON argument that name what the case shows, in file-name
/// order.
const NAMING_FIELDS: [&str; 4] = ["page", "sheet", "scope", "scheme"];

/// The path prefix of the case at `index`: `DIR/<surface>-<NN>-<slug>`.
pub fn prefix(dir: &Path, surface: Surface, index: usize, case: &Case) -> String {
    let stem = format!(
        "{}-{:02}-{}",
        surface.name(),
        index + 1,
        slug(surface, case)
    );
    dir.join(stem).to_string_lossy().into_owned()
}

/// What the case shows, as a file-name part: the key unless it is the
/// surface's own name, then the argument (a plain string, or the naming
/// fields of a JSON object).
fn slug(surface: Surface, case: &Case) -> String {
    let key = (case.key != surface.name()).then_some(case.key.as_str());
    let words: Vec<String> = key
        .into_iter()
        .map(str::to_owned)
        .chain(argument_words(&case.argument))
        .collect();
    let slug = sanitize(&words.join("-"));
    if slug.is_empty() {
        case.action.as_str().to_owned()
    } else {
        slug
    }
}

fn argument_words(argument: &str) -> Vec<String> {
    match serde_json::from_str::<Value>(argument) {
        Ok(Value::Object(object)) => NAMING_FIELDS
            .iter()
            .filter_map(|field| object.get(*field).and_then(Value::as_str))
            .map(str::to_owned)
            .collect(),
        Ok(Value::String(text)) => vec![text],
        Ok(_) => Vec::new(),
        Err(_) => vec![argument.to_owned()],
    }
}

/// Lowercase letters and digits of any script, runs of anything else as one
/// `-`, at most [`MAX_SLUG`] characters. Unlike the ASCII-only naming of
/// wye-ui's gallery, a case named in another script (a translated page or
/// rule name) keeps its words instead of falling back to the action.
fn sanitize(text: &str) -> String {
    let mut slug = String::new();
    let mut gap = false;
    for c in text.chars().flat_map(char::to_lowercase) {
        if !c.is_alphanumeric() {
            gap = !slug.is_empty();
            continue;
        }
        if slug.chars().count() >= MAX_SLUG {
            break;
        }
        if gap {
            slug.push('-');
            gap = false;
        }
        slug.push(c);
    }
    slug
}

/// Save every visible window as `<prefix>.png`, `<prefix>-w2.png`, …; the
/// paths saved, in order.
///
/// # Errors
///
/// When a window cannot be rendered or the file cannot be written.
pub fn save_windows(prefix: &str) -> anyhow::Result<Vec<String>> {
    let windows: Vec<gtk::Window> = gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Window>().ok())
        .filter(|window| window.is_visible() && window.is_mapped())
        .collect();
    let mut saved = Vec::new();
    for (index, window) in windows.iter().enumerate() {
        let stem = if index == 0 {
            prefix.to_owned()
        } else {
            format!("{prefix}-w{}", index + 1)
        };
        let path = format!("{stem}.png");
        // The tray-menu popup's window is only the transparent anchor of
        // its popover: nothing to save but the popover.
        if save(window.upcast_ref(), Path::new(&path))? {
            saved.push(path);
        }
        // A popover is a surface of its own, which the window's render node
        // does not hold: an open menu (BLK-04's target menu) or help popover.
        for (number, popover) in open_popovers(window.upcast_ref()).iter().enumerate() {
            let path = format!("{stem}-p{}.png", number + 1);
            if save(popover.upcast_ref(), Path::new(&path))? {
                saved.push(path);
            }
        }
    }
    Ok(saved)
}

/// The popovers under `widget` that are on screen, in widget-tree order.
fn open_popovers(widget: &gtk::Widget) -> Vec<gtk::Popover> {
    let mut open = Vec::new();
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(popover) = current.downcast_ref::<gtk::Popover>()
            && popover.is_visible()
            && popover.is_mapped()
        {
            open.push(popover.clone());
        }
        open.extend(open_popovers(&current));
        child = current.next_sibling();
    }
    open
}

#[allow(
    clippy::cast_precision_loss,
    reason = "surface sizes and scale factors are small integers, exact in f32"
)]
/// Save what `native` draws to `path`; false when it draws nothing.
fn save(native: &gtk::Native, path: &Path) -> anyhow::Result<bool> {
    let (width, height) = (native.width(), native.height());
    anyhow::ensure!(width > 0 && height > 0, "a surface has no size yet");
    let scale = native
        .surface()
        .map_or(1.0, |surface| surface.scale())
        .max(1.0);
    let paintable = gtk::WidgetPaintable::new(Some(native));
    let snapshot = gtk::Snapshot::new();
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a surface scale is a small factor such as 1, 1.5 or 2"
    )]
    snapshot.scale(scale as f32, scale as f32);
    paintable.snapshot(&snapshot, f64::from(width), f64::from(height));
    let Some(node) = snapshot.to_node() else {
        return Ok(false);
    };
    let renderer = native.renderer().context("the surface has no renderer")?;
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a surface scale is a small factor such as 1, 1.5 or 2"
    )]
    let viewport = graphene::Rect::new(
        0.0,
        0.0,
        width as f32 * scale as f32,
        height as f32 * scale as f32,
    );
    let texture = gsk::Renderer::render_texture(&renderer, &node, Some(&viewport));
    texture
        .save_to_png(path)
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::Action;

    fn case(key: &str, argument: &str) -> Case {
        Case {
            action: Action::Show,
            key: key.to_owned(),
            argument: argument.to_owned(),
        }
    }

    #[test]
    fn a_page_argument_names_the_file() {
        let dir = Path::new("/tmp/shots");
        assert_eq!(
            prefix(dir, Surface::Settings, 0, &case("settings", "browsers")),
            "/tmp/shots/settings-01-browsers"
        );
        assert_eq!(
            prefix(
                dir,
                Surface::Settings,
                1,
                &case(
                    "settings",
                    r#"{"page":"general","scheme":"dark","fixture":{}}"#
                )
            ),
            "/tmp/shots/settings-02-general-dark"
        );
        assert_eq!(
            prefix(
                dir,
                Surface::Settings,
                2,
                &case("rule-editor", r#"{"domain":"x"}"#)
            ),
            "/tmp/shots/settings-03-rule-editor"
        );
        assert_eq!(
            prefix(dir, Surface::About, 0, &case("about", "")),
            "/tmp/shots/about-01-show"
        );
    }

    #[test]
    fn slugs_are_short_and_plain() {
        let long = "A".repeat(100);
        let slug = sanitize(&format!("--Ünïcode {long}"));
        assert!(slug.chars().count() <= MAX_SLUG);
        assert!(slug.starts_with("ünïcode-aaa"));
        assert!(!slug.ends_with('-'));
        // A case named in another script keeps its words.
        assert_eq!(sanitize("Правила · Работа"), "правила-работа");
        assert_eq!(sanitize("  -- "), "");
    }
}
