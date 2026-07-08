//! Volter: a production-grade, async-first web framework for Rust.
//!
//! This is the crate most users depend on directly — it re-exports the
//! pieces from `volter-core`, `volter-router`, `volter-extract`, and
//! `volter-middleware` that make up the public API, the same way `axum`
//! re-exports `axum-core`.
//!
//! See the workspace root `PROJECT.md`, `TOOLS.md`, `RULES.md`, and
//! `ARCHITECTURE.md` for the full design. This crate itself has no logic —
//! it should stay a thin re-export layer so internal crate boundaries can
//! move without breaking users, per `ARCHITECTURE.md` → "Extension points
//! for third parties".

#![deny(missing_docs)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

/// Core traits, types, and adapters.
pub use volter_core::{
    empty_body, full_body, http, Body, BoxBody, BoxError, FromRequest, FromRequestParts, Handler,
    HandlerService, IntoResponse, Request, Response,
};

/// Router for request routing.
pub use volter_router::{get, post, MethodRouter, RouteAttr, Router};

/// Standard extractors.
pub use volter_extract::{Extension, Json, JsonRejection, Path, Query, QueryRejection};

/// Re-export of `serde_urlencoded` so derive macros can access it through
/// `::volter::serde_urlencoded`.
pub use volter_extract::serde_urlencoded;

/// Typed application state extractor (defined in `volter-core`).
pub use volter_core::State;

/// High-level server — [`serve`](server::serve).
mod server;

pub use server::{serve, serve_with};

/// Built-in middleware layers.
///
/// [`TraceLayer`] adds per-request tracing spans (method, path, status,
/// latency).
///
/// [`TimeoutLayer`] enforces a per-request timeout and returns `408 Request
/// Timeout` on expiry.
///
/// [`CatchPanicLayer`] catches panics from handlers and returns `500 Internal
/// Server Error`.
///
/// [`CorsLayer`] adds Cross-Origin Resource Sharing headers with permissive
/// or fine-grained configuration.
///
/// [`CompressionLayer`] compresses response bodies using `Accept-Encoding`
/// negotiation (gzip, br, zstd, deflate).
///
/// [`RequestBodyLimitLayer`] limits request body size and returns
/// `413 Payload Too Large` when exceeded.
///
/// [`ConcurrencyLimitLayer`] limits the number of concurrently executing
/// requests.
///
/// [`RateLimitLayer`] limits the request rate using a fixed-window counter
/// and returns `429 Too Many Requests` when the limit is exceeded.
///
/// [`RequestIdLayer`] assigns every request a unique [`RequestId`] and
/// sets the `X-Request-Id` response header.
///
/// All are used via [`Router::layer()`]:
pub use volter_middleware::{
    CatchPanicLayer, CompressionLayer, ConcurrencyLimitLayer, CorsLayer, RateLimitLayer,
    RequestBodyLimitLayer, RequestId, RequestIdLayer, TimeoutLayer, TraceLayer,
};

/// Derive macros for extractors.
#[cfg(feature = "macros")]
pub use volter_macros::*;

/// WebSocket support.
#[cfg(feature = "ws")]
pub use volter_ws as ws;
