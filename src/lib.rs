//! Pusher HTTP API client for Rust
//! 
//! This library provides a safe, fast, and idiomatic Rust client for the Pusher HTTP API.

pub mod auth;
pub mod config;
pub mod errors;
pub mod events;
pub mod pusher;
pub mod token;
pub mod util;
pub mod webhook;

pub use pusher::Pusher;
pub use config::Config;
pub use errors::{PusherError, RequestError, WebhookError};
pub use token::Token;
pub use webhook::Webhook;

/// Result type alias for Pusher operations
pub type Result<T> = std::result::Result<T, PusherError>;
