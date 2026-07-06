//! CORS example for Volter.
//!
//! Demonstrates using `volter::CorsLayer` with `Router::layer(...)`.
//!
//! Run with: `cargo run -p cors-example`
//!
//! Test with:
//!   curl -H 'Origin: https://example.com' http://localhost:3000/hello
//!   curl -X OPTIONS -H 'Origin: https://example.com' \
//!        -H 'Access-Control-Request-Method: GET' \
//!        http://localhost:3000/hello

use tokio::net::TcpListener;
use volter::{get, serve, BoxError, CorsLayer, Router};

async fn hello() -> &'static str {
    "Hello, CORS!"
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let app = Router::new()
        .route("/hello", get(hello))
        .layer(CorsLayer::permissive());

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");
    eprintln!("Try: curl -H 'Origin: https://example.com' http://localhost:3000/hello");
    eprintln!("Try: curl -X OPTIONS -H 'Origin: https://example.com' -H 'Access-Control-Request-Method: GET' http://localhost:3000/hello");

    serve(listener, app).await
}
