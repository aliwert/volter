//! Request-Id example for Volter.
//!
//! Demonstrates using `volter::RequestIdLayer` with `Router::layer(...)`.
//!
//! Run with: `cargo run -p request-id-example`
//!
//! Test with:
//!   curl -v http://localhost:3000/       # observe X-Request-Id header
//!   curl -v http://localhost:3000/id     # response body is the request id
//!   curl -v -H "X-Request-Id: 01ARZ3NDEKTSV4RRFFQ69G5FAV" \
//!        http://localhost:3000/          # client-provided ID is preserved

use tokio::net::TcpListener;
use volter::{get, serve, BoxError, Extension, RequestId, RequestIdLayer, Router};

/// Returns the request's ULID so the client can see what was generated.
async fn echo_id(Extension(id): Extension<RequestId>) -> String {
    id.to_string()
}

async fn hello() -> &'static str {
    "Hello! Check the X-Request-Id response header."
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let app = Router::new()
        .route("/", get(hello))
        .route("/id", get(echo_id))
        .layer(RequestIdLayer::new());

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");

    serve(listener, app).await
}
