use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use tower::util::BoxCloneService;
use tower::{Layer, Service};

use volter_core::{empty_body, http, Request, Response};

/// The concrete service type used internally by `Router::layer()`.
type Svc = BoxCloneService<Request, Response, Infallible>;

/// A [`tower::Layer`] that limits the maximum HTTP request body size.
///
/// Requests with a `Content-Length` header exceeding the configured limit
/// are rejected with `413 Payload Too Large` **before** the handler runs.
/// Requests without a `Content-Length` header are passed through unchanged.
///
/// # Quick start
///
/// ```rust
/// use volter_middleware::RequestBodyLimitLayer;
/// use volter_router::{Router, get};
///
/// async fn handler() -> &'static str { "ok" }
///
/// let app = Router::new()
///     .route("/", get(handler))
///     .layer(RequestBodyLimitLayer::new(1024 * 1024)); // 1 MB
/// ```
#[derive(Clone, Copy, Debug)]
pub struct RequestBodyLimitLayer {
    limit: usize,
}

impl RequestBodyLimitLayer {
    /// Create a new `RequestBodyLimitLayer` with the given byte limit.
    ///
    /// Requests whose `Content-Length` exceeds `limit` will receive a
    /// `413 Payload Too Large` response without reaching the handler.
    pub fn new(limit: usize) -> Self {
        RequestBodyLimitLayer { limit }
    }
}

impl Layer<Svc> for RequestBodyLimitLayer {
    type Service = RequestBodyLimitService;

    fn layer(&self, service: Svc) -> Self::Service {
        RequestBodyLimitService {
            inner: service,
            limit: self.limit,
        }
    }
}

/// The [`Service`] produced by [`RequestBodyLimitLayer`].
pub struct RequestBodyLimitService {
    inner: Svc,
    limit: usize,
}

impl Clone for RequestBodyLimitService {
    fn clone(&self) -> Self {
        RequestBodyLimitService {
            inner: self.inner.clone(),
            limit: self.limit,
        }
    }
}

impl Service<Request> for RequestBodyLimitService {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let content_length = req
            .headers()
            .get(http::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        if let Some(len) = content_length {
            if len > self.limit as u64 {
                return Box::pin(async move { Ok(payload_too_large()) });
            }
        }

        let mut inner = self.inner.clone();
        Box::pin(async move { inner.call(req).await })
    }
}

fn payload_too_large() -> Response {
    let mut response = Response::new(empty_body());
    *response.status_mut() = http::StatusCode::PAYLOAD_TOO_LARGE;
    response
}
