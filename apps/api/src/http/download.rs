//! Serves the portable Windows client for staff to download from the server.
//!
//! The exe is mounted at runtime from the `release/` directory (see docker-compose.yml), so
//! publishing a new client needs no API-image rebuild and the build still works on a Linux
//! server that has no Windows binary.

use std::path::{Path, PathBuf};

use axum::{
    body::Body,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::state::AppState;

const DEFAULT_PATH: &str = "/app/client/App Directory.exe";

fn exe_path(state: &AppState) -> PathBuf {
    std::env::var("CLIENT_EXE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| state.config.client_exe.clone())
        .unwrap_or_else(|| PathBuf::from(DEFAULT_PATH))
}

pub async fn get(State(state): State<AppState>) -> Response {
    let path = exe_path(&state);
    let bytes = match std::fs::read(Path::new(&path)) {
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
