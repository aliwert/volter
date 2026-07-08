//! Integration tests for `RouteAttr` and route attribute macros `#[get]` /
//! `#[post]` / `#[put]` / `#[patch]` / `#[delete]` / `#[head]` / `#[options]`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use tower::Service;
use volter::*;
use volter_core::full_body;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn request(method: http::Method, path: &str) -> http::Request<BoxBody> {
    http::Request::builder()
        .uri(path)
        .method(method)
        .body(volter_core::empty_body())
        .unwrap()
}

fn json_request(method: http::Method, path: &str, body: &[u8]) -> http::Request<BoxBody> {
    http::Request::builder()
        .uri(path)
        .method(method)
        .header("content-type", "application/json")
        .body(full_body(bytes::Bytes::copy_from_slice(body)))
        .unwrap()
}

// ---------------------------------------------------------------------------
// RouteAttr construction — GET / POST (regression)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn route_attr_get_basic() {
    async fn hello() -> &'static str {
        "Hello!"
    }

    const ROUTE: RouteAttr = RouteAttr::get("/hello");
    let mut app: Router = Router::new().route_attr(ROUTE, hello);

    let resp = app
        .call(request(http::Method::GET, "/hello"))
        .await
        .unwrap();
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

    let resp = app
        .call(json_request(
            http::Method::POST,
            "/users",
            br#"{"name":"Alice"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

// ---------------------------------------------------------------------------
// RouteAttr construction — new methods
// ---------------------------------------------------------------------------

#[tokio::test]
async fn route_attr_put_basic() {
    #[derive(serde::Deserialize)]
    struct Payload {
        name: String,
    }

    async fn update(Json(payload): Json<Payload>) -> &'static str {
        assert_eq!(payload.name, "Updated");
        "updated"
    }

    const ROUTE: RouteAttr = RouteAttr::put("/items");
    let mut app: Router = Router::new().route_attr(ROUTE, update);

    let resp = app
        .call(json_request(
            http::Method::PUT,
            "/items",
            br#"{"name":"Updated"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[tokio::test]
async fn route_attr_patch_basic() {
    #[derive(serde::Deserialize)]
    struct Payload {
        name: String,
    }

    async fn modify(Json(payload): Json<Payload>) -> &'static str {
        assert_eq!(payload.name, "Patched");
        "patched"
    }

    const ROUTE: RouteAttr = RouteAttr::patch("/items");
    let mut app: Router = Router::new().route_attr(ROUTE, modify);

    let resp = app
        .call(json_request(
            http::Method::PATCH,
            "/items",
            br#"{"name":"Patched"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[tokio::test]
async fn route_attr_delete_basic() {
    async fn remove() -> &'static str {
        "deleted"
    }

    const ROUTE: RouteAttr = RouteAttr::delete("/items/:id");
    let mut app: Router = Router::new().route_attr(ROUTE, remove);

    let resp = app
        .call(request(http::Method::DELETE, "/items/42"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[tokio::test]
async fn route_attr_head_basic() {
    async fn headers() -> &'static str {
        "ignored for HEAD"
    }

    const ROUTE: RouteAttr = RouteAttr::head("/items");
    let mut app: Router = Router::new().route_attr(ROUTE, headers);

    let resp = app
        .call(request(http::Method::HEAD, "/items"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[tokio::test]
async fn route_attr_options_basic() {
    async fn info() -> &'static str {
        "GET, POST, PUT, DELETE"
    }

    const ROUTE: RouteAttr = RouteAttr::options("/items");
    let mut app: Router = Router::new().route_attr(ROUTE, info);

    let resp = app
        .call(request(http::Method::OPTIONS, "/items"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Wrong method → 405
// ---------------------------------------------------------------------------

#[tokio::test]
async fn route_attr_wrong_method_returns_405() {
    async fn hello() -> &'static str {
        "Hello!"
    }

    const ROUTE: RouteAttr = RouteAttr::get("/hello");
    let mut app: Router = Router::new().route_attr(ROUTE, hello);

    for method in [
        http::Method::PUT,
        http::Method::PATCH,
        http::Method::DELETE,
        http::Method::HEAD,
        http::Method::OPTIONS,
        http::Method::POST,
    ] {
        let resp = app.call(request(method.clone(), "/hello")).await.unwrap();
        assert_eq!(
            resp.status(),
            http::StatusCode::METHOD_NOT_ALLOWED,
            "expected 405 for {method}"
        );
    }
}

#[tokio::test]
async fn route_attr_unknown_path_returns_404() {
    async fn hello() -> &'static str {
        "Hello!"
    }

    const ROUTE: RouteAttr = RouteAttr::get("/hello");
    let mut app: Router = Router::new().route_attr(ROUTE, hello);

    let resp = app
        .call(request(http::Method::GET, "/unknown"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Stateful handlers
// ---------------------------------------------------------------------------

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

    let resp = app
        .call(request(http::Method::GET, "/state"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Path extractor
// ---------------------------------------------------------------------------

#[tokio::test]
async fn route_attr_with_path_params() {
    async fn user(Path(id): Path<u64>) -> String {
        format!("User {}", id)
    }

    const ROUTE: RouteAttr = RouteAttr::get("/users/:id");
    let mut app: Router = Router::new().route_attr(ROUTE, user);

    let resp = app
        .call(request(http::Method::GET, "/users/42"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Json extractor on PUT / PATCH
// ---------------------------------------------------------------------------

#[tokio::test]
async fn route_attr_put_with_json() {
    #[derive(serde::Deserialize)]
    struct UpdateUser {
        name: String,
    }

    async fn update(Json(payload): Json<UpdateUser>) -> String {
        format!("Updated to {}", payload.name)
    }

    const ROUTE: RouteAttr = RouteAttr::put("/users/:id");
    let mut app: Router = Router::new().route_attr(ROUTE, update);

    let resp = app
        .call(json_request(
            http::Method::PUT,
            "/users/1",
            br#"{"name":"Bob"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[tokio::test]
async fn route_attr_patch_with_json() {
    #[derive(serde::Deserialize)]
    struct PatchUser {
        age: u8,
    }

    async fn patch(Json(payload): Json<PatchUser>) -> String {
        format!("Age now {}", payload.age)
    }

    const ROUTE: RouteAttr = RouteAttr::patch("/users/:id");
    let mut app: Router = Router::new().route_attr(ROUTE, patch);

    let resp = app
        .call(json_request(
            http::Method::PATCH,
            "/users/1",
            br#"{"age":31}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Multiple methods on the same path
// ---------------------------------------------------------------------------

#[tokio::test]
async fn route_attr_multiple_methods_same_path() {
    async fn list() -> &'static str {
        "list"
    }
    async fn create() -> &'static str {
        "create"
    }
    async fn remove() -> &'static str {
        "delete"
    }

    const LIST: RouteAttr = RouteAttr::get("/items");
    const CREATE: RouteAttr = RouteAttr::post("/items");
    const DELETE: RouteAttr = RouteAttr::delete("/items/:id");
    let mut app: Router = Router::new()
        .route_attr(LIST, list)
        .route_attr(CREATE, create)
        .route_attr(DELETE, remove);

    let get_resp = app
        .call(request(http::Method::GET, "/items"))
        .await
        .unwrap();
    assert_eq!(get_resp.status(), http::StatusCode::OK);

    let post_resp = app
        .call(request(http::Method::POST, "/items"))
        .await
        .unwrap();
    assert_eq!(post_resp.status(), http::StatusCode::OK);

    let del_resp = app
        .call(request(http::Method::DELETE, "/items/1"))
        .await
        .unwrap();
    assert_eq!(del_resp.status(), http::StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Free function routing — new methods
// ---------------------------------------------------------------------------

#[tokio::test]
async fn free_function_put() {
    async fn update() -> &'static str {
        "updated"
    }
    let mut app: Router = Router::new().route("/items", put(update));
    let resp = app
        .call(request(http::Method::PUT, "/items"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[tokio::test]
async fn free_function_patch() {
    async fn modify() -> &'static str {
        "patched"
    }
    let mut app: Router = Router::new().route("/items", patch(modify));
    let resp = app
        .call(request(http::Method::PATCH, "/items"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[tokio::test]
async fn free_function_delete() {
    async fn remove() -> &'static str {
        "deleted"
    }
    let mut app: Router = Router::new().route("/items/:id", delete(remove));
    let resp = app
        .call(request(http::Method::DELETE, "/items/1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[tokio::test]
async fn free_function_head() {
    async fn headers() -> &'static str {
        "unused"
    }
    let mut app: Router = Router::new().route("/items", head(headers));
    let resp = app
        .call(request(http::Method::HEAD, "/items"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[tokio::test]
async fn free_function_options() {
    async fn info() -> &'static str {
        "all"
    }
    let mut app: Router = Router::new().route("/items", options(info));
    let resp = app
        .call(request(http::Method::OPTIONS, "/items"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Route attribute macro tests — new methods
// ---------------------------------------------------------------------------

#[put("/route-attr-macro-put")]
async fn route_attr_macro_put_handler() -> &'static str {
    "from put macro"
}

#[tokio::test]
async fn route_attr_macro_put() {
    let mut app: Router = Router::new().route_attr(
        ROUTE_ATTR_MACRO_PUT_HANDLER_ROUTE,
        route_attr_macro_put_handler,
    );
    let resp = app
        .call(request(http::Method::PUT, "/route-attr-macro-put"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[patch("/route-attr-macro-patch")]
async fn route_attr_macro_patch_handler() -> &'static str {
    "from patch macro"
}

#[tokio::test]
async fn route_attr_macro_patch() {
    let mut app: Router = Router::new().route_attr(
        ROUTE_ATTR_MACRO_PATCH_HANDLER_ROUTE,
        route_attr_macro_patch_handler,
    );
    let resp = app
        .call(request(http::Method::PATCH, "/route-attr-macro-patch"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[delete("/route-attr-macro-delete")]
async fn route_attr_macro_delete_handler() -> &'static str {
    "from delete macro"
}

#[tokio::test]
async fn route_attr_macro_delete() {
    let mut app: Router = Router::new().route_attr(
        ROUTE_ATTR_MACRO_DELETE_HANDLER_ROUTE,
        route_attr_macro_delete_handler,
    );
    let resp = app
        .call(request(http::Method::DELETE, "/route-attr-macro-delete"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[head("/route-attr-macro-head")]
async fn route_attr_macro_head_handler() -> &'static str {
    "from head macro"
}

#[tokio::test]
async fn route_attr_macro_head() {
    let mut app: Router = Router::new().route_attr(
        ROUTE_ATTR_MACRO_HEAD_HANDLER_ROUTE,
        route_attr_macro_head_handler,
    );
    let resp = app
        .call(request(http::Method::HEAD, "/route-attr-macro-head"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[options("/route-attr-macro-options")]
async fn route_attr_macro_options_handler() -> &'static str {
    "from options macro"
}

#[tokio::test]
async fn route_attr_macro_options() {
    let mut app: Router = Router::new().route_attr(
        ROUTE_ATTR_MACRO_OPTIONS_HANDLER_ROUTE,
        route_attr_macro_options_handler,
    );
    let resp = app
        .call(request(http::Method::OPTIONS, "/route-attr-macro-options"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}

// ---------------------------------------------------------------------------
// Previous tests continue to pass
// ---------------------------------------------------------------------------

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

    let resp = app
        .call(request(http::Method::GET, "/list?page=3"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
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

    let root_resp = app.call(request(http::Method::GET, "/")).await.unwrap();
    assert_eq!(root_resp.status(), http::StatusCode::OK);

    let users_resp = app
        .call(request(http::Method::GET, "/users"))
        .await
        .unwrap();
    assert_eq!(users_resp.status(), http::StatusCode::OK);
}

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
        .call(request(http::Method::GET, "/route-attr-macro-get"))
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
    let resp = app
        .call(request(http::Method::POST, "/route-attr-macro-post"))
        .await
        .unwrap();
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
        .call(request(http::Method::GET, "/macro-extractors/42?page=1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
}
