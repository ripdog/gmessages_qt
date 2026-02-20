mod app_state;

use core::pin::Pin;

pub use app_state::AppStateRust;
pub use app_state::ConversationListRust;
pub use app_state::MessageListRust;
pub use app_state::SessionControllerRust;

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

    // ── AppState ─────────────────────────────────────────────────

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
        fn cancel_login(self: Pin<&mut AppState>);

        #[qinvokable]
        fn initialize(self: Pin<&mut AppState>);

        #[qinvokable]
        fn logout(self: Pin<&mut AppState>, reason: &QString);
    }

    impl cxx_qt::Threading for AppState {}

    // ── SessionController ────────────────────────────────────────

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, running)]
        #[qproperty(QString, status)]
        type SessionController = super::SessionControllerRust;

        #[qsignal]
        fn message_received(
            self: Pin<&mut SessionController>,
            conversation_id: &QString,
            participant_id: &QString,
            body: &QString,
            transport_type: i64,
            message_id: &QString,
            tmp_id: &QString,
            timestamp_micros: i64,
            status_code: i32,
            is_media: bool,
            media_id: &QString,
            decryption_key: &QString,
            mime_type: &QString,
        );

        #[qsignal]
        fn conversation_updated(
            self: Pin<&mut SessionController>,
            conversation_id: &QString,
            name: &QString,
            preview: &QString,
            unread: bool,
            last_message_timestamp: i64,
            is_group_chat: bool,
        );

        #[qinvokable]
        fn start(self: Pin<&mut SessionController>);

        #[qinvokable]
        fn stop(self: Pin<&mut SessionController>);
    }

    impl cxx_qt::Threading for SessionController {}
    impl cxx_qt::Threading for ConversationList {}

    // ── ConversationList ─────────────────────────────────────────

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

        #[qinvokable]
        fn conversation_id(self: &ConversationList, row: i32) -> QString;

        #[qinvokable]
        fn me_participant_id(self: &ConversationList, row: i32) -> QString;

        #[qinvokable]
        fn handle_conversation_event(
            self: Pin<&mut ConversationList>,
            conversation_id: &QString,
            name: &QString,
            preview: &QString,
            unread: bool,
            last_message_timestamp: i64,
            is_group_chat: bool,
        );

        #[qinvokable]
        fn mark_conversation_read(self: Pin<&mut ConversationList>, conversation_id: &QString);

        #[qinvokable]
        fn update_preview(
            self: Pin<&mut ConversationList>,
            conversation_id: &QString,
            preview: &QString,
            timestamp_micros: i64,
        );

        #[qsignal]
        fn auth_error(self: Pin<&mut ConversationList>, message: &QString);

        #[inherit]
        #[rust_name = "begin_reset_model"]
        fn beginResetModel(self: Pin<&mut Self>);

        #[inherit]
        #[rust_name = "end_reset_model"]
        fn endResetModel(self: Pin<&mut Self>);

        #[inherit]
        #[rust_name = "data_changed"]
        fn dataChanged(self: Pin<&mut Self>, top_left: &QModelIndex, bottom_right: &QModelIndex);

        #[inherit]
        fn index(self: &Self, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;
    }

    // ── MessageList ──────────────────────────────────────────────

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QAbstractListModel]
        #[qproperty(bool, loading)]
        type MessageList = super::MessageListRust;

        #[cxx_override]
        #[rust_name = "row_count"]
        fn rowCount(&self, parent: &QModelIndex) -> i32;

        #[cxx_override]
        fn data(&self, index: &QModelIndex, role: i32) -> QVariant;

        #[cxx_override]
        #[rust_name = "role_names"]
        fn roleNames(&self) -> QHash_i32_QByteArray;

        #[qinvokable]
        fn load(self: Pin<&mut MessageList>, conversation_id: &QString);

        #[qinvokable]
        fn send_message(self: Pin<&mut MessageList>, text: &QString);

        #[qinvokable]
        fn send_typing(self: Pin<&mut MessageList>, typing: bool);

        #[qinvokable]
        fn handle_message_event(
            self: Pin<&mut MessageList>,
            conversation_id: &QString,
            participant_id: &QString,
            body: &QString,
            transport_type: i64,
            message_id: &QString,
            tmp_id: &QString,
            timestamp_micros: i64,
            status_code: i32,
            is_media: bool,
        );

        #[qinvokable]
        fn queue_media_download(
            self: Pin<&mut MessageList>,
            message_id: &QString,
            media_id: &QString,
            decryption_key: &QString,
            mime_type: &QString,
        );

        #[qsignal]
        fn auth_error(self: Pin<&mut MessageList>, message: &QString);

        #[inherit]
        #[rust_name = "begin_reset_model"]
        fn beginResetModel(self: Pin<&mut Self>);

        #[inherit]
        #[rust_name = "end_reset_model"]
        fn endResetModel(self: Pin<&mut Self>);

        #[inherit]
        #[rust_name = "begin_insert_rows"]
        fn beginInsertRows(self: Pin<&mut Self>, parent: &QModelIndex, first: i32, last: i32);

        #[inherit]
        #[rust_name = "end_insert_rows"]
        fn endInsertRows(self: Pin<&mut Self>);

        #[inherit]
        #[rust_name = "data_changed"]
        fn dataChanged(self: Pin<&mut Self>, top_left: &QModelIndex, bottom_right: &QModelIndex);

        #[inherit]
        fn index(self: &Self, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;
    }

    impl cxx_qt::Threading for MessageList {}
}
