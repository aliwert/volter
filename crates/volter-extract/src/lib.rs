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
//! - `State<T>` — typed application state, see `ARCHITECTURE.md`.
//! - `Extension<T>` — request-scoped values injected by middleware.
//! - `TypedHeader<T>` — a single strongly-typed header value.
//!
//! Every rejection here must implement `IntoResponse` and must never panic
//! on malformed input (see `RULES.md` #1) — a bad `Content-Length` header
//! or truncated body is user input, not a programmer error.
//!
//! TODO(v0.1): implement `Json`, `Query`, `Path`, `State` first — these
//! cover the large majority of real handlers.

#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
