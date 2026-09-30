//! The C++ shim in `cpp/wye_shim.{h,cpp}` (design B): the application
//! object and xdg-activation tokens. The picker declares the shim's blur
//! itself (`super::picker`).
//!
//! Only [`super::window_effects`] and `crate::qt_app` call it.

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("wye-ui/cpp/wye_shim.h");
        include!("cxx-qt-lib/qstring.h");
        include!(<QtCore/QObject>);
        include!(<QtGui/QWindow>);
        include!(<QtWidgets/QApplication>);

        type QString = cxx_qt_lib::QString;
        type QObject = cxx_qt::QObject;
        /// A top-level window; QML's `Window` is one.
        type QWindow;
        /// The application object; created once, before any QML.
        type QApplication;

        /// Create the application with the process arguments.
        #[namespace = "wye"]
        #[cxx_name = "applicationNew"]
        fn application_new(
            args: &[String],
            desktop_file_name: &QString,
            display_name: &QString,
        ) -> UniquePtr<QApplication>;

        /// Run the event loop until `exit`.
        #[namespace = "wye"]
        #[cxx_name = "applicationExec"]
        fn application_exec() -> i32;

        /// Request an activation token; `receiver` gets the signal
        /// `activationTokenReady(QString)`. False when not on Wayland.
        #[namespace = "wye"]
        #[cxx_name = "requestActivationToken"]
        fn request_activation_token(
            window: Pin<&mut QWindow>,
            app_id: &QString,
            receiver: Pin<&mut QObject>,
        ) -> bool;
    }
}
