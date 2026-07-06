//! Integration tests for `TraceLayer`.
//!
//! These tests verify that `TraceLayer` composes with `Router::layer()`
//! without asserting on actual log output.

use tower::Service;
use volter_middleware::TraceLayer;

fn get(path: &str) -> volter_core::http::Request<volter_core::BoxBody> {
    volter_core::http::Request::builder()
        .method("GET")
        .uri(path)
        .body(volter_core::empty_body())
        .unwrap()
}

#[tokio::test]
async fn trace_layer_wraps_router() {
    async fn handler() -> &'static str {
        "hello"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(TraceLayer::new());

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn response_status_unchanged() {
    async fn handler() -> volter_core::http::StatusCode {
        volter_core::http::StatusCode::NOT_FOUND
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(TraceLayer::new());

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn trace_layer_is_clone() {
    let layer = TraceLayer::new();
    let _cloned = layer.clone();
}

#[tokio::test]
async fn trace_layer_works_with_nested_layers() {
    async fn handler() -> &'static str {
        "nested"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(TraceLayer::new())
        .layer(TraceLayer::new());

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 200);
}
