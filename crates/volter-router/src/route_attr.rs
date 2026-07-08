//! [`RouteAttr`] — a path + HTTP-method descriptor returned by route
//! attribute macros like `#[get("/")]`.
//!
//! The handler function is **not** stored in the const — it's provided at
//! [`Router::route_attr`] call time.  This avoids the `impl Trait` naming
//! problem that prevents storing async function types in a `const`.

use volter_core::Handler;

use crate::{get, post, MethodRouter};

/// The HTTP method stored in a [`RouteAttr`].
enum RouteMethod {
    Get,
    Post,
}

/// A descriptor that carries a request path and HTTP method, created by
/// route attribute macros like `#[get("/")]`.
///
/// The handler function is passed separately to
/// [`Router::route_attr`](crate::Router::route_attr) at setup time:
///
/// ```rust
/// use volter_router::{Router, RouteAttr};
///
/// async fn index() -> &'static str { "Hello!" }
///
/// const INDEX_ROUTE: RouteAttr = RouteAttr::get("/");
///
/// let app: Router = Router::new().route_attr(INDEX_ROUTE, index);
/// ```
pub struct RouteAttr {
    path: &'static str,
    method: RouteMethod,
}

impl RouteAttr {
    /// Create a new `RouteAttr` for a GET handler.
    pub const fn get(path: &'static str) -> Self {
        Self {
            path,
            method: RouteMethod::Get,
        }
    }

    /// Create a new `RouteAttr` for a POST handler.
    pub const fn post(path: &'static str) -> Self {
        Self {
            path,
            method: RouteMethod::Post,
        }
    }

    /// Build a [`MethodRouter`] from this descriptor and a handler.
    pub(crate) fn into_method_router<S, H, T>(self, handler: H) -> MethodRouter<S>
    where
        H: Handler<T, S> + Sync,
        T: 'static,
        S: Clone + Send + 'static,
    {
        match self.method {
            RouteMethod::Get => get(handler),
            RouteMethod::Post => post(handler),
        }
    }

    /// The path stored in this descriptor.
    pub fn path(&self) -> &str {
        self.path
    }
}
