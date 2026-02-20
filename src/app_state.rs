use crate::ffi;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::{Datelike, Local, Timelike};
use core::pin::Pin;
use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::QString;
use futures_util::StreamExt;
use libgmessages_rs::{auth::AuthData, gmclient::GMClient, store::AuthDataStore};
use qrcode::render::svg;
use qrcode::QrCode;
use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc, OnceLock};
use std::time::Duration;
use uuid::Uuid;

use crate::ffi::QHash_i32_QByteArray;
use crate::ffi::QModelIndex;
use crate::ffi::QVariant;

// ── Shared session infrastructure ────────────────────────────────

/// A single shared tokio runtime + optional GMClient, used by all QObjects.
/// Created once at app startup and never destroyed.
struct SharedSession {
    runtime: tokio::runtime::Runtime,
    client: tokio::sync::RwLock<Option<GMClient>>,
}

/// Global singleton.
fn shared() -> &'static SharedSession {
    static INSTANCE: OnceLock<SharedSession> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime");
        SharedSession {
            runtime,
            client: tokio::sync::RwLock::new(None),
        }
    })
}

/// Spawn a future on the shared runtime (non-blocking).
fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    shared().runtime.spawn(future)
}

/// Get a clone of the current GMClient, or None if not logged in.
async fn get_client() -> Option<GMClient> {
    shared().client.read().await.clone()
}

/// Store a new GMClient after login.
async fn set_client(client: GMClient) {
    *shared().client.write().await = Some(client);
}

/// Clear the stored client (logout / auth error).
async fn clear_client() {
    *shared().client.write().await = None;
}

/// Helper: load auth from disk and create+store a GMClient.
/// Returns the client or an error string.
async fn ensure_client() -> Result<GMClient, String> {
    // Fast path: already have a client
    if let Some(c) = get_client().await {
        return Ok(c);
    }
    // Slow path: load from disk
    let store = AuthDataStore::default_store();
    let auth = store
        .load()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "not logged in".to_string())?;
    let client = GMClient::new(auth);
    set_client(client.clone()).await;
    Ok(client)
}

/// Create a SessionHandler from a client, set the session ID, and return it.
async fn make_handler(client: &GMClient) -> Result<libgmessages_rs::gmclient::SessionHandler, String> {
    let mut handler = libgmessages_rs::gmclient::SessionHandler::new(client.clone());
    let auth_handle = client.auth();
    let auth_session = {
        let auth = auth_handle.lock().await;
        auth.session_id().to_string().to_lowercase()
    };
    handler.set_session_id(auth_session).await;
    Ok(handler)
}

/// Run the response loop + GetUpdates handshake for a handler, returning
/// a JoinHandle that drives the response loop.  The caller should abort it
/// when done with the handler.
async fn start_handler_loop(
    handler: &libgmessages_rs::gmclient::SessionHandler,
) -> tokio::task::JoinHandle<()> {
    let session_id = handler.session_id().to_string();
    let _ = handler
        .client()
        .send_rpc_message_with_id_and_session_no_payload(
            libgmessages_rs::proto::rpc::ActionType::GetUpdates,
            libgmessages_rs::proto::rpc::MessageType::BugleMessage,
            &session_id,
            &session_id,
            true,
        )
        .await;

    let loop_handler = handler.clone();
    tokio::spawn(async move {
        let _ = loop_handler.start_response_loop().await;
    })
}

// ── QR code helper ───────────────────────────────────────────────

fn qr_to_svg_data_url(data: &str) -> Result<String, Box<dyn std::error::Error>> {
    let code = QrCode::new(data.as_bytes())?;
    let svg_str = code
        .render::<svg::Color>()
        .min_dimensions(256, 256)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();
    let encoded = STANDARD.encode(svg_str.as_bytes());
    Ok(format!("data:image/svg+xml;base64,{encoded}"))
}

// ── AppState ─────────────────────────────────────────────────────

pub struct AppStateRust {
    pub logged_in: bool,
    pub login_in_progress: bool,
    pub qr_url: QString,
    pub qr_svg_data_url: QString,
    pub status_message: QString,
    login_stop: Arc<AtomicBool>,
}

impl Default for AppStateRust {
    fn default() -> Self {
        Self {
            logged_in: false,
            login_in_progress: false,
            qr_url: QString::from(""),
            qr_svg_data_url: QString::from(""),
            status_message: QString::from("Not logged in"),
            login_stop: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl crate::ffi::AppState {
    pub fn start_login(mut self: Pin<&mut Self>) {
        if *self.logged_in() || *self.login_in_progress() {
            return;
        }

        self.as_mut().set_login_in_progress(true);
        self.as_mut().set_qr_url(QString::from(""));
        self.as_mut().set_qr_svg_data_url(QString::from(""));
        self.as_mut()
            .set_status_message(QString::from("Starting QR login..."));

        // Reset the stop flag
        self.rust()
            .login_stop
            .store(false, std::sync::atomic::Ordering::SeqCst);
        let stop_flag = self.rust().login_stop.clone();

        let qt_thread: CxxQtThread<ffi::AppState> = self.qt_thread();
        let ui_thread = qt_thread.clone();

        spawn(async move {
            let store = AuthDataStore::default_store();

            let result: Result<bool, String> = async {
                loop {
                    if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                        return Ok(false);
                    }

                    let auth = AuthData::new().map_err(|e| e.to_string())?;
                    let client = GMClient::new(auth);
                    let _ = client.fetch_config().await;

                    let (qr_url, stream) = match client.start_qr_pairing_stream().await {
                        Ok(res) => res,
                        Err(e) => {
                            eprintln!("Failed to start QR pairing stream: {}", e);
                            tokio::time::sleep(Duration::from_secs(2)).await;
                            continue;
                        }
                    };

                    let qr_url_string = qr_url.to_string();
                    let svg_data_url =
                        qr_to_svg_data_url(&qr_url_string).map_err(|e| e.to_string())?;

                    let _ = ui_thread.queue(move |mut qobject: Pin<&mut ffi::AppState>| {
                        qobject.as_mut().set_qr_url(QString::from(&qr_url_string));
                        qobject
                            .as_mut()
                            .set_qr_svg_data_url(QString::from(&svg_data_url));
                        qobject
                            .as_mut()
                            .set_status_message(QString::from("Scan the QR code"));
                    });

                    let paired = tokio::time::timeout(
                        Duration::from_secs(20),
                        client.wait_for_qr_pairing_on_stream(stream, Duration::from_secs(20)),
                    )
                    .await;

                    match paired {
                        Ok(Ok(Some(_))) => {
                            // Save auth to disk
                            let auth_handle = client.auth();
                            let auth = auth_handle.lock().await;
                            store.save(&auth).map_err(|e| e.to_string())?;
                            drop(auth);
                            // Store client in shared session
                            set_client(client).await;
                            return Ok(true);
                        }
                        Ok(Ok(None)) => continue,
                        Ok(Err(e)) => {
                            eprintln!("QR pairing stream error: {}", e);
                            tokio::time::sleep(Duration::from_secs(2)).await;
                            continue;
                        }
                        Err(_) => continue, // timeout → refresh QR
                    }
                }
            }
            .await;

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
                            .set_status_message(QString::from("Login cancelled"));
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

    pub fn cancel_login(self: Pin<&mut Self>) {
        self.rust()
            .login_stop
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn initialize(self: Pin<&mut Self>) {
        let qt_thread: CxxQtThread<ffi::AppState> = self.qt_thread();

        spawn(async move {
            let store = AuthDataStore::default_store();
            let result = store.load();

            match result {
                Ok(Some(auth)) => {
                    // Pre-populate the shared client
                    let client = GMClient::new(auth);
                    set_client(client).await;
                    let _ = qt_thread.queue(|mut qobject: Pin<&mut ffi::AppState>| {
                        qobject.as_mut().set_logged_in(true);
                        qobject.as_mut().set_login_in_progress(false);
                        qobject
                            .as_mut()
                            .set_status_message(QString::from("Logged in"));
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
                        qobject
                            .as_mut()
                            .set_status_message(QString::from(&message));
                    });
                }
            }
        });
    }

    pub fn logout(self: Pin<&mut Self>, reason: &QString) {
        let message = reason.to_string();
        let qt_thread: CxxQtThread<ffi::AppState> = self.qt_thread();

        spawn(async move {
            clear_client().await;
            let store = AuthDataStore::default_store();
            if let Err(e) = store.delete() {
                eprintln!("auth delete failed: {e}");
            }
            let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::AppState>| {
                qobject.as_mut().set_logged_in(false);
                qobject.as_mut().set_login_in_progress(false);
                if message.is_empty() {
                    qobject
                        .as_mut()
                        .set_status_message(QString::from("Not logged in"));
                } else {
                    qobject
                        .as_mut()
                        .set_status_message(QString::from(message.as_str()));
                }
            });
        });
    }
}

// ── SessionController ────────────────────────────────────────────

pub struct SessionControllerRust {
    pub running: bool,
    pub status: QString,
    should_stop: Arc<AtomicBool>,
}

impl Default for SessionControllerRust {
    fn default() -> Self {
        Self {
            running: false,
            status: QString::from("Idle"),
            should_stop: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl crate::ffi::SessionController {
    pub fn start(mut self: Pin<&mut Self>) {
        if *self.running() {
            return;
        }

        self.as_mut().set_running(true);
        self.as_mut()
            .set_status(QString::from("Starting session..."));
        self.rust()
            .should_stop
            .store(false, std::sync::atomic::Ordering::SeqCst);

        let qt_thread: CxxQtThread<ffi::SessionController> = self.qt_thread();
        let stop_flag = self.rust().should_stop.clone();

        let session_thread = qt_thread.clone();

        spawn(async move {
            let result: Result<(), String> = async {
                let client = ensure_client().await?;
                let mut handler = make_handler(&client).await?;

                // Outer loop: reconnect on stream drop
                loop {
                    if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }

                    let stream = client
                        .start_long_poll_stream()
                        .await
                        .map_err(|e| e.to_string())?;

                    let session_id = handler.session_id().to_string();
                    let _ = client
                        .send_rpc_message_with_id_and_session_no_payload(
                            libgmessages_rs::proto::rpc::ActionType::GetUpdates,
                            libgmessages_rs::proto::rpc::MessageType::BugleMessage,
                            &session_id,
                            &session_id,
                            true,
                        )
                        .await;

                    let inner_result = run_long_poll_loop(
                        stream,
                        &handler,
                        &stop_flag,
                        &session_thread,
                    )
                    .await;

                    match inner_result {
                        Ok(StreamEndReason::Stopped) => break,
                        Ok(StreamEndReason::StreamEnded) => {
                            // Reconnect after a short delay
                            let _ = session_thread.queue(
                                |mut qobject: Pin<&mut ffi::SessionController>| {
                                    qobject
                                        .as_mut()
                                        .set_status(QString::from("Reconnecting..."));
                                },
                            );
                            tokio::time::sleep(Duration::from_secs(2)).await;
                            // Refresh session ID for reconnection
                            handler.reset_session_id();
                            let auth_handle = client.auth();
                            let auth_session = {
                                let auth = auth_handle.lock().await;
                                auth.session_id().to_string().to_lowercase()
                            };
                            handler.set_session_id(auth_session).await;
                            continue;
                        }
                        Err(e) => return Err(e),
                    }
                }
                Ok(())
            }
            .await;

            match result {
                Ok(()) => {
                    let _ = qt_thread.queue(|mut qobject: Pin<&mut ffi::SessionController>| {
                        qobject
                            .as_mut()
                            .set_status(QString::from("Session ended"));
                        qobject.as_mut().set_running(false);
                    });
                }
                Err(error) => {
                    let message = format!("Session error: {error}");
                    let _ =
                        qt_thread.queue(move |mut qobject: Pin<&mut ffi::SessionController>| {
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

/// Why the long-poll inner loop ended.
enum StreamEndReason {
    Stopped,
    StreamEnded,
}

/// The inner long-poll processing loop.  Returns when the stream ends or stop
/// is requested.
async fn run_long_poll_loop(
    stream: impl futures_util::Stream<
        Item = Result<
            libgmessages_rs::proto::rpc::LongPollingPayload,
            libgmessages_rs::gmclient::GMClientError,
        >,
    >,
    handler: &libgmessages_rs::gmclient::SessionHandler,
    stop_flag: &Arc<AtomicBool>,
    qt_thread: &CxxQtThread<ffi::SessionController>,
) -> Result<StreamEndReason, String> {
    let mut stream = std::pin::pin!(stream);
    let mut skip_count: i32 = 0;

    // Also spawn the periodic GetUpdates heartbeat
    let heartbeat_stop = stop_flag.clone();
    let heartbeat_handler = handler.clone();
    let heartbeat = tokio::spawn(async move {
        while !heartbeat_stop.load(std::sync::atomic::Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let session_id = heartbeat_handler.session_id().to_string();
            let _ = heartbeat_handler
                .client()
                .send_rpc_message_with_id_and_session_no_payload(
                    libgmessages_rs::proto::rpc::ActionType::GetUpdates,
                    libgmessages_rs::proto::rpc::MessageType::BugleMessage,
                    &session_id,
                    &session_id,
                    true,
                )
                .await;
        }
    });

    let result = loop {
        if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
            break Ok(StreamEndReason::Stopped);
        }

        let item = stream.next().await;
        let Some(item) = item else {
            break Ok(StreamEndReason::StreamEnded);
        };
        let payload = match item {
            Ok(p) => p,
            Err(e) => {
                eprintln!("long-poll stream error: {e}");
                break Ok(StreamEndReason::StreamEnded);
            }
        };

        if let Some(ack) = payload.ack.as_ref() {
            if let Some(count) = ack.count {
                skip_count = count;
            }
        }
        let Some(data) = payload.data.as_ref() else {
            continue;
        };
        if skip_count > 0 {
            skip_count -= 1;
            continue;
        }
        if data.bugle_route
            != libgmessages_rs::proto::rpc::BugleRoute::DataEvent as i32
        {
            continue;
        }

        let updates = handler
            .client()
            .decode_update_events_from_message(data)
            .await;
        let updates = match updates {
            Ok(Some(u)) => u,
            Ok(None) => continue,
            Err(e) => {
                eprintln!("decode_update_events error: {e}");
                continue;
            }
        };

        let Some(event) = updates.event else { continue };

        match event {
            libgmessages_rs::proto::events::update_events::Event::MessageEvent(
                message_event,
            ) => {
                for message in message_event.data {
                    let body = extract_message_body(&message);

                    let conversation_id = message.conversation_id.clone();
                    let participant_id = message.participant_id.clone();
                    let transport_type = message.r#type;
                    let message_id = extract_message_id(&message);
                    let tmp_id = message.tmp_id.clone();
                    let status_code = message
                        .message_status
                        .as_ref()
                        .map(|s| s.status)
                        .unwrap_or(0);
                    let timestamp_micros = message.timestamp;

                    let _ = qt_thread.queue(
                        move |mut qobject: Pin<&mut ffi::SessionController>| {
                            qobject.as_mut().message_received(
                                &QString::from(conversation_id.as_str()),
                                &QString::from(participant_id.as_str()),
                                &QString::from(body.as_str()),
                                transport_type,
                                &QString::from(message_id.as_str()),
                                &QString::from(tmp_id.as_str()),
                                timestamp_micros,
                                status_code,
                            );
                        },
                    );

                    if !message.message_id.is_empty() {
                        let client = handler.client();
                        let ack_id = message.message_id.clone();
                        tokio::spawn(async move {
                            let _ = client.ack_messages(vec![ack_id]).await;
                        });
                    }
                }
            }
            libgmessages_rs::proto::events::update_events::Event::ConversationEvent(
                convo_event,
            ) => {
                for convo in convo_event.data {
                    let conversation_id = convo.conversation_id.clone();
                    let name = convo.name.clone();
                    let unread = convo.unread;
                    let last_message_timestamp = convo.last_message_timestamp;
                    let preview = build_preview(&convo);
                    let is_group_chat = convo.is_group_chat;

                    let _ = qt_thread.queue(
                        move |mut qobject: Pin<&mut ffi::SessionController>| {
                            qobject.as_mut().conversation_updated(
                                &QString::from(conversation_id.as_str()),
                                &QString::from(name.as_str()),
                                &QString::from(preview.as_str()),
                                unread,
                                last_message_timestamp,
                                is_group_chat,
                            );
                        },
                    );
                }
            }
            _ => {} // Ignore typing events, settings, etc. for now
        }
    };

    heartbeat.abort();
    result
}

/// Extract text body from a Message, returning empty string if none.
fn extract_message_body(message: &libgmessages_rs::proto::conversations::Message) -> String {
    message
        .message_info
        .iter()
        .find_map(|info| match &info.data {
            Some(
                libgmessages_rs::proto::conversations::message_info::Data::MessageContent(
                    content,
                ),
            ) => {
                let text = content.content.trim();
                if text.is_empty() {
                    None
                } else {
                    Some(text.to_string())
                }
            }
            _ => None,
        })
        .unwrap_or_default()
}

/// Extract the best message ID from a Message.
fn extract_message_id(message: &libgmessages_rs::proto::conversations::Message) -> String {
    if !message.message_id.is_empty() {
        message.message_id.clone()
    } else if !message.tmp_id.is_empty() {
        message.tmp_id.clone()
    } else {
        message
            .another_message_id
            .as_ref()
            .map(|id| id.message_id.clone())
            .unwrap_or_default()
    }
}

/// Extract media information from a Message, if present.
fn extract_message_media(
    message: &libgmessages_rs::proto::conversations::Message,
) -> Option<(String, Vec<u8>, String)> {
    message
        .message_info
        .iter()
        .find_map(|info| match &info.data {
            Some(libgmessages_rs::proto::conversations::message_info::Data::MediaContent(
                media,
            )) => {
                let id = if !media.thumbnail_media_id.is_empty() {
                    media.thumbnail_media_id.clone()
                } else {
                    media.media_id.clone()
                };
                let key = if !media.thumbnail_decryption_key.is_empty() {
                    media.thumbnail_decryption_key.clone()
                } else {
                    media.decryption_key.clone()
                };
                Some((id, key, media.mime_type.clone()))
            }
            _ => None,
        })
}

// ── ConversationList ─────────────────────────────────────────────

#[derive(Clone)]
pub struct ConversationItem {
    name: QString,
    preview: QString,
    avatar_url: QString,
    avatar_identifier: String,
    is_group_chat: bool,
    unread: bool,
    conversation_id: String,
    me_participant_id: String,
    last_message_timestamp: i64,
    last_message_time: QString,
}

pub struct ConversationListRust {
    all_items: Vec<ConversationItem>,
    filtered_items: Vec<ConversationItem>,
    filter_text: String,
    pub loading: bool,
    avatar_by_identifier: HashMap<String, String>,
}

impl Default for ConversationListRust {
    fn default() -> Self {
        Self {
            all_items: Vec::new(),
            filtered_items: Vec::new(),
            filter_text: String::new(),
            loading: false,
            avatar_by_identifier: HashMap::new(),
        }
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
            2 => QVariant::from(&item.last_message_time),
            3 => QVariant::from(&item.avatar_url),
            4 => QVariant::from(&item.is_group_chat),
            5 => QVariant::from(&item.unread),
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> QHash_i32_QByteArray {
        let mut roles = QHash_i32_QByteArray::default();
        roles.insert(0, "name".into());
        roles.insert(1, "preview".into());
        roles.insert(2, "time".into());
        roles.insert(3, "avatar_url".into());
        roles.insert(4, "is_group_chat".into());
        roles.insert(5, "unread".into());
        roles
    }

    pub fn load(mut self: Pin<&mut Self>) {
        self.as_mut().set_loading(true);
        let qt_thread: CxxQtThread<ffi::ConversationList> = self.qt_thread();
        let ui_thread = qt_thread.clone();

        spawn(async move {
            let result: Result<(), String> = async {
                let client = ensure_client().await?;
                let handler = make_handler(&client).await?;
                let response_loop = start_handler_loop(&handler).await;

                let request =
                    libgmessages_rs::proto::client::ListConversationsRequest {
                        count: 40,
                        folder:
                            libgmessages_rs::proto::client::list_conversations_request::Folder::Inbox
                                as i32,
                        cursor: None,
                    };
                let response: libgmessages_rs::proto::client::ListConversationsResponse =
                    handler
                        .send_request(
                            libgmessages_rs::proto::rpc::ActionType::ListConversations,
                            libgmessages_rs::proto::rpc::MessageType::BugleMessage,
                            &request,
                        )
                        .await
                        .map_err(|e| e.to_string())?;

                let mut items: Vec<ConversationItem> = response
                    .conversations
                    .into_iter()
                    .map(|convo| conversation_to_item(&convo))
                    .collect();

                items.sort_by(|a, b| b.last_message_timestamp.cmp(&a.last_message_timestamp));

                // Collect avatar identifiers
                let mut avatar_identifiers: Vec<String> = Vec::new();
                let mut avatar_by_identifier: HashMap<String, String> = HashMap::new();
                for item in &items {
                    if item.avatar_identifier.is_empty() {
                        continue;
                    }
                    if avatar_by_identifier.contains_key(&item.avatar_identifier) {
                        continue;
                    }
                    avatar_identifiers.push(item.avatar_identifier.clone());
                    avatar_by_identifier
                        .insert(item.avatar_identifier.clone(), String::new());
                }

                // Push items to UI immediately
                let _ = ui_thread.queue(move |mut qobject: Pin<&mut ffi::ConversationList>| {
                    qobject.as_mut().begin_reset_model();
                    let mut rust = qobject.as_mut().rust_mut();
                    rust.avatar_by_identifier.clear();
                    rust.all_items = items;
                    rust.filtered_items =
                        filter_items(&rust.all_items, &rust.filter_text);
                    qobject.as_mut().set_loading(false);
                    qobject.as_mut().end_reset_model();
                });

                // Fetch avatars
                if !avatar_identifiers.is_empty() {
                    let request = libgmessages_rs::proto::client::GetThumbnailRequest {
                        identifiers: avatar_identifiers.clone(),
                    };
                    let attempts = [
                        (
                            true,
                            libgmessages_rs::proto::rpc::MessageType::BugleAnnotation,
                        ),
                        (
                            true,
                            libgmessages_rs::proto::rpc::MessageType::BugleMessage,
                        ),
                        (
                            false,
                            libgmessages_rs::proto::rpc::MessageType::BugleAnnotation,
                        ),
                        (
                            false,
                            libgmessages_rs::proto::rpc::MessageType::BugleMessage,
                        ),
                        (
                            false,
                            libgmessages_rs::proto::rpc::MessageType::UnknownMessageType,
                        ),
                    ];
                    for (encrypted, message_type) in attempts {
                        let attempt: Result<
                            libgmessages_rs::proto::client::GetThumbnailResponse,
                            _,
                        > = if encrypted {
                            handler
                                .send_request(
                                    libgmessages_rs::proto::rpc::ActionType::GetContactsThumbnail,
                                    message_type,
                                    &request,
                                )
                                .await
                        } else {
                            handler
                                .send_request_dont_encrypt(
                                    libgmessages_rs::proto::rpc::ActionType::GetContactsThumbnail,
                                    message_type,
                                    &request,
                                    Duration::from_secs(5),
                                )
                                .await
                        };
                        if let Ok(response) = attempt {
                            if response.thumbnail.is_empty() {
                                continue;
                            }
                            for thumb in response.thumbnail {
                                if let Some(data) = thumb.data.as_ref() {
                                    if data.image_buffer.is_empty() {
                                        continue;
                                    }
                                    let ext = detect_extension(&data.image_buffer);
                                    let encoded = STANDARD.encode(&data.image_buffer);
                                    let url = format!("data:image/{ext};base64,{encoded}");
                                    avatar_by_identifier.insert(thumb.identifier, url);
                                }
                            }
                            break;
                        }
                    }
                }

                if !avatar_by_identifier.is_empty() {
                    let _ = ui_thread.queue(
                        move |mut qobject: Pin<&mut ffi::ConversationList>| {
                            qobject.as_mut().begin_reset_model();
                            let mut rust = qobject.as_mut().rust_mut();
                            rust.avatar_by_identifier = avatar_by_identifier.clone();
                            for item in &mut rust.all_items {
                                if let Some(url) =
                                    avatar_by_identifier.get(&item.avatar_identifier)
                                {
                                    if !url.is_empty() {
                                        item.avatar_url = QString::from(url.as_str());
                                    }
                                }
                            }
                            rust.filtered_items =
                                filter_items(&rust.all_items, &rust.filter_text);
                            qobject.as_mut().end_reset_model();
                        },
                    );
                }

                response_loop.abort();
                Ok(())
            }
            .await;

            if let Err(error) = result {
                let is_auth_error =
                    error.contains("authentication credential") || error.contains("401") || error.contains("403");
                eprintln!("conversation load failed: {error}");
                let _ = qt_thread.queue(
                    move |mut qobject: Pin<&mut ffi::ConversationList>| {
                        qobject.as_mut().rust_mut().avatar_by_identifier.clear();
                        qobject.as_mut().set_loading(false);
                        if is_auth_error {
                            qobject
                                .as_mut()
                                .auth_error(&QString::from(error.as_str()));
                        }
                    },
                );
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

    pub fn conversation_id(&self, row: i32) -> QString {
        let index = row.max(0) as usize;
        if index >= self.filtered_items.len() {
            return QString::from("");
        }
        QString::from(self.filtered_items[index].conversation_id.as_str())
    }

    pub fn me_participant_id(&self, row: i32) -> QString {
        let index = row.max(0) as usize;
        if index >= self.filtered_items.len() {
            return QString::from("");
        }
        QString::from(self.filtered_items[index].me_participant_id.as_str())
    }

    /// Called from QML when the session controller emits conversation_updated.
    /// Updates an existing conversation in-place using dataChanged, or inserts it.
    pub fn handle_conversation_event(
        mut self: Pin<&mut Self>,
        conversation_id: &QString,
        name: &QString,
        preview: &QString,
        unread: bool,
        last_message_timestamp: i64,
        is_group_chat: bool,
    ) {
        let convo_id = conversation_id.to_string();
        let name_str = name.to_string();
        let preview_str = preview.to_string();
        let time_str = format_human_timestamp(last_message_timestamp);

        // Find in all_items
        if let Some(pos) = self
            .rust()
            .all_items
            .iter()
            .position(|item| item.conversation_id == convo_id)
        {
            // Update in-place
            let mut rust = self.as_mut().rust_mut();
            let item = &mut rust.all_items[pos];
            item.name = QString::from(name_str.as_str());
            item.preview = QString::from(preview_str.as_str());
            item.unread = unread;
            item.last_message_timestamp = last_message_timestamp;
            item.last_message_time = QString::from(time_str.as_str());
            item.is_group_chat = is_group_chat;

            // Re-sort all_items by timestamp
            rust.all_items
                .sort_by(|a, b| b.last_message_timestamp.cmp(&a.last_message_timestamp));

            // Rebuild filtered list and emit full reset (sorting changed positions)
            rust.filtered_items = filter_items(&rust.all_items, &rust.filter_text);
            // We need to use begin_reset_model since sort may change positions
            drop(rust);
            // Actually we need to call begin/end on self
            self.as_mut().begin_reset_model();
            self.as_mut().end_reset_model();
        } else {
            // New conversation — insert it
            let new_item = ConversationItem {
                name: QString::from(name_str.as_str()),
                preview: QString::from(preview_str.as_str()),
                avatar_url: QString::from(""),
                avatar_identifier: String::new(),
                is_group_chat,
                unread,
                conversation_id: convo_id,
                me_participant_id: String::new(),
                last_message_timestamp,
                last_message_time: QString::from(time_str.as_str()),
            };
            self.as_mut().begin_reset_model();
            let mut rust = self.as_mut().rust_mut();
            rust.all_items.push(new_item);
            rust.all_items
                .sort_by(|a, b| b.last_message_timestamp.cmp(&a.last_message_timestamp));
            rust.filtered_items = filter_items(&rust.all_items, &rust.filter_text);
            self.as_mut().end_reset_model();
        }
    }

    pub fn mark_conversation_read(mut self: Pin<&mut Self>, conversation_id: &QString) {
        let convo_id = conversation_id.to_string();
        if let Some(pos) = self.rust().all_items.iter().position(|item| item.conversation_id == convo_id) {
            let mut rust = self.as_mut().rust_mut();
            if rust.all_items[pos].unread {
                rust.all_items[pos].unread = false;
                rust.filtered_items = filter_items(&rust.all_items, &rust.filter_text);
                drop(rust);
                self.as_mut().begin_reset_model();
                self.as_mut().end_reset_model();
            }
        }
    }
}

/// Convert a proto Conversation to a ConversationItem.
fn conversation_to_item(
    convo: &libgmessages_rs::proto::conversations::Conversation,
) -> ConversationItem {
    let preview = QString::from(build_preview(convo));
    let name = QString::from(convo.name.as_str());
    let conversation_id = convo.conversation_id.clone();
    let is_group_chat = convo.is_group_chat;
    let unread = convo.unread;
    let me_participant_id = convo
        .participants
        .iter()
        .find_map(|p| {
            if p.is_me {
                p.id.as_ref().map(|id| id.participant_id.clone())
            } else {
                None
            }
        })
        .unwrap_or_default();
    let last_message_timestamp = convo.last_message_timestamp;
    let last_message_time = QString::from(format_human_timestamp(last_message_timestamp));
    let avatar_identifier = convo
        .participants
        .iter()
        .find_map(|p| {
            if p.is_me {
                None
            } else if !p.contact_id.is_empty() {
                Some(p.contact_id.clone())
            } else {
                p.id.as_ref().map(|id| id.participant_id.clone())
            }
        })
        .unwrap_or_default();

    ConversationItem {
        name,
        preview,
        avatar_url: QString::from(""),
        avatar_identifier,
        is_group_chat,
        unread,
        conversation_id,
        me_participant_id,
        last_message_timestamp,
        last_message_time,
    }
}

// ── MessageList ──────────────────────────────────────────────────

pub struct MessageItem {
    body: QString,
    from_me: bool,
    transport_type: i64,
    timestamp_micros: i64,
    message_id: String,
    status: QString,
    media_url: QString,
    is_media: bool,
}

pub struct MessageListRust {
    pub loading: bool,
    messages: Vec<MessageItem>,
    selected_conversation_id: String,
    me_participant_id: String,
}

impl Default for MessageListRust {
    fn default() -> Self {
        Self {
            loading: false,
            messages: Vec::new(),
            selected_conversation_id: String::new(),
            me_participant_id: String::new(),
        }
    }
}

impl crate::ffi::MessageList {
    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.messages.len() as i32
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let row = index.row() as usize;
        if row >= self.messages.len() {
            return QVariant::default();
        }

        let item = &self.messages[row];
        match role {
            0 => QVariant::from(&item.body),
            1 => QVariant::from(&item.from_me),
            2 => QVariant::from(&item.transport_type),
            3 => QVariant::from(&item.timestamp_micros),
            4 => QVariant::from(&QString::from(item.message_id.as_str())),
            5 => QVariant::from(&item.status),
            6 => QVariant::from(&QString::from(format_human_message_time(
                item.timestamp_micros,
            ))),
            7 => QVariant::from(&QString::from(format_section_date(
                item.timestamp_micros,
            ))),
            8 => QVariant::from(&item.media_url),
            9 => QVariant::from(&item.is_media),
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> QHash_i32_QByteArray {
        let mut roles = QHash_i32_QByteArray::default();
        roles.insert(0, "body".into());
        roles.insert(1, "from_me".into());
        roles.insert(2, "transport_type".into());
        roles.insert(3, "timestamp_micros".into());
        roles.insert(4, "message_id".into());
        roles.insert(5, "status".into());
        roles.insert(6, "time".into());
        roles.insert(7, "section_date".into());
        roles.insert(8, "media_url".into());
        roles.insert(9, "is_media".into());
        roles
    }

    pub fn load(mut self: Pin<&mut Self>, conversation_id: &QString) {
        let conversation_id = conversation_id.to_string();
        if conversation_id.is_empty() {
            return;
        }

        self.as_mut().begin_reset_model();
        let mut rust = self.as_mut().rust_mut();
        rust.messages.clear();
        rust.messages.shrink_to_fit();
        rust.selected_conversation_id = conversation_id.clone();
        rust.me_participant_id.clear();
        drop(rust);
        self.as_mut().end_reset_model();
        self.as_mut().set_loading(true);

        let qt_thread: CxxQtThread<ffi::MessageList> = self.qt_thread();

        spawn(async move {
            let result: Result<(Vec<MessageItem>, String, Vec<(String, String, Vec<u8>, String)>), String> = async {
                let client = ensure_client().await?;
                let handler = make_handler(&client).await?;
                let response_loop = start_handler_loop(&handler).await;

                let request = libgmessages_rs::proto::client::ListMessagesRequest {
                    conversation_id: conversation_id.clone(),
                    count: 50,
                    cursor: None,
                };
                let response: libgmessages_rs::proto::client::ListMessagesResponse =
                    handler
                        .send_request(
                            libgmessages_rs::proto::rpc::ActionType::ListMessages,
                            libgmessages_rs::proto::rpc::MessageType::BugleMessage,
                            &request,
                        )
                        .await
                        .map_err(|e| e.to_string())?;

                let convo_request =
                    libgmessages_rs::proto::client::GetConversationRequest {
                        conversation_id: conversation_id.clone(),
                    };
                let convo_response: libgmessages_rs::proto::client::GetConversationResponse =
                    handler
                        .send_request(
                            libgmessages_rs::proto::rpc::ActionType::GetConversation,
                            libgmessages_rs::proto::rpc::MessageType::BugleMessage,
                            &convo_request,
                        )
                        .await
                        .map_err(|e| e.to_string())?;

                let me_participant_id = convo_response
                    .conversation
                    .as_ref()
                    .and_then(|convo| {
                        convo.participants.iter().find_map(|p| {
                            if p.is_me {
                                p.id.as_ref().map(|id| id.participant_id.clone())
                            } else {
                                None
                            }
                        })
                    })
                    .unwrap_or_default();

                response_loop.abort();

                let mut media_downloads = Vec::new();
                let mut messages: Vec<MessageItem> = response
                    .messages
                    .into_iter()
                    .filter_map(|message| {
                        let body = extract_message_body(&message);
                        let media = extract_message_media(&message);
                        if body.is_empty() && media.is_none() {
                            return None;
                        }
                        let from_me = !me_participant_id.is_empty()
                            && message.participant_id == me_participant_id;
                        let status_code = message
                            .message_status
                            .as_ref()
                            .map(|s| s.status)
                            .unwrap_or(0);
                        let status = map_message_status(status_code, from_me);
                        let message_id = extract_message_id(&message);

                        let is_media = media.is_some();
                        if let Some((id, key, mime)) = media {
                            media_downloads.push((message_id.clone(), id, key, mime));
                        }

                        Some(MessageItem {
                            body: QString::from(body),
                            from_me,
                            transport_type: message.r#type,
                            timestamp_micros: message.timestamp,
                            message_id,
                            status: QString::from(status),
                            media_url: QString::from(""),
                            is_media,
                        })
                    })
                    .collect();

                messages.sort_by_key(|item| item.timestamp_micros);

                // Mark conversation as read
                if let Some(last_msg) = messages.last() {
                    if !last_msg.message_id.is_empty() {
                        let _ = client
                            .mark_message_read(
                                &conversation_id,
                                &last_msg.message_id,
                            )
                            .await;
                    }
                }

                Ok((messages, me_participant_id, media_downloads))
            }
            .await;

            match result {
                Ok((messages, me_participant_id, media_downloads)) => {
                    let _ = qt_thread.queue(
                        move |mut qobject: Pin<&mut ffi::MessageList>| {
                            qobject.as_mut().begin_reset_model();
                            let mut rust = qobject.as_mut().rust_mut();
                            rust.messages = messages;
                            rust.messages.shrink_to_fit();
                            rust.me_participant_id = me_participant_id;
                            qobject.as_mut().set_loading(false);
                            qobject.as_mut().end_reset_model();
                        },
                    );

                    // Start background download of media
                    if !media_downloads.is_empty() {
                        let ui_for_media = qt_thread.clone();
                        spawn(async move {
                            if let Some(client) = get_client().await {
                                for (msg_id, media_id, key, mime) in media_downloads {
                                    if let Ok(data) = client.download_media(&media_id, &key).await {
                                        let b64 = STANDARD.encode(&data);
                                        let uri = format!("data:{};base64,{}", mime, b64);
                                        let _ = ui_for_media.queue(move |mut qobject: Pin<&mut ffi::MessageList>| {
                                            let mut rust = qobject.as_mut().rust_mut();
                                            if let Some(pos) = rust.messages.iter().position(|m| m.message_id == msg_id) {
                                                rust.messages[pos].media_url = QString::from(uri.as_str());
                                                drop(rust);
                                                let model_index = qobject.as_ref().index(pos as i32, 0, &QModelIndex::default());
                                                qobject.as_mut().data_changed(&model_index, &model_index);
                                            }
                                        });
                                    }
                                }
                            }
                        });
                    }
                }
                Err(error) => {
                    let is_auth_error = error.contains("authentication credential")
                        || error.contains("401") || error.contains("403");
                    eprintln!("message load failed: {error}");
                    let _ = qt_thread.queue(
                        move |mut qobject: Pin<&mut ffi::MessageList>| {
                            qobject.as_mut().set_loading(false);
                            if is_auth_error {
                                qobject
                                    .as_mut()
                                    .auth_error(&QString::from(error.as_str()));
                            }
                        },
                    );
                }
            }
        });
    }

    pub fn send_message(mut self: Pin<&mut Self>, text: &QString) {
        let body = text.to_string().trim().to_string();
        if body.is_empty() {
            return;
        }

        let conversation_id = self.rust().selected_conversation_id.clone();
        if conversation_id.is_empty() {
            return;
        }

        let now_micros = chrono::Utc::now().timestamp_micros();
        let tmp_id = Uuid::new_v4().to_string().to_lowercase();

        // Optimistic insert
        self.as_mut().begin_reset_model();
        let mut rust = self.as_mut().rust_mut();
        rust.messages.push(MessageItem {
            body: QString::from(body.clone()),
            from_me: true,
            transport_type: 4,
            timestamp_micros: now_micros,
            message_id: tmp_id.clone(),
            status: QString::from("sending"),
            media_url: QString::from(""),
            is_media: false,
        });
        rust.messages.sort_by_key(|item| item.timestamp_micros);
        drop(rust);
        self.as_mut().end_reset_model();

        let qt_thread: CxxQtThread<ffi::MessageList> = self.qt_thread();
        let tmp_id_for_fail = tmp_id.clone();

        spawn(async move {
            let result: Result<(), String> = async {
                let client = ensure_client().await?;
                let handler = make_handler(&client).await?;
                let response_loop = start_handler_loop(&handler).await;

                let message_info = libgmessages_rs::proto::conversations::MessageInfo {
                    action_message_id: None,
                    data: Some(
                        libgmessages_rs::proto::conversations::message_info::Data::MessageContent(
                            libgmessages_rs::proto::conversations::MessageContent {
                                content: body.clone(),
                            },
                        ),
                    ),
                };
                let payload = libgmessages_rs::proto::client::MessagePayload {
                    tmp_id: tmp_id.clone(),
                    message_payload_content: Some(
                        libgmessages_rs::proto::client::MessagePayloadContent {
                            message_content: Some(
                                libgmessages_rs::proto::conversations::MessageContent {
                                    content: body.clone(),
                                },
                            ),
                        },
                    ),
                    conversation_id: conversation_id.clone(),
                    participant_id: String::new(),
                    message_info: vec![message_info],
                    tmp_id2: tmp_id.clone(),
                };
                let request = libgmessages_rs::proto::client::SendMessageRequest {
                    conversation_id: conversation_id.clone(),
                    message_payload: Some(payload),
                    sim_payload: None,
                    tmp_id,
                    force_rcs: false,
                    reply: None,
                };

                let _: libgmessages_rs::proto::client::SendMessageResponse = handler
                    .send_request(
                        libgmessages_rs::proto::rpc::ActionType::SendMessage,
                        libgmessages_rs::proto::rpc::MessageType::BugleMessage,
                        &request,
                    )
                    .await
                    .map_err(|e| e.to_string())?;

                response_loop.abort();
                Ok(())
            }
            .await;

            if let Err(error) = result {
                eprintln!("message send failed: {error}");
                let is_auth_error =
                    error.contains("authentication credential") || error.contains("401") || error.contains("403");
                if is_auth_error {
                    clear_client().await;
                    let store = AuthDataStore::default_store();
                    if let Err(e) = store.delete() {
                        eprintln!("auth delete failed: {e}");
                    }
                }
                let _ = qt_thread.queue(
                    move |mut qobject: Pin<&mut ffi::MessageList>| {
                        qobject.as_mut().begin_reset_model();
                        let mut rust = qobject.as_mut().rust_mut();
                        if let Some(item) = rust
                            .messages
                            .iter_mut()
                            .find(|item| item.message_id == tmp_id_for_fail)
                        {
                            item.status = QString::from("failed");
                        }
                        qobject.as_mut().end_reset_model();
                        if is_auth_error {
                            qobject
                                .as_mut()
                                .auth_error(&QString::from(error.as_str()));
                        }
                    },
                );
            }
        });
    }

    pub fn send_typing(self: Pin<&mut Self>, typing: bool) {
        let conversation_id = self.rust().selected_conversation_id.clone();
        if conversation_id.is_empty() {
            return;
        }

        spawn(async move {
            if let Some(client) = get_client().await {
                if let Err(e) = client.send_typing_update(&conversation_id, typing).await {
                    eprintln!("send_typing_update failed: {e}");
                }
            }
        });
    }

    pub fn handle_message_event(
        mut self: Pin<&mut Self>,
        conversation_id: &QString,
        participant_id: &QString,
        body: &QString,
        transport_type: i64,
        message_id: &QString,
        tmp_id: &QString,
        timestamp_micros: i64,
        status_code: i32,
    ) {
        let conversation_id = conversation_id.to_string();
        if conversation_id.is_empty() {
            return;
        }
        if conversation_id != self.rust().selected_conversation_id {
            return;
        }

        let participant_id = participant_id.to_string();
        let from_me = !self.rust().me_participant_id.is_empty()
            && participant_id == self.rust().me_participant_id;

        let message_id = message_id.to_string();
        let tmp_id = tmp_id.to_string();
        let body = body.to_string();

        // Try to find and update an existing message (by message_id or tmp_id match)
        if !message_id.is_empty() || !tmp_id.is_empty() {
            let index = self.rust().messages.iter().position(|item| {
                (!message_id.is_empty() && item.message_id == message_id)
                    || (!tmp_id.is_empty() && item.message_id == tmp_id)
            });
            if let Some(index) = index {
                let next_status = map_message_status(status_code, from_me);
                let mut rust = self.as_mut().rust_mut();
                if let Some(item) = rust.messages.get_mut(index) {
                    item.status = QString::from(next_status);
                    // Update the message_id if we had a tmp_id match
                    if !message_id.is_empty() && item.message_id != message_id {
                        item.message_id = message_id;
                    }
                }
                drop(rust);
                // Emit dataChanged for just this row
                let model_index = self.as_ref().index(index as i32, 0, &QModelIndex::default());
                self.as_mut().data_changed(&model_index, &model_index);
                return;
            }
        }

        if body.trim().is_empty() {
            return;
        }

        // New message: insert at the correct sorted position
        let status = map_message_status(status_code, from_me);
        let new_item = MessageItem {
            body: QString::from(body),
            from_me,
            transport_type,
            timestamp_micros,
            message_id,
            status: QString::from(status),
            media_url: QString::from(""),
            is_media: false,
        };

        // Find insertion index (sorted by timestamp ascending)
        let insert_pos = self
            .rust()
            .messages
            .partition_point(|item| item.timestamp_micros <= timestamp_micros);
        let insert_pos_i32 = insert_pos as i32;

        self.as_mut()
            .begin_insert_rows(&QModelIndex::default(), insert_pos_i32, insert_pos_i32);
        self.as_mut()
            .rust_mut()
            .messages
            .insert(insert_pos, new_item);
        self.as_mut().end_insert_rows();
    }
}

// ── Free functions ───────────────────────────────────────────────

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

fn format_human_timestamp(timestamp_micros: i64) -> String {
    if timestamp_micros <= 0 {
        return String::new();
    }
    let timestamp_millis = timestamp_micros / 1000;
    let utc = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(timestamp_millis);
    let Some(utc_time) = utc else {
        return String::new();
    };
    let time = utc_time.with_timezone(&Local);
    let now = Local::now();
    let delta = now.signed_duration_since(time);
    let delta_secs = delta.num_seconds();

    if delta_secs < 60 {
        return "Now".to_string();
    }
    if delta_secs < 3600 {
        let minutes = (delta_secs / 60).max(1);
        return format!("{minutes}m ago");
    }
    let same_day = now.date_naive() == time.date_naive();
    if same_day {
        return format!("{:02}:{:02}", time.hour(), time.minute());
    }
    if now.year() == time.year() {
        let day = time.day();
        let month = match time.month() {
            1 => "Jan",
            2 => "Feb",
            3 => "Mar",
            4 => "Apr",
            5 => "May",
            6 => "Jun",
            7 => "Jul",
            8 => "Aug",
            9 => "Sep",
            10 => "Oct",
            11 => "Nov",
            _ => "Dec",
        };
        return format!("{day} {month}");
    }
    let year = (time.year() % 100).abs();
    format!("{}/{:02}/{:02}", time.day(), time.month(), year)
}

fn format_human_message_time(timestamp_micros: i64) -> String {
    if timestamp_micros <= 0 {
        return String::new();
    }
    let timestamp_millis = timestamp_micros / 1000;
    let utc = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(timestamp_millis);
    let Some(utc_time) = utc else {
        return String::new();
    };
    let time = utc_time.with_timezone(&Local);
    format!("{:02}:{:02}", time.hour(), time.minute())
}

fn format_section_date(timestamp_micros: i64) -> String {
    if timestamp_micros <= 0 {
        return String::new();
    }
    let timestamp_millis = timestamp_micros / 1000;
    let utc = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(timestamp_millis);
    let Some(utc_time) = utc else {
        return String::new();
    };
    let time = utc_time.with_timezone(&Local);
    let now = Local::now();

    if now.date_naive() == time.date_naive() {
        return "Today".to_string();
    }
    let yesterday = now.date_naive() - chrono::Duration::days(1);
    if yesterday == time.date_naive() {
        return "Yesterday".to_string();
    }
    let day = time.day();
    let month = match time.month() {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        _ => "December",
    };
    if now.year() == time.year() {
        format!("{day} {month}")
    } else {
        format!("{day} {month} {}", time.year())
    }
}

fn build_preview(convo: &libgmessages_rs::proto::conversations::Conversation) -> String {
    let Some(latest) = convo.latest_message.as_ref() else {
        return String::new();
    };
    let mut prefix = String::new();
    if latest.from_me != 0 {
        prefix.push_str("You: ");
    } else if convo.is_group_chat && !latest.display_name.is_empty() {
        prefix.push_str(&latest.display_name);
        prefix.push_str(": ");
    }
    let mut snippet = latest.display_content.trim().to_string();
    if snippet.is_empty() {
        snippet = "Attachment".to_string();
    }
    format!("{prefix}{snippet}")
}

fn detect_extension(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "png"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "jpg"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "gif"
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "webp"
    } else {
        "bin"
    }
}

fn map_message_status(status_code: i32, from_me: bool) -> &'static str {
    if !from_me {
        return "received";
    }
    match status_code {
        5 | 6 | 7 => "sending",
        2 => "received",  // OUTGOING_DELIVERED
        11 => "read",     // OUTGOING_DISPLAYED
        _ => "sent",
    }
}
