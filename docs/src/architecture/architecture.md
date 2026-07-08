# Architecture

## Crate Layout

Volter is organized as a monorepo with multiple crates:

```
volter/                          # Umbrella crate — re-exports everything
├── crates/
│   ├── volter-core/             # Core traits: Handler, FromRequest, IntoResponse
│   ├── volter-extract/          # Extractors: Json, Query, Path, Extension
│   ├── volter-router/           # Router, MethodRouter, RouteAttr
│   ├── volter-middleware/       # Built-in middleware layers
│   ├── volter-ws/               # WebSocket support
│   ├── volter-macros/           # Derive and attribute macros
│   ├── volter-testing/          # TestClient for integration tests
│   └── volter-cli/              # CLI tool for scaffolding
├── examples/
└── docs/
```

## Core Flow

```
HTTP Request
    │
    ▼
hyper::Server ──► Router ──► MethodRouter
                                  │
                           ┌──────┴──────┐
                           ▼              ▼
                      Handler A      Handler B
                           │              │
                      Extractor       Extractor
                      Chain           Chain
                           │              │
                           ▼              ▼
                     IntoResponse    IntoResponse
                           │              │
                           └──────┬──────┘
                                  ▼
                            HTTP Response
```

1. **hyper** accepts the TCP connection and parses the HTTP request
2. **Router** matches the path against registered routes
3. **MethodRouter** checks the HTTP method (GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS)
4. **Handler** is called, which runs the extractor chain
5. **Extractors** pull data from the request (path, query, body, state, etc.)
6. **Handler function** runs with extracted parameters
7. Return value is converted via **IntoResponse** into an HTTP response

## Key Traits

### `FromRequestParts<S>`

Defines extraction from the request's head (URI, method, headers, extensions)
without consuming the body. Used by `Path`, `Query`, `State`, `Extension`.

```rust
pub trait FromRequestParts<S>: Sized {
    type Rejection: IntoResponse;
    type Future: Future<Output = Result<Self, Self::Rejection>> + Send;
    fn from_request_parts(parts: &mut Parts, state: &S) -> Self::Future;
}
```

### `FromRequest<S, B>`

Defines extraction that consumes the request body. Used by `Json`.

```rust
pub trait FromRequest<S, B = BoxBody>: Sized {
    type Rejection: IntoResponse;
    type Future: Future<Output = Result<Self, Self::Rejection>> + Send;
    fn from_request(req: Request<B>, state: &S) -> Self::Future;
}
```

### `Handler<T, S>`

Converts a handler function into a tower `Service`. The blanket impl for
functions with extractor parameters handles chaining extractors.

### `IntoResponse`

Converts any response type into `Response<BoxBody>`. Implemented for common
types: `&'static str`, `String`, `Json<T>`, `StatusCode`, `Result<T, E>`.

## Middleware

Middleware wraps the router in a tower service stack. Each `Router::layer()`
call adds an outer wrapper. The onion model means:

- Routes before `.layer()` are wrapped
- Routes after `.layer()` are not wrapped
- Later layers wrap earlier layers

## Stateless vs Stateful

- `Router::new()` — state type defaults to `()`
- `Router::with_state(state)` — state type is inferred from the value
- Handlers can extract `State<T>` where `T` must match the router's state type

The state is cloned at service setup time and stored in the router's tower
service, available to every handler and middleware.
