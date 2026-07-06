//! The [`Handler`] trait and its blanket impls.
//!
//! A `Handler` is any async function whose arguments are extractors and
//! whose return type implements [`IntoResponse`].  Blanket impls exist for
//! zero-argument functions, single-extractor functions (both parts-only and
//! body-consuming), and multi-extractor functions (N >= 2 arguments).
//!
//! For multi-extractor handlers the convention is:
//!
//! - The first N-1 arguments implement [`FromRequestParts`].
//! - The last argument implements [`FromRequest`].
//!
//! Since every [`FromRequestParts`] type also automatically implements
//! [`FromRequest`] (see `extract.rs`), a single blanket impl per tuple size
//! handles both "all parts" and "body last" cases.

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
// Blanket impl for single-extractor handlers
// ---------------------------------------------------------------------------

/// Blanket impl for handlers that take a single extractor argument.
///
/// Any async fn `F: Fn(E) -> Fut` where `Fut: Future<Output = R>`,
/// `R: IntoResponse`, and `E: FromRequest<S, BoxBody>` is a valid handler.
///
/// The argument `E` is extracted via [`FromRequest::from_request`], which
/// gets the full request (including body).  For parts-only extractors the
/// blanket in `extract.rs` turns this into a simple parts extraction,
/// dropping the body.  For body-consuming extractors (like [`Json<T>`]) the
/// body is read and deserialized as normal.
///
/// This single blanket replaces what were previously separate impls for
/// [`FromRequestParts`] (marker `(E,)`) and [`FromRequest`] (marker
/// `(E, BoxBody)`), because every [`FromRequestParts`] type now also
/// implements [`FromRequest`] via the blanket in `extract.rs`.
impl<F, Fut, R, S, E> Handler<(E,), S> for F
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

// ---------------------------------------------------------------------------
// Multi-extractor handler impls (2, 3, 4, 5 arguments)
// ---------------------------------------------------------------------------

/// 2-argument handler: `fn(A, B) -> Fut`.
///
/// `A` is extracted via [`FromRequestParts`], `B` is extracted via
/// [`FromRequest`].  If `A` fails, the body is never touched.
impl<F, Fut, R, S, A, B> Handler<(A, B), S> for F
where
    F: Fn(A, B) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: IntoResponse,
    S: Clone + Send + 'static,
    A: FromRequestParts<S> + Send + 'static,
    B: FromRequest<S, BoxBody> + Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

    fn call(self, req: Request, state: S) -> Self::Future {
        Box::pin(async move {
            let (mut parts, body) = req.into_parts();
            let a = match A::from_request_parts(&mut parts, &state).await {
                Ok(v) => v,
                Err(e) => return e.into_response(),
            };
            let b = match B::from_request(Request::from_parts(parts, body), &state).await {
                Ok(v) => v,
                Err(e) => return e.into_response(),
            };
            (self)(a, b).await.into_response()
        })
    }
}

/// 3-argument handler: `fn(A, B, C) -> Fut`.
///
/// `A` and `B` are extracted via [`FromRequestParts`]; `C` is extracted
/// via [`FromRequest`].  If any of `A` or `B` fails, the body is never
/// touched.
impl<F, Fut, R, S, A, B, C> Handler<(A, B, C), S> for F
where
    F: Fn(A, B, C) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: IntoResponse,
    S: Clone + Send + 'static,
    A: FromRequestParts<S> + Send + 'static,
    B: FromRequestParts<S> + Send + 'static,
    C: FromRequest<S, BoxBody> + Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

    fn call(self, req: Request, state: S) -> Self::Future {
        Box::pin(async move {
            let (mut parts, body) = req.into_parts();
            let a = match A::from_request_parts(&mut parts, &state).await {
                Ok(v) => v,
                Err(e) => return e.into_response(),
            };
            let b = match B::from_request_parts(&mut parts, &state).await {
                Ok(v) => v,
                Err(e) => return e.into_response(),
            };
            let c = match C::from_request(Request::from_parts(parts, body), &state).await {
                Ok(v) => v,
                Err(e) => return e.into_response(),
            };
            (self)(a, b, c).await.into_response()
        })
    }
}

/// 4-argument handler: `fn(A, B, C, D) -> Fut`.
///
/// `A`, `B`, and `C` are extracted via [`FromRequestParts`]; `D` is
/// extracted via [`FromRequest`].
impl<F, Fut, R, S, A, B, C, D> Handler<(A, B, C, D), S> for F
where
    F: Fn(A, B, C, D) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: IntoResponse,
    S: Clone + Send + 'static,
    A: FromRequestParts<S> + Send + 'static,
    B: FromRequestParts<S> + Send + 'static,
    C: FromRequestParts<S> + Send + 'static,
    D: FromRequest<S, BoxBody> + Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

    fn call(self, req: Request, state: S) -> Self::Future {
        Box::pin(async move {
            let (mut parts, body) = req.into_parts();
            let a = match A::from_request_parts(&mut parts, &state).await {
                Ok(v) => v,
                Err(e) => return e.into_response(),
            };
            let b = match B::from_request_parts(&mut parts, &state).await {
                Ok(v) => v,
                Err(e) => return e.into_response(),
            };
            let c = match C::from_request_parts(&mut parts, &state).await {
                Ok(v) => v,
                Err(e) => return e.into_response(),
            };
            let d = match D::from_request(Request::from_parts(parts, body), &state).await {
                Ok(v) => v,
                Err(e) => return e.into_response(),
            };
            (self)(a, b, c, d).await.into_response()
        })
    }
}

/// 5-argument handler: `fn(A, B, C, D, E) -> Fut`.
///
/// `A`, `B`, `C`, `D` are extracted via [`FromRequestParts`]; `E` is
/// extracted via [`FromRequest`].
impl<F, Fut, R, S, A, B, C, D, E> Handler<(A, B, C, D, E), S> for F
where
    F: Fn(A, B, C, D, E) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: IntoResponse,
    S: Clone + Send + 'static,
    A: FromRequestParts<S> + Send + 'static,
    B: FromRequestParts<S> + Send + 'static,
    C: FromRequestParts<S> + Send + 'static,
    D: FromRequestParts<S> + Send + 'static,
    E: FromRequest<S, BoxBody> + Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

    fn call(self, req: Request, state: S) -> Self::Future {
        Box::pin(async move {
            let (mut parts, body) = req.into_parts();
            let a = match A::from_request_parts(&mut parts, &state).await {
                Ok(v) => v,
                Err(e) => return e.into_response(),
            };
            let b = match B::from_request_parts(&mut parts, &state).await {
                Ok(v) => v,
                Err(e) => return e.into_response(),
            };
            let c = match C::from_request_parts(&mut parts, &state).await {
                Ok(v) => v,
                Err(e) => return e.into_response(),
            };
            let d = match D::from_request_parts(&mut parts, &state).await {
                Ok(v) => v,
                Err(e) => return e.into_response(),
            };
            let e = match E::from_request(Request::from_parts(parts, body), &state).await {
                Ok(v) => v,
                Err(e) => return e.into_response(),
            };
            (self)(a, b, c, d, e).await.into_response()
        })
    }
}
