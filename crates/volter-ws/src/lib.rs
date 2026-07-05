//! WebSocket support for volter.
//!
//! See `ARCHITECTURE.md` → "WebSocket architecture". A WebSocket endpoint
//! is just another route — there is no separate "WebSocket app" concept.
//!
//! Planned public surface:
//!
//! - `WebSocketUpgrade` — a `FromRequestParts` extractor. A handler takes
//!   it as an argument and returns a value describing what to do once the
//!   connection upgrades (mirrors axum's `ws.on_upgrade(|socket| ...)`
//!   pattern).
//! - `WebSocket` — the upgraded connection: `.send(Message)` /
//!   `.recv() -> Option<Message>`, built on `tokio-tungstenite`.
//!
//! TODO(v0.2): implement the upgrade extractor and connection wrapper (see
//! `PROJECT.md` milestones).

#![deny(missing_docs)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

/// An extractor for WebSocket upgrades.
///
/// Implements `FromRequestParts` — a handler takes it as an argument
/// and returns a value describing how to handle the upgraded connection.
/// TODO(v0.2): implement the upgrade handshake and connection wrapper.
pub struct WebSocketUpgrade;
