# Introduction

Volter is a production-grade, async-first web framework for Rust. It follows the
same architectural principles as [`tower`] — every component is a [`Service`] or
a [`Layer`], making the framework naturally composable.

## Why Volter?

Rust has several excellent web frameworks. Volter was built for teams that want:

- **No panics in request paths** — deny-level lints (`unwrap_used`, `expect_used`,
  `panic`, `indexing_slicing`) are enforced in every library crate.
- **Tower-native composition** — use any `tower::Layer` or `tower::Service`
  without adapters. Middleware, routing, and handlers all speak the same
  `Service` protocol.
- **Compile-time safety** — wrong state type? It won't compile. Missing
  extractor? It won't compile.
- **No macros required** — the core API is pure Rust. Derive macros and
  attribute macros are optional sugar.

## Design Principles

1. **Everything is a Service.** `Router<S>` implements `tower::Service<Request>`.
   Middleware is `tower::Layer`. Handlers become `Service`s via `HandlerService`.

2. **Rejection is a first-class concept.** Every extractor defines its own
   rejection type that implements `IntoResponse`. A missing query parameter and
   an invalid JSON body produce different, predictable HTTP responses.

3. **State is typed.** Application state is checked at compile time. If your
   handler extracts `State<AppConfig>`, you must provide a `Router<AppConfig>`.

4. **Panics are caught.** Wrap your router with `CatchPanicLayer` to turn
   handler panics into `500 Internal Server Error` responses, keeping the
   server alive.

## Crate Layout

Volter is organised as a set of focused crates, all re-exported through the
top-level `volter` crate:

| Crate               | Purpose                                                        |
| ------------------- | -------------------------------------------------------------- |
| `volter`            | Meta-crate — re-exports everything                             |
| `volter-core`       | Core traits: `Handler`, `FromRequest`, `IntoResponse`, `State` |
| `volter-router`     | `Router`, `MethodRouter`, route construction                   |
| `volter-extract`    | Extractors: `Json`, `Query`, `Path`, `Extension`               |
| `volter-middleware` | Built-in middleware: `TraceLayer`, `CorsLayer`, etc.           |
| `volter-ws`         | WebSocket support                                              |
| `volter-macros`     | Optional derive and attribute macros                           |
| `volter-testing`    | `TestClient` for integration tests                             |
| `volter-cli`        | CLI tool for scaffolding (`volter new`)                        |

## Quick Start

```rust
use tokio::net::TcpListener;
use volter::{get, serve, Router};

async fn hello() -> &'static str {
    "Hello, World!"
}

#[tokio::main]
async fn main() -> Result<(), volter::BoxError> {
    let app = Router::new().route("/", get(hello));
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    serve(listener, app).await
}
```

[`tower`]: https://docs.rs/tower
[`Service`]: https://docs.rs/tower/latest/tower/trait.Service.html
[`Layer`]: https://docs.rs/tower/latest/tower/trait.Layer.html
