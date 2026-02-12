mod app_state;

use core::pin::Pin;

pub use app_state::AppStateRust;
pub use app_state::SessionControllerRust;
pub use app_state::ConversationListRust;

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "C++" {
        include!(< QtCore/QAbstractListModel >);
        type QAbstractListModel;

        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;

        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;

        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
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

        #[qinvokable]
        fn initialize(self: Pin<&mut AppState>);
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
    impl cxx_qt::Threading for ConversationList {}

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QAbstractListModel]
        #[qproperty(bool, loading)]
        type ConversationList = super::ConversationListRust;

        #[cxx_override]
        #[rust_name = "row_count"]
        fn rowCount(&self, parent: &QModelIndex) -> i32;

        #[cxx_override]
        fn data(&self, index: &QModelIndex, role: i32) -> QVariant;

        #[cxx_override]
        #[rust_name = "role_names"]
        fn roleNames(&self) -> QHash_i32_QByteArray;

        #[qinvokable]
        fn load(self: Pin<&mut ConversationList>);

        #[qinvokable]
        fn apply_filter(self: Pin<&mut ConversationList>, filter: &QString);

        #[inherit]
        #[rust_name = "begin_reset_model"]
        fn beginResetModel(self: Pin<&mut Self>);

        #[inherit]
        #[rust_name = "end_reset_model"]
        fn endResetModel(self: Pin<&mut Self>);
    }
}
