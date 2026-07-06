//! Compression example for Volter.
//!
//! Demonstrates using `volter::CompressionLayer` with `Router::layer(...)`.
//!
//! Run with: `cargo run -p compression-example`
//!
//! Test with:
//!   curl -H 'Accept-Encoding: gzip' -o /dev/null -w '%{size_download}' http://localhost:3000/large
//!   curl -o /dev/null -w '%{size_download}' http://localhost:3000/large

use tokio::net::TcpListener;
use volter::{get, serve, BoxError, CompressionLayer, Router};

/// A handler that returns a large enough body to be worth compressing.
async fn large() -> String {
    "x".repeat(4096)
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let app = Router::new()
        .route("/large", get(large))
        .layer(CompressionLayer::new());

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");
    eprintln!("Try: curl -H 'Accept-Encoding: gzip' -o /dev/null -w '%{{size_download}}' http://localhost:3000/large");
    eprintln!("Try: curl -o /dev/null -w '%{{size_download}}' http://localhost:3000/large  (no compression)");

    serve(listener, app).await
}
