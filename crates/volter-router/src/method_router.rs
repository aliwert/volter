//! [`MethodRouter`] — dispatches a request to the handler registered for
//! the request's HTTP method.
//!
//! This type exists so the per-method routing logic can eventually support
//! `GET`, `POST`, `PUT`, `DELETE`, etc. without breaking the public API
//! of `Router::route` or the route constructor functions in [`route`].

use std::collections::HashMap;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;

use http::Method;
use tower::util::BoxCloneService;
use tower::Service;

use crate::error::{method_not_allowed_response, not_found_response};
use volter_core::{Handler, HandlerService, Request, Response};

// ---------------------------------------------------------------------------
// Type alias
// ---------------------------------------------------------------------------

/// A boxed, `Send` future returned by the router's `Service::call`.
pub(crate) type BoxedFuture = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send>>;

// ---------------------------------------------------------------------------
// MethodRouter
// ---------------------------------------------------------------------------

/// Per-method request dispatcher.
///
/// A `MethodRouter` holds at most one handler per HTTP method.  When a
/// request arrives, it selects the handler whose method matches the request
/// method and delegates to it.
pub struct MethodRouter {
    services: HashMap<Method, BoxCloneService<Request, Response, Infallible>>,
}

impl MethodRouter {
    /// Create an empty method router.
    pub(crate) fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }

    /// Register a handler for GET requests.
    pub fn get<H, T>(&mut self, handler: H)
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        let service = HandlerService::new(handler, ());
        self.services
            .insert(Method::GET, BoxCloneService::new(service));
    }

    /// Dispatch a request to the handler for the given method.
    ///
    /// Returns:
    /// - The handler's response if a handler is registered for the method.
    /// - `405 Method Not Allowed` if the path exists but the method is not
    ///   supported.
    pub(crate) fn call(&self, req: Request) -> BoxedFuture {
        let method = req.method().clone();

        if let Some(service) = self.services.get(&method) {
            let mut svc = service.clone();
            Box::pin(async move { svc.call(req).await })
        } else if self.services.is_empty() {
            Box::pin(async move { Ok(not_found_response()) })
        } else {
            Box::pin(async move { Ok(method_not_allowed_response()) })
        }
    }
}

impl Clone for MethodRouter {
    fn clone(&self) -> Self {
        Self {
            services: self.services.clone(),
        }
    }
}
