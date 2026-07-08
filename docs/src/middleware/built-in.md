# Built-in Middleware Layers

Volter provides nine built-in middleware layers, all in the `volter` crate.

## TraceLayer

Logs every request with method, path, status code, and latency:

```rust
use volter::*;

Router::new()
    .route("/", get(handler))
    .layer(TraceLayer::new());
```

Spans are emitted via `tracing`. Set up a subscriber in `main()`:

```bash
cargo add tracing-subscriber --features env-filter
```

```rust
tracing_subscriber::fmt()
    .with_env_filter("info")
    .init();
```

## TimeoutLayer

Limits how long a request can take:

```rust
use std::time::Duration;
use volter::*;

Router::new()
    .route("/slow", get(slow_handler))
    .layer(TimeoutLayer::new(Duration::from_secs(10)));
```

Returns `408 Request Timeout` if the handler exceeds the duration.

## CatchPanicLayer

Catches panics from handlers and returns `500 Internal Server Error`:

```rust
use volter::*;

Router::new()
    .route("/", get(handler))
    .layer(CatchPanicLayer::new());
```

Without this layer, a handler panic would crash the connection.

## RequestIdLayer

Assigns every request a unique `RequestId` and sets the `X-Request-Id`
response header:

```rust
use volter::*;

Router::new()
    .route("/", get(handler))
    .layer(RequestIdLayer::new());

async fn handler(Extension(id): Extension<RequestId>) -> String {
    format!("Request {id} received")
}
```

## CorsLayer

Cross-Origin Resource Sharing — configure which origins, methods, and headers
are allowed:

```rust
use volter::*;

Router::new()
    .route("/api", get(handler))
    .layer(CorsLayer::permissive()); // Allow everything
```

For fine-grained control:

```rust
CorsLayer::new()
    .allow_origin("https://myapp.com")
    .allow_methods([http::Method::GET, http::Method::POST, http::Method::PUT, http::Method::DELETE])
    .allow_headers([http::header::CONTENT_TYPE])
    .allow_credentials();
```

## CompressionLayer

Compresses response bodies based on the `Accept-Encoding` request header:

```rust
use volter::*;

Router::new()
    .route("/", get(large_response))
    .layer(CompressionLayer::new()); // gzip, br, zstd, deflate
```

Choose specific algorithms:

```rust
CompressionLayer::gzip()    // gzip only
CompressionLayer::br()      // brotli only
CompressionLayer::zstd()    // zstd only
CompressionLayer::deflate() // deflate only
```

## RequestBodyLimitLayer

Rejects requests whose `Content-Length` exceeds a threshold:

```rust
use volter::*;

Router::new()
    .route("/upload", post(upload_handler))
    .layer(RequestBodyLimitLayer::new(1024 * 1024)); // 1 MB limit
```

Returns `413 Payload Too Large` when exceeded.

## ConcurrencyLimitLayer

Limits the number of concurrently executing requests. Excess requests are
queued (not rejected):

```rust
use volter::*;

Router::new()
    .route("/", get(handler))
    .layer(ConcurrencyLimitLayer::new(10)); // max 10 concurrent
```

## RateLimitLayer

Fixed-window rate limiter:

```rust
use std::time::Duration;
use volter::*;

Router::new()
    .route("/", get(handler))
    .layer(RateLimitLayer::new(100, Duration::from_secs(60)));
    // 100 requests per 60-second window
```

Returns `429 Too Many Requests` when the limit is exceeded.
