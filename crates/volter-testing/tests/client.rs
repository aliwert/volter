//! Integration tests for `TestClient`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use http::HeaderName;
use http::HeaderValue;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use volter_extract::Json;
use volter_router::{get, post, Router};
use volter_testing::TestClient;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn ok_handler() -> &'static str {
    "ok"
}

// ---------------------------------------------------------------------------
// GET requests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_request() {
    let client = TestClient::new(Router::new().route("/", get(ok_handler)));

    let response = client.get("/").send().await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn get_request_with_text() {
    let client = TestClient::new(Router::new().route("/", get(ok_handler)));

    let response = client.get("/").send().await;
    let text = response.text().await.unwrap();
    assert_eq!(text, "ok");
}

// ---------------------------------------------------------------------------
// POST requests
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Payload {
    name: String,
    count: u32,
}

async fn echo_json(Json(p): Json<Payload>) -> Json<Payload> {
    Json(p)
}

#[tokio::test]
async fn post_json_request() {
    let client = TestClient::new(Router::new().route("/echo", post(echo_json)));

    let payload = Payload {
        name: "ferris".into(),
        count: 42,
    };

    let response = client.post("/echo").json(&payload).send().await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn post_json_response_body() {
    let client = TestClient::new(Router::new().route("/echo", post(echo_json)));

    let payload = Payload {
        name: "ferris".into(),
        count: 42,
    };

    let response = client.post("/echo").json(&payload).send().await;
    let body: Payload = response.json().await.unwrap();
    assert_eq!(body, payload);
}

// ---------------------------------------------------------------------------
// Request headers
// ---------------------------------------------------------------------------

#[tokio::test]
async fn headers_round_trip() {
    let client = TestClient::new(Router::new().route("/", get(ok_handler)));

    let response = client
        .get("/")
        .header(
            HeaderName::from_static("x-custom"),
            HeaderValue::from_static("my-value"),
        )
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::OK);
}

// ---------------------------------------------------------------------------
// 404 responses
// ---------------------------------------------------------------------------

#[tokio::test]
async fn not_found_returns_404() {
    let client = TestClient::new(Router::new().route("/exists", get(ok_handler)));

    let response = client.get("/missing").send().await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Custom HTTP methods
// ---------------------------------------------------------------------------

#[tokio::test]
async fn custom_method() {
    use http::Method;

    let client = TestClient::new(Router::new().route("/", get(ok_handler)));

    // Send PUT to a GET-only route — should get 405 Method Not Allowed.
    let response = client.request(Method::PUT, "/").send().await;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

// ---------------------------------------------------------------------------
// Middleware integration
// ---------------------------------------------------------------------------

#[tokio::test]
async fn middleware_integration() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    async fn tracked_handler(c: Arc<AtomicUsize>) -> &'static str {
        c.fetch_add(1, Ordering::SeqCst);
        "tracked"
    }

    let app = Router::new()
        .route("/", get(move || tracked_handler(counter_clone.clone())))
        .layer(volter_middleware::TraceLayer::new());

    let client = TestClient::new(app);

    let r1 = client.get("/").send().await;
    assert_eq!(r1.status(), StatusCode::OK);
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    let r2 = client.get("/").send().await;
    assert_eq!(r2.status(), StatusCode::OK);
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn request_id_middleware() {
    use volter_middleware::RequestIdLayer;

    let app = Router::new()
        .route("/", get(ok_handler))
        .layer(RequestIdLayer::new());

    let client = TestClient::new(app);

    let response = client.get("/").send().await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("x-request-id").is_some());
}

// ---------------------------------------------------------------------------
// State support
// ---------------------------------------------------------------------------

#[tokio::test]
async fn client_works_with_stateful_router() {
    use volter_core::State;

    async fn state_handler(State(count): State<Arc<AtomicUsize>>) -> String {
        format!("count={}", count.load(Ordering::SeqCst))
    }

    let state = Arc::new(AtomicUsize::new(7));
    let app = Router::with_state(state).route("/", get(state_handler));

    let client = TestClient::new(app);

    let response = client.get("/").send().await;
    let text = response.text().await.unwrap();
    assert_eq!(text, "count=7");
}
