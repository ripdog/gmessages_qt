use crate::ffi;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::{Datelike, Local, Timelike};
use core::pin::Pin;
use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::QString;
use libgmessages_rs::{
    auth::AuthData,
    gmclient::GMClient,
    store::AuthDataStore,
};
use futures_util::StreamExt;
use qrcode::render::svg;
use qrcode::QrCode;
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;
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

                    let paired = tokio::time::timeout(
                        Duration::from_secs(20),
                        client.wait_for_qr_pairing_on_stream(stream, Duration::from_secs(20)),
                    )
                    .await;

                    match paired {
                        Ok(Ok(Some(_))) => {
                            let auth_handle = client.auth();
                            let auth = auth_handle.lock().await;
                            store
                                .save(&auth)
                                .map_err(|error| error.to_string())?;
                            return Ok(true);
                        }
                        Ok(Ok(None)) => {
                            continue;
                        }
                        Ok(Err(error)) => {
                            return Err(error.to_string());
                        }
                        Err(_) => {
                            continue;
                        }
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

    pub fn logout(self: Pin<&mut Self>, reason: &QString) {
        let message = reason.to_string();
        let qt_thread: CxxQtThread<ffi::AppState> = self.qt_thread();
        std::thread::spawn(move || {
            let store = AuthDataStore::default_store();
            if let Err(error) = store.delete() {
                eprintln!("auth delete failed: {error}");
            }
            let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::AppState>| {
                qobject.as_mut().set_logged_in(false);
                qobject.as_mut().set_login_in_progress(false);
                if message.is_empty() {
                    qobject.as_mut().set_status_message(QString::from("Not logged in"));
                } else {
                    qobject.as_mut().set_status_message(QString::from(message.as_str()));
                }
            });
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

            let status_thread = qt_thread.clone();
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

                let stream = handler.client().start_long_poll_stream().await.map_err(|error| error.to_string())?;
                let mut skip_count: i32 = 0;

                let loop_handler = handler.clone();
                let loop_stop = stop_flag.clone();
                let session_thread = status_thread.clone();
                let loop_task = tokio::spawn(async move {
                    let mut stream = stream;
                    loop {
                        if loop_stop.load(std::sync::atomic::Ordering::Relaxed) {
                            break;
                        }
                        let item = stream.next().await;
                        let Some(item) = item else { break; };
                        let payload = match item {
                            Ok(payload) => payload,
                            Err(err) => return Err(libgmessages_rs::gmclient::SessionError::Client(err)),
                        };
                        if let Some(ack) = payload.ack.as_ref() {
                            if let Some(count) = ack.count {
                                skip_count = count;
                            }
                        }
                        let Some(data) = payload.data.as_ref() else { continue; };
                        if skip_count > 0 {
                            skip_count -= 1;
                            continue;
                        }
                        if data.bugle_route != libgmessages_rs::proto::rpc::BugleRoute::DataEvent as i32 {
                            continue;
                        }

                        let updates = loop_handler
                            .client()
                            .decode_update_events_from_message(data)
                            .await
                            .map_err(libgmessages_rs::gmclient::SessionError::from)?;
                        let Some(updates) = updates else { continue; };
                        if let Some(event) = updates.event {
                            if let libgmessages_rs::proto::events::update_events::Event::MessageEvent(message_event) = event {
                                for message in message_event.data {
                                    let body = message
                                        .message_info
                                        .iter()
                                        .find_map(|info| match &info.data {
                                            Some(libgmessages_rs::proto::conversations::message_info::Data::MessageContent(content)) => {
                                                let content_text = content.content.trim();
                                                if content_text.is_empty() {
                                                    None
                                                } else {
                                                    Some(content_text.to_string())
                                                }
                                            }
                                            _ => None,
                                        })
                                        .unwrap_or_default();
                                    if body.is_empty() {
                                        continue;
                                    }

                                    let conversation_id = message.conversation_id.clone();
                                    let participant_id = message.participant_id.clone();
                                    let transport_type = message.r#type;
                                    let body_text = body.clone();
                                        let request_id = if !message.message_id.is_empty() {
                                            message.message_id.clone()
                                        } else if !message.tmp_id.is_empty() {
                                            message.tmp_id.clone()
                                        } else {
                                            message.another_message_id.as_ref().map(|id| id.message_id.clone()).unwrap_or_default()
                                        };

                                        let tmp_id = message.tmp_id.clone();
                                        let status_code = message
                                            .message_status
                                            .as_ref()
                                            .map(|status| status.status)
                                            .unwrap_or(0);

                                    let timestamp_micros = message.timestamp;

                                    let request_id_for_signal = request_id.clone();
                                        let _ = session_thread.queue(move |mut qobject: Pin<&mut ffi::SessionController>| {
                                            qobject.as_mut().message_received(
                                                &QString::from(conversation_id.clone()),
                                                &QString::from(participant_id.clone()),
                                                &QString::from(body_text.clone()),
                                                transport_type,
                                                &QString::from(request_id_for_signal.clone()),
                                                &QString::from(tmp_id.clone()),
                                                timestamp_micros,
                                                status_code,
                                            );
                                        });

                                    if !request_id.is_empty() {
                                        let client_clone = loop_handler.client();
                                        tokio::spawn(async move {
                                            let _ = client_clone.ack_messages(vec![request_id]).await;
                                        });
                                    }
                                }
                            }
                        }
                    }
                    Ok(())
                });

                let event_stop = stop_flag.clone();
                let event_handler = handler.clone();
                let event_task: tokio::task::JoinHandle<Result<(), libgmessages_rs::gmclient::SessionError>> = tokio::spawn(async move {
                    while !event_stop.load(std::sync::atomic::Ordering::Relaxed) {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        let _ = event_handler
                            .client()
                            .send_rpc_message_with_id_and_session_no_payload(
                                libgmessages_rs::proto::rpc::ActionType::GetUpdates,
                                libgmessages_rs::proto::rpc::MessageType::BugleMessage,
                                event_handler.session_id(),
                                event_handler.session_id(),
                                true,
                            )
                            .await;
                    }
                    Ok(())
                });

                let loop_result = loop_task
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|result| result.map_err(|error| error.to_string()))?;
                event_task.abort();
                Ok(loop_result)
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

            let result: Result<(), String> = runtime.block_on(async move {
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

                let mut items: Vec<ConversationItem> = response
                    .conversations
                    .into_iter()
                    .map(|convo| {
                        let preview = QString::from(build_preview(&convo));
                        let name = QString::from(convo.name);
                        let conversation_id = convo.conversation_id;
                        let is_group_chat = convo.is_group_chat;
                        let unread = convo.unread;
                        let me_participant_id = convo
                            .participants
                            .iter()
                            .find_map(|participant| {
                                if participant.is_me {
                                    participant
                                        .id
                                        .as_ref()
                                        .map(|id| id.participant_id.clone())
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
                            .find_map(|participant| {
                                if participant.is_me {
                                    None
                                } else if !participant.contact_id.is_empty() {
                                    Some(participant.contact_id.clone())
                                } else {
                                    participant
                                        .id
                                        .as_ref()
                                        .map(|id| id.participant_id.clone())
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
                    })
                    .collect();

                items.sort_by(|left, right| right.last_message_timestamp.cmp(&left.last_message_timestamp));

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
                    avatar_by_identifier.insert(item.avatar_identifier.clone(), String::new());
                }

                let _ = ui_thread.queue(move |mut qobject: Pin<&mut ffi::ConversationList>| {
                    qobject.as_mut().begin_reset_model();
                    let mut rust = qobject.as_mut().rust_mut();
                    rust.avatar_by_identifier.clear();
                    rust.all_items = items;
                    rust.filtered_items = filter_items(&rust.all_items, &rust.filter_text);
                    qobject.as_mut().set_loading(false);
                    qobject.as_mut().end_reset_model();
                });

                if !avatar_identifiers.is_empty() {
                    let request = libgmessages_rs::proto::client::GetThumbnailRequest {
                        identifiers: avatar_identifiers.clone(),
                    };
                    let attempts = [
                        (true, libgmessages_rs::proto::rpc::MessageType::BugleAnnotation),
                        (true, libgmessages_rs::proto::rpc::MessageType::BugleMessage),
                        (false, libgmessages_rs::proto::rpc::MessageType::BugleAnnotation),
                        (false, libgmessages_rs::proto::rpc::MessageType::BugleMessage),
                        (false, libgmessages_rs::proto::rpc::MessageType::UnknownMessageType),
                    ];
                    for (encrypted, message_type) in attempts {
                        let attempt: Result<libgmessages_rs::proto::client::GetThumbnailResponse, _> = if encrypted {
                            handler.send_request(
                                libgmessages_rs::proto::rpc::ActionType::GetContactsThumbnail,
                                message_type,
                                &request,
                            ).await
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
                    let _ = ui_thread.queue(move |mut qobject: Pin<&mut ffi::ConversationList>| {
                        let mut rust = qobject.as_mut().rust_mut();
                        let avatar_by_identifier = avatar_by_identifier;
                        rust.avatar_by_identifier = avatar_by_identifier.clone();
                        for item in &mut rust.all_items {
                            if let Some(url) = avatar_by_identifier.get(&item.avatar_identifier) {
                                if !url.is_empty() {
                                    item.avatar_url = QString::from(url.as_str());
                                }
                            }
                        }
                        rust.filtered_items = filter_items(&rust.all_items, &rust.filter_text);
                    });
                }

                Ok(())
            });

            if let Err(error) = result {
                let is_auth_error = error.contains("authentication credential") || error.contains("401");
                eprintln!("conversation load failed: {error}");
                let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::ConversationList>| {
                    qobject.as_mut().rust_mut().avatar_by_identifier.clear();
                    qobject.as_mut().set_loading(false);
                    if is_auth_error {
                        qobject.as_mut().auth_error(&QString::from(error.as_str()));
                    }
                });
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

pub struct MessageItem {
    body: QString,
    from_me: bool,
    transport_type: i64,
    timestamp_micros: i64,
    message_id: String,
    status: QString,
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
            6 => QVariant::from(&QString::from(format_human_message_time(item.timestamp_micros))),
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
        self.as_mut().end_reset_model();
        self.as_mut().set_loading(true);
        let qt_thread: CxxQtThread<ffi::MessageList> = self.qt_thread();
        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("message load failed: {error}");
                    return;
                }
            };

            let result: Result<(Vec<MessageItem>, String), String> = runtime.block_on(async move {
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

                let request = libgmessages_rs::proto::client::ListMessagesRequest {
                    conversation_id: conversation_id.clone(),
                    count: 50,
                    cursor: None,
                };
                let response: libgmessages_rs::proto::client::ListMessagesResponse = handler
                    .send_request(
                        libgmessages_rs::proto::rpc::ActionType::ListMessages,
                        libgmessages_rs::proto::rpc::MessageType::BugleMessage,
                        &request,
                    )
                    .await
                    .map_err(|error| error.to_string())?;

                let convo_request = libgmessages_rs::proto::client::GetConversationRequest {
                    conversation_id: conversation_id.clone(),
                };
                let convo_response: libgmessages_rs::proto::client::GetConversationResponse = handler
                    .send_request(
                        libgmessages_rs::proto::rpc::ActionType::GetConversation,
                        libgmessages_rs::proto::rpc::MessageType::BugleMessage,
                        &convo_request,
                    )
                    .await
                    .map_err(|error| error.to_string())?;

                let me_participant_id = convo_response
                    .conversation
                    .as_ref()
                    .and_then(|convo| {
                        convo.participants.iter().find_map(|participant| {
                            if participant.is_me {
                                participant
                                    .id
                                    .as_ref()
                                    .map(|id| id.participant_id.clone())
                            } else {
                                None
                            }
                        })
                    })
                    .unwrap_or_default();

                task.abort();

                let mut messages: Vec<MessageItem> = response
                    .messages
                    .into_iter()
                    .filter_map(|message| {
                        let body = message
                            .message_info
                            .iter()
                            .find_map(|info| match &info.data {
                                Some(libgmessages_rs::proto::conversations::message_info::Data::MessageContent(content)) => {
                                    let content_text = content.content.trim();
                                    if content_text.is_empty() {
                                        None
                                    } else {
                                        Some(content_text.to_string())
                                    }
                                }
                                _ => None,
                            })
                            .unwrap_or_default();
                        if body.is_empty() {
                            return None;
                        }
                        let from_me = !me_participant_id.is_empty()
                            && message.participant_id == me_participant_id;
                        let status_code = message
                            .message_status
                            .as_ref()
                            .map(|status| status.status)
                            .unwrap_or(0);
                        let status = map_message_status(status_code, from_me);
                        println!("message status: {status:?} body: {body:?}");
                        let message_id = if !message.message_id.is_empty() {
                            message.message_id.clone()
                        } else if !message.tmp_id.is_empty() {
                            message.tmp_id.clone()
                        } else {
                            message
                                .another_message_id
                                .as_ref()
                                .map(|id| id.message_id.clone())
                                .unwrap_or_default()
                        };

                        Some(MessageItem {
                            body: QString::from(body),
                            from_me,
                            transport_type: message.r#type,
                            timestamp_micros: message.timestamp,
                            message_id,
                            status: QString::from(status),
                        })
                    })
                    .collect();

                messages.sort_by_key(|item| item.timestamp_micros);

                Ok((messages, me_participant_id))
            });

            match result {
                Ok((messages, me_participant_id)) => {
                    let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::MessageList>| {
                    qobject.as_mut().begin_reset_model();
                    let mut rust = qobject.as_mut().rust_mut();
                    rust.messages = messages;
                    rust.messages.shrink_to_fit();
                    rust.me_participant_id = me_participant_id;
                    qobject.as_mut().set_loading(false);
                    qobject.as_mut().end_reset_model();
                });
                }
                Err(error) => {
                    let is_auth_error = error.contains("authentication credential") || error.contains("401");
                    eprintln!("message load failed: {error}");
                    let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::MessageList>| {
                        qobject.as_mut().set_loading(false);
                        if is_auth_error {
                            qobject.as_mut().auth_error(&QString::from(error.as_str()));
                        }
                    });
                }
            }
        });
    }

    pub fn send_message(mut self: Pin<&mut Self>, text: &QString) {
        let body = text.to_string();
        let body = body.trim().to_string();
        if body.is_empty() {
            return;
        }

        let conversation_id = self.rust().selected_conversation_id.clone();
        if conversation_id.is_empty() {
            return;
        }

        let now_micros = chrono::Utc::now().timestamp_micros();
        let tmp_id = Uuid::new_v4().to_string().to_lowercase();
        let qt_thread: CxxQtThread<ffi::MessageList> = self.qt_thread();

        eprintln!(
            "optimistic message insert: convo_id={conversation_id:?} tmp_id={tmp_id:?} body={body:?} timestamp_micros={now_micros}"
        );

        self.as_mut().begin_reset_model();
        let mut rust = self.as_mut().rust_mut();
        rust.messages.push(MessageItem {
            body: QString::from(body.clone()),
            from_me: true,
            transport_type: 4,
            timestamp_micros: now_micros,
            message_id: tmp_id.clone(),
            status: QString::from("sending"),
        });
        rust.messages.sort_by_key(|item| item.timestamp_micros);
        self.as_mut().end_reset_model();

        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("message send failed: {error}");
                    return;
                }
            };

            let tmp_id_for_fail = tmp_id.clone();
            let tmp_id_for_runtime = tmp_id.clone();
            let tmp_id_for_log = tmp_id.clone();
            let convo_id_for_log = conversation_id.clone();
            let body_for_log = body.clone();
            let result: Result<(), String> = runtime.block_on(async move {
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
                    message_payload_content: Some(libgmessages_rs::proto::client::MessagePayloadContent {
                        message_content: Some(libgmessages_rs::proto::conversations::MessageContent {
                            content: body.clone(),
                        }),
                    }),
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
                    .map_err(|error| error.to_string())?;

                eprintln!(
                    "send_message success: convo_id={conversation_id:?} tmp_id={tmp_id_for_runtime:?} body={body:?}"
                );

                task.abort();
                Ok(())
            });

            if let Err(error) = result {
                eprintln!("message send failed: {error}");
                eprintln!(
                    "send_message failed: convo_id={convo_id_for_log:?} tmp_id={tmp_id_for_log:?} body={body_for_log:?}"
                );
                if error.contains("authentication credential") || error.contains("401") {
                    let store = AuthDataStore::default_store();
                    if let Err(error) = store.delete() {
                        eprintln!("auth delete failed: {error}");
                    }
                }
                let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::MessageList>| {
                    qobject.as_mut().begin_reset_model();
                    let mut rust = qobject.as_mut().rust_mut();
                    if let Some(item) = rust.messages.iter_mut().find(|item| item.message_id == tmp_id_for_fail) {
                        item.status = QString::from("failed");
                    }
                    qobject.as_mut().end_reset_model();
                });
                return;
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

        if from_me {
            let mapped_status = map_message_status(status_code, from_me);
            eprintln!(
                "message echo received: convo_id={conversation_id:?} participant_id={participant_id:?} message_id={message_id:?} tmp_id={tmp_id:?} transport_type={transport_type} timestamp_micros={timestamp_micros} status_code={status_code} mapped_status={mapped_status:?} body={body:?}"
            );
        }
        if !message_id.is_empty() || !tmp_id.is_empty() {
            let index = self.rust().messages.iter().position(|item| {
                (!message_id.is_empty() && item.message_id == message_id)
                    || (!tmp_id.is_empty() && item.message_id == tmp_id)
            });
            if let Some(index) = index {
                self.as_mut().begin_reset_model();
                let mut rust = self.as_mut().rust_mut();
                if let Some(item) = rust.messages.get_mut(index) {
                    let next_status = map_message_status(status_code, from_me);
                    if from_me {
                        eprintln!(
                            "echo matched optimistic: index={index} message_id={message_id:?} tmp_id={tmp_id:?} updating_status={next_status:?}"
                        );
                    }
                    item.status = QString::from(next_status);
                }
                self.as_mut().end_reset_model();
                return;
            }
        }
        if body.trim().is_empty() {
            return;
        }

        if from_me {
            let mapped_status = map_message_status(status_code, from_me);
            eprintln!(
                "echo insert new: convo_id={conversation_id:?} participant_id={participant_id:?} message_id={message_id:?} tmp_id={tmp_id:?} transport_type={transport_type} timestamp_micros={timestamp_micros} status_code={status_code} mapped_status={mapped_status:?} body={body:?}"
            );
        }

        self.as_mut().begin_reset_model();
        let mut rust = self.as_mut().rust_mut();
        let status = map_message_status(status_code, from_me);
        let new_item = MessageItem {
            body: QString::from(body),
            from_me,
            transport_type,
            timestamp_micros,
            message_id,
            status: QString::from(status),
        };
        rust.messages.push(new_item);
        rust.messages.sort_by_key(|item| item.timestamp_micros);
        self.as_mut().end_reset_model();
    }
}
