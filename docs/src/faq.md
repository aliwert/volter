# FAQ

## General

### What is Volter?

Volter is a Rust web framework built on hyper, tokio, and tower. It uses an
extractor-based architecture similar to Axum, with a focus on compile-time
safety, composability, and ergonomics.

### Why build another web framework?

Volter was designed as a learning and experimentation project to explore web
framework architecture in Rust. It prioritizes clear code, minimal
dependencies, and a developer experience that feels natural to Rust
programmers.

### Is Volter production-ready?

Volter is a hobby and educational project. While it works well for real
applications, it does not have the ecosystem maturity of Axum or Actix-Web.

## Routing

### Can I nest routers?

Yes, use `Router::nest()`:

```rust
let api = Router::new()
    .route("/users", get(list_users));

let app = Router::new()
    .nest("/api", api);
```

## Extractors

### Can I use multiple extractors in one handler?

Yes, use a tuple:

```rust
async fn handler(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
    Json(body): Json<CreateUser>,
) -> impl IntoResponse { ... }
```

### Why doesn't `State<T>` return a `Result`?

`State<T>` extraction is guaranteed to succeed at compile time — the state
type is checked when the router is constructed. The rejection type is
`Infallible`.

### Can I create custom extractors?

Yes. Implement `FromRequestParts` or `FromRequest` for your type, or use the
derive macros.

## Macros

### Do I need macros to use Volter?

No. All macros are optional. The core API (`Router::new().route("/",
get(handler))`) works without any macros.

### Why do `#[get]` and `#[post]` generate a const instead of registering directly?

The const stores only the path and HTTP method, not the handler type. This
avoids type inference issues with `impl Future` types in const generics and
keeps macro expansion simple and robust.

## Performance

### How does Volter compare to Axum?

Volter is built on the same hyper/tokio/tower stack as Axum, so raw request
dispatch throughput is comparable. See [Performance](./performance.md) for the
current Criterion benchmark results.

### Does Volter support streaming?

Body streaming works via hyper's body API. Streaming JSON parsing is not
built in — the body is fully buffered before deserialization.

## Compatibility

### What Rust version do I need?

Volter requires Rust 1.79 or later.

### Does Volter work with WASM?

No. Volter depends on tokio and hyper, which require OS-level I/O.

### Can I use Volter with other tower middleware?

Yes. Any `tower::Layer` works directly with `Router::layer()`.
