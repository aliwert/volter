use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tower::util::BoxCloneService;
use tower::{Layer, Service};

use volter_core::{http, Request, Response};

/// The concrete service type used internally by `Router::layer()`.
type Svc = BoxCloneService<Request, Response, Infallible>;

/// A [`tower::Layer`] that enforces a per-request timeout.
///
/// If the inner service does not produce a response within the configured
/// duration, a [`408 Request Timeout`](http::StatusCode::REQUEST_TIMEOUT)
/// response is returned.
///
/// # Example
///
/// ```rust
/// use std::time::Duration;
/// use volter_middleware::TimeoutLayer;
/// use volter_router::{Router, get};
///
/// async fn handler() -> &'static str { "ok" }
///
/// let app = Router::new()
///     .route("/", get(handler))
///     .layer(TimeoutLayer::new(Duration::from_secs(5)));
/// ```
#[derive(Clone)]
pub struct TimeoutLayer {
    duration: Duration,
}

impl TimeoutLayer {
    /// Create a new `TimeoutLayer` with the given duration.
    ///
    /// After `duration` elapses, the in-flight request is aborted and a
    /// `408 Request Timeout` response is returned.
    pub fn new(duration: Duration) -> Self {
        TimeoutLayer { duration }
    }
}

impl Layer<Svc> for TimeoutLayer {
    type Service = TimeoutService;

    fn layer(&self, service: Svc) -> Self::Service {
        TimeoutService {
            inner: service,
            duration: self.duration,
        }
    }
}

/// The [`Service`] produced by [`TimeoutLayer`].
///
/// Wraps an inner service and aborts requests that exceed the configured
/// timeout.
pub struct TimeoutService {
    inner: Svc,
    duration: Duration,
}

impl Clone for TimeoutService {
    fn clone(&self) -> Self {
        TimeoutService {
            inner: self.inner.clone(),
            duration: self.duration,
        }
    }
}

impl Service<Request> for TimeoutService {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let mut inner = self.inner.clone();
        let duration = self.duration;

        Box::pin(async move {
            match tokio::time::timeout(duration, inner.call(req)).await {
                Ok(result) => result,
                Err(_elapsed) => {
                    let mut response = Response::new(volter_core::empty_body());
                    *response.status_mut() = http::StatusCode::REQUEST_TIMEOUT;
                    Ok(response)
                }
            }
        })
    }
}
