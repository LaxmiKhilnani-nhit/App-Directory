//! Serves the portable Windows client for staff to download from the server.
//!
//! The exe is mounted at runtime from the `release/` directory (see docker-compose.yml), so
//! publishing a new client needs no API-image rebuild and the build still works on a Linux
//! server that has no Windows binary.

use std::path::Path;

use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

const DEFAULT_PATH: &str = "/app/client/App Directory.exe";

fn exe_path() -> String {
    std::env::var("CLIENT_EXE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_PATH.to_string())
}

pub async fn get() -> Response {
    let bytes = match std::fs::read(Path::new(&exe_path())) {
        Ok(bytes) if !bytes.is_empty() => bytes,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": {
                        "code": "client_not_available",
                        "message": "The Windows client has not been placed in the server's release folder yet."
                    }
                })),
            )
                .into_response();
        }
    };

    (
        StatusCode::OK,
        [
            ("content-type", "application/octet-stream"),
            ("content-disposition", "attachment; filename=\"App Directory.exe\""),
            ("cache-control", "no-store"),
        ],
        Body::from(bytes),
    )
        .into_response()
}

/// Whether a client binary is currently available (useful for the portal and diagnostics).
pub fn available() -> bool {
    std::fs::metadata(Path::new(&exe_path()))
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}
