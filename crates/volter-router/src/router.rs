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

    #[tokio::test]
    async fn multi_body_not_consumed_on_path_failure() {
        #[derive(serde::Deserialize)]
        struct ItemParams {
            id: u64,
        }

        #[derive(serde::Deserialize)]
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
}
