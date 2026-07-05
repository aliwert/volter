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
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub use volter_core::{FromRequest, FromRequestParts, Handler, IntoResponse, Request, Response};

#[cfg(feature = "macros")]
pub use volter_macros::*;

#[cfg(feature = "ws")]
pub use volter_ws as ws;

// TODO: re-export `volter_router::Router` once it exists.
// TODO: re-export `volter_extract::{Json, Query, Path, State, Extension}`
//       once implemented.
// TODO: re-export `volter_middleware` layers under a `middleware` module.
