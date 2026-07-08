# Project Structure

A typical Volter project follows standard Rust conventions. Here's a recommended
layout for a small-to-medium application:

```
my-api/
├── Cargo.toml
└── src/
    ├── main.rs           # Server setup and routing
    ├── handlers/         # Request handlers
    │   ├── mod.rs
    │   ├── users.rs
    │   └── posts.rs
    ├── models/           # Data types (Serialize, Deserialize)
    │   ├── mod.rs
    │   └── user.rs
    ├── state.rs          # Application state definition
    └── errors.rs         # Custom error types
```

## The `main.rs` Entry Point

Keep `main.rs` focused on wiring — creating the router, attaching middleware,
and starting the server:

```rust
use tokio::net::TcpListener;
use volter::*;

mod handlers;
mod models;
mod state;

#[tokio::main]
async fn main() -> Result<(), volter::BoxError> {
    let app = Router::with_state(state::load())
        .nest("/api/v1", api_routes())
        .layer(TraceLayer::new())
        .layer(CatchPanicLayer::new());

    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    serve(listener, app).await
}

fn api_routes() -> Router<state::AppState> {
    Router::new()
        .route("/users", get(handlers::users::list))
        .route("/users/:id", get(handlers::users::get_by_id))
        .route("/posts", get(handlers::posts::list))
}
```

## Handlers Module

Each handler file exports `async fn`s that receive extractors:

```rust
// src/handlers/users.rs
use volter::*;
use crate::models::user::User;
use crate::state::AppState;

pub async fn list(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.users.values().cloned().collect::<Vec<User>>())
}

pub async fn get_by_id(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<Json<User>, StatusCode> {
    state.users.get(&id).map(|u| Json(u.clone())).ok_or(StatusCode::NOT_FOUND)
}
```

## Models

Models are plain structs with Serde derives:

```rust
// src/models/user.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
}
```

## When to Split

Use separate files when a module has more than ~100 lines or handles multiple
endpoints. For very small APIs (2-3 endpoints), a single `main.rs` is fine.

## General Guidelines

- One handler function per endpoint, named after the action: `list_users`,
  `create_user`, `get_user_by_id`
- Group related handlers in the same file (`users.rs` for all `/users/*` routes)
- Keep `main.rs` under 50 lines — it should be a wiring diagram, not a
  business-logic dump
- Put custom `IntoResponse` and error types in a dedicated `errors.rs` module
