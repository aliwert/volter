//! Router merge example for Volter.
//!
//! Demonstrates `Router::merge()` for combining two independent routers.
//!
//! Run with: `cargo run -p merge`
//!
//! Test endpoints:
//! - `GET /users`     → from the first router
//! - `GET /admin`     → from the second router, merged in

use tokio::net::TcpListener;
use volter::{get, serve, Router};

async fn users() -> &'static str {
    "users"
}

async fn admin() -> &'static str {
    "admin"
}

#[tokio::main]
async fn main() -> Result<(), volter::BoxError> {
    let api = Router::new().route("/users", get(users));
    let admin = Router::new().route("/admin", get(admin));

    let app = api.merge(admin);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");
    eprintln!("  GET /users");
    eprintln!("  GET /admin");

    serve(listener, app).await
}
