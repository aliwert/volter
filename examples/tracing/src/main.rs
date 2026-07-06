//! Tracing example for Volter.
//!
//! Demonstrates using `volter::TraceLayer` with `Router::layer(...)`.
//!
//! Run with: `cargo run -p tracing-example`
//!
//! Test with:
//!   curl http://localhost:3000/
//!   curl http://localhost:3000/foo

use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;
use volter::{get, serve, BoxError, Router, TraceLayer};

async fn hello() -> &'static str {
    "Hello, world!"
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let app = Router::new()
        .route("/", get(hello))
        .route("/{*path}", get(hello))
        .layer(TraceLayer::new());

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");

    serve(listener, app).await
}
