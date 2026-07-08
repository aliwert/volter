# Middleware

Middleware in Volter is a [`tower::Layer`] wrapping a [`tower::Service`]. Every
middleware is a drop-in generic — you can use Volter's built-in layers, tower's
own layers, or third-party `tower::Layer` implementations without adapters.

## How Middleware Works

Middleware wraps the router (or a subset of routes) in an onion model:

```rust
Router::new()
    .route("/public", get(public_handler))   // NOT wrapped
    .layer(OuterLayer::new())                 // Wraps all pre-layer routes
    .route("/admin", get(admin_handler))      // NOT wrapped
    .layer(InnerLayer::new())                 // Inner wraps Outer+pre-layer routes
    .route("/api", get(api_handler));         // NOT wrapped
```

- Routes registered **before** a `.layer()` call are wrapped by that layer
- Routes registered **after** a `.layer()` call are not wrapped
- Multiple `.layer()` calls compose: the last call is the outermost layer
- Post-layer routes are tried first, then the layered (wrapped) service

## Using Built-in Middleware

```rust
use volter::*;
use std::time::Duration;

let app = Router::new()
    .route("/", get(handler))
    .layer(TraceLayer::new())            // Request logging
    .layer(CatchPanicLayer::new())       // Panic recovery → 500
    .layer(TimeoutLayer::new(Duration::from_secs(30)))  // Timeout → 408
    .layer(RequestIdLayer::new());       // Unique X-Request-Id per request
```

## Middleware Order Matters

Layer order follows tower's onion model — the first `.layer()` call is
innermost, the last is outermost. Requests travel from outer to inner;
responses travel from inner to outer:

```
Request → TraceLayer → CatchPanicLayer → TimeoutLayer → Handler → Response
                                                                     ↓
Response ← TraceLayer ← CatchPanicLayer ← TimeoutLayer ← ← ← ← ← ← ←
```

Place `CatchPanicLayer` **inside** timeout and tracing so panics are caught
before the error response propagates out.

## Composing with Other Middleware

Because everything uses tower, any `tower::Layer` works directly:

```rust
use tower::limit::ConcurrencyLimitLayer;

let app = Router::new()
    .route("/", get(handler))
    .layer(ConcurrencyLimitLayer::new(100));
```

## See Also

- [Built-in Layers](built-in.md) — details of each Volter middleware
- [Custom Middleware](custom.md) — writing your own middleware

[`tower::Layer`]: https://docs.rs/tower/latest/tower/trait.Layer.html
[`tower::Service`]: https://docs.rs/tower/latest/tower/trait.Service.html
