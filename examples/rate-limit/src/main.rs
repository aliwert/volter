//! Rate limit example for Volter.
//!
//! Demonstrates using `volter::RateLimitLayer` with `Router::layer(...)`.
//!
//! Run with: `cargo run -p rate-limit-example`
//!
//! Test:
//!   # First 3 requests succeed
//!   curl http://localhost:3000/
//!   curl http://localhost:3000/
//!   curl http://localhost:3000/
//!   # 4th returns 429 Too Many Requests
//!   curl http://localhost:3000/
//!   # Wait 10 s, then requests succeed again
//!   sleep 10 && curl http://localhost:3000/

use std::time::Duration;
use tokio::net::TcpListener;
use volter::{get, serve, BoxError, RateLimitLayer, Router};

/// A fast handler.
async fn hello() -> &'static str {
    "Hello, world!"
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let app = Router::new()
        .route("/", get(hello))
        // Allow 3 requests every 10 seconds.
        .layer(RateLimitLayer::new(3, Duration::from_secs(10)));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");
    eprintln!("Limit: 3 requests per 10 seconds");
    eprintln!("Try:");
    eprintln!("  curl http://localhost:3000/   (x3 — succeed)");
    eprintln!("  curl http://localhost:3000/   (429 — rate limited)");
    eprintln!("  sleep 10 && curl http://localhost:3000/   (recovers)");

    serve(listener, app).await
}
