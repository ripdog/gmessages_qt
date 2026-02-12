use crate::ffi;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use core::pin::Pin;
use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::QString;
use libgmessages_rs::{
    auth::AuthData,
    gmclient::GMClient,
    store::AuthDataStore,
};
use qrcode::render::svg;
use qrcode::QrCode;
use std::time::Duration;
use crate::ffi::QHash_i32_QByteArray;
use crate::ffi::QModelIndex;
use crate::ffi::QVariant;

pub struct AppStateRust {
    pub logged_in: bool,
    pub login_in_progress: bool,
    pub qr_url: QString,
    pub qr_svg_data_url: QString,
    pub status_message: QString,
}

impl Default for AppStateRust {
    fn default() -> Self {
        Self {
            logged_in: false,
            login_in_progress: false,
            qr_url: QString::from(""),
            qr_svg_data_url: QString::from(""),
            status_message: QString::from("Not logged in"),
        }
    }
}

pub struct SessionControllerRust {
    pub running: bool,
    pub status: QString,
    should_stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Default for SessionControllerRust {
    fn default() -> Self {
        Self {
            running: false,
            status: QString::from("Idle"),
            should_stop: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

#[derive(Clone)]
pub struct ConversationItem {
    name: QString,
    preview: QString,
}

pub struct ConversationListRust {
    all_items: Vec<ConversationItem>,
    filtered_items: Vec<ConversationItem>,
    filter_text: String,
    pub loading: bool,
}

impl Default for ConversationListRust {
    fn default() -> Self {
        Self {
            all_items: Vec::new(),
            filtered_items: Vec::new(),
            filter_text: String::new(),
            loading: false,
        }
    }
}

fn qr_to_svg_data_url(data: &str) -> Result<String, Box<dyn std::error::Error>> {
    let code = QrCode::new(data.as_bytes())?;
    let svg = code
        .render::<svg::Color>()
        .min_dimensions(256, 256)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();
    let encoded = STANDARD.encode(svg.as_bytes());
    Ok(format!("data:image/svg+xml;base64,{encoded}"))
}

impl crate::ffi::AppState {
    pub fn start_login(mut self: Pin<&mut Self>) {
        if *self.logged_in() || *self.login_in_progress() {
            return;
        }

        self.as_mut().set_login_in_progress(true);
        self.as_mut().set_qr_url(QString::from(""));
        self.as_mut().set_qr_svg_data_url(QString::from(""));
        self.as_mut().set_status_message(QString::from("Starting QR login..."));

        let qt_thread: CxxQtThread<ffi::AppState> = self.qt_thread();

        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let message = format!("Login failed: {error}");
                    let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::AppState>| {
                        qobject
                            .as_mut()
                            .set_status_message(QString::from(&message));
                        qobject.as_mut().set_login_in_progress(false);
                    });
                    return;
                }
            };

            let ui_thread = qt_thread.clone();

            let result: Result<bool, String> = runtime.block_on(async move {
                let store = AuthDataStore::default_store();

                loop {
                    let auth = AuthData::new().map_err(|error| error.to_string())?;
                    let client = GMClient::new(auth);
                    let _ = client.fetch_config().await;

                    let (qr_url, stream) = client
                        .start_qr_pairing_stream()
                        .await
                        .map_err(|error| error.to_string())?;

                    let qr_url_string = qr_url.to_string();
                    let svg_data_url = qr_to_svg_data_url(&qr_url_string)
                        .map_err(|error| error.to_string())?;

                    let _ = ui_thread.queue(move |mut qobject: Pin<&mut ffi::AppState>| {
                        qobject.as_mut().set_qr_url(QString::from(&qr_url_string));
                        qobject
                            .as_mut()
                            .set_qr_svg_data_url(QString::from(&svg_data_url));
                        qobject
                            .as_mut()
                            .set_status_message(QString::from("Scan the QR code"));
                    });

                    let paired = client
                        .wait_for_qr_pairing_on_stream(stream, Duration::from_secs(20))
                        .await
                        .map_err(|error| error.to_string())?;

                    if let Some(_) = paired {
                        let auth_handle = client.auth();
                        let auth = auth_handle.lock().await;
                        store
                            .save(&auth)
                            .map_err(|error| error.to_string())?;
                        return Ok(true);
                    }
                }
            });

            match result {
                Ok(true) => {
                    let _ = qt_thread.queue(|mut qobject: Pin<&mut ffi::AppState>| {
                        qobject.as_mut().set_logged_in(true);
                        qobject.as_mut().set_login_in_progress(false);
                        qobject
                            .as_mut()
                            .set_status_message(QString::from("Logged in"));
                    });
                }
                Ok(false) => {
                    let _ = qt_thread.queue(|mut qobject: Pin<&mut ffi::AppState>| {
                        qobject.as_mut().set_login_in_progress(false);
                        qobject
                            .as_mut()
                            .set_status_message(QString::from("Pairing ended"));
                    });
                }
                Err(error) => {
                    let message = format!("Login failed: {error}");
                    let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::AppState>| {
                        qobject.as_mut().set_login_in_progress(false);
                        qobject
                            .as_mut()
                            .set_status_message(QString::from(&message));
                    });
                }
            }
        });
    }

    pub fn initialize(self: Pin<&mut Self>) {
        let qt_thread: CxxQtThread<ffi::AppState> = self.qt_thread();

        std::thread::spawn(move || {
            let store = AuthDataStore::default_store();
            let result = store.load();

            match result {
                Ok(Some(_auth)) => {
                    let _ = qt_thread.queue(|mut qobject: Pin<&mut ffi::AppState>| {
                        qobject.as_mut().set_logged_in(true);
                        qobject.as_mut().set_login_in_progress(false);
                        qobject.as_mut().set_status_message(QString::from("Logged in"));
                    });
                }
                Ok(None) => {
                    let _ = qt_thread.queue(|mut qobject: Pin<&mut ffi::AppState>| {
                        qobject.as_mut().set_logged_in(false);
                        qobject.as_mut().set_login_in_progress(false);
                        qobject
                            .as_mut()
                            .set_status_message(QString::from("Not logged in"));
                    });
                }
                Err(error) => {
                    let message = format!("Auth load failed: {error}");
                    let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::AppState>| {
                        qobject.as_mut().set_logged_in(false);
                        qobject.as_mut().set_login_in_progress(false);
                        qobject.as_mut().set_status_message(QString::from(&message));
                    });
                }
            }
        });
    }
}

impl crate::ffi::SessionController {
    pub fn start(mut self: Pin<&mut Self>) {
        if *self.running() {
            return;
        }

        self.as_mut().set_running(true);
        self.as_mut().set_status(QString::from("Starting session..."));
        self.rust()
            .should_stop
            .store(false, std::sync::atomic::Ordering::SeqCst);

        let qt_thread: CxxQtThread<ffi::SessionController> = self.qt_thread();
        let stop_flag = self.rust().should_stop.clone();

        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let message = format!("Session failed: {error}");
                    let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::SessionController>| {
                        qobject.as_mut().set_status(QString::from(&message));
                        qobject.as_mut().set_running(false);
                    });
                    return;
                }
            };

            let result: Result<(), String> = runtime.block_on(async move {
                let store = AuthDataStore::default_store();
                let auth = store
                    .load()
                    .map_err(|error| error.to_string())?
                    .unwrap_or(AuthData::new().map_err(|error| error.to_string())?);

                let client = GMClient::new(auth);
                let mut handler = libgmessages_rs::gmclient::SessionHandler::new(client.clone());
                let auth_handle = client.auth();
                let auth_session = {
                    let auth = auth_handle.lock().await;
                    auth.session_id().to_string().to_lowercase()
                };
                handler.set_session_id(auth_session).await;

                let _ = client
                    .send_rpc_message_with_id_and_session_no_payload(
                        libgmessages_rs::proto::rpc::ActionType::GetUpdates,
                        libgmessages_rs::proto::rpc::MessageType::BugleMessage,
                        handler.session_id(),
                        handler.session_id(),
                        true,
                    )
                    .await;

                tokio::select! {
                    result = handler.start_response_loop() => {
                        result.map_err(|error| error.to_string())
                    }
                    _ = async {
                        while !stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            tokio::time::sleep(Duration::from_millis(250)).await;
                        }
                    } => {
                        Ok(())
                    }
                }
            });

            match result {
                Ok(()) => {
                    let _ = qt_thread.queue(|mut qobject: Pin<&mut ffi::SessionController>| {
                        qobject.as_mut().set_status(QString::from("Session ended"));
                        qobject.as_mut().set_running(false);
                    });
                }
                Err(error) => {
                    let message = format!("Session failed: {error}");
                    let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::SessionController>| {
                        qobject.as_mut().set_status(QString::from(&message));
                        qobject.as_mut().set_running(false);
                    });
                }
            }
        });
    }

    pub fn stop(mut self: Pin<&mut Self>) {
        if !*self.running() {
            return;
        }
        self.rust()
            .should_stop
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.as_mut().set_running(false);
        self.as_mut().set_status(QString::from("Stopping..."));
    }
}

impl crate::ffi::ConversationList {
    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.filtered_items.len() as i32
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let row = index.row() as usize;
        if row >= self.filtered_items.len() {
            return QVariant::default();
        }

        let item = &self.filtered_items[row];
        match role {
            0 => QVariant::from(&item.name),
            1 => QVariant::from(&item.preview),
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> QHash_i32_QByteArray {
        let mut roles = QHash_i32_QByteArray::default();
        roles.insert(0, "name".into());
        roles.insert(1, "preview".into());
        roles
    }

    pub fn load(mut self: Pin<&mut Self>) {
        self.as_mut().set_loading(true);
        let qt_thread: CxxQtThread<ffi::ConversationList> = self.qt_thread();

        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("conversation load failed: {error}");
                    return;
                }
            };

            let result: Result<Vec<ConversationItem>, String> = runtime.block_on(async move {
                let store = AuthDataStore::default_store();
                let auth = store
                    .load()
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| "not logged in".to_string())?;

                let client = GMClient::new(auth);
                let mut handler = libgmessages_rs::gmclient::SessionHandler::new(client.clone());
                let auth_handle = client.auth();
                let auth_session = {
                    let auth = auth_handle.lock().await;
                    auth.session_id().to_string().to_lowercase()
                };
                handler.set_session_id(auth_session).await;

                let active_session_id = handler.session_id().to_string();
                let loop_handler = handler.clone();
                let task = tokio::spawn(async move {
                    let _ = loop_handler.start_response_loop().await;
                });

                tokio::time::sleep(Duration::from_millis(500)).await;
                let _ = handler
                    .client()
                    .send_rpc_message_with_id_and_session_no_payload(
                        libgmessages_rs::proto::rpc::ActionType::GetUpdates,
                        libgmessages_rs::proto::rpc::MessageType::BugleMessage,
                        &active_session_id,
                        &active_session_id,
                        true,
                    )
                    .await;

                let request = libgmessages_rs::proto::client::ListConversationsRequest {
                    count: 40,
                    folder: libgmessages_rs::proto::client::list_conversations_request::Folder::Inbox as i32,
                    cursor: None,
                };
                let response: libgmessages_rs::proto::client::ListConversationsResponse = handler
                    .send_request(
                        libgmessages_rs::proto::rpc::ActionType::ListConversations,
                        libgmessages_rs::proto::rpc::MessageType::BugleMessage,
                        &request,
                    )
                    .await
                    .map_err(|error| error.to_string())?;

                task.abort();

                let items = response
                    .conversations
                    .into_iter()
                    .map(|convo| ConversationItem {
                        name: QString::from(convo.name),
                        preview: QString::from(""),
                    })
                    .collect();

                Ok(items)
            });

            match result {
                Ok(items) => {
                    let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::ConversationList>| {
                        qobject.as_mut().begin_reset_model();
                        let mut rust = qobject.as_mut().rust_mut();
                        rust.all_items = items;
                        rust.filtered_items = filter_items(&rust.all_items, &rust.filter_text);
                        qobject.as_mut().set_loading(false);
                        qobject.as_mut().end_reset_model();
                    });
                }
                Err(error) => {
                    eprintln!("conversation load failed: {error}");
                    let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::ConversationList>| {
                        qobject.as_mut().set_loading(false);
                    });
                }
            }
        });
    }

    pub fn apply_filter(mut self: Pin<&mut Self>, filter: &QString) {
        let filter_text = filter.to_string();
        self.as_mut().begin_reset_model();
        let mut rust = self.as_mut().rust_mut();
        rust.filter_text = filter_text;
        rust.filtered_items = filter_items(&rust.all_items, &rust.filter_text);
        self.as_mut().end_reset_model();
    }
}

fn filter_items(items: &[ConversationItem], filter_text: &str) -> Vec<ConversationItem> {
    let needle = filter_text.trim().to_lowercase();
    if needle.is_empty() {
        return items.to_vec();
    }

    items
        .iter()
        .filter(|item| {
            let name = item.name.to_string().to_lowercase();
            let preview = item.preview.to_string().to_lowercase();
            name.contains(&needle) || preview.contains(&needle)
        })
        .cloned()
        .collect()
}
