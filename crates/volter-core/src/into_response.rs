//! The [`IntoResponse`] trait and its implementations.
//!
//! Every type that can be converted into an HTTP response implements this
//! trait.  This includes the response type itself, common HTTP primitives
//! like [`StatusCode`], string types, tuples, and the `Result` type.

use crate::body::{empty_body, full_body, Response};
use bytes::Bytes;
use http::StatusCode;

/// Turn a value into an HTTP [`Response`].
///
/// This is how handler return types, and extractor rejections, become
/// actual responses.  See `RULES.md` #4 ("Error handling") for the
/// constraints implementors of this trait must follow.
pub trait IntoResponse {
    /// Convert `self` into a [`Response`].
    fn into_response(self) -> Response;
}

// ---------------------------------------------------------------------------
// Implementations
// ---------------------------------------------------------------------------

impl IntoResponse for Response {
    fn into_response(self) -> Response {
        self
    }
}

impl IntoResponse for StatusCode {
    fn into_response(self) -> Response {
        let mut response = Response::new(empty_body());
        *response.status_mut() = self;
        response
    }
}

impl IntoResponse for &'static str {
    fn into_response(self) -> Response {
        Response::new(full_body(Bytes::from_static(self.as_bytes())))
    }
}

impl IntoResponse for String {
    fn into_response(self) -> Response {
        Response::new(full_body(Bytes::from(self)))
    }
}

impl<T: IntoResponse> IntoResponse for (StatusCode, T) {
    fn into_response(self) -> Response {
        let (status, inner) = self;
        let mut response = inner.into_response();
        *response.status_mut() = status;
        response
    }
}

/// `()` produces a `204 No Content` response with an empty body.
impl IntoResponse for () {
    fn into_response(self) -> Response {
        let mut response = Response::new(empty_body());
        *response.status_mut() = StatusCode::NO_CONTENT;
        response
    }
}

/// `Result<T, E>` delegates to either the `Ok` or the `Err` branch, both of
/// which must implement [`IntoResponse`].
///
/// - `Ok(value)` returns `value.into_response()`.
/// - `Err(error)` returns `error.into_response()`.
impl<T: IntoResponse, E: IntoResponse> IntoResponse for Result<T, E> {
    fn into_response(self) -> Response {
        match self {
            Ok(value) => value.into_response(),
            Err(error) => error.into_response(),
        }
    }
}
