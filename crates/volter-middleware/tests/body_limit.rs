//! Integration tests for `RequestBodyLimitLayer`.

use std::time::Duration;

use tower::Service;
use volter_core::full_body;
use volter_middleware::RequestBodyLimitLayer;

fn body_from(data: &[u8]) -> volter_core::BoxBody {
    full_body(bytes::Bytes::copy_from_slice(data))
}

fn request_with_len(data: &[u8]) -> volter_core::http::Request<volter_core::BoxBody> {
    let len = data.len();
    let body = body_from(data);
    volter_core::http::Request::builder()
        .method("GET")
        .uri("/")
        .header("content-type", "application/json")
        .header("content-length", len.to_string())
        .body(body)
        .unwrap()
}

/// Helper: request with no Content-Length header.
fn request_no_cl(body: volter_core::BoxBody) -> volter_core::http::Request<volter_core::BoxBody> {
    volter_core::http::Request::builder()
        .method("GET")
        .uri("/")
        .body(body)
        .unwrap()
}

#[tokio::test]
async fn body_below_limit_succeeds() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RequestBodyLimitLayer::new(1024));

    let response = app
        .call(request_with_len(b"{\"name\": \"test\"}"))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn body_at_limit_succeeds() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RequestBodyLimitLayer::new(10));

    let response = app
        .call(request_with_len(b"1234567890")) // exactly 10 bytes
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn body_above_limit_returns_413() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RequestBodyLimitLayer::new(10));

    let response = app
        .call(request_with_len(b"12345678901")) // 11 bytes, exceeds 10 byte limit
        .await
        .unwrap();
    assert_eq!(response.status(), 413);
}

#[tokio::test]
async fn json_extractor_rejected_when_body_too_large() {
    use serde::Deserialize;
    use volter_extract::Json;

    #[derive(Deserialize)]
    struct User {
        _name: String,
    }

    async fn handler(_: Json<User>) -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RequestBodyLimitLayer::new(10));

    let response = app.call(request_with_len(b"12345678901")).await.unwrap();
    // Handler shouldn't run — body limit rejected before extraction
    assert_eq!(response.status(), 413);
}

#[tokio::test]
async fn different_limits_work() {
    async fn handler() -> &'static str {
        "ok"
    }

    // 100 byte limit — body below
    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RequestBodyLimitLayer::new(100));

    let response = app.call(request_with_len(&[b'a'; 50])).await.unwrap();
    assert_eq!(response.status(), 200);

    // 100 byte limit — body above
    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RequestBodyLimitLayer::new(100));

    let response = app.call(request_with_len(&[b'a'; 101])).await.unwrap();
    assert_eq!(response.status(), 413);
}

#[tokio::test]
async fn body_limit_composes_with_timeout() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RequestBodyLimitLayer::new(10))
        .layer(volter_middleware::TimeoutLayer::new(Duration::from_secs(
            30,
        )));

    // Body below limit
    let response = app.call(request_with_len(b"small")).await.unwrap();
    assert_eq!(response.status(), 200);

    // Body above limit — should get 413 from body limit, not timeout
    let response = app.call(request_with_len(b"toolargebody")).await.unwrap();
    assert_eq!(response.status(), 413);
}

#[tokio::test]
async fn body_limit_composes_with_catch_panic() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RequestBodyLimitLayer::new(10))
        .layer(volter_middleware::CatchPanicLayer::new());

    let response = app.call(request_with_len(b"toolargebody")).await.unwrap();
    assert_eq!(response.status(), 413);
}

#[tokio::test]
async fn body_limit_composes_with_trace() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RequestBodyLimitLayer::new(10))
        .layer(volter_middleware::TraceLayer::new());

    let response = app.call(request_with_len(b"toolargebody")).await.unwrap();
    assert_eq!(response.status(), 413);
}

#[tokio::test]
async fn body_without_content_length_passes_through() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(RequestBodyLimitLayer::new(10));

    // Request without Content-Length header
    let response = app
        .call(request_no_cl(body_from(
            b"this is a long body without a content-length header",
        )))
        .await
        .unwrap();
    // Without Content-Length, we can't reject early
    assert_eq!(response.status(), 200);
}
