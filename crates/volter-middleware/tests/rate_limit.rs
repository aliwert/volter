//! Integration tests for `RateLimitLayer`.

use std::time::Duration;

use tower::Service;
use volter_middleware::RateLimitLayer;

fn get(path: &str) -> volter_core::http::Request<volter_core::BoxBody> {
    volter_core::http::Request::builder()
        .method("GET")
        .uri(path)
        .body(volter_core::empty_body())
        .unwrap()
}

// ---------------------------------------------------------------------------
// Happy path — requests within the limit
// ---------------------------------------------------------------------------

#[tokio::test]
async fn requests_below_limit_succeed() {
    async fn handler() -> &'static str {
        "ok"
    }

    // 100 requests per 60 s — we only send 3, all should pass.
    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RateLimitLayer::new(100, Duration::from_secs(60)));

    for _ in 0..3 {
        let response = app.call(get("/")).await.unwrap();
        assert_eq!(response.status(), 200);
    }
}

#[tokio::test]
async fn request_at_limit_succeeds() {
    async fn handler() -> &'static str {
        "ok"
    }

    // Exactly 5 requests with a limit of 5 — all should pass.
    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RateLimitLayer::new(5, Duration::from_secs(60)));

    for _ in 0..5 {
        let response = app.call(get("/")).await.unwrap();
        assert_eq!(response.status(), 200);
    }
}

// ---------------------------------------------------------------------------
// Rate limit exceeded
// ---------------------------------------------------------------------------

#[tokio::test]
async fn request_above_limit_returns_429() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RateLimitLayer::new(2, Duration::from_secs(60)));

    // First two requests succeed.
    let r1 = app.call(get("/")).await.unwrap();
    assert_eq!(r1.status(), 200);

    let r2 = app.call(get("/")).await.unwrap();
    assert_eq!(r2.status(), 200);

    // Third request is rate-limited.
    let r3 = app.call(get("/")).await.unwrap();
    assert_eq!(r3.status(), 429);
}

// ---------------------------------------------------------------------------
// Retry-After header
// ---------------------------------------------------------------------------

#[tokio::test]
async fn retry_after_header_present_on_429() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RateLimitLayer::new(1, Duration::from_secs(60)));

    // Consume the only permit.
    let _ = app.call(get("/")).await.unwrap();

    // Exceeded — must carry Retry-After.
    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 429);
    assert!(response.headers().get("retry-after").is_some());
}

#[tokio::test]
async fn retry_after_value_is_reasonable() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RateLimitLayer::new(1, Duration::from_secs(60)));

    let _ = app.call(get("/")).await.unwrap();
    let response = app.call(get("/")).await.unwrap();

    let retry_after: u64 = response
        .headers()
        .get("retry-after")
        .unwrap()
        .to_str()
        .unwrap()
        .parse()
        .unwrap();

    // Should be between 1 and 60 (window duration).
    assert!(retry_after >= 1, "retry-after should be >= 1");
    assert!(retry_after <= 60, "retry-after should be <= window");
}

// ---------------------------------------------------------------------------
// Window reset
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rate_resets_after_window() {
    async fn handler() -> &'static str {
        "ok"
    }

    // 1 request per 50 ms window.
    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RateLimitLayer::new(1, Duration::from_millis(50)));

    // First request succeeds.
    let r1 = app.call(get("/")).await.unwrap();
    assert_eq!(r1.status(), 200);

    // Second request (same window) is denied.
    let r2 = app.call(get("/")).await.unwrap();
    assert_eq!(r2.status(), 429);

    // Wait for the window to pass.
    tokio::time::sleep(Duration::from_millis(60)).await;

    // Now it should succeed again.
    let r3 = app.call(get("/")).await.unwrap();
    assert_eq!(r3.status(), 200);
}

// ---------------------------------------------------------------------------
// max_requests = 0 denies everything
// ---------------------------------------------------------------------------

#[tokio::test]
async fn zero_limit_denies_all_requests() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RateLimitLayer::new(0, Duration::from_secs(60)));

    let r1 = app.call(get("/")).await.unwrap();
    assert_eq!(r1.status(), 429);

    let r2 = app.call(get("/")).await.unwrap();
    assert_eq!(r2.status(), 429);
}

// ---------------------------------------------------------------------------
// Composition with existing middleware
// ---------------------------------------------------------------------------

#[tokio::test]
async fn composes_with_timeout() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RateLimitLayer::new(10, Duration::from_secs(60)))
        .layer(volter_middleware::TimeoutLayer::new(Duration::from_secs(
            30,
        )));

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn composes_with_catch_panic() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RateLimitLayer::new(10, Duration::from_secs(60)))
        .layer(volter_middleware::CatchPanicLayer::new());

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn composes_with_trace() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RateLimitLayer::new(10, Duration::from_secs(60)))
        .layer(volter_middleware::TraceLayer::new());

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn composes_with_concurrency_limit() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RateLimitLayer::new(10, Duration::from_secs(60)))
        .layer(volter_middleware::ConcurrencyLimitLayer::new(5));

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 200);
}
