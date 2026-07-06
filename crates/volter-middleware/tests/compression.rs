//! Integration tests for `CompressionLayer`.

use std::time::Duration;

use flate2::read::GzDecoder;
use tower::Service;
use volter_middleware::CompressionLayer;

fn large_body() -> String {
    // DefaultPredicate only compresses bodies > 32 bytes
    "x".repeat(128)
}

fn small_body() -> &'static str {
    "hello"
}

fn request(accept_encoding: Option<&str>) -> volter_core::http::Request<volter_core::BoxBody> {
    let mut builder = volter_core::http::Request::builder().method("GET").uri("/");
    if let Some(ae) = accept_encoding {
        builder = builder.header("accept-encoding", ae);
    }
    builder.body(volter_core::empty_body()).unwrap()
}

#[tokio::test]
async fn compression_works_with_gzip() {
    async fn handler() -> String {
        large_body()
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CompressionLayer::new());

    let response = app.call(request(Some("gzip"))).await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response
            .headers()
            .get("content-encoding")
            .and_then(|v| v.to_str().ok()),
        Some("gzip")
    );

    // Read compressed body and verify it decompresses correctly
    let body = response.into_body();
    let compressed = http_body_util::BodyExt::collect(body)
        .await
        .unwrap()
        .to_bytes();
    let mut decoder = GzDecoder::new(&compressed[..]);
    let mut decompressed = String::new();
    std::io::Read::read_to_string(&mut decoder, &mut decompressed).unwrap();
    assert_eq!(decompressed, large_body());
}

#[tokio::test]
async fn no_compression_without_accept_encoding() {
    async fn handler() -> String {
        large_body()
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CompressionLayer::new());

    let response = app.call(request(None)).await.unwrap();
    assert_eq!(response.status(), 200);
    assert!(
        response.headers().get("content-encoding").is_none(),
        "no compression without Accept-Encoding"
    );

    // Body should be uncompressed
    let body = response.into_body();
    let bytes = http_body_util::BodyExt::collect(body)
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(bytes, large_body().as_bytes());
}

#[tokio::test]
async fn status_code_preserved() {
    async fn handler() -> (http::StatusCode, String) {
        (http::StatusCode::NOT_FOUND, large_body())
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CompressionLayer::new());

    let response = app.call(request(Some("gzip"))).await.unwrap();
    assert_eq!(response.status(), 404);
    assert_eq!(
        response
            .headers()
            .get("content-encoding")
            .and_then(|v| v.to_str().ok()),
        Some("gzip")
    );
}

#[tokio::test]
async fn small_body_not_compressed() {
    async fn handler() -> &'static str {
        small_body()
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CompressionLayer::new());

    let response = app.call(request(Some("gzip"))).await.unwrap();
    assert_eq!(response.status(), 200);
    // DefaultPredicate's SizeAbove only compresses bodies > 32 bytes
    assert!(
        response.headers().get("content-encoding").is_none(),
        "small body should not be compressed"
    );

    let body = response.into_body();
    let bytes = http_body_util::BodyExt::collect(body)
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(bytes, small_body().as_bytes());
}

#[tokio::test]
async fn compression_composes_with_timeout() {
    async fn handler() -> String {
        large_body()
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CompressionLayer::new())
        .layer(volter_middleware::TimeoutLayer::new(Duration::from_secs(
            30,
        )));

    let response = app.call(request(Some("gzip"))).await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response
            .headers()
            .get("content-encoding")
            .and_then(|v| v.to_str().ok()),
        Some("gzip")
    );
}

#[tokio::test]
async fn compression_composes_with_trace() {
    async fn handler() -> String {
        large_body()
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CompressionLayer::new())
        .layer(volter_middleware::TraceLayer::new());

    let response = app.call(request(Some("gzip"))).await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response
            .headers()
            .get("content-encoding")
            .and_then(|v| v.to_str().ok()),
        Some("gzip")
    );
}

#[tokio::test]
async fn compression_gzip_layer_works() {
    async fn handler() -> String {
        large_body()
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CompressionLayer::gzip());

    let response = app.call(request(Some("gzip"))).await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response
            .headers()
            .get("content-encoding")
            .and_then(|v| v.to_str().ok()),
        Some("gzip")
    );
}

#[tokio::test]
async fn compression_rejects_unsupported_encoding() {
    async fn handler() -> String {
        large_body()
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CompressionLayer::gzip());

    // Client asks for br, but we only accept gzip
    let response = app.call(request(Some("br"))).await.unwrap();
    assert_eq!(response.status(), 200);
    assert!(
        response.headers().get("content-encoding").is_none(),
        "should not compress when client asks for unsupported encoding"
    );
}
