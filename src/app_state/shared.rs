use libgmessages_rs::{gmclient::GMClient, store::AuthDataStore};
use std::sync::OnceLock;
use base64::Engine;


// ── Shared session infrastructure ────────────────────────────────

/// A single shared tokio runtime + optional GMClient, used by all QObjects.
/// Created once at app startup and never destroyed.
pub struct SharedSession {
    runtime: tokio::runtime::Runtime,
    client: tokio::sync::RwLock<Option<GMClient>>,
    pub avatars: tokio::sync::RwLock<std::collections::HashMap<String, String>>,
    pub handler: tokio::sync::RwLock<Option<libgmessages_rs::gmclient::SessionHandler>>,
}

/// Global singleton.
pub fn shared() -> &'static SharedSession {
    static INSTANCE: OnceLock<SharedSession> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime");
        SharedSession {
            runtime,
            client: tokio::sync::RwLock::new(None),
            avatars: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            handler: tokio::sync::RwLock::new(None),
        }
    })
}

/// Spawn a future on the shared runtime (non-blocking).
pub fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    shared().runtime.spawn(future)
}

/// Get a clone of the current GMClient, or None if not logged in.
pub async fn get_client() -> Option<GMClient> {
    shared().client.read().await.clone()
}

/// Store a new GMClient after login.
pub async fn set_client(client: GMClient) {
    *shared().client.write().await = Some(client);
}

/// Clear the stored client (logout / auth error).
pub async fn clear_client() {
    *shared().client.write().await = None;
    *shared().handler.write().await = None;
}

/// Helper: load auth from disk and create+store a GMClient.
/// Returns the client or an error string.
pub async fn ensure_client() -> Result<GMClient, String> {
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
pub async fn make_handler(client: &GMClient) -> Result<libgmessages_rs::gmclient::SessionHandler, String> {
    let mut guard = shared().handler.write().await;
    if let Some(h) = guard.as_ref() {
        return Ok(h.clone());
    }
    
    let mut handler = libgmessages_rs::gmclient::SessionHandler::new(client.clone());
    let auth_handle = client.auth();
    let auth_session = {
        let auth = auth_handle.lock().await;
        auth.session_id().to_string().to_lowercase()
    };
    handler.set_session_id(auth_session).await;
    
    *guard = Some(handler.clone());
    Ok(handler)
}



/// Helper: fetch avatars asynchronously and cache them.
pub async fn fetch_avatars_async(
    client: GMClient,
    identifiers: Vec<String>,
) -> std::collections::HashMap<String, String> {
    let mut results = std::collections::HashMap::new();
    let mut to_fetch = Vec::new();
    {
        let cache = shared().avatars.read().await;
        for id in &identifiers {
            if let Some(url) = cache.get(id) {
                results.insert(id.clone(), url.clone());
            } else {
                if !to_fetch.contains(id) {
                    to_fetch.push(id.clone());
                }
            }
        }
    }
    if to_fetch.is_empty() {
        return results;
    }

    if let Ok(handler) = make_handler(&client).await {
        let request = libgmessages_rs::proto::client::GetThumbnailRequest {
            identifiers: to_fetch.clone(),
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
                handler.send_request_dont_encrypt(libgmessages_rs::proto::rpc::ActionType::GetContactsThumbnail, message_type, &request, std::time::Duration::from_secs(5)).await
            };
            if let Ok(response) = attempt {
                if !response.thumbnail.is_empty() {
                    let mut cache = shared().avatars.write().await;
                    for thumb in response.thumbnail {
                        if let Some(data) = thumb.data.as_ref() {
                            if !data.image_buffer.is_empty() {
                                let ext = crate::app_state::utils::detect_extension(&data.image_buffer);
                                
                                let safe_id = thumb.identifier.replace("/", "_").replace("+", "_").replace("=", "").replace("-", "_");
                                let safe_id = if safe_id.is_empty() { "unknown".to_string() } else { safe_id };
                                
                                let tmp_dir = std::env::temp_dir().join("kourier_avatars");
                                let _ = std::fs::create_dir_all(&tmp_dir);
                                let path = tmp_dir.join(format!("{}.{}", safe_id, ext));
                                
                                let _ = std::fs::write(&path, &data.image_buffer);
                                let url = format!("file://{}", path.to_string_lossy());
                                
                                cache.insert(thumb.identifier.clone(), url.clone());
                                results.insert(thumb.identifier.clone(), url);
                            }
                        }
                    }
                    break;
                }
            }
        }
    }

    results
}
