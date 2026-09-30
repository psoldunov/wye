//! Running the `KWin` query script in `QuickJS` against a stub `workspace`,
//! the way `KWin`'s script engine would: syntax, property names and the
//! arguments of its `callDBus`.

use rquickjs::{Context, Runtime};

/// What the stub `KWin` shows the script.
#[derive(Debug, Clone)]
pub struct Scene {
    /// `workspace.cursorPos`.
    pub cursor: (f64, f64),
    /// The output under the pointer: name and top-left corner; `None` for
    /// `screenAt` answering `null`.
    pub output: Option<(&'static str, f64, f64)>,
    /// `workspace.activeWindow`: pid, desktop file name, resource class;
    /// `None` for no active window.
    pub window: Option<(i32, &'static str, &'static str)>,
}

impl Scene {
    /// A Dolphin window on the right-hand monitor, whose origin is
    /// fractional as with fractional scaling.
    pub fn dolphin() -> Self {
        Self {
            cursor: (2030.0, 1252.0),
            output: Some(("DP-1", 1920.5, 0.0)),
            window: Some((4242, "org.kde.dolphin", "dolphin")),
        }
    }

    fn stub(&self) -> String {
        let output = self.output.map_or_else(
            || "null".to_owned(),
            |(name, x, y)| {
                format!(
                    "{{ name: {name:?}, geometry: {{ x: {x}, y: {y}, width: 2560, height: 1440 }} }}"
                )
            },
        );
        let window = self.window.map_or_else(
            || "null".to_owned(),
            |(pid, desktop, class)| {
                format!("{{ pid: {pid}, desktopFileName: {desktop:?}, resourceClass: {class:?} }}")
            },
        );
        format!(
            r#"
var workspace = {{
    cursorPos: {{ x: {x}, y: {y} }},
    screenAt: function (point) {{
        if (point !== workspace.cursorPos) throw new Error("screenAt got another point");
        return {output};
    }},
    activeWindow: {window},
}};
var recorded = null;
function callDBus() {{
    var args = Array.prototype.slice.call(arguments);
    args.forEach(function (arg) {{
        if (typeof arg === "number" && !Number.isInteger(arg)) {{
            throw new Error("KWin would send " + arg + " as a double");
        }}
        if (typeof arg !== "number" && typeof arg !== "string") {{
            throw new Error("KWin cannot marshal " + typeof arg);
        }}
    }});
    if (recorded !== null) throw new Error("callDBus called twice");
    recorded = JSON.stringify(args);
}}
"#,
            x = self.cursor.0,
            y = self.cursor.1,
        )
    }
}

/// Run `script` against `scene`; the arguments it passed to `callDBus`, as
/// JSON.
///
/// # Panics
///
/// When the script does not compile, throws, or never calls `callDBus`.
pub fn run(script: &str, scene: &Scene) -> serde_json::Value {
    let runtime = Runtime::new().expect("a QuickJS runtime");
    let context = Context::full(&runtime).expect("a QuickJS context");
    let recorded: String = context.with(|context| {
        let run = |source: String| {
            context
                .eval::<(), _>(source)
                .map_err(|error| describe(&context, &error))
        };
        run(scene.stub()).expect("the stub runs");
        run(script.to_owned()).expect("the script runs");
        context
            .eval::<String, _>("recorded")
            .expect("the script called callDBus")
    });
    serde_json::from_str(&recorded).expect("JSON")
}

fn describe(context: &rquickjs::Ctx<'_>, error: &rquickjs::Error) -> String {
    let thrown = context.catch();
    let message = thrown
        .as_exception()
        .and_then(rquickjs::Exception::message)
        .unwrap_or_default();
    format!("{error}: {message}")
}
