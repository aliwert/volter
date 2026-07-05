//! Core traits and types for volter.
//!
//! This crate defines the foundational abstractions the rest of the volter
//! ecosystem builds on: [`Handler`], [`FromRequestParts`], [`FromRequest`],
//! and [`IntoResponse`]. See `ARCHITECTURE.md` at the workspace root for the
//! reasoning behind this design, and `RULES.md` for the constraints every
//! implementation of these traits must follow (no panics on user input, no
//! blocking calls, structured errors only).
//!
//! **Status:** v0.1 sketch. The trait signatures below (particularly the
//! `impl Future ... + Send` return positions) are a starting point and will
//! need refinement once the router and macro crates are built against them
//! — expect to revisit `Send`/object-safety bounds here first.

#![deny(missing_docs)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::future::Future;

pub use http;

/// A type-erased, streaming HTTP body used throughout volter.
pub type BoxBody = http_body_util::combinators::BoxBody<bytes::Bytes, BoxError>;

/// A boxed, dynamic error used where callers need `Send + Sync + 'static`
/// without naming a concrete streaming-body error type.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The request type used throughout volter: a thin alias over
/// [`http::Request`] with a boxed, streaming body.
pub type Request<B = BoxBody> = http::Request<B>;

/// The response type used throughout volter.
pub type Response<B = BoxBody> = http::Response<B>;

/// Turn a value into an HTTP [`Response`].
///
/// This is how handler return types, and extractor rejections, become
/// actual responses. See `RULES.md` #4 ("Error handling") for the
/// constraints implementors of this trait must follow.
pub trait IntoResponse {
    /// Convert `self` into a [`Response`].
    fn into_response(self) -> Response;
}

/// Extract a typed value from the request's [`http::request::Parts`]
/// (method, uri, headers, extensions) without consuming the body.
///
/// Implement this for anything derivable from headers/uri/method alone —
/// e.g. `Path<T>`, `Query<T>`, typed headers, `State<T>`.
pub trait FromRequestParts<S>: Sized {
    /// The response returned when extraction fails.
    type Rejection: IntoResponse;

    /// Perform the extraction.
    fn from_request_parts(
        parts: &mut http::request::Parts,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send;
}

/// Extract a typed value that may need the request body (e.g. `Json<T>`).
///
/// Only one `FromRequest` extractor may appear per handler, and it must be
/// the last argument, since it consumes the body.
pub trait FromRequest<S, B = BoxBody>: Sized {
    /// The response returned when extraction fails.
    type Rejection: IntoResponse;

    /// Perform the extraction, consuming the request.
    fn from_request(
        req: http::Request<B>,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send;
}

/// A request handler: any async function whose arguments are extractors,
/// and whose return type implements [`IntoResponse`].
///
/// `T` is a marker type parameter used to disambiguate handler function
/// arities/argument types via blanket impls (see `volter`'s handler-impl
/// macro invocations once that crate exists).
pub trait Handler<T, S>: Clone + Send + Sized + 'static {
    /// The future returned by [`Handler::call`].
    type Future: Future<Output = Response> + Send + 'static;

    /// Invoke the handler.
    fn call(self, req: Request, state: S) -> Self::Future;
}
