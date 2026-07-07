//! [`TestRequestBuilder`] — accumulates request parameters and sends them
//! through the router.

use bytes::Bytes;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use http::Method;
use serde::Serialize;
use tower::Service;

use volter_core::BoxBody;
use volter_core::Request;
use volter_router::Router;

use crate::response::TestResponse;

/// A request builder produced by [`TestClient`](crate::TestClient).
///
/// Accumulates method, path, headers, and body, then executes the request
/// via [`send`](TestRequestBuilder::send).
///
/// # Examples
///
/// ```rust
/// use volter_testing::TestClient;
/// use volter_router::{Router, get};
///
/// async fn handler() -> &'static str { "ok" }
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() {
/// let client = TestClient::new(Router::new().route("/", get(handler)));
/// let response = client.get("/").send().await;
/// assert_eq!(response.status(), 200);
/// # }
/// ```
pub struct TestRequestBuilder<S> {
    pub(crate) router: Router<S>,
    pub(crate) method: Method,
    pub(crate) path: String,
    pub(crate) headers: HeaderMap,
    pub(crate) body: Option<Bytes>,
}

impl<S: Clone + Send + 'static> TestRequestBuilder<S> {
    /// Add a header to the request.
    ///
    /// Existing headers with the same name are **not** removed — the value is
    /// appended.
    pub fn header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.headers.append(name, value);
        self
    }

    /// Set the request body to a JSON-serialized value.
    ///
    /// Also sets the `Content-Type` header to `application/json`.
    pub fn json<T: Serialize>(mut self, value: &T) -> Self {
        // Serialization to Vec<u8> only fails for types containing non-finite
        // f32/f64 values — acceptable in a test helper where the caller
        // controls the input.
        let bytes = serde_json::to_vec(value).unwrap_or_else(|_| Vec::new());
        self.headers.insert(
            http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        self.body = Some(Bytes::from(bytes));
        self
    }

    /// Set the request body to a raw byte slice.
    pub fn body(mut self, body: impl Into<Bytes>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Build the `http::Request`, dispatch it through the cloned router, and
    /// return a [`TestResponse`].
    pub async fn send(mut self) -> TestResponse {
        let request = self.build_request();
        let response = self
            .router
            .call(request)
            .await
            .unwrap_or_else(|never| match never {});
        TestResponse::new(response)
    }

    /// Construct the inner `http::Request<BoxBody>`.
    fn build_request(&self) -> Request<BoxBody> {
        let body: BoxBody = match &self.body {
            Some(bytes) => volter_core::full_body(bytes.clone()),
            None => volter_core::empty_body(),
        };

        let mut request = Request::builder()
            .method(self.method.clone())
            .uri(&self.path)
            .body(body)
            // Builder failure only happens for invalid method / uri, both
            // of which are caller-supplied — acceptable in a test helper.
            .unwrap_or_else(|_| Request::new(volter_core::empty_body()));

        // Copy headers into the request.
        for (name, value) in &self.headers {
            request.headers_mut().insert(name, value.clone());
        }

        request
    }
}
