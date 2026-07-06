//! Integration tests for `CatchPanicLayer`.

use std::time::Duration;

use tower::Service;
use volter_middleware::{CatchPanicLayer, TimeoutLayer};

fn get(path: &str) -> volter_core::http::Request<volter_core::BoxBody> {
    volter_core::http::Request::builder()
        .method("GET")
        .uri(path)
        .body(volter_core::empty_body())
        .unwrap()
}

#[tokio::test]
async fn normal_handler_succeeds() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CatchPanicLayer::new());

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn panic_handler_returns_500() {
    async fn handler() -> &'static str {
        panic!("test panic from handler");
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CatchPanicLayer::new());

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 500);
}

#[tokio::test]
async fn panic_with_string_message() {
    async fn handler() -> String {
        panic!("something went wrong");
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CatchPanicLayer::new());

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 500);
}

#[tokio::test]
async fn trace_and_catch_panic_compose() {
    async fn handler() -> &'static str {
        panic!("composed panic");
    }

    // CatchPanicLayer inner, TraceLayer outer.
    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CatchPanicLayer::new())
        .layer(volter_middleware::TraceLayer::new());

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 500);
}

#[tokio::test]
async fn timeout_and_catch_panic_compose() {
    async fn handler() -> &'static str {
        panic!("timed out and panicked");
    }

    // CatchPanicLayer inner, TimeoutLayer outer.
    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CatchPanicLayer::new())
        .layer(TimeoutLayer::new(Duration::from_secs(30)));

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 500);
}

#[tokio::test]
async fn catch_panic_then_timeout_compose() {
    async fn handler() -> &'static str {
        tokio::time::sleep(Duration::from_millis(100)).await;
        "eventually ok"
    }

    // TimeoutLayer inner, CatchPanicLayer outer.
    // Timeout fires first (5ms), CatchPanicLayer catches nothing (timeout returns Ok(408)).
    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(TimeoutLayer::new(Duration::from_millis(5)))
        .layer(CatchPanicLayer::new());

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 408);
}
