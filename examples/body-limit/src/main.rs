//! Request body limit example for Volter.
//!
//! Demonstrates using `volter::RequestBodyLimitLayer` with `Router::layer(...)`.
//!
//! Run with: `cargo run -p body-limit-example`
//!
//! Test with:
//!   curl -X POST http://localhost:3000/users \
//!     -H 'Content-Type: application/json' \
//!     -H 'Content-Length: 18' \
//!     -d '{"name":"Alice"}'                          # 200 OK
//!   curl -X POST http://localhost:3000/users \
//!     -H 'Content-Type: application/json' \
//!     -H 'Content-Length: 2000' \
//!     -d "$(python3 -c 'print("x" * 2000)')"          # 413 Payload Too Large

use serde::Deserialize;
use tokio::net::TcpListener;
use volter::{get, serve, BoxError, Json, RequestBodyLimitLayer, Router};

#[derive(Deserialize)]
struct CreateUser {
    #[allow(dead_code)]
    name: String,
}

async fn create_user(_payload: Json<CreateUser>) -> &'static str {
    "user created"
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let app = Router::new()
        .route("/users", get(create_user))
        .layer(RequestBodyLimitLayer::new(1024)); // 1 KB limit

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");
    eprintln!("Try: curl -X POST http://localhost:3000/users -H 'Content-Type: application/json' -H 'Content-Length: 18' -d '{{\"name\":\"Alice\"}}'");
    eprintln!("Try: curl -X POST http://localhost:3000/users -H 'Content-Type: application/json' -H 'Content-Length: 2000' -d \"$(python3 -c 'print(\"x\" * 2000)')\"");

    serve(listener, app).await
}
