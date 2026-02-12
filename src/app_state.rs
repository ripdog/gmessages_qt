use crate::ffi;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use core::pin::Pin;
use cxx_qt::{CxxQtThread, Threading};
use cxx_qt_lib::QString;
use libgmessages_rs::{
    auth::AuthData,
    gmclient::GMClient,
    store::AuthDataStore,
};
use qrcode::render::svg;
use qrcode::QrCode;
use std::time::Duration;

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
                let auth = store
                    .load()
                    .map_err(|error| error.to_string())?
                    .unwrap_or(AuthData::new().map_err(|error| error.to_string())?);

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
                    .wait_for_qr_pairing_on_stream(stream, Duration::from_secs(300))
                    .await
                    .map_err(|error| error.to_string())?;

                match paired {
                    Some(_) => {
                        let auth_handle = client.auth();
                        let auth = auth_handle.lock().await;
                        store
                            .save(&auth)
                            .map_err(|error| error.to_string())?;
                        Ok(true)
                    }
                    None => Ok(false),
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
}
