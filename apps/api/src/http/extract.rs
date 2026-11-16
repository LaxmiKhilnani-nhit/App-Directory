use std::net::{IpAddr, SocketAddr};

use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{header::AUTHORIZATION, request::Parts},
};

use crate::domain::auth::{self, AuthSession};
use crate::error::ApiError;
use crate::state::AppState;

/// Resolves `Authorization: Bearer <token>` to a live session and user.
pub struct AuthUser(pub AuthSession);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();

        let token = header
            .strip_prefix("Bearer ")
            .or_else(|| header.strip_prefix("bearer "))
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .ok_or_else(|| ApiError::unauthorized("Sign in required."))?;

        let pool = state
            .pool
            .as_ref()
            .ok_or_else(|| ApiError::internal("Database is not configured."))?;

        let session = auth::authenticate(pool, token)
            .await
            .map_err(ApiError::from)?;

        Ok(AuthUser(session))
    }
}

/// Best-effort client address for rate limiting and the audit log.
///
/// Populated only when the server is started with `into_make_service_with_connect_info`
/// (which `main.rs` does); never fails, so tests that build requests without connection
/// info still work.
pub struct ClientIp(pub Option<IpAddr>);

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let address = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ConnectInfo(address)| address.ip());

        Ok(ClientIp(address))
    }
}
