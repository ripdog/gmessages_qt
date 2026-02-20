use libgmessages_rs::{gmclient::GMClient, store::AuthDataStore};
use std::sync::OnceLock;


// ── Shared session infrastructure ────────────────────────────────

/// A single shared tokio runtime + optional GMClient, used by all QObjects.
/// Created once at app startup and never destroyed.
pub struct SharedSession {
    runtime: tokio::runtime::Runtime,
    client: tokio::sync::RwLock<Option<GMClient>>,
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
pub async fn start_handler_loop(
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

