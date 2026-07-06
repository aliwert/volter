use std::convert::Infallible;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::FutureExt;
use tower::util::BoxCloneService;
use tower::{Layer, Service};

use volter_core::{http, Request, Response};

/// The concrete service type used internally by `Router::layer()`.
type Svc = BoxCloneService<Request, Response, Infallible>;

/// A [`tower::Layer`] that catches panics from the inner service and converts
/// them into HTTP 500 responses.
///
/// Panics are logged via [`tracing::error!`]. The panic message is never
/// exposed to the client.
///
/// This is a **safety net** — handlers should not panic in the first place.
/// See [`RULES.md`](https://github.com/aliwert/volter/blob/main/RULES.md) #1.
///
/// Uses `futures_util::FutureExt::catch_unwind` under the hood.
///
/// # Example
///
/// ```rust
/// use volter_middleware::CatchPanicLayer;
/// use volter_router::{Router, get};
///
/// async fn handler() -> &'static str { "ok" }
///
/// let app = Router::new()
///     .route("/", get(handler))
///     .layer(CatchPanicLayer::new());
/// ```
#[derive(Clone, Default)]
pub struct CatchPanicLayer {
    _private: (),
}

impl CatchPanicLayer {
    /// Create a new `CatchPanicLayer`.
    pub fn new() -> Self {
        CatchPanicLayer { _private: () }
    }
}

impl Layer<Svc> for CatchPanicLayer {
    type Service = CatchPanicService;

    fn layer(&self, service: Svc) -> Self::Service {
        CatchPanicService { inner: service }
    }
}

/// The [`Service`] produced by [`CatchPanicLayer`].
///
/// Wraps an inner service and recovers panics into 500 responses.
pub struct CatchPanicService {
    inner: Svc,
}

impl Clone for CatchPanicService {
    fn clone(&self) -> Self {
        CatchPanicService {
            inner: self.inner.clone(),
        }
    }
}

impl Service<Request> for CatchPanicService {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let mut inner = self.inner.clone();

        Box::pin(async move {
            let result = AssertUnwindSafe(inner.call(req)).catch_unwind().await;

            match result {
                Ok(Ok(response)) => Ok(response),
                Ok(Err(infallible)) => match infallible {},
                Err(panic_payload) => {
                    let msg: &str = panic_payload
                        .downcast_ref::<&str>()
                        .copied()
                        .or_else(|| panic_payload.downcast_ref::<String>().map(|s| s.as_str()))
                        .unwrap_or("(no message)");

                    tracing::error!(panic = %msg, "request handler panicked");

                    let mut response = Response::new(volter_core::empty_body());
                    *response.status_mut() = http::StatusCode::INTERNAL_SERVER_ERROR;
                    Ok(response)
                }
            }
        })
    }
}
