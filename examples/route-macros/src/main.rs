//! Demonstrates route attribute macros for all HTTP methods.
//!
//! Run with: `cargo run -p route-macros`

use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use volter::http::StatusCode;
use volter::*;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id: u64,
    name: String,
    age: u8,
}

#[derive(Clone)]
struct AppState {
    users: Vec<User>,
}

// ---------------------------------------------------------------------------
// GET handlers
// ---------------------------------------------------------------------------

#[get("/users")]
async fn list_users(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.users.clone())
}

#[get("/users/:id")]
async fn get_user(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<Json<User>, StatusCode> {
    state
        .users
        .iter()
        .find(|u| u.id == id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

// ---------------------------------------------------------------------------
// POST handler
// ---------------------------------------------------------------------------

#[post("/users")]
async fn create_user(
    State(_state): State<AppState>,
    Json(payload): Json<User>,
) -> impl IntoResponse {
    // In a real app this would mutate shared state.
    // Here we just echo back.
    Json(payload)
}

// ---------------------------------------------------------------------------
// PUT handler
// ---------------------------------------------------------------------------

#[put("/users/:id")]
async fn update_user(Path(id): Path<u64>, Json(_payload): Json<User>) -> String {
    format!("Updated user {id}")
}

// ---------------------------------------------------------------------------
// PATCH handler
// ---------------------------------------------------------------------------

#[patch("/users/:id")]
async fn patch_user(Path(id): Path<u64>, Json(_payload): Json<User>) -> String {
    format!("Patched user {id}")
}

// ---------------------------------------------------------------------------
// DELETE handler
// ---------------------------------------------------------------------------

#[delete("/users/:id")]
async fn delete_user(Path(id): Path<u64>) -> String {
    format!("Deleted user {id}")
}

// ---------------------------------------------------------------------------
// HEAD handler
// ---------------------------------------------------------------------------

#[head("/users")]
async fn users_head() {}

// ---------------------------------------------------------------------------
// OPTIONS handler
// ---------------------------------------------------------------------------

#[options("/users")]
async fn users_options() -> &'static str {
    "GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS"
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), volter::BoxError> {
    let state = AppState {
        users: vec![User {
            id: 1,
            name: "Alice".into(),
            age: 30,
        }],
    };

    let app = Router::with_state(state)
        .route_attr(LIST_USERS_ROUTE, list_users)
        .route_attr(GET_USER_ROUTE, get_user)
        .route_attr(CREATE_USER_ROUTE, create_user)
        .route_attr(UPDATE_USER_ROUTE, update_user)
        .route_attr(PATCH_USER_ROUTE, patch_user)
        .route_attr(DELETE_USER_ROUTE, delete_user)
        .route_attr(USERS_HEAD_ROUTE, users_head)
        .route_attr(USERS_OPTIONS_ROUTE, users_options);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");
    eprintln!("  GET     /users          (list)");
    eprintln!("  GET     /users/:id      (get by id)");
    eprintln!("  POST    /users          (create)");
    eprintln!("  PUT     /users/:id      (update)");
    eprintln!("  PATCH   /users/:id      (patch)");
    eprintln!("  DELETE  /users/:id      (delete)");
    eprintln!("  HEAD    /users          (headers only)");
    eprintln!("  OPTIONS /users          (allowed methods)");

    serve(listener, app).await
}
