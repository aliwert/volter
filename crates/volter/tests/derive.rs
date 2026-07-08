//! Integration tests for the `FromRequestParts` and `FromRequest` derive macros.
//!
//! These tests verify that the generated trait implementations work correctly
//! with real requests, including rejection handling.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use tower::Service;
use volter::*;
use volter_core::full_body;

// ---------------------------------------------------------------------------
// Helper: create a basic GET request with optional query string
// ---------------------------------------------------------------------------

fn get_request(path: &str) -> http::Request<BoxBody> {
    http::Request::builder()
        .uri(path)
        .method(http::Method::GET)
        .body(volter_core::empty_body())
        .unwrap()
}

fn post_json(path: &str, body: &[u8]) -> http::Request<BoxBody> {
    http::Request::builder()
        .uri(path)
        .method(http::Method::POST)
        .header("content-type", "application/json")
        .body(full_body(bytes::Bytes::copy_from_slice(body)))
        .unwrap()
}

// ---------------------------------------------------------------------------
// FromRequestParts derive
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize, FromRequestParts)]
struct Pagination {
    page: u32,
    per_page: u32,
}

#[tokio::test]
async fn from_request_parts_extracts_query_params() {
    let req = get_request("/list?page=2&per_page=20");
    let (mut parts, _body) = req.into_parts();

    let result = Pagination::from_request_parts(&mut parts, &()).await;
    let pagination = result.unwrap();
    assert_eq!(pagination.page, 2);
    assert_eq!(pagination.per_page, 20);
}

#[tokio::test]
async fn from_request_parts_rejects_missing_params() {
    let req = get_request("/list");
    let (mut parts, _body) = req.into_parts();

    let result = Pagination::from_request_parts(&mut parts, &()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn from_request_parts_rejects_invalid_params() {
    let req = get_request("/list?page=abc&per_page=20");
    let (mut parts, _body) = req.into_parts();

    let result = Pagination::from_request_parts(&mut parts, &()).await;
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// FromRequest derive
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize, FromRequest)]
struct CreateUser {
    name: String,
    age: u32,
}

#[tokio::test]
async fn from_request_extracts_json_body() {
    let req = post_json("/users", br#"{"name":"Alice","age":30}"#);

    let result = CreateUser::from_request(req, &()).await;
    let user = result.unwrap();
    assert_eq!(user.name, "Alice");
    assert_eq!(user.age, 30);
}

#[tokio::test]
async fn from_request_rejects_invalid_json() {
    let req = post_json("/users", br#"not valid json"#);

    let result = CreateUser::from_request(req, &()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn from_request_rejects_missing_content_type() {
    let req = http::Request::builder()
        .uri("/users")
        .method(http::Method::POST)
        .body(full_body(bytes::Bytes::from_static(b"{}")))
        .unwrap();

    let result = CreateUser::from_request(req, &()).await;
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Integration with Router and handler
// ---------------------------------------------------------------------------

async fn create_user(user: CreateUser) -> String {
    format!("created {} (age {})", user.name, user.age)
}

#[tokio::test]
async fn from_request_works_in_handler() {
    let mut app = Router::new().route("/users", post(create_user));

    let req = post_json("/users", br#"{"name":"Bob","age":25}"#);
    let resp = app.call(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Rejection as IntoResponse
// ---------------------------------------------------------------------------

#[tokio::test]
async fn from_request_parts_rejection_is_into_response() {
    let req = get_request("/list?page=abc");
    let (mut parts, _body) = req.into_parts();
    let result = Pagination::from_request_parts(&mut parts, &()).await;
    let err = result.unwrap_err();
    let resp = err.into_response();
    assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn from_request_rejection_is_into_response() {
    let req = post_json("/users", br#"not valid json"#);
    let result = CreateUser::from_request(req, &()).await;
    let err = result.unwrap_err();
    let resp = err.into_response();
    assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
}
