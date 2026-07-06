//! Built-in middleware for volter, expressed as `tower::Layer`s.
//!
//! Anything here composes with any other `tower`/`tower-http` layer — see
//! `ARCHITECTURE.md` → "Middleware model".
//!
//! Implemented:
//!
//! - [`TraceLayer`] — per-request tracing spans (method, path, status,
//!   latency).
//! - [`TimeoutLayer`] — per-request timeout with a `408 Request Timeout`
//!   response on expiry.
//!
//! Planned:
//! - `CorsLayer` — thin, opinionated wrapper over `tower_http::cors`.
//! - `RequestBodyLimitLayer` — reject oversized bodies before they're fully
//!   buffered.
//! - `CatchPanicLayer` — converts a panic that slips through handler code
//!   into a 500 response and a `tracing::error!` log line. This is a
//!   safety net, not a substitute for `RULES.md` #1 — panics should not be
//!   reachable from user input in the first place.
//!
//! TODO(v0.1): `CatchPanicLayer`.
//! TODO(v0.3): rate limiting, connection limits (see `PROJECT.md`
//! milestones).

#![deny(missing_docs)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod timeout;
mod trace;

pub use timeout::TimeoutLayer;
pub use trace::TraceLayer;

/// A [`tower::Layer`] that sets CORS headers on responses.
///
/// Thin, opinionated wrapper over `tower_http::cors::CorsLayer`.
/// TODO(v0.1): implement as a proper `tower::Layer`.
pub struct Cors;
