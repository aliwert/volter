//! Radix-tree (trie) based router for volter.
//!
//! See `ARCHITECTURE.md` → "Router architecture" at the workspace root for
//! the full design. This crate is not implemented yet — it's the next
//! piece to build after `volter-core`'s traits settle.
//!
//! Planned public surface:
//!
//! - `Router<S>` — the main entry point. Implements `tower::Service` so it
//!   composes with any `tower`/`tower-http` middleware.
//! - `Router::route(path, method_router)` — register a handler for a path.
//! - `Router::layer(middleware)` — wrap the whole router in a `tower::Layer`.
//! - `Router::with_state(state)` — attach typed application state, checked
//!   at compile time (see `ARCHITECTURE.md` → "State & dependency injection").
//! - Path syntax: static segments (`/users`), named params (`/users/:id`),
//!   and a trailing wildcard (`/files/*rest`).
//! - Each registered route also carries a metadata slot (initially unused)
//!   reserved for future OpenAPI/schema generation, so adding that later
//!   doesn't require a breaking change to the router's public API.
//!
//! TODO(v0.1): implement the trie itself, benchmark against `matchit`
//! before deciding to keep a custom implementation (see `TOOLS.md` →
//! "Routing").

#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
