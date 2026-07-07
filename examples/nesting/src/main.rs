//! Router nesting example for Volter.
//!
//! Demonstrates `Router::nest()` for composing routers under a path prefix.
//!
//! Run with: `cargo run -p nesting`
//!
//! Test endpoints:
//! - `GET /health`        → root route
//! - `GET /api/users`     → nested route
//! - `GET /api/posts`     → nested route

use tokio::net::TcpListener;
use volter::{get, serve, Router};

async fn health() -> &'static str {
    "ok"
}

async fn users() -> &'static str {
    "users"
}

async fn posts() -> &'static str {
    "posts"
}

#[tokio::main]
async fn main() -> Result<(), volter::BoxError> {
    let api = Router::new()
        .route("/users", get(users))
        .route("/posts", get(posts));

    let app = Router::new()
        .route("/health", get(health))
        .nest("/api", api);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");
    eprintln!("  GET /health");
    eprintln!("  GET /api/users");
    eprintln!("  GET /api/posts");

    serve(listener, app).await
}
