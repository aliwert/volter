//! Demonstrates `#[get("/")]` and `#[post("/")]` route attribute macros.
//!
//! Run with: `cargo run -p route-macros`

use serde::Deserialize;
use tokio::net::TcpListener;
use volter::*;

// ---------------------------------------------------------------------------
// GET handler with route attribute macro
// ---------------------------------------------------------------------------

/// Return a greeting at the root path.
#[get("/")]
async fn index() -> &'static str {
    "Hello, World!"
}

/// Return a user-friendly message.
#[get("/hello")]
async fn hello() -> &'static str {
    "Hello from route-macros!"
}

// ---------------------------------------------------------------------------
// POST handler with route attribute macro
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct CreateUser {
    name: String,
    age: u8,
}

/// Create a user from a JSON body.
#[post("/users")]
async fn create_user(Json(payload): Json<CreateUser>) -> String {
    format!("Created user: {} (age {})", payload.name, payload.age)
}

// ---------------------------------------------------------------------------
// GET handler with path parameters and query string
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct PageQuery {
    page: u32,
}

/// Show a user page with a query parameter.
#[get("/users/:id")]
async fn user_page(Path(id): Path<u64>, Query(query): Query<PageQuery>) -> String {
    format!("User {} page {}", id, query.page)
}

// ---------------------------------------------------------------------------
// Handler with state
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AppState {
    name: String,
}

/// Return the application name from state.
#[get("/state")]
async fn get_state(State(state): State<AppState>) -> String {
    format!("App: {}", state.name)
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), volter::BoxError> {
    let app = Router::with_state(AppState {
        name: "route-macros".into(),
    })
    .route_attr(INDEX_ROUTE, index)
    .route_attr(HELLO_ROUTE, hello)
    .route_attr(CREATE_USER_ROUTE, create_user)
    .route_attr(USER_PAGE_ROUTE, user_page)
    .route_attr(GET_STATE_ROUTE, get_state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");
    eprintln!("  GET  /                  (route attribute macro)");
    eprintln!("  GET  /hello             (route attribute macro)");
    eprintln!("  POST /users             (route attribute macro with Json)");
    eprintln!("  GET  /users/:id?page=N  (route attribute macro with Path + Query)");
    eprintln!("  GET  /state             (route attribute macro with State)");

    serve(listener, app).await
}
