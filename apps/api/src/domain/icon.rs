use axum::http::HeaderMap;

/// Icon bytes fetched from the app's own site.
pub struct FetchedIcon {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}

/// Fetches `<origin>/favicon.ico` for an app URL.
pub async fn fetch_favicon(app_url: &str) -> Result<FetchedIcon, String> {
    let parsed = reqwest::Url::parse(app_url).map_err(|_| "The app URL is not valid.".to_string())?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "The app URL has no host.".to_string())?;

    let mut origin = format!("{}://{}", parsed.scheme(), host);
    if let Some(port) = parsed.port() {
        origin.push_str(&format!(":{port}"));
    }
    let candidate = format!("{origin}/favicon.ico");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .user_agent("AppDirectory/0.1 (favicon fetcher)")
        .build()
        .map_err(|error| format!("Could not create the HTTP client: {error}"))?;

    let response = client
        .get(&candidate)
        .send()
        .await
        .map_err(|error| format!("Could not reach {candidate}: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("{candidate} returned HTTP {}.", response.status().as_u16()));
    }
    if let Some(length) = response.content_length() {
        if length as usize > MAX_ICON_BYTES {
            return Err("That favicon is larger than 512 KB.".to_string());
        }
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("Could not read the favicon: {error}"))?
        .to_vec();
    if bytes.is_empty() {
        return Err("The favicon was empty.".to_string());
    }
    if bytes.len() > MAX_ICON_BYTES {
        return Err("That favicon is larger than 512 KB.".to_string());
    }
    let mime = detect_mime(&bytes)
        .ok_or_else(|| "That file is not a recognised image format.".to_string())?;

    Ok(FetchedIcon { bytes, mime })
}

pub const MAX_ICON_BYTES: usize = 512 * 1024;

/// Detects the format from magic bytes. The HTTP `Content-Type` of an intranet server is
/// frequently wrong, so it is only used as a hint by callers.
pub fn detect_mime(bytes: &[u8]) -> Option<&'static str> {
    const PNG: &[u8] = &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    const ICO: &[u8] = &[0x00, 0x00, 0x01, 0x00];
    const JPEG: &[u8] = &[0xff, 0xd8, 0xff];

    if bytes.starts_with(PNG) {
        return Some("image/png");
    }
    if bytes.starts_with(ICO) {
        return Some("image/x-icon");
    }
    if bytes.starts_with(JPEG) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }

    let text = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]);
    let lower = text.trim_start().to_lowercase();
    if lower.starts_with("<svg") || (lower.starts_with("<?xml") && lower.contains("<svg")) {
        return Some("image/svg+xml");
    }
    None
}

/// Derives the weak ETag stored for an icon (same shape as the old Postgres API).
pub fn etag_for(bytes: &[u8]) -> String {
    // FNV-1a is plenty for a cache validator; it is not a security primitive.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    format!("W/\"{hash:016x}\"")
}

/// Whether `If-None-Match` matches the stored ETag (with or without the `W/` prefix).
pub fn etag_matches(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(axum::http::header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .is_some_and(|candidate| candidate == etag || candidate == etag.trim_start_matches("W/"))
}
