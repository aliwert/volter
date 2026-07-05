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

/// Extract typed application state.
///
/// `State<T>` extracts a value of type `T` from the router's application
/// state.  The state is cloned on each request — it must implement
/// [`Clone`].
///
/// The extraction always succeeds (the rejection type is [`Infallible`])
/// because the state type is checked at compile time: a handler that asks
/// for `State<Foo>` on a router configured with `Bar` will not compile.
///
/// # Example
///
/// ```rust
/// use volter_core::{State, Handler};
///
/// #[derive(Clone)]
/// struct AppState { counter: u64 }
///
/// async fn handler(State(state): State<AppState>) -> String {
///     format!("counter: {}", state.counter)
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct State<T>(pub T);

impl<S: Clone + Send + 'static> FromRequestParts<S> for State<S> {
    type Rejection = std::convert::Infallible;
    type Future = std::future::Ready<Result<Self, Self::Rejection>>;

    fn from_request_parts(_parts: &mut http::request::Parts, state: &S) -> Self::Future {
        std::future::ready(Ok(State(state.clone())))
    }
}
