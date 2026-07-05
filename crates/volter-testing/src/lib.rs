//! In-process test client for volter applications.
//!
//! See `ARCHITECTURE.md` → "Testing architecture". `TestClient` wraps a
//! `Router` (a `tower::Service`) and drives it directly in memory — no real
//! TCP socket is bound, so tests are fast and don't fight over ports in CI.
//!
//! Planned public surface (modeled after `reqwest`-style ergonomics):
//!
//! ```ignore
//! let client = TestClient::new(router);
//! let response = client.get("/users/1").send().await;
//! assert_eq!(response.status(), StatusCode::OK);
//! let body: User = response.json().await;
//! ```
//!
//! TODO(v0.2): implement `TestClient`, `TestRequestBuilder`, and
//! `TestResponse` once `volter-router`'s `Router` type exists.

#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
