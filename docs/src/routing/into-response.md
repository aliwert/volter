# IntoResponse

Every handler returns a value that implements `IntoResponse`. The trait converts
your return value into an HTTP response.

## Built-in Implementations

### `&'static str` and `String`

Return a string — it becomes the response body with status `200 OK`:

```rust
async fn hello() -> &'static str {
    "Hello, World!"
}

async fn greet(name: String) -> String {
    format!("Hello, {name}!")
}
```

### `StatusCode`

Return just a status code with an empty body:

```rust
async fn delete() -> StatusCode {
    StatusCode::NO_CONTENT
}
```

### `(StatusCode, T)`

Pair a custom status code with any `IntoResponse` body:

```rust
async fn create() -> (StatusCode, &'static str) {
    (StatusCode::CREATED, "resource created")
}
```

### `Result<T, E>`

Return success or error — both `T` and `E` must implement `IntoResponse`:

```rust
async fn get_user(id: Path<u64>) -> Result<String, StatusCode> {
    match find_user(*id) {
        Some(name) => Ok(name),
        None => Err(StatusCode::NOT_FOUND),
    }
}
```

The `Ok` variant produces a response from `T`. The `Err` variant produces a
response from `E`. This pattern is the idiomatic way to handle errors in Volter.

### `()`

Return nothing — produces `204 No Content`:

```rust
async fn log_visit() { /* side effect only */ }
```

### `Response`

Return a fully-formed `http::Response<BoxBody>` directly. This gives you
complete control over headers, status, and body:

```rust
use volter::http::header;

async fn custom() -> Response {
    Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(full_body("custom response"))
        .unwrap()
}
```

## `Json<T>` as a Response

Any `Serialize` type wrapped in `Json` becomes a JSON response:

```rust
use serde::Serialize;

#[derive(Serialize)]
struct User {
    name: String,
}

async fn get_user() -> Json<User> {
    Json(User { name: "Alice".into() })
}
```

This produces `200 OK` with `Content-Type: application/json`.

## Custom Implementations

You can implement `IntoResponse` for your own types:

```rust
use volter::{IntoResponse, Response, BoxBody, full_body};

struct Html(pub String);

impl IntoResponse for Html {
    fn into_response(self) -> Response {
        Response::builder()
            .header("content-type", "text/html")
            .body(full_body(self.0))
            .unwrap()
    }
}
```

## Summary Table

| Type              | Status           | Body             |
| ----------------- | ---------------- | ---------------- |
| `&'static str`    | `200 OK`         | The string bytes |
| `String`          | `200 OK`         | The string bytes |
| `StatusCode`      | The code itself  | Empty            |
| `(StatusCode, T)` | Custom status    | Inner value      |
| `Result<T, E>`    | Ok or Err        | Delegated        |
| `()`              | `204 No Content` | Empty            |
| `Response`        | Passthrough      | Passthrough      |
| `Json<T>`         | `200 OK`         | JSON bytes       |
