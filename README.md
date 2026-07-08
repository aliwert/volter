# Volter

A production-grade, async-first web framework for Rust, built on
[hyper], [tokio], and [tower].

Volter follows the same architectural principles as tower — every component is
a [`Service`] or a [`Layer`], making the framework naturally composable with
the broader tower ecosystem.

## Features

- **Extractor-based handlers** — `Json<T>`, `Query<T>`, `Path<T>`,
  `Extension<T>`, `State<T>`, and custom extractors via `FromRequest` /
  `FromRequestParts`.
- **Tower-native middleware** — any `tower::Layer` works directly with
  `Router::layer()`. Built-in layers include tracing, timeout, CORS,
  compression, rate-limiting, request ID, panic catching, and body limits.
- **Compile-time safety** — state types and extractor parameters are checked
  at compile time. Wrong state type? It won't compile.
- **No panics in request paths** — deny-level lints (`unwrap_used`,
  `expect_used`, `panic`, `indexing_slicing`) are enforced across every
  library crate.
- **Optional macros** — the core API is pure Rust. Derive macros for custom
  extractors and attribute route macros (`#[get]`, `#[post]`) are optional.
- **WebSocket support** — behind the `ws` feature flag.
- **In-process testing** — `TestClient` dispatches requests directly through
  the router without binding a real socket.

## Quick start

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

## Documentation

- [API reference (docs.rs)](https://docs.rs/volter/latest/volter/)
- [mdBook guide](https://github.com/aliwert/volter/tree/main/docs/src)
- [Examples](https://github.com/aliwert/volter/tree/main/examples)

## Crate layout

| Crate                                                                              | Description                                                    |
| ---------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| [`volter`](https://docs.rs/volter/latest/volter/)                                  | Umbrella crate — re-exports everything                         |
| [`volter-core`](https://docs.rs/volter-core/latest/volter_core/)                   | Core traits: `Handler`, `FromRequest`, `IntoResponse`, `State` |
| [`volter-router`](https://docs.rs/volter-router/latest/volter_router/)             | `Router`, `MethodRouter`, route construction                   |
| [`volter-extract`](https://docs.rs/volter-extract/latest/volter_extract/)          | Extractors: `Json`, `Query`, `Path`, `Extension`               |
| [`volter-middleware`](https://docs.rs/volter-middleware/latest/volter_middleware/) | Built-in middleware: `TraceLayer`, `CorsLayer`, etc.           |
| [`volter-ws`](https://docs.rs/volter-ws/latest/volter_ws/)                         | WebSocket support                                              |
| [`volter-macros`](https://docs.rs/volter-macros/latest/volter_macros/)             | Derive and route attribute macros                              |
| [`volter-testing`](https://docs.rs/volter-testing/latest/volter_testing/)          | `TestClient` for integration tests                             |
| [`volter-cli`](https://docs.rs/volter-cli/latest/volter_cli/)                      | CLI tool for project scaffolding                               |

## Performance

See the [performance page](docs/src/performance.md) for current Criterion
benchmark results.

[hyper]: https://hyper.rs
[tokio]: https://tokio.rs
[tower]: https://docs.rs/tower
[`Service`]: https://docs.rs/tower/latest/tower/trait.Service.html
[`Layer`]: https://docs.rs/tower/latest/tower/trait.Layer.html
