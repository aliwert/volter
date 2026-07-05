//! Built-in middleware for volter, expressed as `tower::Layer`s.
//!
//! Anything here composes with any other `tower`/`tower-http` layer — see
//! `ARCHITECTURE.md` → "Middleware model". Nothing below is implemented
//! yet.
//!
//! Planned middleware:
//!
//! - `TraceLayer` — request/response tracing spans (thin, opinionated
//!   wrapper over `tower_http::trace`).
//! - `TimeoutLayer` — per-request timeout, returning a proper HTTP
//!   response (not a dropped connection) on expiry.
//! - `CorsLayer` — thin, opinionated wrapper over `tower_http::cors`.
//! - `RequestBodyLimitLayer` — reject oversized bodies before they're fully
//!   buffered.
//! - `CatchPanicLayer` — converts a panic that slips through handler code
//!   into a 500 response and a `tracing::error!` log line. This is a
//!   safety net, not a substitute for `RULES.md` #1 — panics should not be
//!   reachable from user input in the first place.
//!
//! TODO(v0.1): `TraceLayer`, `TimeoutLayer`, `CatchPanicLayer`.
//! TODO(v0.3): rate limiting, connection limits (see `PROJECT.md`
//! milestones).

#![deny(missing_docs)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

/// A [`tower::Layer`] that adds tracing spans per request.
///
/// Thin, opinionated wrapper over `tower_http::trace::TraceLayer`.
/// TODO(v0.1): implement as a proper `tower::Layer`.
pub struct Trace;

/// A [`tower::Layer`] that enforces a per-request timeout.
///
/// Returns a proper HTTP 408 / 504 response on expiry instead of
/// dropping the connection.
/// TODO(v0.1): implement as a proper `tower::Layer`.
pub struct Timeout;

/// A [`tower::Layer`] that sets CORS headers on responses.
///
/// Thin, opinionated wrapper over `tower_http::cors::CorsLayer`.
/// TODO(v0.1): implement as a proper `tower::Layer`.
pub struct Cors;
