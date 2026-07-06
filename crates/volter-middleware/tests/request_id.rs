//! Integration tests for `RequestIdLayer`.

use std::collections::HashSet;
use std::time::Duration;

use tower::Service;
use volter_middleware::{CatchPanicLayer, RequestId, RequestIdLayer, TimeoutLayer};

fn get(path: &str) -> volter_core::http::Request<volter_core::BoxBody> {
    volter_core::http::Request::builder()
        .method("GET")
        .uri(path)
        .body(volter_core::empty_body())
        .unwrap()
}

fn get_with_header(
    path: &str,
    header_name: &str,
    header_value: &str,
) -> volter_core::http::Request<volter_core::BoxBody> {
    volter_core::http::Request::builder()
        .method("GET")
        .uri(path)
        .header(header_name, header_value)
        .body(volter_core::empty_body())
        .unwrap()
}

#[tokio::test]
async fn generated_request_id() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RequestIdLayer::new());

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 200);
    let header = response.headers().get("x-request-id");
    assert!(header.is_some(), "response should have X-Request-Id header");
}

#[tokio::test]
async fn existing_request_id_preserved() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RequestIdLayer::new());

    let provided_id = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    let response = app
        .call(get_with_header("/", "x-request-id", provided_id))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let response_id = response
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok());
    assert_eq!(response_id, Some(provided_id));
}

#[tokio::test]
async fn response_contains_x_request_id() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RequestIdLayer::new());

    let response = app.call(get("/")).await.unwrap();
    let header = response.headers().get("x-request-id");
    assert!(
        header.is_some(),
        "response should contain X-Request-Id header"
    );
    let value = header.and_then(|v| v.to_str().ok()).unwrap();
    assert!(!value.is_empty(), "X-Request-Id should not be empty");
    assert_eq!(value.len(), 26, "ULID should be 26 characters");
}

#[tokio::test]
async fn extension_request_id_works() {
    async fn handler(ext: volter_extract::Extension<RequestId>) -> String {
        ext.0.to_string()
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RequestIdLayer::new());

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.headers().get("x-request-id").is_some());
}

#[tokio::test]
async fn multiple_concurrent_requests_different_ids() {
    async fn handler() -> &'static str {
        "ok"
    }

    let app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RequestIdLayer::new());

    let mut app1 = app.clone();
    let mut app2 = app.clone();

    let (response1, response2) = tokio::join!(app1.call(get("/a")), app2.call(get("/b")),);

    let id1 = response1
        .unwrap()
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .to_owned();
    let id2 = response2
        .unwrap()
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .to_owned();

    assert_ne!(id1, id2, "concurrent requests should have different IDs");
}

#[tokio::test]
async fn request_id_layer_composes_with_trace() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RequestIdLayer::new())
        .layer(volter_middleware::TraceLayer::new());

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.headers().get("x-request-id").is_some());
}

#[tokio::test]
async fn request_id_layer_composes_with_timeout() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RequestIdLayer::new())
        .layer(TimeoutLayer::new(Duration::from_secs(30)));

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.headers().get("x-request-id").is_some());
}

#[tokio::test]
async fn request_id_layer_composes_with_catch_panic() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RequestIdLayer::new())
        .layer(CatchPanicLayer::new());

    let response = app.call(get("/")).await.unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.headers().get("x-request-id").is_some());
}

#[tokio::test]
async fn request_id_hash_and_eq() {
    let id1 = RequestId::new();
    let id2 = id1;
    assert_eq!(id1, id2);

    let id3 = RequestId::new();
    let mut set = HashSet::new();
    set.insert(id1);
    assert!(set.contains(&id2));
    assert!(!set.contains(&id3));
}
