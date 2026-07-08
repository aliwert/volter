# Your First Application

Let's build a simple JSON API for managing users. You'll learn how Volter handles
routing, JSON extraction, state, and error responses — all in about 50 lines.

## What We're Building

A read-only user API with two endpoints:

- `GET /users` — returns a list of users
- `GET /users/:id` — returns a single user by ID

## Step 1: Define the State

Application state is any value you pass to `Router::with_state()`. Handlers
access it via the `State` extractor.

```rust
use std::collections::HashMap;
use volter::*;

#[derive(Clone)]
struct AppState {
    users: HashMap<u64, String>,
}
```

The state must implement `Clone` because the router clones it when creating
boxed services at setup time.

## Step 2: Define Handlers

Each handler is an `async fn` that takes extractors as parameters and returns
anything that implements `IntoResponse`.

```rust
use std::sync::OnceLock;

fn default_state() -> AppState {
    let mut users = HashMap::new();
    users.insert(1, "Alice".into());
    users.insert(2, "Bob".into());
    AppState { users }
}

async fn list_users(State(state): State<AppState>) -> String {
    let names: Vec<&str> = state.users.values().map(String::as_str).collect();
    names.join(", ")
}

async fn get_user(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<String, StatusCode> {
    match state.users.get(&id) {
        Some(name) => Ok(name.clone()),
        None => Err(StatusCode::NOT_FOUND),
    }
}
```

Note how `get_user` returns a `Result`: the `Ok` variant produces a `200 OK`
response, and the `Err` variant (a `StatusCode`) produces the error response
directly.

## Step 3: Wire It Up

```rust
#[tokio::main]
async fn main() -> Result<(), volter::BoxError> {
    let app = Router::with_state(default_state())
        .route("/users", get(list_users))
        .route("/users/:id", get(get_user));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    eprintln!("Listening on http://0.0.0.0:3000");
    serve(listener, app).await
}
```

## Step 4: Run and Test

```bash
cargo run
```

In another terminal:

```bash
# List all users
curl http://localhost:3000/users
# Output: Alice, Bob

# Get user by ID
curl http://localhost:3000/users/1
# Output: Alice

# Non-existent user
curl -w "\n%{http_code}" http://localhost:3000/users/99
# Output: 404
```

## What You Learned

- **State management** — typed shared state via `Router::with_state()` and
  `State<T>`
- **Path parameters** — `:id` in the route pattern, extracted via `Path<T>`
- **Result-based error handling** — returning `Result<T, E>` where both `T` and
  `E` implement `IntoResponse`
- **Zero macros** — everything works with plain Rust functions

## Next Steps

- Learn about [Routing](../routing/routing.md) in detail
- See how to [extract JSON bodies](../extractors/json.md)
- Read about [middleware](../middleware/overview.md) for logging, CORS, etc.
