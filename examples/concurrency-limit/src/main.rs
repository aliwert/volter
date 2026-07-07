//! Concurrency limit example for Volter.
//!
//! Demonstrates using `volter::ConcurrencyLimitLayer` with `Router::layer(...)`.
//!
//! Run with: `cargo run -p concurrency-limit-example`
//!
//! Test with (two concurrent requests):
//!   curl http://localhost:3000/slow &
//!   curl http://localhost:3000/slow &
//!   wait

use std::time::Duration;
use tokio::net::TcpListener;
use volter::{get, serve, BoxError, ConcurrencyLimitLayer, Router};

/// A handler that takes 2 seconds — useful for observing concurrency limits.
async fn slow() -> &'static str {
    tokio::time::sleep(Duration::from_secs(2)).await;
    "done"
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let app = Router::new()
        .route("/slow", get(slow))
        // Only 1 concurrent request at a time
        .layer(ConcurrencyLimitLayer::new(1));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");
    eprintln!("Try (two concurrent requests, one will wait):");
    eprintln!("  curl http://localhost:3000/slow &");
    eprintln!("  curl http://localhost:3000/slow &");
    eprintln!("  wait");

    serve(listener, app).await
}
