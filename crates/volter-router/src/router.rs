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
use tower::Service;

use volter_core::{Request, Response, UrlParams};

use crate::error::{method_not_allowed_response, not_found_response};
use crate::method_router::{BoxedFuture, MethodRouter};
use crate::pattern::RoutePattern;

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
// Router
// ---------------------------------------------------------------------------

/// A Volter router.
///
/// Routes incoming HTTP requests to handlers based on the request path and
/// HTTP method.  Implements [`tower::Service`] so it composes with the
/// entire `tower` / `tower-http` middleware ecosystem.
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
pub struct Router<S = ()> {
    state: S,
    static_routes: HashMap<String, HashMap<Method, BoxCloneService<Request, Response, Infallible>>>,
    param_routes: Vec<ParamRoute>,
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

        // 1. Try static routes (exact match, O(1)).
        if let Some(services) = self.static_routes.get(&path) {
            return dispatch_services(services, method, req);
        }

        // 2. Try parameterized routes (linear scan).
        for param_route in &self.param_routes {
            if let Some(params) = param_route.pattern.matches(&path) {
                let mut req = req;
                req.extensions_mut().insert(UrlParams(params));
                return dispatch_services(&param_route.services, method, req);
            }
        }

        // 3. No route matched.
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::get;
    use http::StatusCode;
    use volter_core::empty_body;

    fn request(method: http::Method, path: &str) -> Request {
        http::Request::builder()
            .method(method)
            .uri(path)
            .body(empty_body())
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
}
