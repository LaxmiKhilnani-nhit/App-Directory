//! App Directory API library.
//!
//! Exposes the modules so integration tests can exercise the domain logic directly against
//! a real database, while `main.rs` only wires configuration and the HTTP server.

pub mod config;
pub mod db;
pub mod domain;
pub mod state;
pub mod static_assets;
