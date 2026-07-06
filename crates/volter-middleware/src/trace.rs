use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;

use tower::util::BoxCloneService;
use tower::{Layer, Service};

use volter_core::{Request, Response};

/// The concrete service type used internally by `Router::layer()`.
type Svc = BoxCloneService<Request, Response, Infallible>;

/// A [`tower::Layer`] that adds per-request tracing spans.
///
/// Each request creates an `http.request` span and logs the response status
/// code and latency upon completion.
///
/// # Example
///
/// ```rust
/// use volter_middleware::TraceLayer;
/// use volter_router::{Router, get};
///
/// async fn handler() -> &'static str { "ok" }
///
/// let app = Router::new()
///     .route("/", get(handler))
///     .layer(TraceLayer::new());
/// ```
///
/// This is a minimal alternative to `tower_http::trace::TraceLayer`. For full
/// control over span creation and response/error logging, use
/// `tower_http::trace::TraceLayer` directly.
#[derive(Clone, Default)]
pub struct TraceLayer {
    _private: (),
}

impl TraceLayer {
    /// Create a new `TraceLayer`.
    pub fn new() -> Self {
        TraceLayer { _private: () }
    }
}

impl Layer<Svc> for TraceLayer {
    type Service = TraceService;

    fn layer(&self, service: Svc) -> Self::Service {
        TraceService { inner: service }
    }
}

/// The [`Service`] produced by [`TraceLayer`].
///
/// Wraps an inner service and emits tracing spans for each request.
pub struct TraceService {
    inner: Svc,
}

impl Clone for TraceService {
    fn clone(&self) -> Self {
        TraceService {
            inner: self.inner.clone(),
        }
    }
}

impl Service<Request> for TraceService {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let method = req.method().clone();
        let uri_path: &str = req
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");

        let span = tracing::info_span!(
            "http.request",
            http.method = %method,
            http.target = %uri_path,
        );

        let start = Instant::now();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            let result = inner.call(req).await;

            match &result {
                Ok(response) => {
                    let status = response.status();
                    let latency = start.elapsed();
                    tracing::info!(
                        parent: &span,
                        http.status_code = status.as_u16(),
                        latency_ms = latency.as_millis() as u64,
                        "response",
                    );
                }
                Err(_) => {
                    let latency = start.elapsed();
                    tracing::error!(
                        parent: &span,
                        latency_ms = latency.as_millis() as u64,
                        "request failed",
                    );
                }
            }

            result
        })
    }
}
