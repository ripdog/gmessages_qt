use crate::ffi;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use core::pin::Pin;
use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::QString;

use libgmessages_rs::store::AuthDataStore;
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

use crate::ffi::QHash_i32_QByteArray;
use crate::ffi::QModelIndex;
use crate::ffi::QVariant;

use super::*;
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
}

pub struct MessageListRust {
    pub loading: bool,
    messages: Vec<MessageItem>,
    selected_conversation_id: String,
    me_participant_id: String,
    cache: HashMap<String, (Vec<MessageItem>, String)>,
}

impl Default for MessageListRust {
    fn default() -> Self {
        Self {
            loading: false,
            messages: Vec::new(),
            selected_conversation_id: String::new(),
            me_participant_id: String::new(),
            cache: HashMap::new(),
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
            10 => QVariant::from(&item.avatar_url),
            11 => QVariant::from(&item.is_info),
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

                let mut me_participant_id = String::new();
                let mut avatar_identifiers: Vec<(String, String)> = Vec::new();
                
                if let Some(convo) = convo_response.conversation.as_ref() {
                    for p in &convo.participants {
                        let pid = p.id.as_ref().map(|id| id.participant_id.clone()).unwrap_or_default();
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
                if !avatar_identifiers.is_empty() {
                    let request = libgmessages_rs::proto::client::GetThumbnailRequest {
                        identifiers: avatar_identifiers.iter().map(|(_, id)| id.clone()).collect(),
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
                            handler.send_request(libgmessages_rs::proto::rpc::ActionType::GetContactsThumbnail, message_type, &request).await
                        } else {
                            handler.send_request_dont_encrypt(libgmessages_rs::proto::rpc::ActionType::GetContactsThumbnail, message_type, &request, Duration::from_secs(5)).await
                        };
                        if let Ok(response) = attempt {
                            if !response.thumbnail.is_empty() {
                                for thumb in response.thumbnail {
                                    if let Some(data) = thumb.data.as_ref() {
                                        if !data.image_buffer.is_empty() {
                                            let ext = detect_extension(&data.image_buffer);
                                            let encoded = STANDARD.encode(&data.image_buffer);
                                            let url = format!("data:image/{ext};base64,{encoded}");
                                            
                                            if let Some((pid, _)) = avatar_identifiers.iter().find(|(_, id)| id == &thumb.identifier) {
                                                avatar_by_participant_id.insert(pid.clone(), url);
                                            }
                                        }
                                    }
                                }
                                break;
                            }
                        }
                    }
                }

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

                        let mut avatar_url = String::new();
                        if !from_me && !message.participant_id.is_empty() {
                            if let Some(url) = avatar_by_participant_id.get(&message.participant_id) {
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
                Ok((new_messages, me_participant_id, media_downloads)) => {
                    let convo_id = conversation_id.clone();
                    let _ = qt_thread.queue(
                        move |mut qobject: Pin<&mut ffi::MessageList>| {
                            let mut rust = qobject.as_mut().rust_mut();
                            if rust.selected_conversation_id == convo_id {
                                if rust.messages.is_empty() {
                                    rust.messages = new_messages.clone();
                                    rust.me_participant_id = me_participant_id.clone();
                                    rust.cache.insert(convo_id.clone(), (new_messages, me_participant_id));
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
                                        if !new_messages.iter().any(|m| m.message_id == msg.message_id) {
                                            if msg.status.to_string() != "sending" && msg.status.to_string() != "failed" {
                                                to_remove.push(i);
                                            }
                                        }
                                    }
                                    drop(rust);
                                    for i in to_remove.into_iter().rev() {
                                        qobject.as_mut().begin_remove_rows(&QModelIndex::default(), i as i32, i as i32);
                                        qobject.as_mut().rust_mut().messages.remove(i);
                                        qobject.as_mut().end_remove_rows();
                                    }

                                    // 2. Diff updates and inserts
                                    for new_msg in &new_messages {
                                        let mut rust = qobject.as_mut().rust_mut();
                                        if let Some(pos) = rust.messages.iter().position(|m| m.message_id == new_msg.message_id) {
                                            // Update
                                            let mut changed = false;
                                            if rust.messages[pos].status.to_string() != new_msg.status.to_string() {
                                                rust.messages[pos].status = QString::from(new_msg.status.to_string().as_str());
                                                changed = true;
                                            }
                                            if rust.messages[pos].body.to_string() != new_msg.body.to_string() {
                                                rust.messages[pos].body = QString::from(new_msg.body.to_string().as_str());
                                                changed = true;
                                            }
                                            drop(rust);
                                            if changed {
                                                let model_index = qobject.as_ref().index(pos as i32, 0, &QModelIndex::default());
                                                qobject.as_mut().data_changed(&model_index, &model_index);
                                            }
                                        } else {
                                            // Insert
                                            let pos = rust.messages.partition_point(|item| item.timestamp_micros <= new_msg.timestamp_micros);
                                            drop(rust);
                                            qobject.as_mut().begin_insert_rows(&QModelIndex::default(), pos as i32, pos as i32);
                                            let mut rust = qobject.as_mut().rust_mut();
                                            rust.messages.insert(pos, new_msg.clone());
                                            drop(rust);
                                            qobject.as_mut().end_insert_rows();
                                        }
                                    }

                                    let mut rust = qobject.as_mut().rust_mut();
                                    rust.me_participant_id = me_participant_id.clone();
                                    let msgs_clone = rust.messages.clone();
                                    rust.cache.insert(convo_id.clone(), (msgs_clone, me_participant_id));
                                }
                            } else {
                                rust.cache.insert(convo_id.clone(), (new_messages, me_participant_id));
                            }
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
        let insert_pos = self.rust().messages.len() as i32;
        self.as_mut().begin_insert_rows(&QModelIndex::default(), insert_pos, insert_pos);
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
            avatar_url: QString::from(""),
            is_info: false,
        });
        // We do not sort here because the new message naturally belongs at the end.
        // It prevents scroll position reset issues.
        drop(rust);
        self.as_mut().end_insert_rows();

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
            is_media,
            avatar_url: QString::from(""),
            is_info: status_code >= 200,
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
                if let Ok(data) = client.download_media(&med_id, &key_bytes).await {
                    let b64 = STANDARD.encode(&data);
                    let uri = format!("data:{};base64,{}", mime, b64);
                    let _ = ui_thread.queue(move |mut qobject: Pin<&mut ffi::MessageList>| {
                        let mut rust = qobject.as_mut().rust_mut();
                        if let Some(pos) = rust.messages.iter().position(|m| m.message_id == msg_id) {
                            rust.messages[pos].media_url = QString::from(uri.as_str());
                            drop(rust);
                            let model_index = qobject.as_ref().index(pos as i32, 0, &QModelIndex::default());
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
}

