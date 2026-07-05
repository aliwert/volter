//! [`HandlerService`] — a [`tower::Service`] adapter that wraps a
//! [`Handler`](crate::handler::Handler) together with its state.

use std::convert::Infallible;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use tower::Service;

use crate::body::Request;
use crate::handler::Handler;

/// A [`tower::Service`] adapter that wraps a [`Handler`] together with its
/// state.
///
/// `H` is the handler type, `T` is the marker type for the handler's
/// extractor arity (e.g. `()` for zero-arg handlers), and `S` is the
/// application state type.
pub struct HandlerService<H, T, S> {
    handler: H,
    state: S,
    _marker: PhantomData<fn() -> T>,
}

impl<H, T, S> HandlerService<H, T, S> {
    /// Create a new `HandlerService` from a handler and state.
    pub fn new(handler: H, state: S) -> Self {
        Self {
            handler,
            state,
            _marker: PhantomData,
        }
    }
}

impl<H, T, S> Clone for HandlerService<H, T, S>
where
    H: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            state: self.state.clone(),
            _marker: PhantomData,
        }
    }
}

impl<H, T, S> Service<Request> for HandlerService<H, T, S>
where
    H: Handler<T, S>,
    S: Clone + Send + 'static,
{
    type Response = crate::body::Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Infallible>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let handler = self.handler.clone();
        let state = self.state.clone();
        Box::pin(async move { Ok(handler.call(req, state).await) })
    }
}
