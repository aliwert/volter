//! Timeout example for Volter.
//!
//! Demonstrates using `volter::TimeoutLayer` with `Router::layer(...)`.
//!
//! Run with: `cargo run -p timeout-example`
//!
//! Test with:
//!   curl http://localhost:3000/fast   # returns immediately
//!   curl http://localhost:3000/slow   # times out after 1 second

use std::time::Duration;

use tokio::net::TcpListener;
use volter::{get, serve, BoxError, Router, TimeoutLayer};

async fn fast() -> &'static str {
    "fast response"
}

async fn slow() -> &'static str {
    tokio::time::sleep(Duration::from_secs(5)).await;
    "slow response (you should never see this)"
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let app = Router::new()
        .route("/fast", get(fast))
        .route("/slow", get(slow))
        .layer(TimeoutLayer::new(Duration::from_secs(1)));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");
    eprintln!("Try: curl http://localhost:3000/fast  (fast, should succeed)");
    eprintln!("Try: curl http://localhost:3000/slow  (times out after 1s)");

    serve(listener, app).await
}
