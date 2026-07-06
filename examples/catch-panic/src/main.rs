//! Catch-panic example for Volter.
//!
//! Demonstrates using `volter::CatchPanicLayer` with `Router::layer(...)`.
//!
//! Run with: `cargo run -p catch-panic-example`
//!
//! Test with:
//!   curl http://localhost:3000/healthy   # returns 200
//!   curl http://localhost:3000/panic     # returns 500 (caught)

use tokio::net::TcpListener;
use volter::{get, serve, BoxError, CatchPanicLayer, Router};

/// A healthy endpoint that returns immediately.
async fn healthy() -> &'static str {
    "all good"
}

/// An endpoint that panics — will be caught by `CatchPanicLayer`.
async fn panics() -> &'static str {
    panic!("something unexpected happened in the handler");
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let app = Router::new()
        .route("/healthy", get(healthy))
        .route("/panic", get(panics))
        .layer(CatchPanicLayer::new());

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");
    eprintln!("Try: curl http://localhost:3000/healthy");
    eprintln!("Try: curl http://localhost:3000/panic  (returns 500)");

    serve(listener, app).await
}
