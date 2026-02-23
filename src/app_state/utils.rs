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
        2 => "received", // OUTGOING_DELIVERED
        11 => "read",    // OUTGOING_DISPLAYED
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
    let tmp_dir = std::env::temp_dir().join("kourier_media");
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

pub fn generate_video_thumbnail(video_path: &std::path::Path) -> Option<String> {
    let thumb_path = video_path.with_extension("thumb.jpg");
    if thumb_path.exists() {
        return Some(format!("file://{}", thumb_path.to_string_lossy()));
    }

    let output = std::process::Command::new("ffmpeg")
        .args(&[
            "-y",
            "-i",
            &video_path.to_string_lossy().into_owned(),
            "-vframes",
            "1",
            "-q:v",
            "2",
            &thumb_path.to_string_lossy().into_owned(),
        ])
        .output()
        .ok()?;

    if output.status.success() {
        Some(format!("file://{}", thumb_path.to_string_lossy()))
    } else {
        None
    }
}

/// Extract the first HTTP(S) URL from a text string.
pub fn extract_first_url(text: &str) -> Option<String> {
    // Simple but effective URL extraction — looks for http:// or https:// followed by
    // non-whitespace characters, trimming trailing punctuation that's likely not part of the URL.
    for word in text.split_whitespace() {
        if word.starts_with("http://") || word.starts_with("https://") {
            let url = word.trim_end_matches(|c: char| {
                c == '.' || c == ',' || c == ')' || c == ']' || c == ';' || c == '!' || c == '?'
            });
            if url.len() > 10 {
                return Some(url.to_string());
            }
        }
    }
    None
}

/// OpenGraph metadata extracted from a web page.
pub struct OgMetadata {
    pub title: String,
    pub image: String,
    pub url: String,
}

/// Fetch OpenGraph metadata (title, image) from a URL.
/// Returns None on any failure (network, parsing, timeout).
pub async fn fetch_og_metadata(url: &str) -> Option<OgMetadata> {
    eprintln!("link_preview: fetching OG metadata for {url}");

    let (fetch_url, user_agent) = if url.starts_with("https://twitter.com/") {
        (
            url.replace("https://twitter.com/", "https://x.com/"),
            "Mozilla/5.0 (compatible; TelegramBot/1.0; +https://core.telegram.org/bots)",
        )
    } else if url.starts_with("https://x.com/") {
        (
            url.to_string(),
            "Mozilla/5.0 (compatible; TelegramBot/1.0; +https://core.telegram.org/bots)",
        )
    } else if url.starts_with("https://www.twitter.com/") {
        (
            url.replace("https://www.twitter.com/", "https://x.com/"),
            "Mozilla/5.0 (compatible; TelegramBot/1.0; +https://core.telegram.org/bots)",
        )
    } else if url.starts_with("https://www.x.com/") {
        (
            url.replace("https://www.x.com/", "https://x.com/"),
            "Mozilla/5.0 (compatible; TelegramBot/1.0; +https://core.telegram.org/bots)",
        )
    } else {
        (
            url.to_string(),
            "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
        )
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent(user_agent)
        .build()
        .ok()?;

    let response = client
        .get(&fetch_url)
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .header("Accept-Language", "en-US,en;q=0.5")
        .send()
        .await;

    let response = match response {
        Ok(r) => r,
        Err(e) => {
            eprintln!("link_preview: request failed for {url}: {e}");
            return None;
        }
    };

    let status = response.status();
    if !status.is_success() {
        eprintln!("link_preview: non-success status {status} for {url}");
        return None;
    }

    // Only parse HTML content
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if !content_type.contains("text/html") && !content_type.contains("application/xhtml") {
        eprintln!("link_preview: non-HTML content-type '{content_type}' for {url}");
        return None;
    }

    // Limit body to 512KB to avoid downloading huge pages
    let body = match response.text().await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("link_preview: failed to read body for {url}: {e}");
            return None;
        }
    };
    let body = if body.len() > 512 * 1024 {
        &body[..512 * 1024]
    } else {
        &body
    };

    let document = scraper::Html::parse_document(body);
    let meta_selector = scraper::Selector::parse("meta").ok()?;

    let mut og_title = String::new();
    let mut og_image = String::new();
    let mut og_url = url.to_string();
    let mut html_title = String::new();
    let mut twitter_title = String::new();
    let mut twitter_image = String::new();

    // Also grab the <title> tag as fallback
    if let Ok(title_sel) = scraper::Selector::parse("title") {
        if let Some(title_el) = document.select(&title_sel).next() {
            html_title = title_el.text().collect::<String>().trim().to_string();
        }
    }

    for element in document.select(&meta_selector) {
        let property = element.value().attr("property").unwrap_or("");
        let name = element.value().attr("name").unwrap_or("");
        let content = element.value().attr("content").unwrap_or("");
        if content.is_empty() {
            continue;
        }

        // OG tags (property attribute)
        match property {
            "og:title" => og_title = content.to_string(),
            "og:image" => og_image = content.to_string(),
            "og:url" => og_url = content.to_string(),
            _ => {}
        }

        // Twitter card tags (name attribute) as fallback
        match name {
            "twitter:title" => twitter_title = content.to_string(),
            "twitter:image" | "twitter:image:src" => twitter_image = content.to_string(),
            _ => {}
        }
    }

    // Use twitter: tags as fallback for og: tags
    if og_title.is_empty() && !twitter_title.is_empty() {
        og_title = twitter_title;
    }
    if og_image.is_empty() && !twitter_image.is_empty() {
        og_image = twitter_image;
    }

    // Use HTML title as final fallback
    if og_title.is_empty() {
        og_title = html_title;
    }

    // Must have at least a title to be useful
    if og_title.is_empty() {
        eprintln!("link_preview: no title found for {url}");
        return None;
    }

    eprintln!(
        "link_preview: found title='{}', image='{}' for {url}",
        og_title,
        if og_image.is_empty() {
            "(none)"
        } else {
            &og_image
        }
    );

    Some(OgMetadata {
        title: og_title,
        image: og_image,
        url: og_url,
    })
}
pub fn cleanup_old_cache_files() {
    let dirs_to_clean = vec![
        std::env::temp_dir().join("kourier_media"),
        std::env::temp_dir().join("kourier_link_previews"),
    ];
    let max_age = std::time::Duration::from_secs(7 * 24 * 60 * 60); // 7 days
    let now = std::time::SystemTime::now();

    for dir in dirs_to_clean {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(age) = now.duration_since(modified) {
                            if age > max_age {
                                let _ = std::fs::remove_file(entry.path());
                            }
                        }
                    }
                }
            }
        }
    }
}
