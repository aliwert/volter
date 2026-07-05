//! Derive and attribute macros for volter.
//!
//! See `ARCHITECTURE.md` → "Macro-generated ergonomics" and `RULES.md` §7
//! before adding a macro here: generated code must stay debuggable (prefer
//! named helper functions over one giant inlined block) and must follow
//! every rule that hand-written code follows — no hidden panics, no hidden
//! blocking calls, no hidden `unsafe`.
//!
//! Planned macros:
//!
//! - `#[derive(FromRequestParts)]` — composite extractor from named fields,
//!   each of which is itself `FromRequestParts`.
//! - `#[derive(FromRequest)]` — same, but for the single body-consuming
//!   field.
//!
//! volter's core routing API must always work without any macro from this
//! crate (`Router::new().route("/x", get(handler))`) — macros are additive
//! sugar, never a requirement.
//!
//! TODO(v0.2): implement the two derive macros above once `volter-core`'s
//! trait signatures are stable.

#![deny(missing_docs)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use proc_macro::TokenStream;

/// Derive macro for `volter_core::FromRequestParts`.
///
/// Generates an implementation of `FromRequestParts` for a struct where
/// each field itself implements `FromRequestParts`.
///
/// TODO(v0.2): implement the actual derive logic.
#[proc_macro_derive(FromRequestParts)]
pub fn derive_from_request_parts(_input: TokenStream) -> TokenStream {
    TokenStream::default()
}

/// Derive macro for `volter_core::FromRequest`.
///
/// Generates an implementation of `FromRequest` for a struct with a single
/// body-consuming field that itself implements `FromRequest`.
///
/// TODO(v0.2): implement the actual derive logic.
#[proc_macro_derive(FromRequest)]
pub fn derive_from_request(_input: TokenStream) -> TokenStream {
    TokenStream::default()
}
