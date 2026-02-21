#![allow(clippy::drop_non_drop)]
use crate::ffi;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use core::pin::Pin;
use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::QString;

use libgmessages_rs::store::AuthDataStore;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use uuid::Uuid;

use crate::ffi::QHash_i32_QByteArray;
use crate::ffi::QModelIndex;
use crate::ffi::QVariant;

use super::*;
use crate::app_state::shared::fetch_avatars_async;
// ── MessageList ──────────────────────────────────────────────────

#[derive(Clone)]
pub struct MessageItem {
    pub body: QString,
    pub from_me: bool,
    pub transport_type: i64,
    pub timestamp_micros: i64,
    pub message_id: String,
    pub status: QString,
    pub media_url: QString,
    pub is_media: bool,
    pub avatar_url: QString,
    pub is_info: bool,
    pub participant_id: String,
    pub mime_type: QString,
    pub thumbnail_url: QString,
    pub upload_progress: f32,
}

pub struct MessageListRust {
    pub loading: bool,
    messages: Vec<MessageItem>,
    selected_conversation_id: String,
    me_participant_id: String,
    cache: HashMap<String, (Vec<MessageItem>, String)>,
    upload_cancellations: HashMap<String, Arc<AtomicBool>>,
}

impl Default for MessageListRust {
    fn default() -> Self {
        Self {
            loading: false,
            messages: Vec::new(),
            selected_conversation_id: String::new(),
            me_participant_id: String::new(),
            cache: HashMap::new(),
            upload_cancellations: HashMap::new(),
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
            7 => QVariant::from(&QString::from(format_section_date(item.timestamp_micros))),
            8 => QVariant::from(&item.media_url),
            9 => QVariant::from(&item.is_media),
            10 => QVariant::from(&item.avatar_url),
            11 => QVariant::from(&item.is_info),
            12 => QVariant::from(&item.mime_type),
            13 => {
                let is_start = if row + 1 < self.messages.len() {
                    let next_item = &self.messages[row + 1];
                    let current_date = format_section_date(item.timestamp_micros);
                    let next_date = format_section_date(next_item.timestamp_micros);
                    current_date != next_date
                } else {
                    true
                };
                QVariant::from(&is_start)
            }
            14 => QVariant::from(&item.thumbnail_url),
            15 => QVariant::from(&item.upload_progress),
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
        roles.insert(10, "avatar_url".into());
        roles.insert(11, "is_info".into());
        roles.insert(12, "mime_type".into());
        roles.insert(13, "is_start_of_day".into());
        roles.insert(14, "thumbnail_url".into());
        roles.insert(15, "upload_progress".into());
        roles
    }

    pub fn load(mut self: Pin<&mut Self>, conversation_id: &QString) {
        let conversation_id = conversation_id.to_string();
        if conversation_id.is_empty() {
            return;
        }

        self.as_mut().begin_reset_model();
        let mut rust = self.as_mut().rust_mut();
        let is_cached = if let Some((cached_msgs, me_id)) = rust.cache.get(&conversation_id) {
            let c = cached_msgs.clone();
            let m = me_id.clone();
            rust.messages = c;
            rust.me_participant_id = m;
            true
        } else {
            rust.messages.clear();
            rust.me_participant_id.clear();
            false
        };
        rust.messages.shrink_to_fit();
        rust.selected_conversation_id = conversation_id.clone();
        drop(rust);
        self.as_mut().end_reset_model();

        if !is_cached {
            self.as_mut().set_loading(true);
        }

        let qt_thread: CxxQtThread<ffi::MessageList> = self.qt_thread();

        spawn(async move {
            let result: Result<
                (
                    Vec<MessageItem>,
                    String,
                    Vec<(String, String, Vec<u8>, String)>,
                    Vec<String>,
                    Vec<(String, String)>,
                ),
                String,
            > = async {
                let client = ensure_client().await?;
                let handler = make_handler(&client).await?;

                let request = libgmessages_rs::proto::client::ListMessagesRequest {
                    conversation_id: conversation_id.clone(),
                    count: 50,
                    cursor: None,
                };
                let req_msgs = async {
                    let res = handler
                        .send_request::<libgmessages_rs::proto::client::ListMessagesResponse>(
                            libgmessages_rs::proto::rpc::ActionType::ListMessages,
                            libgmessages_rs::proto::rpc::MessageType::BugleMessage,
                            &request,
                        )
                        .await;
                    res
                };

                let convo_request = libgmessages_rs::proto::client::GetConversationRequest {
                    conversation_id: conversation_id.clone(),
                };

                let req_convo = async {
                    let res = handler
                        .send_request::<libgmessages_rs::proto::client::GetConversationResponse>(
                            libgmessages_rs::proto::rpc::ActionType::GetConversation,
                            libgmessages_rs::proto::rpc::MessageType::BugleMessage,
                            &convo_request,
                        )
                        .await;
                    res
                };

                let (response, convo_response) =
                    tokio::try_join!(req_msgs, req_convo).map_err(|e| e.to_string())?;

                let mut me_participant_id = String::new();
                let mut avatar_identifiers: Vec<(String, String)> = Vec::new();

                if let Some(convo) = convo_response.conversation.as_ref() {
                    for p in &convo.participants {
                        let pid =
                            p.id.as_ref()
                                .map(|id| id.participant_id.clone())
                                .unwrap_or_default();
                        if p.is_me {
                            me_participant_id = pid;
                        } else {
                            let identifier = if !p.contact_id.is_empty() {
                                p.contact_id.clone()
                            } else {
                                pid.clone()
                            };
                            if !identifier.is_empty() {
                                avatar_identifiers.push((pid, identifier));
                            }
                        }
                    }
                }

                let mut avatar_by_participant_id: HashMap<String, String> = HashMap::new();
                let mut identifiers_to_fetch = Vec::new();

                {
                    let cache = shared().avatars.read().await;
                    for (pid, id) in &avatar_identifiers {
                        if let Some(url) = cache.get(id) {
                            avatar_by_participant_id.insert(pid.clone(), url.clone());
                        } else {
                            if !identifiers_to_fetch.contains(id) {
                                identifiers_to_fetch.push(id.clone());
                            }
                        }
                    }
                }

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
                        let media_mime = if let Some((id, key, ref mime)) = media {
                            let m = mime.clone();
                            media_downloads.push((message_id.clone(), id, key, mime.clone()));
                            m
                        } else {
                            String::new()
                        };

                        let mut avatar_url = String::new();
                        if !from_me && !message.participant_id.is_empty() {
                            if let Some(url) = avatar_by_participant_id.get(&message.participant_id)
                            {
                                avatar_url = url.clone();
                            }
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
                            avatar_url: QString::from(avatar_url.as_str()),
                            is_info: status_code >= 200,
                            participant_id: message.participant_id.clone(),
                            mime_type: QString::from(media_mime.as_str()),
                            thumbnail_url: QString::from(""),
                            upload_progress: 1.0,
                        })
                    })
                    .collect();

                messages.sort_by(|a, b| b.timestamp_micros.cmp(&a.timestamp_micros));

                // Mark conversation as read
                if let Some(last_msg) = messages.last() {
                    if !last_msg.message_id.is_empty() {
                        let msg_id = last_msg.message_id.clone();
                        let convo_id = conversation_id.clone();
                        let client_clone = client.clone();
                        spawn(async move {
                            let _ = client_clone.mark_message_read(&convo_id, &msg_id).await;
                        });
                    }
                }

                Ok((
                    messages,
                    me_participant_id,
                    media_downloads,
                    identifiers_to_fetch,
                    avatar_identifiers,
                ))
            }
            .await;

            match result {
                Ok((
                    new_messages,
                    me_participant_id,
                    media_downloads,
                    identifiers_to_fetch,
                    avatar_identifiers,
                )) => {
                    let convo_id = conversation_id.clone();
                    let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::MessageList>| {
                        let mut rust = qobject.as_mut().rust_mut();
                        if rust.selected_conversation_id == convo_id {
                            if rust.messages.is_empty() {
                                rust.messages = new_messages.clone();
                                rust.me_participant_id = me_participant_id.clone();
                                rust.cache
                                    .insert(convo_id.clone(), (new_messages, me_participant_id));
                                drop(rust);
                                qobject.as_mut().begin_reset_model();
                                qobject.as_mut().set_loading(false);
                                qobject.as_mut().end_reset_model();
                            } else {
                                // Set loading down immediately
                                drop(rust);
                                qobject.as_mut().set_loading(false);

                                // 1. Remove messages entirely deleted from server
                                let mut to_remove = Vec::new();
                                let rust = qobject.as_mut().rust_mut();
                                for (i, msg) in rust.messages.iter().enumerate() {
                                    if !new_messages.iter().any(|m| m.message_id == msg.message_id)
                                    {
                                        if msg.status.to_string() != "sending"
                                            && msg.status.to_string() != "failed"
                                        {
                                            to_remove.push(i);
                                        }
                                    }
                                }
                                drop(rust);
                                for i in to_remove.into_iter().rev() {
                                    qobject.as_mut().begin_remove_rows(
                                        &QModelIndex::default(),
                                        i as i32,
                                        i as i32,
                                    );
                                    qobject.as_mut().rust_mut().messages.remove(i);
                                    qobject.as_mut().end_remove_rows();
                                }

                                // 2. Diff updates and inserts
                                for new_msg in &new_messages {
                                    let mut rust = qobject.as_mut().rust_mut();
                                    if let Some(pos) = rust
                                        .messages
                                        .iter()
                                        .position(|m| m.message_id == new_msg.message_id)
                                    {
                                        // Update
                                        let mut changed = false;
                                        if rust.messages[pos].status.to_string()
                                            != new_msg.status.to_string()
                                        {
                                            rust.messages[pos].status =
                                                QString::from(new_msg.status.to_string().as_str());
                                            changed = true;
                                        }
                                        if rust.messages[pos].body.to_string()
                                            != new_msg.body.to_string()
                                        {
                                            rust.messages[pos].body =
                                                QString::from(new_msg.body.to_string().as_str());
                                            changed = true;
                                        }
                                        drop(rust);
                                        if changed {
                                            let model_index = qobject.as_ref().index(
                                                pos as i32,
                                                0,
                                                &QModelIndex::default(),
                                            );
                                            qobject
                                                .as_mut()
                                                .data_changed(&model_index, &model_index);
                                        }
                                    } else {
                                        // Insert
                                        let pos = rust.messages.partition_point(|item| {
                                            item.timestamp_micros >= new_msg.timestamp_micros
                                        });
                                        drop(rust);
                                        qobject.as_mut().begin_insert_rows(
                                            &QModelIndex::default(),
                                            pos as i32,
                                            pos as i32,
                                        );
                                        let mut rust = qobject.as_mut().rust_mut();
                                        rust.messages.insert(pos, new_msg.clone());
                                        drop(rust);
                                        qobject.as_mut().end_insert_rows();
                                    }
                                }

                                let mut rust = qobject.as_mut().rust_mut();
                                rust.me_participant_id = me_participant_id.clone();
                                let msgs_clone = rust.messages.clone();
                                if rust.cache.len() >= 10 && !rust.cache.contains_key(&convo_id) {
                                    let key_to_remove = rust.cache.keys().next().cloned().unwrap();
                                    rust.cache.remove(&key_to_remove);
                                }
                                rust.cache
                                    .insert(convo_id.clone(), (msgs_clone, me_participant_id));
                            }
                        } else {
                            if rust.cache.len() >= 10 && !rust.cache.contains_key(&convo_id) {
                                let key_to_remove = rust.cache.keys().next().cloned().unwrap();
                                rust.cache.remove(&key_to_remove);
                            }
                            rust.cache
                                .insert(convo_id.clone(), (new_messages, me_participant_id));
                        }
                    });

                    // Start background download of media
                    if !media_downloads.is_empty() {
                        let ui_for_media = qt_thread.clone();
                        spawn(async move {
                            if let Some(client) = get_client().await {
                                for (msg_id, media_id, key, mime) in media_downloads {
                                    let ext = crate::app_state::utils::mime_to_extension(&mime);
                                    let safe_id = media_id
                                        .replace("/", "_")
                                        .replace("+", "_")
                                        .replace("=", "")
                                        .replace("-", "_");
                                    let safe_id = if safe_id.is_empty() {
                                        msg_id.replace("-", "_")
                                    } else {
                                        safe_id
                                    };

                                    let tmp_dir = std::env::temp_dir().join("kourier_media");
                                    let _ = std::fs::create_dir_all(&tmp_dir);
                                    let path = tmp_dir.join(format!("{}.{}", safe_id, ext));

                                    if !path.exists() {
                                        if let Ok(data) =
                                            client.download_media(&media_id, &key).await
                                        {
                                            let _ = crate::app_state::utils::media_data_to_uri(
                                                &data, &mime, &safe_id,
                                            );
                                        }
                                    }

                                    if path.exists() {
                                        let uri = format!("file://{}", path.to_string_lossy());
                                        let thumb_uri = if mime.starts_with("video/") {
                                            crate::app_state::utils::generate_video_thumbnail(&path)
                                                .unwrap_or_default()
                                        } else {
                                            String::new()
                                        };
                                        let _ = ui_for_media.queue(
                                            move |mut qobject: Pin<&mut ffi::MessageList>| {
                                                let mut rust = qobject.as_mut().rust_mut();
                                                if let Some(pos) = rust
                                                    .messages
                                                    .iter()
                                                    .position(|m| m.message_id == msg_id)
                                                {
                                                    rust.messages[pos].media_url =
                                                        QString::from(uri.as_str());
                                                    if !thumb_uri.is_empty() {
                                                        rust.messages[pos].thumbnail_url =
                                                            QString::from(thumb_uri.as_str());
                                                    }
                                                    drop(rust);
                                                    let model_index = qobject.as_ref().index(
                                                        pos as i32,
                                                        0,
                                                        &QModelIndex::default(),
                                                    );
                                                    qobject
                                                        .as_mut()
                                                        .data_changed(&model_index, &model_index);
                                                }
                                            },
                                        );
                                    }
                                }
                            }
                        });
                    }

                    // Start background avatar load
                    if !identifiers_to_fetch.is_empty() {
                        let ui_for_avatars = qt_thread.clone();
                        let mapped_identifiers = avatar_identifiers.clone();
                        let conversation_id_clone = conversation_id.clone();
                        spawn(async move {
                            if let Some(client_for_avatars) = get_client().await {
                                let new_avatars =
                                    fetch_avatars_async(client_for_avatars, identifiers_to_fetch)
                                        .await;
                                if !new_avatars.is_empty() {
                                    let mut pid_to_url = HashMap::<String, String>::new();
                                    for (pid, id) in &mapped_identifiers {
                                        if let Some(url) = new_avatars.get(id) {
                                            pid_to_url.insert(pid.to_string(), url.to_string());
                                        }
                                    }
                                    let _ = ui_for_avatars.queue(
                                        move |mut qobject: Pin<&mut ffi::MessageList>| {
                                            let mut rust = qobject.as_mut().rust_mut();
                                            if rust.selected_conversation_id
                                                == conversation_id_clone
                                            {
                                                let mut changes = Vec::new();
                                                for (pos, item) in
                                                    rust.messages.iter_mut().enumerate()
                                                {
                                                    if !item.participant_id.is_empty() {
                                                        if let Some(url) =
                                                            pid_to_url.get(&item.participant_id)
                                                        {
                                                            item.avatar_url =
                                                                QString::from(url.as_str());
                                                            changes.push(pos as i32);
                                                        }
                                                    }
                                                }
                                                let msgs = rust.messages.clone();
                                                let me_id = rust.me_participant_id.clone();
                                                rust.cache
                                                    .insert(conversation_id_clone, (msgs, me_id));
                                                drop(rust);
                                                for pos in changes {
                                                    let model_index = qobject.as_ref().index(
                                                        pos,
                                                        0,
                                                        &QModelIndex::default(),
                                                    );
                                                    qobject
                                                        .as_mut()
                                                        .data_changed(&model_index, &model_index);
                                                }
                                            }
                                        },
                                    );
                                }
                            }
                        });
                    }
                }
                Err(error) => {
                    let is_auth_error = error.contains("authentication credential")
                        || error.contains("401")
                        || error.contains("403");
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
        let insert_pos = 0;
        self.as_mut()
            .begin_insert_rows(&QModelIndex::default(), insert_pos, insert_pos);
        let mut rust = self.as_mut().rust_mut();
        rust.messages.insert(
            0,
            MessageItem {
                body: QString::from(body.clone()),
                from_me: true,
                transport_type: 4,
                timestamp_micros: now_micros,
                message_id: tmp_id.clone(),
                status: QString::from("sending"),
                media_url: QString::from(""),
                is_media: false,
                avatar_url: QString::from(""),
                is_info: false,
                participant_id: String::new(),
                mime_type: QString::from(""),
                thumbnail_url: QString::from(""),
                upload_progress: 1.0,
            },
        );
        // We do not sort here because the new message naturally belongs at the beginning (index 0).
        // It prevents scroll position reset issues.
        drop(rust);
        self.as_mut().end_insert_rows();

        let qt_thread: CxxQtThread<ffi::MessageList> = self.qt_thread();
        let tmp_id_for_fail = tmp_id.clone();

        spawn(async move {
            let result: Result<(), String> = async {
                let client = ensure_client().await?;
                let handler = make_handler(&client).await?;

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

                Ok(())
            }
            .await;

            if let Err(error) = result {
                eprintln!("message send failed: {error}");
                let is_auth_error = error.contains("authentication credential")
                    || error.contains("401")
                    || error.contains("403");
                if is_auth_error {
                    clear_client().await;
                    let store = AuthDataStore::default_store();
                    if let Err(e) = store.delete() {
                        eprintln!("auth delete failed: {e}");
                    }
                }
                let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::MessageList>| {
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
                        qobject.as_mut().auth_error(&QString::from(error.as_str()));
                    }
                });
            }
        });
    }

    pub fn send_media(mut self: Pin<&mut Self>, file_url: &QString, text: &QString) {
        let file_path = file_url.to_string();
        let path = if file_path.starts_with("file://") {
            file_path[7..].to_string()
        } else {
            file_path
        };

        if path.is_empty() {
            return;
        }

        let conversation_id = self.rust().selected_conversation_id.clone();
        if conversation_id.is_empty() {
            return;
        }

        let now_micros = chrono::Utc::now().timestamp_micros();
        let tmp_id = Uuid::new_v4().to_string().to_lowercase();
        let caption = text.to_string().trim().to_string();

        // Reject files over 100 MB (RCS limit) — metadata is instant, no file read
        const MAX_BYTES: u64 = 100 * 1024 * 1024;
        match std::fs::metadata(&path) {
            Ok(meta) if meta.len() > MAX_BYTES => {
                eprintln!(
                    "send_media: file too large ({:.1} MB), RCS limit is 100 MB",
                    meta.len() as f64 / 1_048_576.0
                );
                return;
            }
            Err(e) => {
                eprintln!("send_media: cannot stat file: {e}");
                return;
            }
            _ => {}
        }

        // Lightweight: determine MIME and file name from extension only (no I/O)
        let path_obj = std::path::Path::new(&path);
        let file_name = path_obj
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let ext = path_obj
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let mime_type = match ext.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "mp4" => "video/mp4",
            "webm" => "video/webm",
            "webp" => "image/webp",
            _ => "application/octet-stream",
        }
        .to_string();

        // Use the original file path as preview — no need to copy gigabytes on the UI thread
        let preview_uri = format!("file://{}", path);

        let body_text = if caption.is_empty() {
            String::new()
        } else {
            caption.clone()
        };

        let insert_pos = 0;
        self.as_mut()
            .begin_insert_rows(&QModelIndex::default(), insert_pos, insert_pos);
        let mut rust = self.as_mut().rust_mut();
        rust.messages.insert(
            0,
            MessageItem {
                body: QString::from(body_text.as_str()),
                from_me: true,
                transport_type: 4,
                timestamp_micros: now_micros,
                message_id: tmp_id.clone(),
                status: QString::from("sending"),
                media_url: QString::from(preview_uri.as_str()),
                is_media: true,
                avatar_url: QString::from(""),
                is_info: false,
                participant_id: String::new(),
                mime_type: QString::from(mime_type.as_str()),
                thumbnail_url: QString::from(""),
                upload_progress: 0.0,
            },
        );
        drop(rust);
        self.as_mut().end_insert_rows();

        let qt_thread: CxxQtThread<ffi::MessageList> = self.qt_thread();
        let tmp_id_for_fail = tmp_id.clone();

        // Create a cancellation token for this upload
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.as_mut()
            .rust_mut()
            .upload_cancellations
            .insert(tmp_id.clone(), cancel_flag.clone());

        spawn(async move {
            // Generate video thumbnail in the background (quick: ffmpeg only extracts one frame)
            if mime_type.starts_with("video/") {
                let thumb_path = path.clone();
                let thumb_tmp = tmp_id.clone();
                let ui_for_thumb = qt_thread.clone();
                let p = std::path::Path::new(&thumb_path);
                if let Some(thumb_uri) = crate::app_state::utils::generate_video_thumbnail(p) {
                    let _ = ui_for_thumb.queue(move |mut qobject: Pin<&mut ffi::MessageList>| {
                        let mut rust = qobject.as_mut().rust_mut();
                        if let Some(pos) =
                            rust.messages.iter().position(|m| m.message_id == thumb_tmp)
                        {
                            rust.messages[pos].thumbnail_url = QString::from(thumb_uri.as_str());
                            drop(rust);
                            let model_index =
                                qobject
                                    .as_ref()
                                    .index(pos as i32, 0, &QModelIndex::default());
                            qobject.as_mut().data_changed(&model_index, &model_index);
                        }
                    });
                }
            }

            // Read the file in the background — this is the operation that was freezing the UI
            let bytes = tokio::task::spawn_blocking(move || std::fs::read(&path))
                .await
                .unwrap_or_else(|_| {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "task join failed",
                    ))
                });
            let bytes = match bytes {
                Ok(b) if !b.is_empty() => b,
                Ok(_) => {
                    eprintln!("Failed to read media file (empty)");
                    return;
                }
                Err(e) => {
                    eprintln!("Failed to read media file: {e}");
                    return;
                }
            };
            let result: Result<(), String> = async {
                let client = ensure_client().await?;
                let handler = make_handler(&client).await?;

                let ui_for_progress = qt_thread.clone();
                let tmp_for_prog = tmp_id.clone();
                let cancel_for_prog = cancel_flag.clone();
                let uploaded_bytes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                let uploaded_for_prog = uploaded_bytes.clone();
                let on_progress = move |uploaded: usize, total: usize| {
                    uploaded_for_prog.store(uploaded, Ordering::Relaxed);
                    let prog = if total > 0 { uploaded as f32 / total as f32 } else { 0.0 };
                    let tmp = tmp_for_prog.clone();
                    let cancelled = cancel_for_prog.load(Ordering::Relaxed);
                    let _ = ui_for_progress.queue(move |mut qobject: Pin<&mut ffi::MessageList>| {
                        let mut rust = qobject.as_mut().rust_mut();
                        if let Some(pos) = rust.messages.iter().position(|m| m.message_id == tmp) {
                            rust.messages[pos].upload_progress = prog;
                            drop(rust);
                            let model_index = qobject.as_ref().index(pos as i32, 0, &QModelIndex::default());
                            qobject.as_mut().data_changed(&model_index, &model_index);
                        }
                    });
                    if cancelled {
                        // The cancellation is handled in the stream by checking the flag
                    }
                };

                let file_size = bytes.len();
                // Upload the media first
                let media = client.upload_media_with_progress(&bytes, &mime_type, &file_name, Some(on_progress), Some(cancel_flag.clone()))
                    .await.map_err(|e| {
                        let uploaded = uploaded_bytes.load(Ordering::Relaxed);
                        eprintln!("media upload failed after {:.2} MB of {:.2} MB ({:.1}%)",
                            uploaded as f64 / 1_048_576.0,
                            file_size as f64 / 1_048_576.0,
                            if file_size > 0 { uploaded as f64 / file_size as f64 * 100.0 } else { 0.0 });
                        e.to_string()
                    })?;

                // Build the send request with OUR tmp_id so the echo event matches
                let mut message_info = vec![libgmessages_rs::proto::conversations::MessageInfo {
                    action_message_id: None,
                    data: Some(
                        libgmessages_rs::proto::conversations::message_info::Data::MediaContent(media),
                    ),
                }];

                // Add text as additional MessageInfo if present
                let message_payload_content = if !caption.is_empty() {
                    message_info.push(libgmessages_rs::proto::conversations::MessageInfo {
                        action_message_id: None,
                        data: Some(
                            libgmessages_rs::proto::conversations::message_info::Data::MessageContent(
                                libgmessages_rs::proto::conversations::MessageContent {
                                    content: caption.clone(),
                                },
                            ),
                        ),
                    });
                    Some(libgmessages_rs::proto::client::MessagePayloadContent {
                        message_content: Some(
                            libgmessages_rs::proto::conversations::MessageContent {
                                content: caption.clone(),
                            },
                        ),
                    })
                } else {
                    None
                };

                let payload = libgmessages_rs::proto::client::MessagePayload {
                    tmp_id: tmp_id.clone(),
                    message_payload_content,
                    conversation_id: conversation_id.clone(),
                    participant_id: String::new(),
                    message_info,
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

                // send_rpc_request may fail with a decode error on the response
                // even though the message was actually sent successfully (Google uses
                // group-encoded protobuf fields that prost can't handle).
                match handler.send_request::<libgmessages_rs::proto::client::SendMessageResponse>(
                    libgmessages_rs::proto::rpc::ActionType::SendMessage,
                    libgmessages_rs::proto::rpc::MessageType::BugleMessage,
                    &request,
                ).await {
                    Ok(_) => {},
                    Err(e) => {
                        let err_str = e.to_string();
                        if err_str.contains("decode") || err_str.contains("end group") {
                            eprintln!("media send response decode error (likely delivered): {err_str}");
                        } else {
                            return Err(err_str);
                        }
                    }
                }

                Ok(())
            }
            .await;

            if let Err(error) = result {
                eprintln!("media send failed: {error}");
                let is_auth_error = error.contains("authentication credential")
                    || error.contains("401")
                    || error.contains("403");
                if is_auth_error {
                    clear_client().await;
                    let store = AuthDataStore::default_store();
                    let _ = store.delete();
                }

                // Check if this was a cancellation
                let was_cancelled = cancel_flag.load(Ordering::Relaxed);
                if was_cancelled {
                    eprintln!("media upload was cancelled by user for {}", tmp_id_for_fail);
                    // Clean up the cancellation token
                    let tmp_cleanup = tmp_id_for_fail.clone();
                    let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::MessageList>| {
                        qobject
                            .as_mut()
                            .rust_mut()
                            .upload_cancellations
                            .remove(&tmp_cleanup);
                    });
                    return; // Don't mark as failed, the message was already removed
                }

                // Clean up the cancellation token
                let tmp_cleanup = tmp_id_for_fail.clone();
                let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::MessageList>| {
                    qobject
                        .as_mut()
                        .rust_mut()
                        .upload_cancellations
                        .remove(&tmp_cleanup);
                });

                let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::MessageList>| {
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
                        qobject.as_mut().auth_error(&QString::from(error.as_str()));
                    }
                });
            } else {
                // Success: clean up the cancellation token
                let tmp_cleanup = tmp_id_for_fail;
                let _ = qt_thread.queue(move |mut qobject: Pin<&mut ffi::MessageList>| {
                    qobject
                        .as_mut()
                        .rust_mut()
                        .upload_cancellations
                        .remove(&tmp_cleanup);
                });
            }
        });
    }

    pub fn get_video_thumbnail(&self, file_url: &QString) -> QString {
        let file_path = file_url.to_string();
        let path = if file_path.starts_with("file://") {
            file_path[7..].to_string()
        } else {
            file_path
        };
        let p = std::path::Path::new(&path);
        let ret = crate::app_state::utils::generate_video_thumbnail(p).unwrap_or_default();
        QString::from(ret.as_str())
    }

    pub fn get_file_size(&self, file_url: &QString) -> i64 {
        let file_path = file_url.to_string();
        let path = if file_path.starts_with("file://") {
            file_path[7..].to_string()
        } else {
            file_path
        };
        std::fs::metadata(&path)
            .map(|m| m.len() as i64)
            .unwrap_or(-1)
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
        is_media: bool,
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
                // if status_code == 300 {
                //     self.as_mut().begin_remove_rows(&QModelIndex::default(), index as i32, index as i32);
                //     let mut rust = self.as_mut().rust_mut();
                //     rust.messages.remove(index);
                //     let convo_id = rust.selected_conversation_id.clone();
                //     let msgs_clone = rust.messages.clone();
                //     let me_id = rust.me_participant_id.clone();
                //     rust.cache.insert(convo_id, (msgs_clone, me_id));
                //     drop(rust);
                //     self.as_mut().end_remove_rows();
                //     return;
                // }

                let next_status = map_message_status(status_code, from_me);
                let mut rust = self.as_mut().rust_mut();
                if let Some(item) = rust.messages.get_mut(index) {
                    item.status = QString::from(next_status);
                    // Update the message_id if we had a tmp_id match
                    if !message_id.is_empty() && item.message_id != message_id {
                        item.message_id = message_id.to_string();
                    }
                }
                drop(rust);
                // Emit dataChanged for just this row
                let model_index = self
                    .as_ref()
                    .index(index as i32, 0, &QModelIndex::default());
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
            is_media,
            avatar_url: QString::from(""),
            is_info: status_code >= 200,
            participant_id: participant_id.clone(),
            mime_type: QString::from(""),
            thumbnail_url: QString::from(""),
            upload_progress: 1.0,
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

        let selected = self.rust().selected_conversation_id.clone();
        if !selected.is_empty() {
            let mut rust = self.as_mut().rust_mut();
            let msgs = rust.messages.clone();
            let me_id = rust.me_participant_id.clone();
            rust.cache.insert(selected, (msgs, me_id));
        }
    }

    pub fn queue_media_download(
        self: Pin<&mut Self>,
        message_id: &QString,
        media_id: &QString,
        decryption_key: &QString,
        mime_type: &QString,
    ) {
        let msg_id = message_id.to_string();
        let med_id = media_id.to_string();
        let key = decryption_key.to_string();
        let mime = mime_type.to_string();

        let ui_thread: CxxQtThread<ffi::MessageList> = self.qt_thread();

        spawn(async move {
            if let Some(client) = get_client().await {
                let key_bytes = STANDARD.decode(&key).unwrap_or_default();
                let safe_id = med_id
                    .replace("/", "_")
                    .replace("+", "_")
                    .replace("=", "")
                    .replace("-", "_");
                let safe_id = if safe_id.is_empty() {
                    msg_id.clone().replace("-", "_")
                } else {
                    safe_id
                };

                let ext = crate::app_state::utils::mime_to_extension(&mime);
                let tmp_dir = std::env::temp_dir().join("kourier_media");
                let path = tmp_dir.join(format!("{}.{}", safe_id, ext));

                let uri = if !path.exists() {
                    if let Ok(data) = client.download_media(&med_id, &key_bytes).await {
                        Some(crate::app_state::utils::media_data_to_uri(
                            &data, &mime, &safe_id,
                        ))
                    } else {
                        None
                    }
                } else {
                    Some(format!("file://{}", path.to_string_lossy()))
                };

                let thumb_uri = if path.exists() && mime.starts_with("video/") {
                    crate::app_state::utils::generate_video_thumbnail(&path)
                } else {
                    None
                };

                if let Some(uri) = uri {
                    let _ = ui_thread.queue(move |mut qobject: Pin<&mut ffi::MessageList>| {
                        let mut rust = qobject.as_mut().rust_mut();
                        if let Some(pos) = rust.messages.iter().position(|m| m.message_id == msg_id)
                        {
                            rust.messages[pos].media_url = QString::from(uri.as_str());
                            if let Some(t) = &thumb_uri {
                                rust.messages[pos].thumbnail_url = QString::from(t.as_str());
                            }
                            drop(rust);
                            let model_index =
                                qobject
                                    .as_ref()
                                    .index(pos as i32, 0, &QModelIndex::default());
                            qobject.as_mut().data_changed(&model_index, &model_index);

                            let mut rust = qobject.as_mut().rust_mut();
                            let selected = rust.selected_conversation_id.clone();
                            if !selected.is_empty() {
                                let msgs = rust.messages.clone();
                                let me_id = rust.me_participant_id.clone();
                                rust.cache.insert(selected, (msgs, me_id));
                            }
                        }
                    });
                }
            }
        });
    }

    pub fn delete_message(mut self: Pin<&mut Self>, message_id: &QString) {
        let msg_id = message_id.to_string();

        // Optimistic removal from the local list
        let pos = self
            .rust()
            .messages
            .iter()
            .position(|m| m.message_id == msg_id);

        if let Some(pos) = pos {
            let pos_i32 = pos as i32;
            self.as_mut()
                .begin_remove_rows(&QModelIndex::default(), pos_i32, pos_i32);
            self.as_mut().rust_mut().messages.remove(pos);
            self.as_mut().end_remove_rows();

            // Update cache
            let selected = self.rust().selected_conversation_id.clone();
            if !selected.is_empty() {
                let mut rust = self.as_mut().rust_mut();
                let msgs = rust.messages.clone();
                let me_id = rust.me_participant_id.clone();
                rust.cache.insert(selected, (msgs, me_id));
            }
        }

        // Send the delete request to the server asynchronously
        // If this is a temp message currently uploading, cancel the upload
        {
            let mut rust = self.as_mut().rust_mut();
            if let Some(cancel) = rust.upload_cancellations.remove(&msg_id) {
                cancel.store(true, Ordering::Relaxed);
                eprintln!("delete_message: cancelling active upload for {}", msg_id);
            }
        }

        spawn(async move {
            if let Some(client) = get_client().await {
                match client.delete_message(&msg_id).await {
                    Ok(_) => {
                        eprintln!("delete_message: deleted {}", msg_id);
                    }
                    Err(e) => {
                        eprintln!("delete_message: server error: {e}");
                    }
                }
            }
        });
    }
    pub fn save_media(self: Pin<&mut Self>, source_url: &QString, mime_type: &QString) -> QString {
        let url = source_url.to_string();
        let mime = mime_type.to_string();

        // Read the source data — either from a data: URI or a file:// path
        let data_bytes = if let Some(rest) = url.strip_prefix("data:") {
            // data:<mime>;base64,<data>
            rest.find(";base64,").and_then(|pos| {
                let b64 = &rest[pos + 8..];
                STANDARD.decode(b64).ok()
            })
        } else if let Some(path) = url.strip_prefix("file://") {
            // file:///path/to/file
            std::fs::read(path).ok()
        } else {
            None
        };

        let Some(data_bytes) = data_bytes else {
            eprintln!(
                "save_media: could not read media from: {}",
                &url[..url.len().min(80)]
            );
            return QString::from("");
        };

        let ext = crate::app_state::utils::mime_to_extension(&mime);

        // Get the Downloads directory
        let downloads_dir = dirs::download_dir().unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join("Downloads")
        });

        // Generate a unique filename
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let short_id = &Uuid::new_v4().to_string()[..8];
        let filename = format!("gmessages_{timestamp}_{short_id}.{ext}");
        let full_path = downloads_dir.join(&filename);

        // Ensure downloads directory exists
        if let Err(e) = std::fs::create_dir_all(&downloads_dir) {
            eprintln!("save_media: failed to create downloads dir: {e}");
            return QString::from("");
        }

        match std::fs::write(&full_path, &data_bytes) {
            Ok(()) => {
                let path_str = full_path.to_string_lossy().to_string();
                eprintln!("save_media: saved to {path_str}");
                QString::from(path_str.as_str())
            }
            Err(e) => {
                eprintln!("save_media: write failed: {e}");
                QString::from("")
            }
        }
    }
}
