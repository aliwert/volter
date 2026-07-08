# Extension

The `Extension<T>` extractor reads values from the request's extension map.
Extensions are inserted by middleware or by the `serve_with()` modifier closure.

## When to Use Extensions

- Per-request values that aren't known at router setup time
- Authenticated user info set by auth middleware
- Request IDs set by `RequestIdLayer`
- Tracing correlation IDs
- Any value that is computed per-request and consumed downstream

Do **not** use extensions for global application state. Use [`State<T>`](state.md)
instead.

## Extracting an Extension

```rust
#[derive(Debug, Clone)]
struct CurrentUser {
    id: u64,
    name: String,
}

async fn profile(Extension(user): Extension<CurrentUser>) -> String {
    format!("Welcome, {}!", user.name)
}
```

## Setting Extensions via Middleware

Middleware can insert extensions before the handler runs:

```rust
use std::task::{Context, Poll};
use tower::{Layer, Service};
use volter::{Request, Response};

#[derive(Clone)]
struct AuthLayer;

impl<S> Layer<S> for AuthLayer {
    type Service = AuthService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        AuthService { inner }
    }
}

#[derive(Clone)]
struct AuthService<S> {
    inner: S,
}

impl<S> Service<Request> for AuthService<S>
where
    S: Service<Request, Response = Response> + Clone + Send + 'static,
    S::Future: Send,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let mut req = req;
        req.extensions_mut().insert(CurrentUser { id: 1, name: "Alice".into() });
        self.inner.call(req)
    }
}
```

## Setting Extensions via `serve_with`

The `serve_with()` function accepts a closure that modifies each request before
dispatch:

```rust
use volter::*;
use std::sync::atomic::{AtomicU64, Ordering};

let counter = AtomicU64::new(0);

let app = Router::new()
    .route("/", get(handler));

let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;

serve_with(listener, app, move |req: &mut Request| {
    let n = counter.fetch_add(1, Ordering::SeqCst);
    req.extensions_mut().insert(RequestId(n));
})
.await
```

## Rejection

```rust
pub enum ExtensionRejection {
    MissingExtension(&'static str),   // → 500 Internal Server Error
}
```

If a handler requests `Extension<T>` but no middleware inserted a value of
type `T`, the rejection returns `500 Internal Server Error` with a message
indicating the missing type.

## Built-in Extension: `RequestId`

The `RequestIdLayer` inserts a unique `RequestId` into every request:

```rust
use volter::*;

let app = Router::new()
    .route("/", get(handler))
    .layer(RequestIdLayer::new());

async fn handler(Extension(id): Extension<RequestId>) -> String {
    format!("Request ID: {id}")
}
```
