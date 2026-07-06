//! Integration tests for `CorsLayer`.

use std::time::Duration;

use tower::Service;
use volter_middleware::CorsLayer;

fn get(path: &str, origin: Option<&str>) -> volter_core::http::Request<volter_core::BoxBody> {
    let mut builder = volter_core::http::Request::builder()
        .method("GET")
        .uri(path);
    if let Some(o) = origin {
        builder = builder.header("origin", o);
    }
    builder.body(volter_core::empty_body()).unwrap()
}

fn options(
    path: &str,
    origin: &str,
    req_method: &str,
) -> volter_core::http::Request<volter_core::BoxBody> {
    volter_core::http::Request::builder()
        .method("OPTIONS")
        .uri(path)
        .header("origin", origin)
        .header("access-control-request-method", req_method)
        .body(volter_core::empty_body())
        .unwrap()
}

fn options_with_headers(
    path: &str,
    origin: &str,
    req_method: &str,
    req_headers: &str,
) -> volter_core::http::Request<volter_core::BoxBody> {
    volter_core::http::Request::builder()
        .method("OPTIONS")
        .uri(path)
        .header("origin", origin)
        .header("access-control-request-method", req_method)
        .header("access-control-request-headers", req_headers)
        .body(volter_core::empty_body())
        .unwrap()
}

#[tokio::test]
async fn permissive_get_with_origin() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CorsLayer::permissive());

    let response = app
        .call(get("/", Some("https://example.com")))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("*")
    );
}

#[tokio::test]
async fn permissive_get_without_origin() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CorsLayer::permissive());

    let response = app.call(get("/", None)).await.unwrap();
    assert_eq!(response.status(), 200);
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none(),
        "no CORS headers without Origin header"
    );
}

#[tokio::test]
async fn permissive_preflight() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CorsLayer::permissive());

    let response = app
        .call(options("/", "https://example.com", "GET"))
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("*")
    );
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-methods")
            .and_then(|v| v.to_str().ok()),
        Some("GET")
    );
}

#[tokio::test]
async fn specific_origin_allowed() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(
            CorsLayer::new()
                .allow_origin("https://allowed.com")
                .allow_any_method()
                .allow_any_header(),
        );

    let response = app
        .call(get("/", Some("https://allowed.com")))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("https://allowed.com")
    );
}

#[tokio::test]
async fn specific_origin_disallowed() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(
            CorsLayer::new()
                .allow_origin("https://allowed.com")
                .allow_any_method()
                .allow_any_header(),
        );

    let response = app.call(get("/", Some("https://evil.com"))).await.unwrap();
    assert_eq!(response.status(), 200);
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none(),
        "disallowed origin should not get CORS headers"
    );
}

#[tokio::test]
async fn multiple_origins() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(
            CorsLayer::new()
                .allow_origin("https://a.com")
                .allow_origin("https://b.com")
                .allow_any_method()
                .allow_any_header(),
        );

    for origin in &["https://a.com", "https://b.com"] {
        let response = app.call(get("/", Some(origin))).await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .and_then(|v| v.to_str().ok()),
            Some(*origin),
            "origin {} should be allowed",
            origin
        );
    }
}

#[tokio::test]
async fn credentials_echos_origin() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CorsLayer::permissive().allow_credentials());

    let response = app
        .call(get("/", Some("https://example.com")))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    // With credentials, should echo origin instead of *
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("https://example.com")
    );
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-credentials")
            .and_then(|v| v.to_str().ok()),
        Some("true")
    );
}

#[tokio::test]
async fn max_age_header() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CorsLayer::permissive().max_age(Duration::from_secs(3600)));

    let response = app
        .call(options("/", "https://example.com", "GET"))
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
    assert_eq!(
        response
            .headers()
            .get("access-control-max-age")
            .and_then(|v| v.to_str().ok()),
        Some("3600")
    );
}

#[tokio::test]
async fn expose_headers() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CorsLayer::permissive().expose_headers([
            http::header::CONTENT_TYPE,
            http::header::HeaderName::from_static("x-custom"),
        ]));

    let response = app
        .call(get("/", Some("https://example.com")))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let expose = response
        .headers()
        .get("access-control-expose-headers")
        .and_then(|v| v.to_str().ok());
    assert!(expose.is_some(), "expose headers should be set");
    let expose = expose.unwrap();
    assert!(expose.contains("content-type"));
    assert!(expose.contains("x-custom"));
}

#[tokio::test]
async fn specific_methods_on_preflight() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(
            CorsLayer::new()
                .allow_origin("https://example.com")
                .allow_methods([http::Method::GET, http::Method::POST])
                .allow_any_header(),
        );

    let response = app
        .call(options("/", "https://example.com", "GET"))
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
    let methods = response
        .headers()
        .get("access-control-allow-methods")
        .and_then(|v| v.to_str().ok())
        .unwrap();
    assert!(methods.contains("GET"));
    assert!(methods.contains("POST"));
}

#[tokio::test]
async fn specific_headers_on_preflight() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(
            CorsLayer::new()
                .allow_origin("https://example.com")
                .allow_any_method()
                .allow_headers([
                    http::header::CONTENT_TYPE,
                    http::header::HeaderName::from_static("x-custom"),
                ]),
        );

    let response = app
        .call(options_with_headers(
            "/",
            "https://example.com",
            "GET",
            "content-type",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
    let headers = response
        .headers()
        .get("access-control-allow-headers")
        .and_then(|v| v.to_str().ok())
        .unwrap();
    assert!(headers.contains("content-type"));
    assert!(headers.contains("x-custom"));
}

#[tokio::test]
async fn preflight_disallowed_origin() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(
            CorsLayer::new()
                .allow_origin("https://allowed.com")
                .allow_any_method()
                .allow_any_header(),
        );

    let response = app
        .call(options("/", "https://evil.com", "GET"))
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none(),
        "disallowed origin preflight should not get CORS headers"
    );
}

#[tokio::test]
async fn vary_header_present() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(
            CorsLayer::new()
                .allow_origin("https://example.com")
                .allow_any_method()
                .allow_any_header(),
        );

    let response = app
        .call(get("/", Some("https://example.com")))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers().get("vary").and_then(|v| v.to_str().ok()),
        Some("origin")
    );
}

#[tokio::test]
async fn non_preflight_options_no_origin() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CorsLayer::permissive());

    // OPTIONS without Origin header should not be treated as preflight;
    // it falls through to the router which has no OPTIONS handler → 405
    let req = volter_core::http::Request::builder()
        .method("OPTIONS")
        .uri("/")
        .body(volter_core::empty_body())
        .unwrap();
    let response = app.call(req).await.unwrap();
    assert_eq!(response.status(), 405);
}

#[tokio::test]
async fn cors_composes_with_timeout() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CorsLayer::permissive())
        .layer(volter_middleware::TimeoutLayer::new(Duration::from_secs(
            30,
        )));

    let response = app
        .call(get("/", Some("https://example.com")))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("*")
    );
}

#[tokio::test]
async fn cors_composes_with_catch_panic() {
    async fn handler() -> &'static str {
        "ok"
    }

    let mut app = volter_router::Router::new()
        .route("/", volter_router::get(handler))
        .layer(CorsLayer::permissive())
        .layer(volter_middleware::CatchPanicLayer::new());

    let response = app
        .call(get("/", Some("https://example.com")))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("*")
    );
}
