//! In-process test client for Volter applications.
//!
//! [`TestClient`] wraps a [`Router`](volter_router::Router) and drives it
//! directly in memory — no real TCP socket is bound, so tests are fast and
//! don't fight over ports in CI.
//!
//! # Quick start
//!
//! ```rust
//! use volter_testing::TestClient;
//! use volter_router::{Router, get};
//!
//! async fn handler() -> &'static str { "ok" }
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() {
//! let client = TestClient::new(Router::new().route("/", get(handler)));
//!
//! let response = client.get("/").send().await;
//! assert_eq!(response.status(), 200);
//!
//! let body = response.text().await.unwrap();
//! assert_eq!(body, "ok");
//! # }
//! ```
//!
//! # JSON round-trip
//!
//! ```rust
//! use serde::{Serialize, Deserialize};
//! use volter_testing::TestClient;
//! use volter_router::{Router, post};
//! use volter_extract::Json;
//!
//! #[derive(Serialize, Deserialize, PartialEq, Debug)]
//! struct Payload { name: String }
//!
//! async fn echo(Json(p): Json<Payload>) -> Json<Payload> { Json(p) }
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() {
//! let client = TestClient::new(Router::new().route("/echo", post(echo)));
//!
//! let response = client
//!     .post("/echo")
//!     .json(&Payload { name: "ferris".into() })
//!     .send()
//!     .await;
//!
//! let body: Payload = response.json().await.unwrap();
//! assert_eq!(body, Payload { name: "ferris".into() });
//! # }
//! ```
//!
//! See `ARCHITECTURE.md` → "Testing architecture" for the full design.

#![deny(missing_docs)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod request;
mod response;

pub use request::TestRequestBuilder;
pub use response::{BodyError, TestResponse};

use http::Method;
use volter_router::Router;

/// An in-process test client for Volter applications.
///
/// Wraps a [`Router`] and provides ergonomic helpers for issuing requests
/// and inspecting responses without binding a real TCP socket.
///
/// The type parameter `S` corresponds to the router's application state.
/// Most tests use a stateless router (`Router::new()`) which gives
/// `TestClient::new(router)` a `TestClient<()>`.
///
/// Each call to `get` / `post` / `request` clones the router (a cheap
/// `Arc` bump) so the client does not require `&mut` access.
///
/// # Example
///
/// ```rust
/// use volter_testing::TestClient;
/// use volter_router::{Router, get};
///
/// async fn handler() -> &'static str { "hello" }
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() {
/// let client = TestClient::new(Router::new().route("/", get(handler)));
/// let resp = client.get("/").send().await;
/// assert_eq!(resp.status(), 200);
/// # }
/// ```
pub struct TestClient<S = ()> {
    router: Router<S>,
}

impl<S: Clone + Send + 'static> TestClient<S> {
    /// Create a new `TestClient` wrapping the given router.
    pub fn new(router: Router<S>) -> Self {
        Self { router }
    }

    /// Start building a GET request to `path`.
    pub fn get(&self, path: &str) -> TestRequestBuilder<S> {
        self.request(Method::GET, path)
    }

    /// Start building a POST request to `path`.
    pub fn post(&self, path: &str) -> TestRequestBuilder<S> {
        self.request(Method::POST, path)
    }

    /// Start building a request with an arbitrary HTTP method.
    pub fn request(&self, method: Method, path: &str) -> TestRequestBuilder<S> {
        TestRequestBuilder {
            router: self.router.clone(),
            method,
            path: path.to_owned(),
            headers: http::HeaderMap::new(),
            body: None,
        }
    }
}
