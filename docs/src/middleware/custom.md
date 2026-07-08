# Custom Middleware

Writing custom middleware means implementing `tower::Layer` and
`tower::Service`. This gives you full, type-safe access to every request and
response.

## Basic Pattern

A middleware consists of two types:

```rust
use std::task::{Context, Poll};
use tower::{Layer, Service};
use volter::{Request, Response};

// 1. The layer — created once, clones for every router clone
#[derive(Clone)]
struct MyLayer;

impl<S> Layer<S> for MyLayer {
    type Service = MyMiddleware<S>;
    fn layer(&self, inner: S) -> Self::Service {
        MyMiddleware { inner }
    }
}

// 2. The service — one per cloned router, calls inner after its work
#[derive(Clone)]
struct MyMiddleware<S> {
    inner: S,
}

impl<S> Service<Request> for MyMiddleware<S>
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
        // Pre-processing: inspect/modify the request
        eprintln!("Incoming request: {} {}", req.method(), req.uri());

        // Delegate to the inner service
        self.inner.call(req)
        // Post-processing would go in a `.map` or `async` block
    }
}
```

## Modifying the Response

To inspect or modify the response, wrap the inner future:

```rust
use std::future::Future;
use std::pin::Pin;

fn call(&mut self, req: Request) -> Self::Future {
    let fut = self.inner.call(req);
    Box::pin(async move {
        let response: Response = fut.await?;
        let status = response.status();
        eprintln!("Response: {status}");
        Ok(response)
    })
}
```

## Injecting Extensions

Add values to the request extension map that downstream handlers can extract
via `Extension<T>`:

```rust
#[derive(Clone)]
struct TimingLayer;

impl<S> Layer<S> for TimingLayer {
    type Service = TimingService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        TimingService { inner }
    }
}

#[derive(Clone)]
struct TimingService<S> {
    inner: S,
}

impl<S> Service<Request> for TimingService<S>
where
    S: Service<Request, Response = Response> + Clone + Send + 'static,
    S::Future: Send,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let start = std::time::Instant::now();
        let fut = self.inner.call(req);
        Box::pin(async move {
            let response = fut.await?;
            let elapsed = start.elapsed();
            eprintln!("Handled in {:?}", elapsed);
            Ok(response)
        })
    }
}
```

## Wrapping Only Specific Routes

Use `.layer()` to wrap routes registered before it:

```rust
let app = Router::new()
    .route("/public", get(public_handler))   // No auth
    .layer(AuthLayer)                          // Auth wraps only above routes
    .route("/admin", get(admin_handler));      // No auth (post-layer)
```

This pattern lets some routes bypass middleware while others are wrapped.

## Important Notes

- The `Service` impl must be `Clone` — the router clones it at setup time
- `poll_ready` should always delegate to `inner.poll_ready`
- Errors must implement `Into<BoxError>` for compatibility
- Prefer async blocks over manual future state machines for response
  modification
