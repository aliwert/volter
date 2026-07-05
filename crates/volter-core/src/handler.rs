//! The [`Handler`] trait and its blanket impl for zero-argument functions.
//!
//! A `Handler` is any async function whose arguments are extractors and
//! whose return type implements [`IntoResponse`].  The blanket impl for
//! zero-argument handlers is provided below; multi-extractor impls will
//! be added in a follow-up PR.

use std::future::Future;
use std::pin::Pin;

use crate::body::{BoxBody, Request, Response};
use crate::extract::{FromRequest, FromRequestParts};
use crate::into_response::IntoResponse;

/// A request handler: any async function whose arguments are extractors,
/// and whose return type implements [`IntoResponse`].
///
/// `T` is a marker type parameter used to disambiguate handler function
/// arities/argument types via blanket impls (see the impl for `()` below).
pub trait Handler<T, S>: Clone + Send + Sized + 'static {
    /// The future returned by [`Handler::call`].
    type Future: Future<Output = Response> + Send + 'static;

    /// Invoke the handler.
    fn call(self, req: Request, state: S) -> Self::Future;
}

// ---------------------------------------------------------------------------
// Blanket impl for zero-argument handlers
// ---------------------------------------------------------------------------

/// Blanket impl for closures / functions that take zero extractors.
///
/// Any async fn `F: Fn() -> Fut` where `Fut: Future<Output = R>` and
/// `R: IntoResponse` is a valid zero-argument handler.
impl<F, Fut, R, S> Handler<(), S> for F
where
    F: Fn() -> Fut + Clone + Send + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: IntoResponse,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

    fn call(self, _req: Request, _state: S) -> Self::Future {
        Box::pin(async move { (self)().await.into_response() })
    }
}

// ---------------------------------------------------------------------------
// Blanket impl for single-extractor (FromRequestParts) handlers
// ---------------------------------------------------------------------------

/// Blanket impl for handlers that take a single extractor argument via
/// [`FromRequestParts`].
///
/// Any async fn `F: Fn(E) -> Fut` where `Fut: Future<Output = R>`,
/// `R: IntoResponse`, and `E: FromRequestParts<S>` is a valid handler.
///
/// Unlike the zero-argument impl which ignores the request entirely, this
/// impl splits the request into parts, calls [`FromRequestParts`] to
/// extract `E`, and only then invokes the handler.  The body is dropped
/// (single-extractor handlers never consume the body by definition).
///
/// This covers built-in extractors (`State<S>`, `Path<T>`) and any
/// third-party type that implements [`FromRequestParts<S>`].
impl<F, Fut, R, S, E> Handler<(E,), S> for F
where
    F: Fn(E) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: IntoResponse,
    S: Clone + Send + 'static,
    E: FromRequestParts<S> + Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

    fn call(self, req: Request, state: S) -> Self::Future {
        Box::pin(async move {
            let (mut parts, _body) = req.into_parts();
            let extracted = match E::from_request_parts(&mut parts, &state).await {
                Ok(v) => v,
                Err(rejection) => return rejection.into_response(),
            };
            (self)(extracted).await.into_response()
        })
    }
}

// ---------------------------------------------------------------------------
// Blanket impl for single-extractor (FromRequest) handlers
// ---------------------------------------------------------------------------

/// Blanket impl for handlers that take a single extractor argument that
/// consumes the body via [`FromRequest`].
///
/// Any async fn `F: Fn(E) -> Fut` where `Fut: Future<Output = R>`,
/// `R: IntoResponse`, and `E: FromRequest<S, BoxBody>` is a valid handler.
///
/// Unlike the [`FromRequestParts`] variant which splits the request and
/// drops the body, this impl passes the full request (including body) to
/// [`FromRequest::from_request`].
///
/// The marker type `(E, BoxBody)` (a 2-tuple) is deliberately different
/// from the `(E,)` 1-tuple used by the [`FromRequestParts`] blanket, so
/// the two impls do not overlap.
impl<F, Fut, R, S, E> Handler<(E, BoxBody), S> for F
where
    F: Fn(E) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: IntoResponse,
    S: Clone + Send + 'static,
    E: FromRequest<S, BoxBody> + Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

    fn call(self, req: Request, state: S) -> Self::Future {
        Box::pin(async move {
            let extracted = match E::from_request(req, &state).await {
                Ok(v) => v,
                Err(rejection) => return rejection.into_response(),
            };
            (self)(extracted).await.into_response()
        })
    }
}
