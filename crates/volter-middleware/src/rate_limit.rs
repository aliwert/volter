use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use tower::util::BoxCloneService;
use tower::{Layer, Service};

use volter_core::{http, Request, Response};

/// The concrete service type used internally by `Router::layer()`.
type Svc = BoxCloneService<Request, Response, Infallible>;

/// A [`tower::Layer`] that limits the request rate using a fixed-window
/// counter.
///
/// When the limit is exceeded, a `429 Too Many Requests` response is
/// returned immediately — requests are **rejected**, not queued.  The
/// response includes a [`Retry-After`](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Retry-After)
/// header indicating when the window resets.
///
/// All cloned service instances share the same rate counter (via `Arc`),
/// providing a global application-level rate limit.
///
/// # Fixed-window trade-off
///
/// A fixed-window counter can exhibit "thundering-herd" behaviour at window
/// boundaries: if exactly `max_requests` arrive in the first millisecond of
/// a window, the remaining `window_length - 1ms` of that window are
/// effectively dead time, and all those clients retry simultaneously at the
/// start of the next window.  This is a deliberate v1 simplicity trade-off;
/// sliding-window or token-bucket variants may be added in a future version
/// under a different type name.
///
/// # `max_requests` = 0
///
/// A limit of zero is accepted and means **all requests are denied**.  Every
/// call returns a `429 Too Many Requests` with a `Retry-After` header equal
/// to the configured window duration.
///
/// # Quick start
///
/// ```rust
/// use std::time::Duration;
/// use volter_middleware::RateLimitLayer;
/// use volter_router::{Router, get};
///
/// async fn handler() -> &'static str { "ok" }
///
/// let app = Router::new()
///     .route("/", get(handler))
///     .layer(RateLimitLayer::new(100, Duration::from_secs(60)));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct RateLimitLayer {
    max_requests: u64,
    window: Duration,
}

impl RateLimitLayer {
    /// Create a new `RateLimitLayer` with the given maximum requests per
    /// time window.
    ///
    /// A zero-length `window` is clamped to 1 ms so that the earliest
    /// possible window boundary is always at least 1 ms away.
    ///
    /// When `max_requests` is zero, every request is denied.
    pub fn new(max_requests: u64, window: Duration) -> Self {
        let window = if window.is_zero() {
            Duration::from_millis(1)
        } else {
            window
        };
        RateLimitLayer {
            max_requests,
            window,
        }
    }
}

impl Layer<Svc> for RateLimitLayer {
    type Service = RateLimitService;

    fn layer(&self, service: Svc) -> Self::Service {
        RateLimitService {
            inner: service,
            state: Arc::new(Mutex::new(RateLimitState {
                window_start: Instant::now(),
                count: 0,
            })),
            max_requests: self.max_requests,
            window: self.window,
        }
    }
}

/// The [`Service`] produced by [`RateLimitLayer`].
pub struct RateLimitService {
    inner: Svc,
    state: Arc<Mutex<RateLimitState>>,
    max_requests: u64,
    window: Duration,
}

impl Clone for RateLimitService {
    fn clone(&self) -> Self {
        RateLimitService {
            inner: self.inner.clone(),
            // Cloning the Arc shares the same rate counter across all
            // clones — global application-level rate limiting.
            state: self.state.clone(),
            max_requests: self.max_requests,
            window: self.window,
        }
    }
}

/// Internal shared state for the fixed-window rate counter.
#[derive(Clone)]
struct RateLimitState {
    /// Start of the current time window.
    window_start: Instant,
    /// Number of requests counted so far in this window.
    count: u64,
}

impl RateLimitState {
    /// Check whether a request should be allowed.  Advances the window if
    /// it has expired, and increments the counter when allowed.
    fn check_and_increment(&mut self, max_requests: u64, window: Duration) -> bool {
        let now = Instant::now();

        if now >= self.window_start + window {
            self.window_start = now;
            self.count = 0;
        }

        if self.count < max_requests {
            self.count += 1;
            true
        } else {
            false
        }
    }

    /// Seconds until the current window ends, rounded up (minimum 1).
    fn retry_after_secs(&self, window: Duration) -> u64 {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.window_start);
        let remaining = window.saturating_sub(elapsed);

        let secs = remaining.as_secs();
        let has_subsec = remaining.subsec_nanos() > 0;
        (secs + u64::from(has_subsec)).max(1)
    }
}

impl Service<Request> for RateLimitService {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let mut inner = self.inner.clone();
        let state = self.state.clone();
        let max_requests = self.max_requests;
        let window = self.window;

        // Phase 1 — rate check.  The mutex is never held across an `.await`
        // point: the lock guard drops when this block ends.
        let decision = {
            let mut guard = state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if guard.check_and_increment(max_requests, window) {
                RateDecision::Allow
            } else {
                RateDecision::Deny {
                    retry_after: guard.retry_after_secs(window),
                }
            }
        };

        match decision {
            RateDecision::Allow => Box::pin(async move { inner.call(req).await }),
            RateDecision::Deny { retry_after } => {
                Box::pin(async move { Ok(rate_limited_response(retry_after)) })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Outcome of a rate-limit check.
enum RateDecision {
    Allow,
    Deny { retry_after: u64 },
}

/// Build a `429 Too Many Requests` response with a `Retry-After` header.
fn rate_limited_response(retry_after_secs: u64) -> Response {
    let mut response = Response::new(volter_core::empty_body());
    *response.status_mut() = http::StatusCode::TOO_MANY_REQUESTS;
    response.headers_mut().insert(
        http::header::RETRY_AFTER,
        http::HeaderValue::from(retry_after_secs),
    );
    response
}
