//! Route constructor functions.
//!
//! This module provides free functions like [`get`] that create
//! [`MethodRouter`] instances from handlers.  As new HTTP methods are
//! supported, their constructors (e.g. `post`, `put`, `delete`) will be
//! added here without breaking callers.

use crate::method_router::MethodRouter;
use volter_core::Handler;

/// Create a [`MethodRouter`] that matches **GET** requests.
///
/// The returned router delegates to `handler` when the request method is
/// `GET`.  Other methods on the same path receive a `405 Method Not Allowed`
/// response.
///
/// The type parameter `S` is inferred from the handler: a zero-argument
/// handler produces a `MethodRouter<()>`; a handler that takes
/// `State<AppState>` produces a `MethodRouter<AppState>`.
///
/// # Example
///
/// ```rust
/// use volter_router::{Router, get};
///
/// async fn index() -> &'static str {
///     "Hello, World!"
/// }
///
/// let app: Router = Router::new().route("/", get(index));
/// ```
pub fn get<H, T, S>(handler: H) -> MethodRouter<S>
where
    H: Handler<T, S> + Sync,
    T: 'static,
    S: Clone + Send + 'static,
{
    let mut router = MethodRouter::new();
    router.get(handler);
    router
}

/// Create a [`MethodRouter`] that matches **POST** requests.
///
/// The returned router delegates to `handler` when the request method is
/// `POST`.  Other methods on the same path receive a `405 Method Not Allowed`
/// response.
///
/// # Example
///
/// ```rust
/// use volter_router::{Router, post};
///
/// async fn create() -> &'static str {
///     "Created"
/// }
///
/// let app: Router = Router::new().route("/", post(create));
/// ```
pub fn post<H, T, S>(handler: H) -> MethodRouter<S>
where
    H: Handler<T, S> + Sync,
    T: 'static,
    S: Clone + Send + 'static,
{
    let mut router = MethodRouter::new();
    router.post(handler);
    router
}

/// Create a [`MethodRouter`] that matches **PUT** requests.
///
/// The returned router delegates to `handler` when the request method is
/// `PUT`.  Other methods on the same path receive a `405 Method Not Allowed`
/// response.
pub fn put<H, T, S>(handler: H) -> MethodRouter<S>
where
    H: Handler<T, S> + Sync,
    T: 'static,
    S: Clone + Send + 'static,
{
    let mut router = MethodRouter::new();
    router.put(handler);
    router
}

/// Create a [`MethodRouter`] that matches **PATCH** requests.
pub fn patch<H, T, S>(handler: H) -> MethodRouter<S>
where
    H: Handler<T, S> + Sync,
    T: 'static,
    S: Clone + Send + 'static,
{
    let mut router = MethodRouter::new();
    router.patch(handler);
    router
}

/// Create a [`MethodRouter`] that matches **DELETE** requests.
pub fn delete<H, T, S>(handler: H) -> MethodRouter<S>
where
    H: Handler<T, S> + Sync,
    T: 'static,
    S: Clone + Send + 'static,
{
    let mut router = MethodRouter::new();
    router.delete(handler);
    router
}

/// Create a [`MethodRouter`] that matches **HEAD** requests.
pub fn head<H, T, S>(handler: H) -> MethodRouter<S>
where
    H: Handler<T, S> + Sync,
    T: 'static,
    S: Clone + Send + 'static,
{
    let mut router = MethodRouter::new();
    router.head(handler);
    router
}

/// Create a [`MethodRouter`] that matches **OPTIONS** requests.
pub fn options<H, T, S>(handler: H) -> MethodRouter<S>
where
    H: Handler<T, S> + Sync,
    T: 'static,
    S: Clone + Send + 'static,
{
    let mut router = MethodRouter::new();
    router.options(handler);
    router
}
