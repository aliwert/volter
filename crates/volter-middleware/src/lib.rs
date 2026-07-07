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
//! - [`CompressionLayer`] — compresses response bodies using
//!   `Accept-Encoding` negotiation (gzip, br, zstd, deflate).
//! - [`RequestBodyLimitLayer`] — limits request body size, returns
//!   `413 Payload Too Large` when exceeded.
//! - [`ConcurrencyLimitLayer`] — limits the number of concurrently executing
//!   requests.
//! - [`RateLimitLayer`] — fixed-window rate limiter that returns `429 Too
//!   Many Requests` when the limit is exceeded.

#![deny(missing_docs)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod body_limit;
mod catch_panic;
mod compression;
mod concurrency_limit;
mod cors;
mod rate_limit;
mod request_id;
mod timeout;
mod trace;

pub use body_limit::RequestBodyLimitLayer;
pub use catch_panic::CatchPanicLayer;
pub use compression::CompressionLayer;
pub use concurrency_limit::ConcurrencyLimitLayer;
pub use cors::CorsLayer;
pub use rate_limit::RateLimitLayer;
pub use request_id::{RequestId, RequestIdLayer};
pub use timeout::TimeoutLayer;
pub use trace::TraceLayer;
