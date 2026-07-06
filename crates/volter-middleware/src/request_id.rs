use std::convert::Infallible;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use tower::util::BoxCloneService;
use tower::{Layer, Service};

use volter_core::{http, Request, Response};

/// The concrete service type used internally by `Router::layer()`.
type Svc = BoxCloneService<Request, Response, Infallible>;

/// A strongly-typed request identifier backed by a [`ulid::Ulid`].
///
/// ULIDs are 128-bit, time-ordered, lexicographically sortable identifiers
/// that encode a 48-bit timestamp (millisecond precision) followed by 80
/// bits of randomness. This makes them more useful for logging,
/// observability, and distributed correlation than UUIDs.
///
/// # Example
///
/// ```rust
/// use volter_middleware::RequestId;
///
/// let id = RequestId::new();
/// println!("{id}");
/// ```
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct RequestId(ulid::Ulid);

impl RequestId {
    /// Generate a new `RequestId` from the current timestamp and random
    /// bytes.
    pub fn new() -> Self {
        RequestId(ulid::Ulid::new())
    }

    /// Parse a `RequestId` from its 26-character Crockford Base32 string
    /// representation.
    ///
    /// Returns `None` if the string is not a valid ULID.
    pub fn from_string(s: &str) -> Option<Self> {
        ulid::Ulid::from_string(s).ok().map(RequestId)
    }
}

impl Default for RequestId {
    fn default() -> Self {
        RequestId::new()
    }
}

impl fmt::Debug for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RequestId")
            .field(&self.0.to_string())
            .finish()
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A [`tower::Layer`] that assigns every request a unique [`RequestId`].
///
/// The middleware:
/// - Checks the incoming `X-Request-Id` header for a valid ULID; if present,
///   it is used as the request identifier instead of generating a new one.
/// - Inserts the [`RequestId`] into the request's [`Extensions`](http::Extensions)
///   so it can be extracted by handlers via [`Extension<RequestId>`].
/// - Sets the outgoing `X-Request-Id` response header to the string
///   representation of the identifier.
///
/// # Example
///
/// ```rust
/// use volter_middleware::RequestIdLayer;
/// use volter_router::{Router, get};
///
/// async fn handler() -> &'static str { "ok" }
///
/// let app = Router::new()
///     .route("/", get(handler))
///     .layer(RequestIdLayer::new());
/// ```
#[derive(Clone, Default)]
pub struct RequestIdLayer {
    _private: (),
}

impl RequestIdLayer {
    /// Create a new `RequestIdLayer`.
    pub fn new() -> Self {
        RequestIdLayer { _private: () }
    }
}

impl Layer<Svc> for RequestIdLayer {
    type Service = RequestIdService;

    fn layer(&self, service: Svc) -> Self::Service {
        RequestIdService { inner: service }
    }
}

/// The [`Service`] produced by [`RequestIdLayer`].
///
/// Wraps an inner service and injects [`RequestId`] into every request.
pub struct RequestIdService {
    inner: Svc,
}

impl Clone for RequestIdService {
    fn clone(&self) -> Self {
        RequestIdService {
            inner: self.inner.clone(),
        }
    }
}

impl Service<Request> for RequestIdService {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: Request) -> Self::Future {
        let mut inner = self.inner.clone();

        Box::pin(async move {
            let request_id = req
                .headers()
                .get(http::header::HeaderName::from_static("x-request-id"))
                .and_then(|v| v.to_str().ok())
                .and_then(RequestId::from_string)
                .unwrap_or_else(RequestId::new);

            req.extensions_mut().insert(request_id);

            let mut response = inner.call(req).await?;

            let header_value = http::HeaderValue::from_str(&request_id.to_string())
                .unwrap_or_else(|_| http::HeaderValue::from_static(""));

            response.headers_mut().insert(
                http::header::HeaderName::from_static("x-request-id"),
                header_value,
            );

            Ok(response)
        })
    }
}
