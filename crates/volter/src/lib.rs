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

/// Core traits and types.
pub use volter_core::{FromRequest, FromRequestParts, Handler, IntoResponse, Request, Response};

/// Router for request routing.
pub use volter_router::Router;

/// Standard extractors.
pub use volter_extract::{Extension, Json, Path, Query, State};

/// Built-in middleware layers.
pub use volter_middleware::{Cors, Timeout, Trace};

/// Derive macros for extractors.
#[cfg(feature = "macros")]
pub use volter_macros::*;

/// WebSocket support.
#[cfg(feature = "ws")]
pub use volter_ws as ws;
