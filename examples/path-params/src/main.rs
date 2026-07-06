//! Path parameters example for Volter.
//!
//! Run with: `cargo run -p path-params`
//!
//! Test with:
//!   curl http://localhost:3000/users/42
//!   curl http://localhost:3000/users/alice

use tokio::net::TcpListener;
use volter::{get, serve, Path, Router};

/// User handler — extracts a `u64` path parameter.
async fn user(Path(id): Path<u64>) -> String {
    format!("User {}", id)
}

/// Name handler — extracts a `String` path parameter.
async fn name(Path(name): Path<String>) -> String {
    format!("Hello, {}!", name)
}

#[tokio::main]
async fn main() -> Result<(), volter::BoxError> {
    let app = Router::new()
        .route("/users/:id", get(user))
        .route("/users/:name", get(name));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");
    eprintln!("Try: curl http://localhost:3000/users/42");

    serve(listener, app).await
}
