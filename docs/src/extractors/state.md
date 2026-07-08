# State

Application state is any value shared across handlers. You provide it to the
router, and handlers extract it via `State<T>`.

## Providing State

Pass state to `Router::with_state()`:

```rust
use volter::*;

#[derive(Clone)]
struct AppState {
    db_url: String,
}

let state = AppState { db_url: "postgres://localhost/db".into() };
let app: Router<AppState> = Router::with_state(state);
```

The state type must implement `Clone` because the router clones it when creating
boxed services at setup time.

## Extracting State

Use `State<T>` as a handler parameter:

```rust
async fn dashboard(State(state): State<AppState>) -> String {
    format!("Connected to {}", state.db_url)
}
```

The `T` in `State<T>` is checked at **compile time**. If you declare
`Router::with_state(AppState)` but a handler expects `State<OtherState>`,
the code won't compile.

## Why No `?` Operator Needed

`State<T>` extraction never fails — the state type is guaranteed to match at
compile time. The rejection type is `Infallible`, so you can use `State`
anywhere in the extractor chain without error handling.

## Stateless Routes

When no state is needed, use `Router::new()` (state defaults to `()`):

```rust
let app: Router = Router::new()
    .route("/health", get(health_check));

async fn health_check() -> &'static str {
    "OK"
}
```

Handlers that don't extract `State` work with any state type, including `()`.

## When to Use State

- Database connection pools
- Configuration values
- HTTP clients
- Any value that should be available to every handler

For per-request values (like an authenticated user), use [`Extension<T>`]
instead.

[`Extension<T>`]: extension.md
