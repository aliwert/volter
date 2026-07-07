use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tower::util::BoxCloneService;
use tower::{Layer, Service};

use volter_core::{Request, Response};

/// The concrete service type used internally by `Router::layer()`.
type Svc = BoxCloneService<Request, Response, Infallible>;

/// A [`tower::Layer`] that limits the number of concurrently executing
/// requests.
///
/// When the limit is reached, subsequent requests wait in a queue until
/// capacity becomes available (a permit is released).  No requests are
/// rejected — they are delayed, not dropped.
///
/// # Quick start
///
/// ```rust
/// use volter_middleware::ConcurrencyLimitLayer;
/// use volter_router::{Router, get};
///
/// async fn handler() -> &'static str { "ok" }
///
/// let app = Router::new()
///     .route("/", get(handler))
///     .layer(ConcurrencyLimitLayer::new(128));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ConcurrencyLimitLayer {
    max: usize,
}

impl ConcurrencyLimitLayer {
    /// Create a new `ConcurrencyLimitLayer` with the given maximum
    /// concurrent requests.
    ///
    /// `max` must be > 0.  If `max` is 0, no requests can execute.
    pub fn new(max: usize) -> Self {
        ConcurrencyLimitLayer { max }
    }
}

impl Layer<Svc> for ConcurrencyLimitLayer {
    type Service = ConcurrencyLimitService;

    fn layer(&self, service: Svc) -> Self::Service {
        ConcurrencyLimitService {
            inner: service,
            semaphore: Arc::new(Semaphore::new(self.max)),
        }
    }
}

/// The [`Service`] produced by [`ConcurrencyLimitLayer`].
pub struct ConcurrencyLimitService {
    inner: Svc,
    semaphore: Arc<Semaphore>,
}

impl Clone for ConcurrencyLimitService {
    fn clone(&self) -> Self {
        ConcurrencyLimitService {
            inner: self.inner.clone(),
            semaphore: self.semaphore.clone(),
        }
    }
}

impl Service<Request> for ConcurrencyLimitService {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let permit = self.semaphore.clone().try_acquire_owned();
        let semaphore = self.semaphore.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            let _permit: OwnedSemaphorePermit = match permit {
                Ok(p) => p,
                Err(_) => semaphore
                    .acquire_owned()
                    .await
                    .unwrap_or_else(|_| unreachable!("semaphore closed unexpectedly")),
            };
            inner.call(req).await
        })
    }
}
