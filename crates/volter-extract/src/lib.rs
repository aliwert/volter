//! Built-in extractors for volter.
//!
//! Each extractor implements `volt_core::FromRequestParts` or
//! `volt_core::FromRequest`, and defines its own `Rejection` type (see
//! `ARCHITECTURE.md` → "Error & rejection model"). None of these are
//! implemented yet — this file records the intended v0.1 surface.
//!
//! Planned extractors:
//!
//! - `Json<T>` — body extractor, `T: serde::de::DeserializeOwned`.
//!   Rejection distinguishes "not valid JSON" from "wrong content-type"
//!   from "JSON that doesn't match `T`'s shape", each mapped to a sensible
//!   HTTP status (400 / 415 / 422 respectively).
//! - `Query<T>` — parses the URL query string via `serde_urlencoded`.
//! - `Path<T>` — typed path parameters from the router's matched segments.
//! - `Extension<T>` — request-scoped values injected by middleware.
//! - `TypedHeader<T>` — a single strongly-typed header value.
//!
//! Every rejection here must implement `IntoResponse` and must never panic
//! on malformed input (see `RULES.md` #1) — a bad `Content-Length` header
//! or truncated body is user input, not a programmer error.
//!
//! TODO(v0.1): implement `Json`, `Query`, `Extension` first — these
//! cover the large majority of real handlers.
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

mod path;

pub use path::{Path, PathRejection};
pub use volter_core::State;

/// Extracts a typed JSON body from a request.
///
/// Wraps a deserialized value of type `T`.
/// TODO(v0.1): implement `FromRequest` for `Json<T>`.
pub struct Json<T>(pub T);

/// Extracts typed query parameters from the URL query string.
///
/// Parses the query string via `serde_urlencoded` into type `T`.
/// TODO(v0.1): implement `FromRequestParts` for `Query<T>`.
pub struct Query<T>(pub T);

/// Extracts a request-scoped value injected by middleware.
///
/// Unlike `State`, a missing `Extension` is a runtime rejection since
/// middleware ordering cannot always be verified statically.
/// TODO(v0.1): implement `FromRequestParts` for `Extension<T>`.
pub struct Extension<T>(pub T);
