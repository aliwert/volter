//! Integration tests for `ConcurrencyLimitLayer`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tower::Service;
use volter_middleware::ConcurrencyLimitLayer;

fn get(path: &str) -> volter_core::http::Request<volter_core::BoxBody> {
    volter_core::http::Request::builder()
        .method("GET")
        .uri(path)
        .body(volter_core::empty_body())
        .unwrap()
}

#[tokio::test]
async fn single_request_succeeds() {
    async fn handler() -> &'static str {
        "ok"
    }

    // Router::call takes &mut self, so need mut.
    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(ConcurrencyLimitLayer::new(10));

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn multiple_requests_within_limit_all_succeed() {
    async fn handler() -> &'static str {
        "ok"
    }

    let app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(ConcurrencyLimitLayer::new(10));

    let mut handles = Vec::new();
    for _ in 0..5 {
        let mut app = app.clone();
        handles.push(tokio::spawn(async move { app.call(get("/")).await }));
    }

    for handle in handles {
        let response = handle.await.unwrap().unwrap();
        assert_eq!(response.status(), 200);
    }
}

#[tokio::test]
async fn limit_one_allows_one_request_at_a_time() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    async fn handler(c: Arc<AtomicUsize>) -> &'static str {
        let prev = c.fetch_add(1, Ordering::SeqCst);
        assert_eq!(prev, 0, "expected no concurrent requests");
        tokio::time::sleep(Duration::from_millis(20)).await;
        c.fetch_sub(1, Ordering::SeqCst);
        "ok"
    }

    let app = volter_router::Router::new()
        .route(
            "/",
            volter_router::get(move || handler(counter_clone.clone())),
        )
        .layer(ConcurrencyLimitLayer::new(1));

    let mut handles = Vec::new();
    for _ in 0..3 {
        let mut app = app.clone();
        handles.push(tokio::spawn(async move { app.call(get("/")).await }));
    }

    for handle in handles {
        let response = handle.await.unwrap().unwrap();
        assert_eq!(response.status(), 200);
    }
}

#[tokio::test]
async fn permit_released_after_request_completes() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    async fn handler(c: Arc<AtomicUsize>) -> &'static str {
        c.fetch_add(1, Ordering::SeqCst);
        "ok"
    }

    let app = volter_router::Router::new()
        .route(
            "/",
            volter_router::get(move || handler(counter_clone.clone())),
        )
        .layer(ConcurrencyLimitLayer::new(1));

    for _ in 0..3 {
        let mut app = app.clone();
        let response = app.call(get("/")).await.unwrap();
        assert_eq!(response.status(), 200);
    }

    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn concurrency_limit_composes_with_timeout() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(ConcurrencyLimitLayer::new(10))
        .layer(volter_middleware::TimeoutLayer::new(Duration::from_secs(
            30,
        )));

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn concurrency_limit_composes_with_catch_panic() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(ConcurrencyLimitLayer::new(10))
        .layer(volter_middleware::CatchPanicLayer::new());

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn concurrency_limit_composes_with_trace() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(ConcurrencyLimitLayer::new(10))
        .layer(volter_middleware::TraceLayer::new());

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 200);
}
