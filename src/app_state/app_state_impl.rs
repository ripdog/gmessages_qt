use crate::ffi;
use core::pin::Pin;
use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::QString;
use libgmessages_rs::{auth::AuthData, gmclient::GMClient, store::AuthDataStore};
use std::sync::{atomic::AtomicBool, Arc};
use std::time::Duration;


use super::*;
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
                        qobject.as_mut().initialized(true);
                    });
                }
                Ok(None) => {
                    let _ = qt_thread.queue(|mut qobject: Pin<&mut ffi::AppState>| {
                        qobject.as_mut().set_logged_in(false);
                        qobject.as_mut().set_login_in_progress(false);
                        qobject
                            .as_mut()
                            .set_status_message(QString::from("Not logged in"));
                        qobject.as_mut().initialized(false);
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
                        qobject.as_mut().initialized(false);
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

