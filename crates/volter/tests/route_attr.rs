//! Integration tests for `RouteAttr` and route attribute macros `#[get]` /
//! `#[post]`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use tower::Service;
use volter::*;
use volter_core::full_body;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn get_request(path: &str) -> http::Request<BoxBody> {
    http::Request::builder()
        .uri(path)
        .method(http::Method::GET)
        .body(volter_core::empty_body())
        .unwrap()
}

fn post_json(path: &str, body: &[u8]) -> http::Request<BoxBody> {
    http::Request::builder()
        .uri(path)
        .method(http::Method::POST)
        .header("content-type", "application/json")
        .body(full_body(bytes::Bytes::copy_from_slice(body)))
        .unwrap()
}

// ---------------------------------------------------------------------------
// RouteAttr construction (no attribute macros)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn route_attr_get_basic() {
    async fn hello() -> &'static str {
        "Hello!"
    }

    const ROUTE: RouteAttr = RouteAttr::get("/hello");
    let mut app: Router = Router::new().route_attr(ROUTE, hello);

    let resp = app.call(get_request("/hello")).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[tokio::test]
async fn route_attr_post_basic() {
    #[derive(serde::Deserialize)]
    struct CreateUser {
        name: String,
    }

    async fn create(Json(payload): Json<CreateUser>) -> &'static str {
        assert_eq!(payload.name, "Alice");
        "created"
    }

    const ROUTE: RouteAttr = RouteAttr::post("/users");
    let mut app: Router = Router::new().route_attr(ROUTE, create);

    let body = br#"{"name":"Alice"}"#;
    let resp = app.call(post_json("/users", body)).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[tokio::test]
async fn route_attr_with_path_params() {
    async fn user(Path(id): Path<u64>) -> String {
        format!("User {}", id)
    }

    const ROUTE: RouteAttr = RouteAttr::get("/users/:id");
    let mut app: Router = Router::new().route_attr(ROUTE, user);

    let resp = app.call(get_request("/users/42")).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[tokio::test]
async fn route_attr_with_state() {
    #[derive(Clone)]
    struct AppState {
        value: u32,
    }

    async fn handler(State(state): State<AppState>) -> String {
        format!("value: {}", state.value)
    }

    const ROUTE: RouteAttr = RouteAttr::get("/state");
    let mut app = Router::with_state(AppState { value: 42 }).route_attr(ROUTE, handler);

    let resp = app.call(get_request("/state")).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[tokio::test]
async fn route_attr_with_query() {
    #[derive(serde::Deserialize)]
    struct PageQuery {
        page: u32,
    }

    async fn handler(Query(query): Query<PageQuery>) -> String {
        format!("Page {}", query.page)
    }

    const ROUTE: RouteAttr = RouteAttr::get("/list");
    let mut app: Router = Router::new().route_attr(ROUTE, handler);

    let resp = app.call(get_request("/list?page=3")).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[tokio::test]
async fn route_attr_wrong_method_returns_405() {
    async fn hello() -> &'static str {
        "Hello!"
    }

    const ROUTE: RouteAttr = RouteAttr::get("/hello");
    let mut app: Router = Router::new().route_attr(ROUTE, hello);

    let req = http::Request::builder()
        .uri("/hello")
        .method(http::Method::POST)
        .body(volter_core::empty_body())
        .unwrap();
    let resp = app.call(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn route_attr_unknown_path_returns_404() {
    async fn hello() -> &'static str {
        "Hello!"
    }

    const ROUTE: RouteAttr = RouteAttr::get("/hello");
    let mut app: Router = Router::new().route_attr(ROUTE, hello);

    let resp = app.call(get_request("/unknown")).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn route_attr_multiple_routes() {
    async fn root() -> &'static str {
        "root"
    }
    async fn users() -> &'static str {
        "users"
    }

    const ROOT: RouteAttr = RouteAttr::get("/");
    const USERS: RouteAttr = RouteAttr::get("/users");
    let mut app: Router = Router::new()
        .route_attr(ROOT, root)
        .route_attr(USERS, users);

    let root_resp = app.call(get_request("/")).await.unwrap();
    assert_eq!(root_resp.status(), http::StatusCode::OK);

    let users_resp = app.call(get_request("/users")).await.unwrap();
    assert_eq!(users_resp.status(), http::StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Route attribute macro tests
// ---------------------------------------------------------------------------

#[get("/route-attr-macro-get")]
async fn route_attr_macro_get_handler() -> &'static str {
    "from macro"
}

#[tokio::test]
async fn route_attr_macro_get() {
    let mut app: Router = Router::new().route_attr(
        ROUTE_ATTR_MACRO_GET_HANDLER_ROUTE,
        route_attr_macro_get_handler,
    );
    let resp = app
        .call(get_request("/route-attr-macro-get"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[post("/route-attr-macro-post")]
async fn route_attr_macro_post_handler() -> &'static str {
    "from macro post"
}

#[tokio::test]
async fn route_attr_macro_post() {
    let mut app: Router = Router::new().route_attr(
        ROUTE_ATTR_MACRO_POST_HANDLER_ROUTE,
        route_attr_macro_post_handler,
    );
    let req = http::Request::builder()
        .uri("/route-attr-macro-post")
        .method(http::Method::POST)
        .body(volter_core::empty_body())
        .unwrap();
    let resp = app.call(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[derive(serde::Deserialize)]
struct HashMapPage {
    page: u32,
}

#[get("/macro-extractors/:id")]
async fn macro_with_extractors(Path(id): Path<u64>, Query(page): Query<HashMapPage>) -> String {
    format!("id={}, page={}", id, page.page)
}

#[tokio::test]
async fn route_attr_macro_with_extractors() {
    let mut app: Router =
        Router::new().route_attr(MACRO_WITH_EXTRACTORS_ROUTE, macro_with_extractors);
    let resp = app
        .call(get_request("/macro-extractors/42?page=1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}
