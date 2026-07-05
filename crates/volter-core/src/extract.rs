//! Extractor traits for request deserialization.
//!
//! These traits define how typed values are extracted from incoming HTTP
//! requests.  [`FromRequestParts`] extracts from the request's metadata
//! (method, URI, headers, extensions) without consuming the body.
//! [`FromRequest`] may consume the request body.

use std::future::Future;

use crate::body::BoxBody;
use crate::into_response::IntoResponse;

/// Extract a typed value from the request's [`http::request::Parts`]
/// (method, URI, headers, extensions) without consuming the body.
///
/// Implement this for anything derivable from headers/URI/method alone —
/// e.g. `Path<T>`, `Query<T>`, typed headers, `State<T>`.
pub trait FromRequestParts<S>: Sized {
    /// The response returned when extraction fails.
    type Rejection: IntoResponse;

    /// The future returned by [`from_request_parts`].
    type Future: Future<Output = Result<Self, Self::Rejection>> + Send;

    /// Perform the extraction.
    fn from_request_parts(parts: &mut http::request::Parts, state: &S) -> Self::Future;
}

/// Extract a typed value that may need the request body (e.g. `Json<T>`).
///
/// Only one `FromRequest` extractor may appear per handler, and it must be
/// the last argument, since it consumes the body.
pub trait FromRequest<S, B = BoxBody>: Sized {
    /// The response returned when extraction fails.
    type Rejection: IntoResponse;

    /// The future returned by [`from_request`].
    type Future: Future<Output = Result<Self, Self::Rejection>> + Send;

    /// Perform the extraction, consuming the request.
    fn from_request(req: http::Request<B>, state: &S) -> Self::Future;
}
