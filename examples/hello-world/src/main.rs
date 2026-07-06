//! Hello, World! example for Volter.
//!
//! Run with: `cargo run -p hello-world`

use tokio::net::TcpListener;
use volter::{get, serve, Router};

/// Root handler — responds with "Hello, World!" to every GET request.
async fn hello_world() -> &'static str {
    "Hello, World!"
}

#[tokio::main]
async fn main() -> Result<(), volter::BoxError> {
    let app = Router::new().route("/", get(hello_world));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");

    serve(listener, app).await
}
