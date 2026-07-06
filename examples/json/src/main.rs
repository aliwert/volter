//! JSON body example for Volter.
//!
//! Run with: `cargo run -p json-example`
//!
//! Test with:
//!   curl -X POST http://localhost:3000/users \
//!     -H "Content-Type: application/json" \
//!     -d '{"name":"Alice","age":30}'

use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use volter::{get, serve, Json, Router};

/// A user creation payload.
#[derive(Deserialize, Serialize)]
struct CreateUser {
    name: String,
    age: u8,
}

/// Create-user handler — echoes the JSON body back as a JSON response.
async fn create_user(Json(payload): Json<CreateUser>) -> Json<CreateUser> {
    Json(payload)
}

#[tokio::main]
async fn main() -> Result<(), volter::BoxError> {
    let app = Router::new().route("/users", get(create_user));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");
    eprintln!("Try: curl -X POST http://localhost:3000/users -H 'Content-Type: application/json' -d '{{\"name\":\"Alice\",\"age\":30}}'");

    serve(listener, app).await
}
