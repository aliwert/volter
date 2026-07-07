//! Request router for Volter.
//!
//! This crate provides the routing layer of the framework:
//!
//! - [`Router`] — the main entry point.  Holds a collection of routes and
//!   implements [`tower::Service`].
//! - [`MethodRouter`] — per-path dispatcher that routes by HTTP method.
//! - [`get`] — construct a [`MethodRouter`] for a GET handler.
//! - [`post`] — construct a [`MethodRouter`] for a POST handler.
//!
//! See the workspace root `ARCHITECTURE.md` → "Router architecture" for
//! the full design.

#![deny(missing_docs)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod error;
mod method_router;
mod pattern;
mod route;
mod router;

pub use method_router::MethodRouter;
pub use route::{get, post};
pub use router::Router;
