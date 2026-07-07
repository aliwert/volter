//! [`MethodRouter`] — dispatches a request to the handler registered for
//! the request's HTTP method.
//!
//! A `MethodRouter` holds at most one handler per HTTP method.  Handlers
//! are stored as type-erased [`HandlerSlot`]s and are finalized into boxed
//! services only when the application state is available (via
//! [`MethodRouter::finalize`]).

use std::collections::HashMap;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use http::Method;
use tower::util::BoxCloneService;

use volter_core::{Handler, HandlerService, Request, Response};

// ---------------------------------------------------------------------------
// Type alias
// ---------------------------------------------------------------------------

/// A boxed, `Send` future returned by the router's `Service::call`.
pub(crate) type BoxedFuture = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send>>;

// ---------------------------------------------------------------------------
// HandlerSlot
// ---------------------------------------------------------------------------

/// A type-erased handler that produces a [`BoxCloneService`] once the
/// application state is provided.
///
/// # Why this exists
///
/// At `get(handler)` time we know the handler type `H` but not the state
/// value — that's owned by the `Router` and provided later in `route()`.
/// We need to store the handler in a form that:
///
/// 1. **Erases `H`**: `MethodRouter` would otherwise need a type parameter
///    for every handler, making the route table heterogeneous storage
///    impossible.
/// 2. **Can be cloned**: `MethodRouter` must be cloneable so `Router` can
///    be cloned.
/// 3. **Is thread-safe**: `Router` implements `tower::Service` which is
///    typically used across tasks.
/// 4. **Defers service creation**: The `BoxCloneService` wrapping
///    `HandlerService` can only be built once the state value is available.
///
/// `Arc<dyn Fn(S) -> BoxCloneService<...> + Send + Sync>` satisfies all
/// four: `dyn` erases `H`, `Arc` provides cloneability, `Send + Sync`
/// provides thread safety, and the closure defers the `BoxCloneService`
/// allocation until `to_service` is called.
///
/// An alternative would be per-request service creation inside
/// `Service::call`, but that would allocate a `BoxCloneService` on every
/// request instead of once at setup.
struct HandlerSlot<S> {
    maker: Arc<dyn Fn(S) -> BoxCloneService<Request, Response, Infallible> + Send + Sync>,
}

impl<S> Clone for HandlerSlot<S> {
    fn clone(&self) -> Self {
        Self {
            maker: self.maker.clone(),
        }
    }
}

impl<S: Clone + Send + 'static> HandlerSlot<S> {
    /// Wrap a [`Handler`] into a slot that can be finalized later.
    fn new<H, T>(handler: H) -> Self
    where
        H: Handler<T, S> + Sync,
        T: 'static,
    {
        let maker = move |state: S| {
            let service = HandlerService::new(handler.clone(), state);
            BoxCloneService::new(service)
        };
        Self {
            maker: Arc::new(maker),
        }
    }

    /// Produce a boxed cloneable service by providing the application state.
    fn to_service(&self, state: S) -> BoxCloneService<Request, Response, Infallible> {
        (self.maker)(state)
    }
}

// ---------------------------------------------------------------------------
// MethodRouter
// ---------------------------------------------------------------------------

/// Per-method request dispatcher.
///
/// A `MethodRouter` holds at most one handler per HTTP method.  When a
/// request arrives, it selects the handler whose method matches the request
/// method and delegates to it.
///
/// The type parameter `S` is the application state type.  Handlers are
/// stored as [`HandlerSlot`]s and finalized into boxed services via
/// [`finalize`](MethodRouter::finalize).
pub struct MethodRouter<S = ()> {
    handlers: HashMap<Method, HandlerSlot<S>>,
}

impl<S: Clone + Send + 'static> MethodRouter<S> {
    /// Create an empty method router.
    pub(crate) fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a handler for GET requests.
    pub fn get<H, T>(&mut self, handler: H)
    where
        H: Handler<T, S> + Sync,
        T: 'static,
    {
        self.handlers.insert(Method::GET, HandlerSlot::new(handler));
    }

    /// Register a handler for POST requests.
    pub fn post<H, T>(&mut self, handler: H)
    where
        H: Handler<T, S> + Sync,
        T: 'static,
    {
        self.handlers
            .insert(Method::POST, HandlerSlot::new(handler));
    }

    /// Finalize all stored handlers into boxed cloneable services.
    ///
    /// Consumes the `MethodRouter` and returns a map from HTTP method to
    /// boxed service.  The returned services have the state `S` baked in
    /// and can be cloned cheaply on each request.
    ///
    /// # Why eager finalization
    ///
    /// `finalize` is called exactly once per route, during `Router::route`,
    /// not on every request.  This means the `BoxCloneService` allocation
    /// is a one-time setup cost.  Per-request, we only clone the already-
    /// boxed service (a cheap `Arc` bump inside `BoxCloneService`).
    pub(crate) fn finalize(
        self,
        state: S,
    ) -> HashMap<Method, BoxCloneService<Request, Response, Infallible>> {
        let mut services = HashMap::new();
        for (method, slot) in self.handlers {
            services.insert(method, slot.to_service(state.clone()));
        }
        services
    }
}

impl Default for MethodRouter<()> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Clone for MethodRouter<S> {
    fn clone(&self) -> Self {
        Self {
            handlers: self.handlers.clone(),
        }
    }
}
