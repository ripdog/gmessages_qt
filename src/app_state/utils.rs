use crate::app_state::conversation_list::ConversationItem;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::{Datelike, Local, Timelike};

use qrcode::render::svg;
use qrcode::QrCode;


// ── QR code helper ───────────────────────────────────────────────

pub fn qr_to_svg_data_url(data: &str) -> Result<String, Box<dyn std::error::Error>> {
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

// ── Free functions ───────────────────────────────────────────────

pub fn filter_items(items: &[ConversationItem], filter_text: &str) -> Vec<ConversationItem> {
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

pub fn format_human_timestamp(timestamp_micros: i64) -> String {
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

pub fn format_human_message_time(timestamp_micros: i64) -> String {
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

pub fn format_section_date(timestamp_micros: i64) -> String {
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

pub fn build_preview(convo: &libgmessages_rs::proto::conversations::Conversation) -> String {
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

pub fn detect_extension(bytes: &[u8]) -> &'static str {
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

pub fn map_message_status(status_code: i32, from_me: bool) -> &'static str {
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

pub fn mime_to_extension(mime: &str) -> &str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/3gpp" => "3gp",
        "video/3gpp2" => "3g2",
        _ => "bin",
    }
}

/// Convert downloaded media bytes into a URI suitable for QML.
/// Images → data: URI (works natively with QML Image).
/// Videos → file: URI (Qt MediaPlayer needs a real file).
pub fn media_data_to_uri(data: &[u8], mime: &str, file_name: &str) -> String {
    let ext = mime_to_extension(mime);
    let tmp_dir = std::env::temp_dir().join("gmessages_media");
    let _ = std::fs::create_dir_all(&tmp_dir);
    let path = tmp_dir.join(format!("{}.{}", file_name, ext));
    
    // Always write to temp file for disk caching
    match std::fs::write(&path, data) {
        Ok(()) => format!("file://{}", path.to_string_lossy()),
        Err(e) => {
            eprintln!("media_data_to_uri: failed to write temp file: {e}");
            // Fallback to data URI
            let b64 = base64::engine::general_purpose::STANDARD.encode(data);
            format!("data:{};base64,{}", mime, b64)
        }
    }
}
