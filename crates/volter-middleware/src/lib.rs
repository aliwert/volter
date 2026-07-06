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
//! - [`CatchPanicLayer`] — catches panics from handlers and returns a `500
//!   Internal Server Error`. Safety net, not a substitute for proper error
//!   handling.
//! - [`RequestIdLayer`] — assigns every request a unique [`RequestId`]
//!   (backed by a ULID), injects it into request extensions, and sets the
//!   `X-Request-Id` response header.
//! - [`CorsLayer`] — permissive or configurable CORS with full preflight
//!   support.
//!
//! Planned:
//! - `RequestBodyLimitLayer` — reject oversized bodies before they're fully
//!   buffered.
//!   TODO(v0.3): rate limiting, connection limits (see `PROJECT.md`
//!   milestones).

#![deny(missing_docs)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod catch_panic;
mod cors;
mod request_id;
mod timeout;
mod trace;

pub use catch_panic::CatchPanicLayer;
pub use cors::CorsLayer;
pub use request_id::{RequestId, RequestIdLayer};
pub use timeout::TimeoutLayer;
pub use trace::TraceLayer;
