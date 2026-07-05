//! HTTP error response helpers.
//!
//! Utilities for building common error responses (404, 405) without using
//! `unwrap`, `expect`, or `panic`.

use http::StatusCode;
use volter_core::{empty_body, Response};

/// Build a `404 Not Found` response with an empty body.
pub(crate) fn not_found_response() -> Response {
    let (mut parts, body) = Response::new(empty_body()).into_parts();
    parts.status = StatusCode::NOT_FOUND;
    Response::from_parts(parts, body)
}

/// Build a `405 Method Not Allowed` response with an empty body.
pub(crate) fn method_not_allowed_response() -> Response {
    let (mut parts, body) = Response::new(empty_body()).into_parts();
    parts.status = StatusCode::METHOD_NOT_ALLOWED;
    Response::from_parts(parts, body)
}
