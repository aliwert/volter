//! The [`Handler`] trait and its blanket impl for zero-argument functions.
//!
//! A `Handler` is any async function whose arguments are extractors and
//! whose return type implements [`IntoResponse`].  The blanket impl for
//! zero-argument handlers is provided below; multi-extractor impls will
//! be added in a follow-up PR.

use std::future::Future;
use std::pin::Pin;

use crate::body::{Request, Response};
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

// TODO(v0.1): add blanket Handler impls for 1+ `FromRequestParts`
// extractors.  This requires a named future type that chains extraction
// with the handler call, since we cannot use `async` blocks inside a
// blanket impl that also needs to name a concrete future type for the
// associated `Handler::Future`.  The approach used in axum is a
// `HandlerFuture<H, T, S, Fut>` struct that first calls each
// `FromRequestParts::from_request_parts`, then passes the extracted
// values to the handler.  That will be the next PR.
