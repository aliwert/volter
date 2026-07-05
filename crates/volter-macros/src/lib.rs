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
