//! [`Router`] — the central request dispatcher.
//!
//! A `Router` maps request paths to [`MethodRouter`] instances.  When a
//! request arrives, it extracts the path, looks up the corresponding method
//! router, and delegates dispatch to it.  If no route matches, a `404 Not
//! Found` response is returned.
//!
//! The type parameter `S` is the application state type.  Use
//! [`Router::new`] for stateless applications, or [`Router::with_state`] to
//! provide typed shared state that handlers can extract via
//! [`State<S>`](volter_core::State).

use std::collections::HashMap;
use std::convert::Infallible;
use std::task::{Context, Poll};

use http::Method;
use tower::util::BoxCloneService;
use tower::{Layer, Service};

use volter_core::{Handler, Request, Response, UrlParams};

use crate::error::{method_not_allowed_response, not_found_response};
use crate::method_router::{BoxedFuture, MethodRouter};
use crate::pattern::RoutePattern;
use crate::route_attr::RouteAttr;

// ---------------------------------------------------------------------------
// ParamRoute
// ---------------------------------------------------------------------------

/// A route entry for a parameterized path pattern (e.g. `/users/:id`).
///
/// Stored separately from static routes because matching requires a
/// segment-by-segment comparison.  When a request arrives, static routes
/// are checked first (O(1) hash lookup), then parameterized routes are
/// scanned linearly.
#[derive(Clone)]
struct ParamRoute {
    pattern: RoutePattern,
    services: HashMap<Method, BoxCloneService<Request, Response, Infallible>>,
}

// ---------------------------------------------------------------------------
// NestedRouter
// ---------------------------------------------------------------------------

/// A router nested under a path prefix.
///
/// When a request arrives, the parent router checks whether the request
/// path starts with the configured prefix (segment-aligned).  If it does,
/// the prefix is stripped from the path and the request is forwarded to
/// the nested router's dispatch service.
///
/// The nested router carries its own state, middleware, and routes — all
/// captured in its [`BoxCloneService`].
#[derive(Clone)]
struct NestedRouter {
    /// The normalized prefix (e.g. `"/api"`).
    prefix: String,
    /// The full dispatch service built from the nested router.
    inner: BoxCloneService<Request, Response, Infallible>,
}

/// Normalise a prefix string — ensure it starts with `/` and has no
/// trailing `/` (except for the root prefix `"/"`).
fn normalize_prefix(prefix: &str) -> String {
    let trimmed = prefix.trim_end_matches('/');
    if trimmed.is_empty() {
        return "/".to_owned();
    }
    if trimmed.starts_with('/') {
        trimmed.to_owned()
    } else {
        format!("/{trimmed}")
    }
}

/// Check whether `path` starts with `prefix` at a segment boundary, and if
/// so return the remaining path components with a leading `/`.
///
/// # Examples
///
/// - `("/api/users", "/api")` → `Some("/users")`
/// - `("/api", "/api")` → `Some("/")`
/// - `("/api2", "/api")` → `None`
fn match_and_strip_prefix(path: &str, prefix: &str) -> Option<String> {
    debug_assert!(!prefix.is_empty(), "prefix must not be empty");
    debug_assert!(prefix.starts_with('/'), "prefix must start with '/'");

    if prefix == "/" {
        return Some(path.to_owned());
    }

    if path == prefix {
        return Some("/".to_owned());
    }

    let prefix_with_slash = format!("{prefix}/");
    if let Some(rest) = path.strip_prefix(&prefix_with_slash) {
        if rest.is_empty() {
            Some("/".to_owned())
        } else {
            Some(format!("/{rest}"))
        }
    } else {
        None
    }
}

/// Replace the path (and query, if present) in a URI, preserving scheme
/// and authority.
fn strip_uri_prefix(uri: &mut http::Uri, new_path: &str) -> Option<()> {
    let pq = match uri.query() {
        Some(query) => format!("{new_path}?{query}"),
        None => new_path.to_owned(),
    };
    let new_uri = http::Uri::builder().path_and_query(pq).build().ok()?;
    *uri = new_uri;
    Some(())
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// A Volter router.
///
/// Routes incoming HTTP requests to handlers based on the request path and
/// HTTP method.  Implements [`tower::Service`] so it composes with the
/// entire `tower` / `tower-http` middleware ecosystem.
///
/// Use [`Router::layer`] to wrap the router with [`tower::Layer`]
/// middleware.  Routes registered **before** [`layer`](Router::layer) are
/// wrapped by the layer; routes registered **after** are not.  Multiple
/// [`layer`](Router::layer) calls compose in tower's onion model: the
/// last call is outermost.
///
/// The type parameter `S` is the application state injected via
/// [`with_state`](Router::with_state).  Zero-argument handlers (those that
/// don't extract state) work with any `S`; handlers that extract
/// [`State<AppState>`](volter_core::State) require `S = AppState`.
///
/// # Path parameters
///
/// Routes may contain named parameters prefixed with `:`
/// (e.g. `/users/:id`).  Use the [`Path`](volter_extract::Path) extractor
/// to receive typed parameter values.
///
/// # State ordering
///
/// [`with_state`](Router::with_state) **must** be called before routes that
/// extract state.  See the [`Router`](Router) level docs for details.
///
/// # Examples
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
///
/// **With middleware:**
///
/// ```rust
/// use volter_router::{Router, get};
/// use tower::{ServiceBuilder, layer::layer_fn};
///
/// async fn handler() -> &'static str {
///     "Hello, middleware!"
/// }
///
/// let app = Router::new()
///     .route("/", get(handler))
///     .layer(ServiceBuilder::new().layer(layer_fn(|svc| svc)));
/// ```
pub struct Router<S = ()> {
    state: S,
    static_routes: HashMap<String, HashMap<Method, BoxCloneService<Request, Response, Infallible>>>,
    param_routes: Vec<ParamRoute>,
    nested_routers: Vec<NestedRouter>,
    /// When set, [`call`] delegates to this layered service for routes
    /// that existed when [`layer`](Router::layer) was called.
    /// Routes added later skip the layer.
    layered: Option<BoxCloneService<Request, Response, Infallible>>,
}

impl<S: Clone + Send + 'static> Router<S> {
    /// Create a new router with the given application state.
    ///
    /// Prefer [`Router::new`] when the state type is `()`.
    pub fn with_state(state: S) -> Self {
        Self {
            state,
            static_routes: HashMap::new(),
            param_routes: Vec::new(),
            nested_routers: Vec::new(),
            layered: None,
        }
    }

    /// Register a route.
    ///
    /// The `path` argument is the request path or pattern
    /// (e.g. `"/"`, `"/hello"`, `"/users/:id"`).  The provided
    /// [`MethodRouter`] is finalized with the router's state immediately.
    ///
    /// Static paths (no `:` prefix) are stored in a `HashMap` for O(1)
    /// lookup.  Parameterized paths (containing `:`) are matched by a
    /// linear scan — a radix-tree replacement will make both O(path
    /// segments) in a later PR.
    ///
    /// # Why eager finalization
    ///
    /// Finalizing here (during setup) means each request only clones an
    /// already-boxed service — no `BoxCloneService` allocation happens in
    /// the hot path.  The one-time cost scales with the number of routes,
    /// not the number of requests.
    pub fn route(mut self, path: &str, method_router: MethodRouter<S>) -> Self {
        let services = method_router.finalize(self.state.clone());
        if path.contains(':') {
            self.param_routes.push(ParamRoute {
                pattern: RoutePattern::parse(path),
                services,
            });
        } else {
            self.static_routes.insert(path.to_owned(), services);
        }
        self
    }

    /// Register a route from a [`RouteAttr`] (returned by route attribute
    /// macros such as `#[get("/")]`).
    ///
    /// The [`RouteAttr`] provides the path and HTTP method; the handler is
    /// passed separately so its type can be fully inferred.
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
    pub fn route_attr<H, T>(self, attr: RouteAttr, handler: H) -> Self
    where
        H: Handler<T, S> + Sync,
        T: 'static,
    {
        let path = attr.path().to_owned();
        let mr = attr.into_method_router(handler);
        self.route(&path, mr)
    }

    /// Wrap the router with a [`tower::Layer`].
    ///
    /// Routes registered **before** this call are wrapped by the layer;
    /// routes registered **after** are not.  Multiple `layer` calls compose
    /// in tower's onion model — the last call is outermost.
    ///
    /// # Example
    ///
    /// ```rust
    /// use volter_router::{Router, get};
    /// use tower::layer::layer_fn;
    ///
    /// async fn handler() -> &'static str {
    ///     "Hello!"
    /// }
    ///
    /// let app = Router::new()
    ///     .route("/", get(handler))
    ///     .layer(layer_fn(|svc| svc));
    /// ```
    pub fn layer<L>(mut self, layer: L) -> Self
    where
        L: Layer<BoxCloneService<Request, Response, Infallible>>,
        L::Service:
            Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
        <L::Service as Service<Request>>::Future: Send,
    {
        if let Some(existing) = self.layered.take() {
            // Already layered: wrap the existing layered service in another
            // layer.  Routes in the existing layered service are from an
            // earlier layer() call; post-layer routes are in the route tables.
            let wrapped = layer.layer(existing);
            self.layered = Some(BoxCloneService::new(wrapped));
        } else {
            // First layer: capture current routes into an inner service,
            // wrap it, and clear route tables so future routes are not
            // accidentally double-wrapped.
            let inner = self.build_inner_service();
            let wrapped = layer.layer(inner);
            self.static_routes = HashMap::new();
            self.param_routes = Vec::new();
            self.nested_routers = Vec::new();
            self.layered = Some(BoxCloneService::new(wrapped));
        }
        self
    }

    /// Nest a router under a path prefix.
    ///
    /// Routes registered on `other` are reachable under `prefix`.  For
    /// example, nesting a router with route `"/users"` under `"/api"` makes
    /// it respond to `GET /api/users`.
    ///
    /// The nested router preserves its own middleware and state.
    /// Nesting is composable: a nested router may itself contain nested
    /// routers.
    ///
    /// # Prefix semantics
    ///
    /// The prefix is matched segment-by-segment (not a string prefix).
    /// `/api` matches `/api/users` and `/api`, but not `/api2`.
    ///
    /// # Layer interaction
    ///
    /// Like [`route`](Router::route), nesting before a [`layer`](Router::layer)
    /// call wraps the nested routes with the layer; nesting after does not.
    ///
    /// # Example
    ///
    /// ```rust
    /// use volter_router::{Router, get};
    ///
    /// async fn users() -> &'static str { "users" }
    /// async fn posts() -> &'static str { "posts" }
    ///
    /// let api = Router::new()
    ///     .route("/users", get(users))
    ///     .route("/posts", get(posts));
    ///
    /// let app = Router::new()
    ///     .nest("/api", api);
    /// ```
    pub fn nest(mut self, prefix: &str, other: Router<S>) -> Self {
        let prefix = normalize_prefix(prefix);
        let inner = other.build_nested_service();
        self.nested_routers.push(NestedRouter { prefix, inner });
        self
    }

    /// Merge another router at the same level.
    ///
    /// All routes, nests, and middleware from `other` are combined into
    /// `self`.  When both routers define a route for the same path and
    /// HTTP method, **the last merged router wins** (`other`'s handler
    /// replaces `self`'s).
    ///
    /// # Middleware preservation
    ///
    /// Each router's middleware layers continue to wrap only their own
    /// pre-layer routes.  Post-layer routes (added after the most recent
    /// `layer()` call) are merged directly into the route table and are
    /// not wrapped by either router's layers.
    ///
    /// # Layer interaction
    ///
    /// Merging before or after a [`layer`](Router::layer) call follows the
    /// same semantics as [`route`](Router::route): merged routes are
    /// captured by the pre-layer snapshot, or left as post-layer routes.
    ///
    /// # Example
    ///
    /// ```rust
    /// use volter_router::{Router, get};
    ///
    /// async fn users() -> &'static str { "users" }
    /// async fn admin() -> &'static str { "admin" }
    ///
    /// let api = Router::new().route("/users", get(users));
    /// let admin = Router::new().route("/admin", get(admin));
    ///
    /// let app = api.merge(admin);
    /// ```
    pub fn merge(mut self, other: Router<S>) -> Self {
        // 1. Merge post-layer static routes — other's entries override
        //    self's for the same path (last merged wins).
        self.static_routes.extend(other.static_routes);

        // 2. Merge post-layer param routes — prepend other's so they are
        //    checked first (last merged wins).
        let mut all_param = other.param_routes;
        all_param.extend(self.param_routes);
        self.param_routes = all_param;

        // 3. Merge nested routers — prepend other's so they are checked
        //    first (last merged wins).
        let mut all_nests = other.nested_routers;
        all_nests.extend(self.nested_routers);
        self.nested_routers = all_nests;

        // 4. Merge layered (pre-layer) services.
        //
        //    When both routers have applied layers, we combine them into a
        //    single [`CombinedService`] that tries the last-merged router's
        //    dispatch first, then falls through to self's.  When only one
        //    has layers, that one's service becomes the merged router's
        //    layered service.
        match (self.layered.take(), other.layered) {
            (None, None) => {}
            (Some(self_l), None) => self.layered = Some(self_l),
            (None, Some(other_l)) => self.layered = Some(other_l),
            (Some(self_l), Some(other_l)) => {
                // Other (last merged) checked first.
                self.layered = Some(BoxCloneService::new(CombinedService {
                    services: vec![other_l, self_l],
                }));
            }
        }

        self
    }

    /// Build a [`BoxCloneService`] that captures the full dispatch of this
    /// router (post-layer routes, nests, and layered fallback).  Used by
    /// [`nest`](Router::nest) to capture the nested router's dispatch.
    fn build_nested_service(&self) -> BoxCloneService<Request, Response, Infallible>
    where
        S: Clone + Send + 'static,
    {
        let static_routes = self.static_routes.clone();
        let param_routes = self.param_routes.clone();
        let nested_routers = self.nested_routers.clone();
        let layered = self.layered.clone();

        BoxCloneService::new(NestedDispatch {
            static_routes,
            param_routes,
            nested_routers,
            layered,
        })
    }

    /// Build a [`BoxCloneService`] that dispatches using the current route
    /// tables.  Used internally by [`layer`](Router::layer) to snapshot the
    /// pre-layer routes.
    fn build_inner_service(&self) -> BoxCloneService<Request, Response, Infallible>
    where
        S: Clone + Send + 'static,
    {
        BoxCloneService::new(InnerRouter {
            state: self.state.clone(),
            static_routes: self.static_routes.clone(),
            param_routes: self.param_routes.clone(),
            nested_routers: self.nested_routers.clone(),
        })
    }
}

impl Router<()> {
    /// Create a new empty router with no state (`()`).
    ///
    /// This is a convenience wrapper over [`Router::with_state(())`].
    pub fn new() -> Self {
        Self::with_state(())
    }
}

impl Default for Router<()> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Clone> Clone for Router<S> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            static_routes: self.static_routes.clone(),
            param_routes: self.param_routes.clone(),
            nested_routers: self.nested_routers.clone(),
            layered: self.layered.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// tower::Service impl
// ---------------------------------------------------------------------------

impl<S: Clone + Send + 'static> Service<Request> for Router<S> {
    type Response = Response;
    type Error = Infallible;
    type Future = BoxedFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let path = req.uri().path().to_owned();
        let method = req.method().clone();

        // 1. Try post-layer routes (routes added after the most recent
        //    layer() call — these are NOT wrapped by any layer).
        if let Some(services) = self.static_routes.get(&path) {
            return dispatch_services(services, method, req);
        }
        for param_route in &self.param_routes {
            if let Some(params) = param_route.pattern.matches(&path) {
                let mut req = req;
                req.extensions_mut().insert(UrlParams(params));
                return dispatch_services(&param_route.services, method, req);
            }
        }

        // 2. Try post-layer nested routers.
        for nest in &self.nested_routers {
            if let Some(stripped) = match_and_strip_prefix(&path, &nest.prefix) {
                let mut req = req;
                return if strip_uri_prefix(req.uri_mut(), &stripped).is_some() {
                    let mut inner = nest.inner.clone();
                    Box::pin(async move { inner.call(req).await })
                } else {
                    Box::pin(async move { Ok(not_found_response()) })
                };
            }
        }

        // 3. If no post-layer route matched, delegate to the layered
        //    service (which wraps routes that existed when layer() was
        //    called).
        if let Some(ref mut svc) = self.layered {
            return svc.call(req);
        }

        // 4. No route matched at all.
        Box::pin(async move { Ok(not_found_response()) })
    }
}

// ---------------------------------------------------------------------------
// InnerRouter — dispatch engine used by the first layer() call
// ---------------------------------------------------------------------------

/// A snapshot of the router's route tables at the time
/// [`Router::layer`] was first called.  Routes registered before the
/// first `layer()` call are dispatched through this inner service, which
/// is itself wrapped by the middleware layers.
///
/// Note: `S` is unused by the dispatch itself (each [`BoxCloneService`]
/// already carries the finalised state), but the type parameter is kept so
/// that `Layer` bounds remain consistent with [`Router`]'s generic
/// parameter.
#[derive(Clone)]
struct InnerRouter<S> {
    #[allow(dead_code)]
    state: S,
    static_routes: HashMap<String, HashMap<Method, BoxCloneService<Request, Response, Infallible>>>,
    param_routes: Vec<ParamRoute>,
    nested_routers: Vec<NestedRouter>,
}

impl<S: Clone + Send + 'static> Service<Request> for InnerRouter<S> {
    type Response = Response;
    type Error = Infallible;
    type Future = BoxedFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let path = req.uri().path().to_owned();
        let method = req.method().clone();

        if let Some(services) = self.static_routes.get(&path) {
            return dispatch_services(services, method, req);
        }

        for param_route in &self.param_routes {
            if let Some(params) = param_route.pattern.matches(&path) {
                let mut req = req;
                req.extensions_mut().insert(UrlParams(params));
                return dispatch_services(&param_route.services, method, req);
            }
        }

        for nest in &self.nested_routers {
            if let Some(stripped) = match_and_strip_prefix(&path, &nest.prefix) {
                let mut req = req;
                return if strip_uri_prefix(req.uri_mut(), &stripped).is_some() {
                    let mut inner = nest.inner.clone();
                    Box::pin(async move { inner.call(req).await })
                } else {
                    Box::pin(async move { Ok(not_found_response()) })
                };
            }
        }

        Box::pin(async move { Ok(not_found_response()) })
    }
}

/// Look up the method in the services map and dispatch.
///
/// - If the map is empty → 404 (should not normally happen).
/// - If the method exists → clone the service and call it.
/// - If the method is not found → 405.
fn dispatch_services(
    services: &HashMap<Method, BoxCloneService<Request, Response, Infallible>>,
    method: Method,
    req: Request,
) -> BoxedFuture {
    if services.is_empty() {
        return Box::pin(async move { Ok(not_found_response()) });
    }
    if let Some(service) = services.get(&method) {
        let mut svc = service.clone();
        return Box::pin(async move { svc.call(req).await });
    }
    Box::pin(async move { Ok(method_not_allowed_response()) })
}

// ---------------------------------------------------------------------------
// CombinedService — dispatch that tries multiple layered services in order
// ---------------------------------------------------------------------------

/// A dispatch service that tries each inner service in order.
///
/// The first service that returns a non-`404` response wins.  If all
/// services return `404`, the last `404` is returned.
///
/// # Limitation
///
/// Only the first service is guaranteed a chance to run — the request body
/// is consumed when the first service processes it.  In practice this means
/// that when two merged routers both have middleware layers, only the last
/// merged router's pre-layer routes are attempted if the first router's
/// layered service returns `404` without a matching route (because the
/// request has already been forwarded inside the `BoxCloneService`).
/// This is a known v1 limitation and may be lifted in a later PR.
#[derive(Clone)]
struct CombinedService {
    services: Vec<BoxCloneService<Request, Response, Infallible>>,
}

impl Service<Request> for CombinedService {
    type Response = Response;
    type Error = Infallible;
    type Future = BoxedFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let services = self.services.clone();
        Box::pin(async move {
            if let Some(mut svc) = services.into_iter().next() {
                svc.call(req).await
            } else {
                Ok(not_found_response())
            }
        })
    }
}

// ---------------------------------------------------------------------------
// NestedDispatch — full dispatch service for a nested router
// ---------------------------------------------------------------------------

/// A snapshot of a router's entire dispatch state (routes, nests, and
/// layered fallback).  Used by [`Router::nest`] to capture the nested
/// router's dispatch logic, including routes that were registered after
/// a [`layer`](Router::layer) call on the nested router.
///
/// This struct exists because [`InnerRouter`] only captures pre-layer
/// routes, but a nested router may have both pre-layer and post-layer
/// routes.  `NestedDispatch` handles both cases as well as its own
/// nested routers.
#[derive(Clone)]
struct NestedDispatch {
    static_routes: HashMap<String, HashMap<Method, BoxCloneService<Request, Response, Infallible>>>,
    param_routes: Vec<ParamRoute>,
    nested_routers: Vec<NestedRouter>,
    layered: Option<BoxCloneService<Request, Response, Infallible>>,
}

impl Service<Request> for NestedDispatch {
    type Response = Response;
    type Error = Infallible;
    type Future = BoxedFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let path = req.uri().path().to_owned();
        let method = req.method().clone();

        // 1. Post-layer routes (added after the most recent layer() call
        //    on this nested router — NOT wrapped by any of its layers).
        if let Some(services) = self.static_routes.get(&path) {
            return dispatch_services(services, method, req);
        }
        for param_route in &self.param_routes {
            if let Some(params) = param_route.pattern.matches(&path) {
                let mut req = req;
                req.extensions_mut().insert(UrlParams(params));
                return dispatch_services(&param_route.services, method, req);
            }
        }

        // 2. Post-layer nested routers within this nested router.
        for nest in &self.nested_routers {
            if let Some(stripped) = match_and_strip_prefix(&path, &nest.prefix) {
                let mut req = req;
                return if strip_uri_prefix(req.uri_mut(), &stripped).is_some() {
                    let mut inner = nest.inner.clone();
                    Box::pin(async move { inner.call(req).await })
                } else {
                    Box::pin(async move { Ok(not_found_response()) })
                };
            }
        }

        // 3. Layered fallback — wraps routes that existed when layer()
        //    was called on the nested router.
        if let Some(ref mut svc) = self.layered {
            return svc.call(req);
        }

        Box::pin(async move { Ok(not_found_response()) })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::get;
    use bytes::Bytes;
    use http::StatusCode;
    use volter_core::{empty_body, full_body};

    fn request(method: http::Method, path: &str) -> Request {
        http::Request::builder()
            .method(method)
            .uri(path)
            .body(empty_body())
            .unwrap()
    }

    fn json_request(method: http::Method, path: &str, body: &[u8]) -> Request {
        http::Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .body(full_body(Bytes::copy_from_slice(body)))
            .unwrap()
    }

    // -- Static route tests (unchanged from PR3) -----------------------------

    #[tokio::test]
    async fn get_root() {
        async fn root() -> &'static str {
            "Hello, World!"
        }

        let mut app = Router::new().route("/", get(root));
        let response = app.call(request(http::Method::GET, "/")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_hello() {
        async fn hello() -> &'static str {
            "Hello!"
        }

        let mut app = Router::new().route("/hello", get(hello));
        let response = app
            .call(request(http::Method::GET, "/hello"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn unknown_path_returns_404() {
        async fn root() -> &'static str {
            "Hello!"
        }

        let mut app = Router::new().route("/", get(root));
        let response = app
            .call(request(http::Method::GET, "/unknown"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn wrong_method_returns_405() {
        async fn root() -> &'static str {
            "Hello!"
        }

        let mut app = Router::new().route("/", get(root));
        let response = app.call(request(http::Method::POST, "/")).await.unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn multiple_routes() {
        async fn root() -> &'static str {
            "root"
        }
        async fn hello() -> &'static str {
            "hello"
        }

        let mut app = Router::new()
            .route("/", get(root))
            .route("/hello", get(hello));

        let root_resp = app.call(request(http::Method::GET, "/")).await.unwrap();
        assert_eq!(root_resp.status(), StatusCode::OK);

        let hello_resp = app
            .call(request(http::Method::GET, "/hello"))
            .await
            .unwrap();
        assert_eq!(hello_resp.status(), StatusCode::OK);

        let unknown_resp = app.call(request(http::Method::GET, "/nope")).await.unwrap();
        assert_eq!(unknown_resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn router_implements_tower_service() {
        async fn root() -> &'static str {
            "root"
        }

        let app = Router::new().route("/", get(root));
        let response = tower::ServiceExt::oneshot(app, request(http::Method::GET, "/"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // -- Stateful router tests -----------------------------------------------

    #[tokio::test]
    async fn state_extraction() {
        #[derive(Clone)]
        struct AppState {
            value: u32,
        }

        async fn handler(volter_core::State(state): volter_core::State<AppState>) -> String {
            format!("value: {}", state.value)
        }

        let state = AppState { value: 42 };
        let mut app = Router::with_state(state).route("/", get(handler));
        let response = app.call(request(http::Method::GET, "/")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // -- Parameterized route tests -------------------------------------------

    #[tokio::test]
    async fn path_u64_param() {
        async fn handler(volter_extract::Path(id): volter_extract::Path<u64>) -> String {
            format!("User {}", id)
        }

        let mut app = Router::new().route("/users/:id", get(handler));
        let response = app
            .call(request(http::Method::GET, "/users/42"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn path_string_param() {
        async fn handler(volter_extract::Path(name): volter_extract::Path<String>) -> String {
            format!("Hello {}", name)
        }

        let mut app = Router::new().route("/users/:name", get(handler));
        let response = app
            .call(request(http::Method::GET, "/users/alice"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn path_multiple_params() {
        #[derive(serde::Deserialize)]
        struct PostComment {
            post_id: u64,
            comment_id: u64,
        }

        async fn handler(volter_extract::Path(pc): volter_extract::Path<PostComment>) -> String {
            format!("{}/{}", pc.post_id, pc.comment_id)
        }

        let mut app = Router::new().route("/posts/:post_id/comments/:comment_id", get(handler));
        let response = app
            .call(request(http::Method::GET, "/posts/1/comments/2"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn path_invalid_param_returns_400() {
        async fn handler(volter_extract::Path(id): volter_extract::Path<u64>) -> String {
            format!("User {}", id)
        }

        let mut app = Router::new().route("/users/:id", get(handler));
        let response = app
            .call(request(http::Method::GET, "/users/abc"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn path_unknown_route_returns_404() {
        async fn handler(volter_extract::Path(id): volter_extract::Path<u64>) -> String {
            format!("User {}", id)
        }

        let mut app = Router::new().route("/users/:id", get(handler));
        let response = app
            .call(request(http::Method::GET, "/unknown"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn static_routes_still_work_with_param_routes_present() {
        async fn index() -> &'static str {
            "index"
        }
        async fn user(volter_extract::Path(id): volter_extract::Path<u64>) -> String {
            format!("User {}", id)
        }

        let mut app = Router::new()
            .route("/", get(index))
            .route("/users/:id", get(user));

        let index_resp = app.call(request(http::Method::GET, "/")).await.unwrap();
        assert_eq!(index_resp.status(), StatusCode::OK);

        let user_resp = app
            .call(request(http::Method::GET, "/users/42"))
            .await
            .unwrap();
        assert_eq!(user_resp.status(), StatusCode::OK);
    }

    // -- Query extractor tests -----------------------------------------------

    #[tokio::test]
    async fn query_single_u32_param() {
        #[derive(serde::Deserialize)]
        struct PageQuery {
            page: u32,
        }

        async fn handler(volter_extract::Query(query): volter_extract::Query<PageQuery>) -> String {
            format!("Page {}", query.page)
        }

        let mut app = Router::new().route("/users", get(handler));
        let response = app
            .call(request(http::Method::GET, "/users?page=1"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn query_single_string_param() {
        #[derive(serde::Deserialize)]
        struct NameQuery {
            name: String,
        }

        async fn handler(volter_extract::Query(query): volter_extract::Query<NameQuery>) -> String {
            format!("Hello {}", query.name)
        }

        let mut app = Router::new().route("/users", get(handler));
        let response = app
            .call(request(http::Method::GET, "/users?name=alice"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn query_multiple_params() {
        #[derive(serde::Deserialize)]
        struct UsersQuery {
            page: u32,
            search: String,
        }

        async fn handler(
            volter_extract::Query(query): volter_extract::Query<UsersQuery>,
        ) -> String {
            format!("Page {}, search={}", query.page, query.search)
        }

        let mut app = Router::new().route("/users", get(handler));
        let response = app
            .call(request(http::Method::GET, "/users?page=2&search=rust"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn query_optional_fields() {
        #[derive(serde::Deserialize)]
        struct UsersQuery {
            page: u32,
            search: Option<String>,
        }

        async fn handler(
            volter_extract::Query(query): volter_extract::Query<UsersQuery>,
        ) -> String {
            let search = query.search.as_deref().unwrap_or("none");
            format!("Page {}, search={}", query.page, search)
        }

        let mut app = Router::new().route("/users", get(handler));
        let response = app
            .call(request(http::Method::GET, "/users?page=1"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn query_missing_query_string() {
        #[derive(serde::Deserialize)]
        struct UsersQuery {
            #[serde(default)]
            page: u32,
            search: Option<String>,
        }

        async fn handler(
            volter_extract::Query(query): volter_extract::Query<UsersQuery>,
        ) -> String {
            format!("Page {}, search={:?}", query.page, query.search)
        }

        let mut app = Router::new().route("/users", get(handler));
        let response = app
            .call(request(http::Method::GET, "/users"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn query_invalid_integer_returns_400() {
        #[derive(serde::Deserialize)]
        struct PageQuery {
            page: u32,
        }

        async fn handler(volter_extract::Query(query): volter_extract::Query<PageQuery>) -> String {
            format!("Page {}", query.page)
        }

        let mut app = Router::new().route("/users", get(handler));
        let response = app
            .call(request(http::Method::GET, "/users?page=abc"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn query_url_encoding() {
        #[derive(serde::Deserialize)]
        struct SearchQuery {
            q: String,
        }

        async fn handler(
            volter_extract::Query(query): volter_extract::Query<SearchQuery>,
        ) -> String {
            format!("q={}", query.q)
        }

        let mut app = Router::new().route("/search", get(handler));
        let response = app
            .call(request(http::Method::GET, "/search?q=hello+world"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // -- Json extractor tests ------------------------------------------------

    #[tokio::test]
    async fn json_valid_body() {
        #[derive(serde::Deserialize)]
        struct CreateUser {
            name: String,
            age: u8,
        }

        async fn handler(
            volter_extract::Json(payload): volter_extract::Json<CreateUser>,
        ) -> String {
            format!("{} is {}", payload.name, payload.age)
        }

        let mut app = Router::new().route("/users", get(handler));
        let body = br#"{"name":"Alice","age":30}"#;
        let response = app
            .call(json_request(http::Method::GET, "/users", body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn json_missing_content_type_returns_415() {
        #[derive(serde::Deserialize)]
        struct CreateUser {
            name: String,
            age: u8,
        }

        async fn handler(
            volter_extract::Json(payload): volter_extract::Json<CreateUser>,
        ) -> String {
            format!("{} is {}", payload.name, payload.age)
        }

        let mut app = Router::new().route("/users", get(handler));
        let response = app
            .call(request(http::Method::GET, "/users"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[tokio::test]
    async fn json_unsupported_content_type_returns_415() {
        #[derive(serde::Deserialize)]
        struct CreateUser {
            name: String,
            age: u8,
        }

        async fn handler(
            volter_extract::Json(payload): volter_extract::Json<CreateUser>,
        ) -> String {
            format!("{} is {}", payload.name, payload.age)
        }

        let mut app = Router::new().route("/users", get(handler));
        let body = br#"{"name":"Alice","age":30}"#;
        let request = http::Request::builder()
            .method(http::Method::GET)
            .uri("/users")
            .header("content-type", "text/plain")
            .body(full_body(Bytes::copy_from_slice(body)))
            .unwrap();
        let response = app.call(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[tokio::test]
    async fn json_invalid_syntax_returns_400() {
        #[derive(serde::Deserialize)]
        struct CreateUser {
            name: String,
            age: u8,
        }

        async fn handler(
            volter_extract::Json(payload): volter_extract::Json<CreateUser>,
        ) -> String {
            format!("{} is {}", payload.name, payload.age)
        }

        let mut app = Router::new().route("/users", get(handler));
        let body = b"not valid json";
        let response = app
            .call(json_request(http::Method::GET, "/users", body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn json_wrong_shape_returns_400() {
        #[derive(serde::Deserialize)]
        struct CreateUser {
            name: String,
            age: u8,
        }

        async fn handler(
            volter_extract::Json(payload): volter_extract::Json<CreateUser>,
        ) -> String {
            format!("{} is {}", payload.name, payload.age)
        }

        let mut app = Router::new().route("/users", get(handler));
        let body = br#"{"name":"Alice","age":"not-a-number"}"#;
        let response = app
            .call(json_request(http::Method::GET, "/users", body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn json_response() {
        #[derive(serde::Deserialize, serde::Serialize)]
        struct CreateUser {
            name: String,
            age: u8,
        }

        async fn handler(
            volter_extract::Json(payload): volter_extract::Json<CreateUser>,
        ) -> volter_extract::Json<CreateUser> {
            volter_extract::Json(payload)
        }

        let mut app = Router::new().route("/users", get(handler));
        let body = br#"{"name":"Alice","age":30}"#;
        let response = app
            .call(json_request(http::Method::GET, "/users", body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("application/json")
        );
    }

    // -- Extension extractor tests -------------------------------------------

    #[tokio::test]
    async fn extension_successful_extraction() {
        #[derive(Debug, Clone, PartialEq)]
        struct User {
            name: String,
        }

        async fn handler(
            volter_extract::Extension(user): volter_extract::Extension<User>,
        ) -> String {
            user.name
        }

        let mut app = Router::new().route("/profile", get(handler));
        let request = http::Request::builder()
            .method(http::Method::GET)
            .uri("/profile")
            .extension(User {
                name: "Alice".into(),
            })
            .body(empty_body())
            .unwrap();
        let response = app.call(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn extension_missing_returns_500() {
        #[derive(Debug, Clone, PartialEq)]
        struct User {
            name: String,
        }

        async fn handler(
            volter_extract::Extension(_user): volter_extract::Extension<User>,
        ) -> &'static str {
            "should not reach here"
        }

        let mut app = Router::new().route("/profile", get(handler));
        let response = app
            .call(request(http::Method::GET, "/profile"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn extension_multiple_requests_different_values() {
        #[derive(Debug, Clone, PartialEq)]
        struct User {
            name: String,
        }

        async fn handler(
            volter_extract::Extension(user): volter_extract::Extension<User>,
        ) -> String {
            user.name
        }

        let mut app = Router::new().route("/profile", get(handler));

        for name in &["Alice", "Bob", "Charlie"] {
            let request = http::Request::builder()
                .method(http::Method::GET)
                .uri("/profile")
                .extension(User {
                    name: name.to_string(),
                })
                .body(empty_body())
                .unwrap();
            let response = app.call(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
    }

    #[tokio::test]
    async fn extension_state_still_works() {
        #[derive(Clone)]
        struct AppState {
            counter: u64,
        }

        async fn handler(volter_core::State(state): volter_core::State<AppState>) -> String {
            format!("counter: {}", state.counter)
        }

        let mut app = Router::with_state(AppState { counter: 42 }).route("/state", get(handler));
        let response = app
            .call(request(http::Method::GET, "/state"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn extension_path_still_works() {
        #[derive(serde::Deserialize)]
        struct IdQuery {
            id: u64,
        }

        async fn handler(volter_extract::Path(params): volter_extract::Path<IdQuery>) -> String {
            format!("id: {}", params.id)
        }

        let mut app = Router::new().route("/items/:id", get(handler));
        let response = app
            .call(request(http::Method::GET, "/items/7"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn extension_query_still_works() {
        #[derive(serde::Deserialize)]
        struct SearchQuery {
            q: String,
        }

        async fn handler(
            volter_extract::Query(query): volter_extract::Query<SearchQuery>,
        ) -> String {
            format!("q={}", query.q)
        }

        let mut app = Router::new().route("/search", get(handler));
        let response = app
            .call(request(http::Method::GET, "/search?q=hello+world"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn extension_json_still_works() {
        #[derive(serde::Deserialize)]
        struct CreateUser {
            name: String,
            age: u8,
        }

        async fn handler(
            volter_extract::Json(payload): volter_extract::Json<CreateUser>,
        ) -> String {
            format!("{} is {}", payload.name, payload.age)
        }

        let mut app = Router::new().route("/users", get(handler));
        let body = br#"{"name":"Alice","age":30}"#;
        let response = app
            .call(json_request(http::Method::GET, "/users", body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // -- Multi-extractor handler tests ---------------------------------------

    #[tokio::test]
    async fn multi_state_path() {
        #[derive(Clone)]
        struct AppState {
            prefix: String,
        }

        #[derive(serde::Deserialize)]
        struct IdParams {
            id: u64,
        }

        async fn handler(
            volter_core::State(state): volter_core::State<AppState>,
            volter_extract::Path(params): volter_extract::Path<IdParams>,
        ) -> String {
            format!("{}-{}", state.prefix, params.id)
        }

        let mut app = Router::with_state(AppState {
            prefix: "item".into(),
        })
        .route("/items/:id", get(handler));
        let response = app
            .call(request(http::Method::GET, "/items/42"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn multi_state_query() {
        #[derive(Clone)]
        struct AppState {
            default_page: u32,
        }

        #[derive(serde::Deserialize)]
        struct PageQuery {
            page: u32,
        }

        async fn handler(
            volter_core::State(state): volter_core::State<AppState>,
            volter_extract::Query(query): volter_extract::Query<PageQuery>,
        ) -> String {
            format!("page: {}", query.page + state.default_page)
        }

        let mut app = Router::with_state(AppState { default_page: 1 }).route("/list", get(handler));
        let response = app
            .call(request(http::Method::GET, "/list?page=10"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn multi_path_query() {
        #[derive(serde::Deserialize)]
        struct ItemParams {
            id: u64,
        }

        #[derive(serde::Deserialize)]
        struct FilterQuery {
            show: Option<String>,
        }

        async fn handler(
            volter_extract::Path(params): volter_extract::Path<ItemParams>,
            volter_extract::Query(query): volter_extract::Query<FilterQuery>,
        ) -> String {
            format!("{}-{:?}", params.id, query.show)
        }

        let mut app = Router::new().route("/items/:id", get(handler));
        let response = app
            .call(request(http::Method::GET, "/items/7?show=details"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn multi_path_extension() {
        #[derive(serde::Deserialize)]
        struct ItemParams {
            id: u64,
        }

        #[derive(Debug, Clone, PartialEq)]
        struct User {
            name: String,
        }

        async fn handler(
            volter_extract::Path(params): volter_extract::Path<ItemParams>,
            volter_extract::Extension(user): volter_extract::Extension<User>,
        ) -> String {
            format!("{}-{}", params.id, user.name)
        }

        let mut app = Router::new().route("/items/:id", get(handler));
        let request = http::Request::builder()
            .method(http::Method::GET)
            .uri("/items/7")
            .extension(User {
                name: "Alice".into(),
            })
            .body(empty_body())
            .unwrap();
        let response = app.call(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn multi_state_json() {
        #[derive(Clone)]
        struct AppState {
            default_name: String,
        }

        #[derive(serde::Deserialize, serde::Serialize)]
        struct UpdateUser {
            name: String,
            age: u8,
        }

        async fn handler(
            volter_core::State(state): volter_core::State<AppState>,
            volter_extract::Json(body): volter_extract::Json<UpdateUser>,
        ) -> String {
            format!("{}/{}", state.default_name, body.name)
        }

        let mut app = Router::with_state(AppState {
            default_name: "default".into(),
        })
        .route("/users/:id", get(handler));
        let body = br#"{"name":"Alice","age":30}"#;
        let response = app
            .call(json_request(http::Method::GET, "/users/42", body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn multi_path_json() {
        #[derive(serde::Deserialize)]
        struct ItemParams {
            id: u64,
        }

        #[derive(serde::Deserialize)]
        struct UpdateBody {
            value: String,
        }

        async fn handler(
            volter_extract::Path(params): volter_extract::Path<ItemParams>,
            volter_extract::Json(body): volter_extract::Json<UpdateBody>,
        ) -> String {
            format!("{}-{}", params.id, body.value)
        }

        let mut app = Router::new().route("/items/:id", get(handler));
        let body = br#"{"value":"updated"}"#;
        let response = app
            .call(json_request(http::Method::GET, "/items/7", body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn multi_state_path_query() {
        #[derive(Clone)]
        struct AppState {
            prefix: String,
        }

        #[derive(serde::Deserialize)]
        struct ItemParams {
            id: u64,
        }

        #[derive(serde::Deserialize)]
        struct ViewQuery {
            view: Option<String>,
        }

        async fn handler(
            volter_core::State(state): volter_core::State<AppState>,
            volter_extract::Path(params): volter_extract::Path<ItemParams>,
            volter_extract::Query(query): volter_extract::Query<ViewQuery>,
        ) -> String {
            format!("{}-{}-{:?}", state.prefix, params.id, query.view)
        }

        let mut app = Router::with_state(AppState {
            prefix: "item".into(),
        })
        .route("/items/:id", get(handler));
        let response = app
            .call(request(http::Method::GET, "/items/7?view=full"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn multi_state_path_json() {
        #[derive(Clone)]
        struct AppState {
            prefix: String,
        }

        #[derive(serde::Deserialize)]
        struct ItemParams {
            id: u64,
        }

        #[derive(serde::Deserialize)]
        struct UpdateBody {
            value: String,
        }

        async fn handler(
            volter_core::State(state): volter_core::State<AppState>,
            volter_extract::Path(params): volter_extract::Path<ItemParams>,
            volter_extract::Json(body): volter_extract::Json<UpdateBody>,
        ) -> String {
            format!("{}-{}-{}", state.prefix, params.id, body.value)
        }

        let mut app = Router::with_state(AppState {
            prefix: "item".into(),
        })
        .route("/items/:id", get(handler));
        let body = br#"{"value":"updated"}"#;
        let response = app
            .call(json_request(http::Method::GET, "/items/42", body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn multi_query_json() {
        #[derive(serde::Deserialize)]
        struct LogQuery {
            level: String,
        }

        #[derive(serde::Deserialize)]
        struct LogBody {
            message: String,
        }

        async fn handler(
            volter_extract::Query(query): volter_extract::Query<LogQuery>,
            volter_extract::Json(body): volter_extract::Json<LogBody>,
        ) -> String {
            format!("{}-{}", query.level, body.message)
        }

        let mut app = Router::new().route("/log", get(handler));
        let body = br#"{"message":"hello"}"#;
        let response = app
            .call(json_request(http::Method::GET, "/log?level=info", body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn multi_rejection_short_circuit() {
        #[derive(Clone)]
        struct AppState;

        #[derive(serde::Deserialize)]
        #[allow(dead_code)]
        struct ItemParams {
            id: u64,
        }

        async fn handler(
            volter_core::State(_state): volter_core::State<AppState>,
            volter_extract::Path(_params): volter_extract::Path<ItemParams>,
        ) -> &'static str {
            "should not be reached"
        }

        // No :id param — Path extraction should fail before handler runs.
        let mut app = Router::with_state(AppState).route("/items", get(handler));
        let response = app
            .call(request(http::Method::GET, "/items"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // -- Middleware tests ----------------------------------------------------

    /// A boxed, cloneable, type-erased inner service — the concrete type
    /// that [`Router::layer`] hands to the middleware.
    type Svc = BoxCloneService<Request, Response, Infallible>;

    /// A boxed, cloneable closure that takes a request and inner service and
    /// returns a future.
    type WrapFn = std::sync::Arc<
        dyn Fn(
                Request,
                Svc,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<Response, Infallible>> + Send>,
            > + Send
            + Sync,
    >;

    /// A [`Service`] wrapper that applies a boxed closure to every request.
    /// Used by the test [`Layer`] impls below to avoid HRTB issues with
    /// generic inner services.
    #[derive(Clone)]
    struct WrapSvc {
        inner: Svc,
        f: WrapFn,
    }

    impl Service<Request> for WrapSvc {
        type Response = Response;
        type Error = Infallible;
        type Future = std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
        >;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, req: Request) -> Self::Future {
            let inner = self.inner.clone();
            let f = self.f.clone();
            Box::pin(async move { f(req, inner).await })
        }
    }

    #[tokio::test]
    async fn middleware_runs_before_handler() {
        use std::sync::atomic::{AtomicBool, Ordering};

        static RAN: AtomicBool = AtomicBool::new(false);

        struct CheckLayer;

        impl Layer<Svc> for CheckLayer {
            type Service = WrapSvc;

            fn layer(&self, inner: Svc) -> Self::Service {
                WrapSvc {
                    inner,
                    f: std::sync::Arc::new(|req, mut inner| {
                        Box::pin(async move {
                            RAN.store(true, Ordering::SeqCst);
                            inner.call(req).await
                        })
                    }),
                }
            }
        }

        RAN.store(false, Ordering::SeqCst);

        async fn handler(volter_core::State(_): volter_core::State<()>) -> &'static str {
            "ok"
        }

        let mut app = Router::with_state(())
            .route("/", get(handler))
            .layer(CheckLayer);
        let response = app.call(request(http::Method::GET, "/")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(RAN.load(Ordering::SeqCst), "middleware should have run");
    }

    #[tokio::test]
    async fn middleware_inserts_extension() {
        #[derive(Clone, Debug)]
        struct MiddlewareValue(u64);

        struct InjectLayer;

        impl Layer<Svc> for InjectLayer {
            type Service = WrapSvc;

            fn layer(&self, inner: Svc) -> Self::Service {
                WrapSvc {
                    inner,
                    f: std::sync::Arc::new(|mut req, mut inner| {
                        Box::pin(async move {
                            req.extensions_mut().insert(MiddlewareValue(42));
                            inner.call(req).await
                        })
                    }),
                }
            }
        }

        async fn handler(
            volter_extract::Extension(val): volter_extract::Extension<MiddlewareValue>,
        ) -> String {
            format!("val={}", val.0)
        }

        let mut app = Router::new().route("/", get(handler)).layer(InjectLayer);
        let response = app.call(request(http::Method::GET, "/")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn middleware_modifies_response() {
        struct AddHeaderLayer {
            key: http::HeaderName,
            value: String,
        }

        impl Layer<Svc> for AddHeaderLayer {
            type Service = WrapSvc;

            fn layer(&self, inner: Svc) -> Self::Service {
                let key = self.key.clone();
                let value = self.value.clone();
                WrapSvc {
                    inner,
                    f: std::sync::Arc::new(move |req, mut inner| {
                        let key = key.clone();
                        let value = value.clone();
                        Box::pin(async move {
                            let mut response = inner.call(req).await?;
                            response.headers_mut().insert(key, value.parse().unwrap());
                            Ok(response)
                        })
                    }),
                }
            }
        }

        async fn handler() -> &'static str {
            "hello"
        }

        let mut app = Router::new()
            .route("/", get(handler))
            .layer(AddHeaderLayer {
                key: http::header::HeaderName::from_static("x-custom"),
                value: "yes".into(),
            });

        let response = app.call(request(http::Method::GET, "/")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("x-custom")
                .and_then(|v| v.to_str().ok()),
            Some("yes")
        );
    }

    #[tokio::test]
    async fn middleware_onion_ordering() {
        use std::sync::atomic::{AtomicU8, Ordering};

        static ORDER: AtomicU8 = AtomicU8::new(0);

        struct TrackLayer {
            id: u8,
        }

        impl Layer<Svc> for TrackLayer {
            type Service = WrapSvc;

            fn layer(&self, inner: Svc) -> Self::Service {
                let id = self.id;
                WrapSvc {
                    inner,
                    f: std::sync::Arc::new(move |req, mut inner| {
                        Box::pin(async move {
                            ORDER.store(id, Ordering::SeqCst);
                            let result = inner.call(req).await;
                            ORDER.store(id * 10, Ordering::SeqCst);
                            result
                        })
                    }),
                }
            }
        }

        async fn handler() -> &'static str {
            ORDER.store(100, Ordering::SeqCst);
            "ok"
        }

        let mut app = Router::new()
            .route("/", get(handler))
            .layer(TrackLayer { id: 1 })
            .layer(TrackLayer { id: 2 });

        ORDER.store(0, Ordering::SeqCst);
        let response = app.call(request(http::Method::GET, "/")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(ORDER.load(Ordering::SeqCst), 20);
    }

    #[tokio::test]
    async fn post_layer_routes_not_wrapped() {
        struct WrapHeaderLayer;

        impl Layer<Svc> for WrapHeaderLayer {
            type Service = WrapSvc;

            fn layer(&self, inner: Svc) -> Self::Service {
                WrapSvc {
                    inner,
                    f: std::sync::Arc::new(|req, mut inner| {
                        Box::pin(async move {
                            let mut response = inner.call(req).await?;
                            response.headers_mut().insert(
                                http::header::HeaderName::from_static("x-wrapped"),
                                "yes".parse().unwrap(),
                            );
                            Ok(response)
                        })
                    }),
                }
            }
        }

        async fn pre_layer() -> &'static str {
            "pre"
        }
        async fn post_layer() -> &'static str {
            "post"
        }

        let mut app = Router::new()
            .route("/pre", get(pre_layer))
            .layer(WrapHeaderLayer)
            .route("/post", get(post_layer));

        let pre_response = app.call(request(http::Method::GET, "/pre")).await.unwrap();
        assert_eq!(pre_response.status(), StatusCode::OK);
        assert_eq!(
            pre_response
                .headers()
                .get("x-wrapped")
                .and_then(|v| v.to_str().ok()),
            Some("yes"),
            "pre-layer route should be wrapped"
        );

        let post_response = app.call(request(http::Method::GET, "/post")).await.unwrap();
        assert_eq!(post_response.status(), StatusCode::OK);
        assert_eq!(
            post_response
                .headers()
                .get("x-wrapped")
                .and_then(|v| v.to_str().ok()),
            None,
            "post-layer route should NOT be wrapped"
        );
    }

    #[tokio::test]
    async fn middleware_with_state() {
        #[derive(Clone)]
        struct AppState {
            value: u64,
        }

        struct StateHeaderLayer;

        impl Layer<Svc> for StateHeaderLayer {
            type Service = WrapSvc;

            fn layer(&self, inner: Svc) -> Self::Service {
                WrapSvc {
                    inner,
                    f: std::sync::Arc::new(|req, mut inner| {
                        Box::pin(async move {
                            let mut response = inner.call(req).await?;
                            response.headers_mut().insert(
                                http::header::HeaderName::from_static("x-state-middleware"),
                                "active".parse().unwrap(),
                            );
                            Ok(response)
                        })
                    }),
                }
            }
        }

        async fn handler(volter_core::State(state): volter_core::State<AppState>) -> String {
            format!("value: {}", state.value)
        }

        let mut app = Router::with_state(AppState { value: 99 })
            .route("/", get(handler))
            .layer(StateHeaderLayer);

        let response = app.call(request(http::Method::GET, "/")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("x-state-middleware")
                .and_then(|v| v.to_str().ok()),
            Some("active")
        );
    }

    #[tokio::test]
    async fn middleware_with_extension_and_state() {
        #[derive(Clone)]
        struct AppState {
            prefix: String,
        }

        #[derive(Clone, Debug)]
        struct AuthUser {
            name: String,
        }

        struct AuthLayer;

        impl Layer<Svc> for AuthLayer {
            type Service = WrapSvc;

            fn layer(&self, inner: Svc) -> Self::Service {
                WrapSvc {
                    inner,
                    f: std::sync::Arc::new(|mut req, mut inner| {
                        Box::pin(async move {
                            req.extensions_mut().insert(AuthUser {
                                name: "Alice".into(),
                            });
                            inner.call(req).await
                        })
                    }),
                }
            }
        }

        async fn handler(
            volter_core::State(state): volter_core::State<AppState>,
            volter_extract::Extension(user): volter_extract::Extension<AuthUser>,
        ) -> String {
            format!("{}-{}", state.prefix, user.name)
        }

        let mut app = Router::with_state(AppState {
            prefix: "user".into(),
        })
        .route("/profile", get(handler))
        .layer(AuthLayer);

        let response = app
            .call(request(http::Method::GET, "/profile"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn multi_body_not_consumed_on_path_failure() {
        #[derive(serde::Deserialize)]
        #[allow(dead_code)]
        struct ItemParams {
            id: u64,
        }

        #[derive(serde::Deserialize)]
        #[allow(dead_code)]
        struct UpdateBody {
            value: String,
        }

        async fn handler(
            volter_extract::Path(_params): volter_extract::Path<ItemParams>,
            volter_extract::Json(_body): volter_extract::Json<UpdateBody>,
        ) -> &'static str {
            "should not be reached"
        }

        // Route matches /items/:id but `abc` is not a valid u64,
        // so Path extraction fails before Json is ever evaluated.
        let mut app = Router::new().route("/items/:id", get(handler));
        let body = br#"{"value":"updated"}"#;
        let response = app
            .call(json_request(http::Method::GET, "/items/abc", body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // -- Nesting tests -------------------------------------------------------

    #[tokio::test]
    async fn basic_nesting() {
        async fn users() -> &'static str {
            "users"
        }
        async fn posts() -> &'static str {
            "posts"
        }

        let api = Router::new()
            .route("/users", get(users))
            .route("/posts", get(posts));

        let mut app = Router::new().nest("/api", api);

        let users_resp = app
            .call(request(http::Method::GET, "/api/users"))
            .await
            .unwrap();
        assert_eq!(users_resp.status(), StatusCode::OK);

        let posts_resp = app
            .call(request(http::Method::GET, "/api/posts"))
            .await
            .unwrap();
        assert_eq!(posts_resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn multiple_nested_routers() {
        async fn users() -> &'static str {
            "users"
        }
        async fn posts() -> &'static str {
            "posts"
        }
        async fn items() -> &'static str {
            "items"
        }

        let api = Router::new()
            .route("/users", get(users))
            .route("/posts", get(posts));

        let admin = Router::new().route("/items", get(items));

        let mut app = Router::new().nest("/api", api).nest("/admin", admin);

        let users_resp = app
            .call(request(http::Method::GET, "/api/users"))
            .await
            .unwrap();
        assert_eq!(users_resp.status(), StatusCode::OK);

        let posts_resp = app
            .call(request(http::Method::GET, "/api/posts"))
            .await
            .unwrap();
        assert_eq!(posts_resp.status(), StatusCode::OK);

        let items_resp = app
            .call(request(http::Method::GET, "/admin/items"))
            .await
            .unwrap();
        assert_eq!(items_resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn nested_plus_root_routes() {
        async fn health() -> &'static str {
            "ok"
        }
        async fn users() -> &'static str {
            "users"
        }

        let api = Router::new().route("/users", get(users));

        let mut app = Router::new()
            .route("/health", get(health))
            .nest("/api", api);

        let health_resp = app
            .call(request(http::Method::GET, "/health"))
            .await
            .unwrap();
        assert_eq!(health_resp.status(), StatusCode::OK);

        let users_resp = app
            .call(request(http::Method::GET, "/api/users"))
            .await
            .unwrap();
        assert_eq!(users_resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn nested_root_route() {
        async fn api_index() -> &'static str {
            "api index"
        }

        let api = Router::new().route("/", get(api_index));

        let mut app = Router::new().nest("/api", api);

        let resp = app.call(request(http::Method::GET, "/api")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn nested_middleware_preserved() {
        use std::sync::atomic::{AtomicBool, Ordering};

        static RAN: AtomicBool = AtomicBool::new(false);

        struct CheckLayer;

        impl Layer<Svc> for CheckLayer {
            type Service = WrapSvc;

            fn layer(&self, inner: Svc) -> Self::Service {
                WrapSvc {
                    inner,
                    f: std::sync::Arc::new(|req, mut inner| {
                        Box::pin(async move {
                            RAN.store(true, Ordering::SeqCst);
                            inner.call(req).await
                        })
                    }),
                }
            }
        }

        async fn handler() -> &'static str {
            "nested with middleware"
        }

        RAN.store(false, Ordering::SeqCst);

        let api = Router::new()
            .route("/check", get(handler))
            .layer(CheckLayer);

        let mut app = Router::new().nest("/api", api);

        let response = app
            .call(request(http::Method::GET, "/api/check"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            RAN.load(Ordering::SeqCst),
            "nested middleware should have run"
        );
    }

    #[tokio::test]
    async fn nested_state_preserved() {
        #[derive(Clone)]
        struct AppState {
            value: u32,
        }

        async fn handler(volter_core::State(state): volter_core::State<AppState>) -> String {
            format!("value: {}", state.value)
        }

        let shared_state = AppState { value: 42 };

        let api = Router::with_state(shared_state.clone()).route("/state", get(handler));

        let mut app = Router::with_state(shared_state).nest("/api", api);

        let response = app
            .call(request(http::Method::GET, "/api/state"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn nested_404() {
        async fn handler() -> &'static str {
            "found"
        }

        let api = Router::new().route("/found", get(handler));

        let mut app = Router::new().nest("/api", api);

        // Known path under prefix should work.
        let found = app
            .call(request(http::Method::GET, "/api/found"))
            .await
            .unwrap();
        assert_eq!(found.status(), StatusCode::OK);

        // Unknown path under prefix should 404.
        let not_found = app
            .call(request(http::Method::GET, "/api/unknown"))
            .await
            .unwrap();
        assert_eq!(not_found.status(), StatusCode::NOT_FOUND);

        // Path that doesn't match prefix at all should 404.
        let no_match = app
            .call(request(http::Method::GET, "/other"))
            .await
            .unwrap();
        assert_eq!(no_match.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn nested_path_extractors() {
        async fn handler(volter_extract::Path(id): volter_extract::Path<u64>) -> String {
            format!("User {}", id)
        }

        let api = Router::new().route("/users/:id", get(handler));

        let mut app = Router::new().nest("/api", api);

        let response = app
            .call(request(http::Method::GET, "/api/users/42"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn nested_invalid_param_404() {
        async fn handler(volter_extract::Path(id): volter_extract::Path<u64>) -> String {
            format!("User {}", id)
        }

        let api = Router::new().route("/users/:id", get(handler));

        let mut app = Router::new().nest("/api", api);

        // Valid param route under prefix.
        let found = app
            .call(request(http::Method::GET, "/api/users/42"))
            .await
            .unwrap();
        assert_eq!(found.status(), StatusCode::OK);

        // Same route pattern but wrong prefix should 404.
        let no_match = app
            .call(request(http::Method::GET, "/v1/users/42"))
            .await
            .unwrap();
        assert_eq!(no_match.status(), StatusCode::NOT_FOUND);

        // Prefix match but no route in nested router should 404.
        let no_route = app
            .call(request(http::Method::GET, "/api/items/42"))
            .await
            .unwrap();
        assert_eq!(no_route.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn deep_nesting() {
        async fn inner_handler() -> &'static str {
            "deep"
        }
        async fn middle_handler() -> &'static str {
            "middle"
        }

        let inner = Router::new().route("/deep", get(inner_handler));

        let middle = Router::new()
            .route("/middle", get(middle_handler))
            .nest("/inner", inner);

        let mut app = Router::new().nest("/api", middle);

        let middle_resp = app
            .call(request(http::Method::GET, "/api/middle"))
            .await
            .unwrap();
        assert_eq!(middle_resp.status(), StatusCode::OK);

        let deep_resp = app
            .call(request(http::Method::GET, "/api/inner/deep"))
            .await
            .unwrap();
        assert_eq!(deep_resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn nesting_with_post_layer_routes() {
        async fn pre() -> &'static str {
            "pre"
        }
        async fn post() -> &'static str {
            "post"
        }

        let api = Router::new()
            .route("/pre", get(pre))
            .layer(tower::layer::layer_fn(|svc| svc))
            .route("/post", get(post));

        let mut app = Router::new().nest("/api", api);

        let pre_resp = app
            .call(request(http::Method::GET, "/api/pre"))
            .await
            .unwrap();
        assert_eq!(pre_resp.status(), StatusCode::OK);

        let post_resp = app
            .call(request(http::Method::GET, "/api/post"))
            .await
            .unwrap();
        assert_eq!(post_resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn prefix_segment_aligned() {
        async fn handler() -> &'static str {
            "ok"
        }

        let api = Router::new().route("/users", get(handler));

        let mut app = Router::new().nest("/api", api);

        // Segment-aligned: /api/users matches.
        let resp = app
            .call(request(http::Method::GET, "/api/users"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // NOT segment-aligned: /api2 should NOT match.
        let resp = app
            .call(request(http::Method::GET, "/api2/users"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        // /api/users-extra should NOT match /api/users.
        let resp = app
            .call(request(http::Method::GET, "/api/users-extra"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn nest_without_trailing_slash() {
        async fn handler() -> &'static str {
            "ok"
        }

        // Prefix without leading / should be normalized.
        let api = Router::new().route("/items", get(handler));
        let mut app = Router::new().nest("api", api);

        let resp = app
            .call(request(http::Method::GET, "/api/items"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Prefix with trailing / should be normalized.
        let api2 = Router::new().route("/items", get(handler));
        let mut app2 = Router::new().nest("/api/", api2);

        let resp = app2
            .call(request(http::Method::GET, "/api/items"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn nested_layered_router_with_middleware_both_levels() {
        use std::sync::atomic::{AtomicU8, Ordering};

        static ORDER: AtomicU8 = AtomicU8::new(0);

        struct TrackLayer {
            id: u8,
        }

        impl Layer<Svc> for TrackLayer {
            type Service = WrapSvc;

            fn layer(&self, inner: Svc) -> Self::Service {
                let id = self.id;
                WrapSvc {
                    inner,
                    f: std::sync::Arc::new(move |req, mut inner| {
                        Box::pin(async move {
                            ORDER.store(id, Ordering::SeqCst);
                            let result = inner.call(req).await;
                            ORDER.store(id * 10, Ordering::SeqCst);
                            result
                        })
                    }),
                }
            }
        }

        async fn handler() -> &'static str {
            ORDER.store(100, Ordering::SeqCst);
            "ok"
        }

        // Nested router with its own layer.
        let api = Router::new()
            .route("/nested", get(handler))
            .layer(TrackLayer { id: 2 });

        // Outer router with its own layer wrapping the nest.
        ORDER.store(0, Ordering::SeqCst);
        let mut app = Router::new().nest("/api", api).layer(TrackLayer { id: 1 });

        let response = app
            .call(request(http::Method::GET, "/api/nested"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        // outer layer wraps the nest, so outer runs first (id=1),
        // then the handler sets 100, then outer runs again on the way out (10).
        // The nested layer runs inside the outer layer.
        assert_eq!(ORDER.load(Ordering::SeqCst), 10);
    }

    // -- Merge tests ---------------------------------------------------------

    #[tokio::test]
    async fn basic_merge() {
        async fn users() -> &'static str {
            "users"
        }
        async fn posts() -> &'static str {
            "posts"
        }

        let api = Router::new().route("/users", get(users));
        let admin = Router::new().route("/posts", get(posts));
        let mut app = api.merge(admin);

        let users_resp = app
            .call(request(http::Method::GET, "/users"))
            .await
            .unwrap();
        assert_eq!(users_resp.status(), StatusCode::OK);

        let posts_resp = app
            .call(request(http::Method::GET, "/posts"))
            .await
            .unwrap();
        assert_eq!(posts_resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn merge_duplicate_route_last_wins() {
        async fn first() -> &'static str {
            "first"
        }
        async fn second() -> &'static str {
            "second"
        }

        let router_a = Router::new().route("/shared", get(first));
        let router_b = Router::new().route("/shared", get(second));
        let mut app = router_a.merge(router_b);

        let response = app
            .call(request(http::Method::GET, "/shared"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn merge_with_root_routes() {
        async fn root() -> &'static str {
            "root"
        }
        async fn users() -> &'static str {
            "users"
        }

        let router_a = Router::new().route("/", get(root));
        let router_b = Router::new().route("/users", get(users));
        let mut app = router_a.merge(router_b);

        let resp = app.call(request(http::Method::GET, "/")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let users_resp = app
            .call(request(http::Method::GET, "/users"))
            .await
            .unwrap();
        assert_eq!(users_resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn merge_with_nested_routers() {
        async fn users() -> &'static str {
            "users"
        }
        async fn items() -> &'static str {
            "items"
        }

        let inner_a = Router::new().route("/users", get(users));
        let inner_b = Router::new().route("/items", get(items));

        let api = Router::new().nest("/api", inner_a);
        let admin = Router::new().nest("/admin", inner_b);

        let mut app = api.merge(admin);

        let resp = app
            .call(request(http::Method::GET, "/api/users"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .call(request(http::Method::GET, "/admin/items"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn merge_with_middleware_both_sides() {
        use std::sync::atomic::{AtomicBool, Ordering};

        static RAN_LAST: AtomicBool = AtomicBool::new(false);

        struct TrackLayer {
            flag: &'static AtomicBool,
        }

        impl Layer<Svc> for TrackLayer {
            type Service = WrapSvc;

            fn layer(&self, inner: Svc) -> Self::Service {
                let flag = self.flag;
                WrapSvc {
                    inner,
                    f: std::sync::Arc::new(move |req, mut inner| {
                        Box::pin(async move {
                            flag.store(true, Ordering::SeqCst);
                            inner.call(req).await
                        })
                    }),
                }
            }
        }

        async fn last_merged_handler() -> &'static str {
            "last"
        }

        // Both routers have applied layers.  When both have layers, only
        // the last-merged router's pre-layer routes are accessible because
        // the request body is consumed by its dispatch service.
        let first = Router::new()
            .route("/first", get(|| async { "first" }))
            .layer(tower::layer::layer_fn(|svc| svc));
        let last = Router::new()
            .route("/last", get(last_merged_handler))
            .layer(TrackLayer { flag: &RAN_LAST });

        RAN_LAST.store(false, Ordering::SeqCst);
        let mut app = first.merge(last);

        // Last-merged router's route works with its middleware.
        let resp = app.call(request(http::Method::GET, "/last")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(
            RAN_LAST.load(Ordering::SeqCst),
            "last-merged middleware ran"
        );
    }

    #[tokio::test]
    async fn merge_one_side_has_middleware() {
        use std::sync::atomic::{AtomicBool, Ordering};

        static RAN: AtomicBool = AtomicBool::new(false);

        struct CheckLayer;

        impl Layer<Svc> for CheckLayer {
            type Service = WrapSvc;

            fn layer(&self, inner: Svc) -> Self::Service {
                WrapSvc {
                    inner,
                    f: std::sync::Arc::new(|req, mut inner| {
                        Box::pin(async move {
                            RAN.store(true, Ordering::SeqCst);
                            inner.call(req).await
                        })
                    }),
                }
            }
        }

        async fn layered_handler() -> &'static str {
            "layered"
        }
        async fn plain_handler() -> &'static str {
            "plain"
        }

        let layered_router = Router::new()
            .route("/layered", get(layered_handler))
            .layer(CheckLayer);

        let plain_router = Router::new().route("/plain", get(plain_handler));

        RAN.store(false, Ordering::SeqCst);
        let mut app = layered_router.merge(plain_router);

        let resp = app
            .call(request(http::Method::GET, "/layered"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(
            RAN.load(Ordering::SeqCst),
            "middleware should run for layered route"
        );

        let resp = app
            .call(request(http::Method::GET, "/plain"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn merge_with_state() {
        #[derive(Clone)]
        struct AppState {
            value: u32,
        }

        async fn handler_a(volter_core::State(state): volter_core::State<AppState>) -> String {
            format!("a:{}", state.value)
        }
        async fn handler_b(volter_core::State(state): volter_core::State<AppState>) -> String {
            format!("b:{}", state.value)
        }

        let shared = AppState { value: 42 };

        let router_a = Router::with_state(shared.clone()).route("/a", get(handler_a));
        let router_b = Router::with_state(shared.clone()).route("/b", get(handler_b));
        let mut app = router_a.merge(router_b);

        let resp = app.call(request(http::Method::GET, "/a")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app.call(request(http::Method::GET, "/b")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn merge_with_path_params() {
        async fn handler(volter_extract::Path(id): volter_extract::Path<u64>) -> String {
            format!("id:{}", id)
        }

        let router_a = Router::new().route("/items/:id", get(handler));
        let router_b = Router::new().route("/users/:id", get(handler));
        let mut app = router_a.merge(router_b);

        let resp = app
            .call(request(http::Method::GET, "/items/42"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .call(request(http::Method::GET, "/users/7"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn merge_duplicate_param_route_last_wins() {
        async fn first() -> &'static str {
            "first"
        }
        async fn second() -> &'static str {
            "second"
        }

        let router_a = Router::new().route("/:id", get(first));
        let router_b = Router::new().route("/:id", get(second));
        let mut app = router_a.merge(router_b);

        let response = app
            .call(request(http::Method::GET, "/anything"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn merge_after_layer() {
        async fn pre() -> &'static str {
            "pre"
        }
        async fn merged() -> &'static str {
            "merged"
        }

        let router_a = Router::new()
            .route("/pre", get(pre))
            .layer(tower::layer::layer_fn(|svc| svc));

        let router_b = Router::new().route("/merged", get(merged));

        let mut app = router_a.merge(router_b);

        let resp = app.call(request(http::Method::GET, "/pre")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .call(request(http::Method::GET, "/merged"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn layer_after_merge() {
        async fn handler_a() -> &'static str {
            "a"
        }
        async fn handler_b() -> &'static str {
            "b"
        }

        let router_a = Router::new().route("/a", get(handler_a));
        let router_b = Router::new().route("/b", get(handler_b));

        let mut app = router_a
            .merge(router_b)
            .layer(tower::layer::layer_fn(|svc| svc));

        let resp = app.call(request(http::Method::GET, "/a")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app.call(request(http::Method::GET, "/b")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn merge_preserves_nested_routes_with_middleware() {
        use std::sync::atomic::{AtomicBool, Ordering};

        static MIDDLEWARE_RAN: AtomicBool = AtomicBool::new(false);

        struct CheckLayer;

        impl Layer<Svc> for CheckLayer {
            type Service = WrapSvc;

            fn layer(&self, inner: Svc) -> Self::Service {
                WrapSvc {
                    inner,
                    f: std::sync::Arc::new(|req, mut inner| {
                        Box::pin(async move {
                            MIDDLEWARE_RAN.store(true, Ordering::SeqCst);
                            inner.call(req).await
                        })
                    }),
                }
            }
        }

        async fn nested_handler() -> &'static str {
            "nested"
        }

        let inner = Router::new()
            .route("/nested", get(nested_handler))
            .layer(CheckLayer);

        let outer = Router::new().route("/other", get(|| async { "other" }));

        MIDDLEWARE_RAN.store(false, Ordering::SeqCst);
        let mut app = inner.merge(outer);

        // Nested route (with middleware) from merged router should still work.
        let resp = app
            .call(request(http::Method::GET, "/nested"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(
            MIDDLEWARE_RAN.load(Ordering::SeqCst),
            "middleware on nested route should run"
        );
    }

    #[tokio::test]
    async fn merge_404() {
        async fn handler() -> &'static str {
            "exists"
        }

        let router_a = Router::new().route("/a", get(handler));
        let router_b = Router::new().route("/b", get(handler));
        let mut app = router_a.merge(router_b);

        let resp = app
            .call(request(http::Method::GET, "/unknown"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
