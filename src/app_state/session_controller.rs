use crate::ffi;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use core::pin::Pin;
use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::QString;
use futures_util::StreamExt;
use std::sync::{atomic::AtomicBool, Arc};
use std::time::Duration;

use super::*;
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

                    // Proactively refresh the tachyon auth token before
                    // starting a new long-poll stream (mirrors what
                    // mautrix-gmessages does to keep sessions alive).
                    match client.refresh_token_if_needed().await {
                        Ok(true) => {
                            // Token was refreshed — persist the new auth
                            // data to disk so the session survives restarts.
                            let store = libgmessages_rs::store::AuthDataStore::default_store();
                            let auth_handle = client.auth();
                            let auth = auth_handle.lock().await;
                            if let Err(e) = store.save(&auth) {
                                eprintln!("failed to save refreshed auth: {e}");
                            }
                        }
                        Ok(false) => { /* token still valid */ }
                        Err(e) => {
                            eprintln!("token refresh failed: {e}");
                            // Continue anyway — the existing token may
                            // still be valid for a while.
                        }
                    }

                    let stream = client
                        .start_long_poll_stream()
                        .await
                        .map_err(|e| e.to_string())?;

                    // Fire-and-forget: tell the server this stream is active.
                    // This MUST happen before any send_request calls (like
                    // ListConversations) because the server won't route RPC
                    // responses down the stream until it has seen GetUpdates.
                    let gu_client = client.clone();
                    let gu_session_id = handler.session_id().to_string();
                    tokio::spawn(async move {
                        let _ = gu_client
                            .send_rpc_message_with_id_and_session_no_payload(
                                libgmessages_rs::proto::rpc::ActionType::GetUpdates,
                                libgmessages_rs::proto::rpc::MessageType::BugleMessage,
                                &gu_session_id,
                                &gu_session_id,
                                true,
                            )
                            .await;
                    });

                    let _ =
                        session_thread.queue(|mut qobject: Pin<&mut ffi::SessionController>| {
                            qobject.as_mut().set_status(QString::from("Connected"));
                            qobject.as_mut().session_started();
                        });

                    let inner_result =
                        run_long_poll_loop(stream, &handler, &stop_flag, &session_thread).await;

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
                        qobject.as_mut().set_status(QString::from("Session ended"));
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

    pub fn fetch_updates(self: Pin<&mut Self>) {
        if !*self.running() {
            return;
        }
        spawn(async move {
            let client = match ensure_client().await {
                Ok(c) => c,
                Err(_) => return,
            };
            let handler = match make_handler(&client).await {
                Ok(h) => h,
                Err(_) => return,
            };
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
        });
    }
}

/// Why the long-poll inner loop ended.
pub(crate) enum StreamEndReason {
    Stopped,
    StreamEnded,
}

/// The inner long-poll processing loop.  Returns when the stream ends or stop
/// is requested.
pub async fn run_long_poll_loop(
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
            tokio::time::sleep(Duration::from_secs(30)).await;
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

    let mut settled = false;
    let settle_duration = Duration::from_secs(3);
    let settle_timer = tokio::time::sleep(settle_duration);
    tokio::pin!(settle_timer);

    let result = loop {
        if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
            break Ok(StreamEndReason::Stopped);
        }

        let item = tokio::select! {
            item = stream.next() => {
                // Reset the settle timer every time we get data
                if !settled {
                    settle_timer.as_mut().reset(tokio::time::Instant::now() + settle_duration);
                }
                item
            }
            _ = &mut settle_timer, if !settled => {
                // No events for 3 seconds — the initial burst is done
                settled = true;
                eprintln!("=== UPDATES SETTLED (3s of quiet) ===");
                let _ = qt_thread.queue(|mut qobject: Pin<&mut ffi::SessionController>| {
                    qobject.as_mut().updates_settled();
                });
                continue;
            }
        };

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
            // Only skip old payloads if there are no pending RPC requests.
            // If a request is pending (e.g. ListConversations), its response
            // might arrive during the skip window — we must process it.
            let pending_len = handler.pending_len().await;
            if pending_len == 0 {
                skip_count -= 1;
                continue;
            }
        }
        let _ = handler.process_payload(data).await;

        if data.bugle_route != libgmessages_rs::proto::rpc::BugleRoute::DataEvent as i32 {
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
            libgmessages_rs::proto::events::update_events::Event::MessageEvent(message_event) => {
                for message in message_event.data {
                    let body = extract_message_body(&message);

                    eprintln!("\n=== NEW MESSAGE RECEIVED ===");
                    eprintln!("Body empty?: {}", body.trim().is_empty());
                    eprintln!("Timestamp parsed: {}", message.timestamp);
                    eprintln!("{:#?}", message);
                    eprintln!("============================\n");

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

                    let media = extract_message_media(&message);
                    let is_media = media.is_some();

                    let (media_id, decryption_key, mime_type, media_width, media_height) =
                        if let Some(m) = &media {
                            (m.0.clone(), STANDARD.encode(&m.1), m.2.clone(), m.3, m.4)
                        } else {
                            (String::new(), String::new(), String::new(), 0, 0)
                        };

                    let _ =
                        qt_thread.queue(move |mut qobject: Pin<&mut ffi::SessionController>| {
                            qobject.as_mut().message_received(
                                &QString::from(conversation_id.as_str()),
                                &QString::from(participant_id.as_str()),
                                &QString::from(body.as_str()),
                                transport_type,
                                &QString::from(message_id.as_str()),
                                &QString::from(tmp_id.as_str()),
                                timestamp_micros,
                                status_code,
                                is_media,
                                &QString::from(media_id.as_str()),
                                &QString::from(decryption_key.as_str()),
                                &QString::from(mime_type.as_str()),
                                media_width,
                                media_height,
                            );
                        });

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
                    let status = convo.status as i32;
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

                    let _ =
                        qt_thread.queue(move |mut qobject: Pin<&mut ffi::SessionController>| {
                            qobject.as_mut().conversation_updated(
                                &QString::from(conversation_id.as_str()),
                                &QString::from(name.as_str()),
                                &QString::from(preview.as_str()),
                                unread,
                                last_message_timestamp,
                                is_group_chat,
                                status,
                                &QString::from(avatar_identifier.as_str()),
                            );
                        });
                }
            }
            _ => {} // Ignore typing events, settings, etc. for now
        }
    };

    heartbeat.abort();
    result
}

/// Extract text body from a Message, returning empty string if none.
pub fn extract_message_body(message: &libgmessages_rs::proto::conversations::Message) -> String {
    message
        .message_info
        .iter()
        .find_map(|info| match &info.data {
            Some(libgmessages_rs::proto::conversations::message_info::Data::MessageContent(
                content,
            )) => {
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
pub fn extract_message_id(message: &libgmessages_rs::proto::conversations::Message) -> String {
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
pub fn extract_message_media(
    message: &libgmessages_rs::proto::conversations::Message,
) -> Option<(String, Vec<u8>, String, i64, i64)> {
    message
        .message_info
        .iter()
        .find_map(|info| match &info.data {
            Some(libgmessages_rs::proto::conversations::message_info::Data::MediaContent(
                media,
            )) => {
                let id = if !media.media_id.is_empty() {
                    media.media_id.clone()
                } else {
                    media.thumbnail_media_id.clone()
                };
                let key = if !media.decryption_key.is_empty() {
                    media.decryption_key.clone()
                } else {
                    media.thumbnail_decryption_key.clone()
                };
                let (width, height) = if let Some(dim) = &media.dimensions {
                    (dim.width, dim.height)
                } else {
                    (0, 0)
                };
                Some((id, key, media.mime_type.clone(), width, height))
            }
            _ => None,
        })
}
