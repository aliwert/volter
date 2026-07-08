# Examples

The Volter repository includes several runnable workspace member packages in
the `examples/` directory. Run any of them from the workspace root with
`cargo run -p <name>`.

## Basic Examples

### Hello World

```bash
cargo run -p hello-world
```

A minimal server with a single route returning "Hello, World!".

### Path Parameters

```bash
cargo run -p path-params
```

Path parameters with `Path<T>` for single and multi-parameter routes.

### Query Parameters

```bash
cargo run -p query-params
```

URL query parameter parsing with `Query<T>`.

### JSON

```bash
cargo run -p json-example
```

JSON request body deserialization and JSON response serialization.

### Merge

```bash
cargo run -p merge
```

Combining independent routers with `Router::merge()`.

### Nesting

```bash
cargo run -p nesting
```

Mounting routers under a path prefix with `Router::nest()`.

### Extensions

```bash
cargo run -p extensions-example
```

Request extensions set by middleware and consumed by handlers.

### Multiple Extractors

```bash
cargo run -p multi-extractors-example
```

Handlers using two extractors (e.g. `State` + `Query`).

## Derive Macros

### Derive Extractors

```bash
cargo run -p derive-extractors
```

Using `#[derive(FromRequestParts)]` and `#[derive(FromRequest)]` on your types.

## Route Attribute Macros

### Route Macros

```bash
cargo run -p route-macros
```

Using `#[get("/")]` and `#[post("/")]` with `Router::route_attr()`.

## WebSocket

```bash
cargo run -p websocket
```

A basic WebSocket echo server.

## Middleware

### Catch Panic

```bash
cargo run -p catch-panic-example
```

Panic recovery with `CatchPanicLayer`.

### Timeout

```bash
cargo run -p timeout-example
```

Request timeouts with `TimeoutLayer`.

### Tracing

```bash
cargo run -p tracing-example
```

Request logging with `TraceLayer`.

### CORS

```bash
cargo run -p cors-example
```

Cross-Origin Resource Sharing with `CorsLayer`.

### Compression

```bash
cargo run -p compression-example
```

Response compression with `CompressionLayer`.

### Body Limit

```bash
cargo run -p body-limit-example
```

Request body size limiting with `RequestBodyLimitLayer`.

### Concurrency Limit

```bash
cargo run -p concurrency-limit-example
```

Limiting concurrent requests with `ConcurrencyLimitLayer`.

### Rate Limit

```bash
cargo run -p rate-limit-example
```

Fixed-window rate limiting with `RateLimitLayer`.

### Request ID

```bash
cargo run -p request-id-example
```

Unique per-request IDs with `RequestIdLayer`.

## Custom Middleware

```bash
cargo run -p middleware-example
```

Implementing `tower::Layer` and `tower::Service` for custom middleware.

## Running Examples

All examples start a server on `http://127.0.0.1:3000` by default. Check the
example source for the exact port and available routes.
