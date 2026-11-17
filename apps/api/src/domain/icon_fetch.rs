use crate::domain::apps::MAX_ICON_BYTES;

/// Icon bytes fetched from the app's own site.
pub struct FetchedIcon {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}

/// Fetches `<origin>/favicon.ico` for the app's URL.
///
/// Icons are stored exactly as served (PNG, ICO, SVG, JPEG, GIF, WebP) and the browser
/// renders them; there is deliberately no image decoding on the server, which keeps an
/// admin-triggered fetch away from image-parser attack surface. The portal and the client
/// both fall back to a coloured letter tile when an app has no icon.
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
        return Err(format!(
            "{candidate} returned HTTP {}.",
            response.status().as_u16()
        ));
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

/// Detects the format from magic bytes; the HTTP `Content-Type` of an intranet server is
/// frequently wrong, so it is only used as a hint by the caller (never trusted).
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
    let trimmed = text.trim_start();
    let lower = trimmed.to_lowercase();
    if lower.starts_with("<svg") || (lower.starts_with("<?xml") && lower.contains("<svg")) {
        return Some("image/svg+xml");
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_common_formats() {
        assert_eq!(
            detect_mime(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00]),
            Some("image/png")
        );
        assert_eq!(detect_mime(&[0x00, 0x00, 0x01, 0x00, 0x01]), Some("image/x-icon"));
        assert_eq!(detect_mime(&[0xff, 0xd8, 0xff, 0xe0]), Some("image/jpeg"));
        assert_eq!(
            detect_mime(b"<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"/>"),
            Some("image/svg+xml")
        );
        assert_eq!(detect_mime(b"not an image at all"), None);
    }
}
