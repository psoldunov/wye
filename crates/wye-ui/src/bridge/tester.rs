//! `TesterBackend`: the Rust side of the rule tester sheet (17-dialogs.md,
//! DLG-TST-01 to DLG-TST-03). It routes a link with `TestLink` (nothing
//! opens) and publishes the trace as the rows the sheet shows
//! ([`crate::rules::tester`]).
//!
//! Used from `qml/rules/RuleTesterSheet.qml`.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, busy)]
        #[qproperty(QString, view_json, cxx_name = "viewJson")]
        #[qproperty(QString, error)]
        type TesterBackend = super::TesterBackendRust;

        /// Route `url` as a link from `source_app` with `held_json` keys
        /// held (DLG-TST-01).
        #[qinvokable]
        fn test(
            self: Pin<&mut Self>,
            url: &QString,
            source_app: &QString,
            held_json: &QString,
            skip_network: bool,
        );

        /// Show `trace_json` instead of asking the service (`--self-test`).
        /// False when it is not a trace.
        #[qinvokable]
        #[cxx_name = "loadTrace"]
        fn load_trace(self: Pin<&mut Self>, trace_json: &QString) -> bool;
    }

    impl cxx_qt::Threading for TesterBackend {}
}

use core::pin::Pin;
use std::collections::HashMap;

use cxx_qt::{CxxQtType as _, Threading as _};
use cxx_qt_lib::QString;
use serde_json::Value;
use zbus::zvariant;

use crate::rules::tester;
use crate::service;

/// The tester's properties and its run counter.
#[derive(Default)]
pub struct TesterBackendRust {
    busy: bool,
    view_json: QString,
    error: QString,
    /// Numbers runs, so a late answer never replaces a newer one.
    run: u64,
    /// A fixture's trace stands in for the service.
    offline: bool,
}

fn qs(text: &str) -> QString {
    QString::from(text)
}

/// The JSON context as D-Bus values.
fn dict(context: &Value) -> HashMap<&str, zvariant::Value<'_>> {
    let Some(object) = context.as_object() else {
        return HashMap::new();
    };
    object
        .iter()
        .filter_map(|(key, value)| {
            let value = match value {
                Value::Bool(flag) => zvariant::Value::from(*flag),
                Value::String(text) => zvariant::Value::from(text.as_str()),
                Value::Array(items) => zvariant::Value::from(
                    items.iter().filter_map(Value::as_str).collect::<Vec<_>>(),
                ),
                _ => return None,
            };
            Some((key.as_str(), value))
        })
        .collect()
}

impl qobject::TesterBackend {
    /// See the bridge declaration.
    pub fn test(
        mut self: Pin<&mut Self>,
        url: &QString,
        source_app: &QString,
        held_json: &QString,
        skip_network: bool,
    ) {
        let url = url.to_string();
        if url.trim().is_empty() {
            // DLG-TST-01: no link, no result. An answer still on its way is for a link that is gone, so it must not
            // land (the run number moves on) and the sheet must not keep spinning for it.
            self.as_mut().rust_mut().get_mut().run += 1;
            self.as_mut().set_busy(false);
            self.as_mut().set_error(QString::default());
            self.as_mut().set_view_json(QString::default());
            return;
        }
        if self.offline {
            return;
        }
        let run = self.run + 1;
        self.as_mut().rust_mut().get_mut().run = run;
        let held: Vec<String> = serde_json::from_str(&held_json.to_string()).unwrap_or_default();
        let context = tester::context(&source_app.to_string(), &held, skip_network);
        self.as_mut().set_busy(true);
        service::request(
            self.qt_thread(),
            move |proxy| async move { proxy.test_link(&url, dict(&context)).await },
            move |mut backend, answer| {
                if backend.run != run {
                    return;
                }
                backend.as_mut().set_busy(false);
                match answer
                    .map_err(|error| error.to_string())
                    .and_then(|json| tester::view(&json))
                {
                    Ok(view) => {
                        backend.as_mut().set_error(QString::default());
                        backend
                            .set_view_json(qs(&serde_json::to_string(&view).unwrap_or_default()));
                    }
                    Err(error) => {
                        backend.as_mut().set_view_json(QString::default());
                        backend.set_error(qs(&error));
                    }
                }
            },
        );
    }

    /// See the bridge declaration.
    pub fn load_trace(mut self: Pin<&mut Self>, trace_json: &QString) -> bool {
        match tester::view(&trace_json.to_string()) {
            Ok(view) => {
                self.as_mut().rust_mut().get_mut().offline = true;
                self.set_view_json(qs(&serde_json::to_string(&view).unwrap_or_default()));
                true
            }
            Err(error) => {
                self.set_error(qs(&error));
                false
            }
        }
    }
}
