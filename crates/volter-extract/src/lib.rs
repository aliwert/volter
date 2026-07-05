//! Built-in extractors for volter.
//!
//! Each extractor implements `volt_core::FromRequestParts` or
//! `volt_core::FromRequest`, and defines its own `Rejection` type (see
//! `ARCHITECTURE.md` → "Error & rejection model").
//!
//! Implemented extractors (v0.1):
//!
//! - [`Json<T>`] — body extractor, `T: serde::de::DeserializeOwned`.
//!   Rejection distinguishes missing/unsupported Content-Type (415), invalid
//!   JSON syntax/shape (400), and body read errors (500).
//! - [`Query<T>`] — parses the URL query string via `serde_urlencoded`.
//! - [`Path<T>`] — typed path parameters from the router's matched segments.
//! - [`Extension<T>`] — request-scoped values injected by middleware.
//! - [`State<T>`](volter_core::State) — typed application state.
//!
//! Planned extractors:
//! - `TypedHeader<T>` — a single strongly-typed header value.
//!
//! Every rejection here must implement `IntoResponse` and must never panic
//! on malformed input (see `RULES.md` #1) — a bad `Content-Length` header
//! or truncated body is user input, not a programmer error.
//!
//! [`State<T>`](volter_core::State) is defined in `volter-core` alongside
//! the `FromRequestParts` trait and the `Handler` blanket impl, so it is
//! re-exported here for convenience.

#![deny(missing_docs)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod extension;
mod json;
mod path;
mod query;

pub use extension::{Extension, ExtensionRejection};
pub use json::{Json, JsonRejection};
pub use path::{Path, PathRejection};
pub use query::{Query, QueryRejection};
pub use volter_core::State;
