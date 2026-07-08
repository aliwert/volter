# Testing

Volter provides a `TestClient` in the `volter-testing` crate. It lets you write
integration tests against your router without starting an HTTP server.

## Adding as a Dependency

`volter-testing` is a workspace crate. In a published project, add it as a
dev-dependency:

```toml
[dev-dependencies]
volter-testing = { git = "https://github.com/anomalyco/volter" }
```

(It is not yet published separately or re-exported from the `volter` crate.)

## Basic Usage

```rust
use volter::*;
use volter_testing::TestClient;

#[tokio::test]
async fn test_hello_endpoint() {
    let app = Router::new().route("/", get(hello));
    let client = TestClient::new(app);

    let response = client.get("/").send().await;
    assert_eq!(response.status(), StatusCode::OK);
}
```

## Testing Status Codes

```rust
#[tokio::test]
async fn test_not_found() {
    let app = Router::new().route("/", get(hello));
    let client = TestClient::new(app);

    let response = client.get("/nonexistent").send().await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
```

## Testing Response Bodies

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct UserResponse {
    id: u64,
    name: String,
}

#[tokio::test]
async fn test_json_response() {
    let app = Router::new().route("/user", get(user_handler));
    let client = TestClient::new(app);

    let response = client.get("/user").send().await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["name"], "Alice");
}
```

## Sending Request Bodies

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct CreateResponse {
    id: u64,
}

#[tokio::test]
async fn test_create_user() {
    let app = Router::new().route("/users", post(create_user));
    let client = TestClient::new(app);

    let response = client
        .post("/users")
        .json(&serde_json::json!({"name": "Alice"}))
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::CREATED);

    let body: CreateResponse = response.json().await.unwrap();
    assert!(body.id > 0);
}
```

## Testing Headers

Set custom headers on the request and check headers on the response:

```rust
#[tokio::test]
async fn test_custom_header() {
    let app = Router::new().route("/", get(echo_header));
    let client = TestClient::new(app);

    let response = client
        .get("/")
        .header(http::header::AUTHORIZATION, "Bearer token123".parse().unwrap())
        .send()
        .await;

    assert_eq!(
        response.headers().get(http::header::CONTENT_TYPE),
        Some(&http::HeaderValue::from_static("application/json"))
    );
}
```

## Testing with State

```rust
#[derive(Clone)]
struct AppState {
    counter: u64,
}

#[tokio::test]
async fn test_with_state() {
    let app = Router::with_state(AppState { counter: 42 })
        .route("/", get(handler));
    let client = TestClient::new(app);
    // ...
}
```

## Testing Middleware

Test that middleware behaves correctly:

```rust
#[tokio::test]
async fn test_timeout() {
    use std::time::Duration;

    async fn slow_handler() -> &'static str {
        tokio::time::sleep(Duration::from_secs(10)).await;
        "done"
    }

    let app = Router::new()
        .route("/slow", get(slow_handler))
        .layer(TimeoutLayer::new(Duration::from_millis(10)));

    let mut client = TestClient::new(app);
    let response = client.get("/slow").send().await;
    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
}
```

## The Testing API

### `TestClient`

```rust
impl<S: Clone + Send + 'static> TestClient<S> {
    pub fn new(router: Router<S>) -> Self;
    pub fn get(&self, path: &str) -> TestRequestBuilder<S>;
    pub fn post(&self, path: &str) -> TestRequestBuilder<S>;
    pub fn request(&self, method: Method, path: &str) -> TestRequestBuilder<S>;
}
```

### `TestRequestBuilder`

```rust
impl TestRequestBuilder {
    pub fn header(self, name: HeaderName, value: HeaderValue) -> Self;
    pub fn json<T: Serialize>(self, value: &T) -> Self;
    pub fn body(self, body: impl Into<Bytes>) -> Self;
    pub async fn send(self) -> TestResponse;
}
```

### `TestResponse`

```rust
impl TestResponse {
    pub fn status(&self) -> StatusCode;
    pub fn headers(&self) -> &HeaderMap;
    pub async fn bytes(self) -> Result<Bytes, BodyError>;
    pub async fn text(self) -> Result<String, BodyError>;
    pub async fn json<T: DeserializeOwned>(self) -> Result<T, BodyError>;
}
```
