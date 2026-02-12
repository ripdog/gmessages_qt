mod app_state;

use core::pin::Pin;

pub use app_state::AppStateRust;
pub use app_state::SessionControllerRust;

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, logged_in)]
        #[qproperty(bool, login_in_progress)]
        #[qproperty(QString, qr_url)]
        #[qproperty(QString, qr_svg_data_url)]
        #[qproperty(QString, status_message)]
        type AppState = super::AppStateRust;

        #[qinvokable]
        fn start_login(self: Pin<&mut AppState>);
    }

    impl cxx_qt::Threading for AppState {}

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, running)]
        #[qproperty(QString, status)]
        type SessionController = super::SessionControllerRust;

        #[qinvokable]
        fn start(self: Pin<&mut SessionController>);

        #[qinvokable]
        fn stop(self: Pin<&mut SessionController>);
    }

    impl cxx_qt::Threading for SessionController {}
}
