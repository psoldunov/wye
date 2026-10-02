//! The transform script editor (16-script-editor.md, SCR-01 to SCR-10): one
//! window for the global script and each rule's, with a `GtkSourceView` code
//! area (SCR-02), a Test group that runs the script on a link as the user
//! types (SCR-04), the Reference beside the code (SCR-06), a reload offer
//! when the file changes on disk (SCR-08) and a question before unsaved
//! changes are closed away (SCR-10).
//!
//! One instance (SET-04): showing it again raises it and opens the script the
//! argument names. The argument is a scope (`global`, `rule:<id>`) as the
//! service sends it, or JSON `{"scope", "ruleName", "fixture"}` from the rule
//! editor and the self-test.
//!
//! GTK-free logic from wye-ui (one source for both frontends; crates/wye-ui
//! owns it): [`document`] (saved against edited text, SCR-07; the source
//! apps), [`editing`] (the indent Return adds, SCR-02), [`opening`] (the
//! argument, the title, the self-test's fixture), [`readiness`] (the first
//! run waits for the script and the link) and [`result`] (what a `RunScript`
//! answer shows).
//!
//! On GTK: [`frame`] (the window), [`code`] (the code area), [`indenter`]
//! (`GtkSourceView`'s auto-indent with wye-ui's rule), [`test_group`] (the
//! Test rows), [`reference`] (the API and examples), [`unsaved`] (the SCR-10
//! question), [`controller`] (the state and the service calls).
//!
//! The self-test's fixture keys `edit` (type over the loaded source),
//! `reference` (show the Reference) and `confirmClose` (close, to show the
//! SCR-10 question) are read by [`controller`].
//!
//! KDE counterpart: crates/wye-ui/qml/script/, crates/wye-ui/src/script_editor/,
//! crates/wye-ui/src/bridge/script_editor.rs.

mod calls;
mod code;
mod controller;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
#[path = "../../../wye-ui/src/script_editor/document.rs"]
#[allow(
    dead_code,
    reason = "the file is shared whole; GTK fills its popup from the choices, not their JSON"
)]
mod document;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
#[path = "../../../wye-ui/src/script_editor/editing.rs"]
#[allow(
    dead_code,
    reason = "the file is shared whole; GtkSourceView matches brackets and inserts tabs itself"
)]
mod editing;
mod frame;
mod indenter;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
#[path = "../../../wye-ui/src/script_editor/opening.rs"]
mod opening;
mod palette;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
#[path = "../../../wye-ui/src/script_editor/readiness.rs"]
mod readiness;
mod reference;
// Compiled from wye-ui's Qt-free model (one source for both frontends); crates/wye-ui owns it.
#[path = "../../../wye-ui/src/script_editor/result.rs"]
#[allow(
    dead_code,
    reason = "the file is shared whole; GTK renders the segments, not their JSON or QML names"
)]
mod result;
mod test_group;
mod unsaved;

use std::cell::OnceCell;
use std::rc::Rc;

use crate::app::Presenter;
use controller::Controller;

/// The script editor surface.
#[derive(Debug)]
pub struct ScriptEditor {
    app: adw::Application,
    controller: OnceCell<Rc<Controller>>,
}

impl ScriptEditor {
    /// The surface; the window is built when first shown.
    #[must_use]
    pub fn new(app: &adw::Application) -> Self {
        Self {
            app: app.clone(),
            controller: OnceCell::new(),
        }
    }
}

impl Presenter for ScriptEditor {
    fn present(&self, _key: &str, argument: &str) {
        let controller = self.controller.get_or_init(|| Controller::new(&self.app));
        controller.show(argument);
    }

    fn before_quit(&self, quit: Box<dyn FnOnce()>) {
        match self.controller.get() {
            Some(controller) => controller.before_quit(quit),
            None => quit(),
        }
    }
}
