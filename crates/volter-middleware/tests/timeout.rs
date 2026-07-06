//! Integration tests for `TimeoutLayer`.
//!
//! Tests use short (1 ms) timeouts and handlers with `tokio::time::sleep` to
//! exercise the timeout path deterministically.

use std::time::Duration;

use tower::Service;
use volter_middleware::TimeoutLayer;

fn get(path: &str) -> volter_core::http::Request<volter_core::BoxBody> {
    volter_core::http::Request::builder()
        .method("GET")
        .uri(path)
        .body(volter_core::empty_body())
        .unwrap()
}

#[tokio::test]
async fn fast_handler_succeeds() {
    async fn handler() -> &'static str {
        "fast"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(TimeoutLayer::new(Duration::from_secs(30)));

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn slow_handler_times_out() {
    async fn handler() -> &'static str {
        tokio::time::sleep(Duration::from_millis(100)).await;
        "slow"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(TimeoutLayer::new(Duration::from_millis(1)));

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 408);
}

#[tokio::test]
async fn configured_duration_respected() {
    // Use a short timeout that fires.
    async fn handler() -> &'static str {
        tokio::time::sleep(Duration::from_millis(50)).await;
        "slow"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(TimeoutLayer::new(Duration::from_millis(5)));

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 408);
}

#[tokio::test]
async fn multiple_timeout_layers_compose() {
    async fn handler() -> &'static str {
        tokio::time::sleep(Duration::from_millis(50)).await;
        "slow"
    }

    // Outer timeout (last .layer() call is outermost) fires first.
    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(TimeoutLayer::new(Duration::from_secs(10)))
        .layer(TimeoutLayer::new(Duration::from_millis(5)));

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 408);
}

#[tokio::test]
async fn multiple_timeout_layers_inner_would_fire_but_outer_wins() {
    async fn handler() -> &'static str {
        tokio::time::sleep(Duration::from_millis(100)).await;
        "slow"
    }

    // Outer timeout (5 ms) fires before inner timeout (200 ms).
    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(TimeoutLayer::new(Duration::from_millis(200)))
        .layer(TimeoutLayer::new(Duration::from_millis(5)));

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 408);
}

#[tokio::test]
async fn trace_and_timeout_compose() {
    async fn handler() -> &'static str {
        tokio::time::sleep(Duration::from_millis(50)).await;
        "slow"
    }

    // TraceLayer outermost, TimeoutLayer inner.
    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(TimeoutLayer::new(Duration::from_millis(5)))
        .layer(volter_middleware::TraceLayer::new());

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 408);
}
