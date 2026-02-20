use crate::ffi;
use core::pin::Pin;
use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::QString;

use std::collections::HashMap;

use crate::ffi::QHash_i32_QByteArray;
use crate::ffi::QModelIndex;
use crate::ffi::QVariant;

use super::*;
use crate::app_state::shared::fetch_avatars_async;
// ── ConversationList ─────────────────────────────────────────────

#[derive(Clone)]
pub struct ConversationItem {
    pub name: QString,
    pub preview: QString,
    pub avatar_url: QString,
    pub avatar_identifier: String,
    pub is_group_chat: bool,
    pub unread: bool,
    pub conversation_id: String,
    pub me_participant_id: String,
    pub last_message_timestamp: i64,
    pub last_message_time: QString,
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

                // Collect avatar identifiers and populate from cache if available
                let mut avatar_identifiers: Vec<String> = Vec::new();
                {
                    let cache = shared().avatars.read().await;
                    for item in &mut items {
                        if item.avatar_identifier.is_empty() {
                            continue;
                        }
                        if let Some(url) = cache.get(&item.avatar_identifier) {
                            item.avatar_url = QString::from(url.as_str());
                        } else {
                            if !avatar_identifiers.contains(&item.avatar_identifier) {
                                avatar_identifiers.push(item.avatar_identifier.clone());
                            }
                        }
                    }
                }

                // Push items to UI immediately
                let _ = ui_thread.queue(move |mut qobject: Pin<&mut ffi::ConversationList>| {
                    qobject.as_mut().begin_reset_model();
                    let mut rust = qobject.as_mut().rust_mut();
                    rust.avatar_by_identifier.clear();
                    rust.all_items = items;
                    rust.filtered_items = filter_items(&rust.all_items, &rust.filter_text);
                    qobject.as_mut().set_loading(false);
                    qobject.as_mut().end_reset_model();
                });

                // Fetch missing avatars in background
                if !avatar_identifiers.is_empty() {
                    let client_for_avatars = client.clone();
                    let ui_for_avatars = ui_thread.clone();
                    spawn(async move {
                        let new_avatars = fetch_avatars_async(client_for_avatars, avatar_identifiers).await;
                        if !new_avatars.is_empty() {
                            let _ = ui_for_avatars.queue(move |mut qobject: Pin<&mut ffi::ConversationList>| {
                                qobject.as_mut().begin_reset_model();
                                let mut rust = qobject.as_mut().rust_mut();
                                for (id, url) in &new_avatars {
                                    rust.avatar_by_identifier.insert(id.clone(), url.clone());
                                }
                                for item in &mut rust.all_items {
                                    if let Some(url) = new_avatars.get(&item.avatar_identifier) {
                                        if !url.is_empty() {
                                            item.avatar_url = QString::from(url.as_str());
                                        }
                                    }
                                }
                                rust.filtered_items = filter_items(&rust.all_items, &rust.filter_text);
                                qobject.as_mut().end_reset_model();
                            });
                        }
                    });
                }

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
            self.as_mut().begin_reset_model();
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
            drop(rust);
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
            drop(rust);
            self.as_mut().end_reset_model();
        }
    }

    pub fn mark_conversation_read(mut self: Pin<&mut Self>, conversation_id: &QString) {
        let convo_id = conversation_id.to_string();
        if let Some(pos) = self.rust().filtered_items.iter().position(|item| item.conversation_id == convo_id) {
            if self.rust().filtered_items[pos].unread {
                let all_pos = self.rust().all_items.iter().position(|item| item.conversation_id == convo_id).unwrap();
                let mut rust = self.as_mut().rust_mut();
                rust.all_items[all_pos].unread = false;
                rust.filtered_items[pos].unread = false;
                drop(rust);
                let model_index = self.as_ref().index(pos as i32, 0, &QModelIndex::default());
                self.as_mut().data_changed(&model_index, &model_index);
            }
        }
    }

    pub fn update_preview(
        mut self: Pin<&mut Self>,
        conversation_id: &QString,
        preview: &QString,
        timestamp_micros: i64,
    ) {
        let convo_id = conversation_id.to_string();
        let preview_str = preview.to_string();
        
        let mut rust = self.as_mut().rust_mut();
        if let Some(pos) = rust.all_items.iter().position(|i| i.conversation_id == convo_id) {
            if timestamp_micros >= rust.all_items[pos].last_message_timestamp {
                rust.all_items[pos].preview = QString::from(preview_str);
                rust.all_items[pos].last_message_timestamp = timestamp_micros;
                rust.all_items[pos].last_message_time = QString::from(format_human_timestamp(timestamp_micros));
                
                rust.all_items.sort_by(|a, b| b.last_message_timestamp.cmp(&a.last_message_timestamp));
                rust.filtered_items = filter_items(&rust.all_items, &rust.filter_text);
                drop(rust);
                self.as_mut().begin_reset_model();
                self.as_mut().end_reset_model();
            }
        }
    }
}

/// Convert a proto Conversation to a ConversationItem.
pub fn conversation_to_item(
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

